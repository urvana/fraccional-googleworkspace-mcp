//! The real [`ProcessRunner`] on `std::process`.

use std::io::{BufRead, BufReader, Read};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::{
    CommandSpec, CoreError, KILLED_EXIT_CODE, OutputLine, ProcessKiller, ProcessOutput,
    ProcessRunner, RunningProcess,
};

/// Runs programs with `std::process`: stdin closed, env and cwd from the [`CommandSpec`].
///
/// # Example
///
/// ```no_run
/// use haulmer_googleworkspace_mcp_core::{CommandSpec, ProcessRunner, SystemRunner};
/// let output = SystemRunner.run(&CommandSpec::new("gws", &["--version"], ".")).unwrap();
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemRunner;

/// Builds the `std::process::Command` for a spec (stdin closed; `env_remove` wins over `env`).
fn to_command(spec: &CommandSpec) -> Command {
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .stdin(Stdio::null());
    #[cfg(unix)]
    command.process_group(0);
    for (name, value) in &spec.env {
        command.env(name, value);
    }
    for name in &spec.env_remove {
        command.env_remove(name);
    }
    command
}

/// Maps a start/wait failure to [`CoreError::Process`], naming the program.
fn process_error(spec: &CommandSpec, error: std::io::Error) -> CoreError {
    CoreError::Process(format!("{}: {error}", spec.program.display()))
}

impl ProcessRunner for SystemRunner {
    /// Runs to completion; a signal-terminated process reports [`KILLED_EXIT_CODE`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// use haulmer_googleworkspace_mcp_core::{CommandSpec, ProcessRunner, SystemRunner};
    /// let output = SystemRunner.run(&CommandSpec::new("gws", &["auth", "status"], ".")).unwrap();
    /// ```
    fn run(&self, command: &CommandSpec) -> Result<ProcessOutput, CoreError> {
        let output = to_command(command)
            .output()
            .map_err(|error| process_error(command, error))?;
        Ok(ProcessOutput {
            exit_code: output.status.code().unwrap_or(KILLED_EXIT_CODE),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }

    /// Starts the process with both streams piped; one thread per stream forwards lines.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use haulmer_googleworkspace_mcp_core::{CommandSpec, ProcessRunner, SystemRunner};
    /// let process = SystemRunner.spawn(&CommandSpec::new("gws", &["auth", "login"], ".")).unwrap();
    /// ```
    fn spawn(&self, command: &CommandSpec) -> Result<Box<dyn RunningProcess>, CoreError> {
        let mut child = to_command(command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| process_error(command, error))?;
        let (sender, lines) = channel();
        if let Some(stdout) = child.stdout.take() {
            forward_lines(stdout, sender.clone(), OutputLine::Stdout);
        }
        if let Some(stderr) = child.stderr.take() {
            forward_lines(stderr, sender, OutputLine::Stderr);
        }
        Ok(Box::new(SystemProcess {
            lines,
            killer: Arc::new(SystemKiller {
                child: Mutex::new(child),
                killed: AtomicBool::new(false),
            }),
        }))
    }
}

/// Reads `stream` line by line on a new thread and sends each line, tagged by `tag`.
fn forward_lines(
    stream: impl Read + Send + 'static,
    sender: Sender<OutputLine>,
    tag: fn(String) -> OutputLine,
) {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buffer);
                    let line = line.trim_end_matches(['\n', '\r']).to_string();
                    if sender.send(tag(line)).is_err() {
                        break;
                    }
                }
            }
        }
    });
}

/// Owns the child so it can be killed from any thread while `wait` polls it.
struct SystemKiller {
    child: Mutex<Child>,
    killed: AtomicBool,
}

impl ProcessKiller for SystemKiller {
    /// Kills the child (ignores "already exited").
    fn kill(&self) {
        self.killed.store(true, Ordering::SeqCst);
        #[cfg(unix)]
        {
            let pid = self.child.lock().unwrap().id();
            let _ = Command::new("kill")
                .args(["-KILL", &format!("-{pid}")])
                .status();
        }
        #[cfg(not(unix))]
        let _ = self.child.lock().unwrap().kill();
    }
}

/// A process started by [`SystemRunner::spawn`].
struct SystemProcess {
    lines: Receiver<OutputLine>,
    killer: Arc<SystemKiller>,
}

impl RunningProcess for SystemProcess {
    /// Next line from either stream; `None` when both are closed.
    fn next_line(&mut self) -> Option<OutputLine> {
        self.lines.recv().ok()
    }

    /// A handle that kills the child.
    fn killer(&self) -> Arc<dyn ProcessKiller> {
        self.killer.clone()
    }

    /// Polls until the child exits, without holding the lock so `kill` stays possible.
    fn wait(&mut self) -> Result<i32, CoreError> {
        loop {
            let status = self.killer.child.lock().unwrap().try_wait();
            match status {
                Err(error) => return Err(CoreError::Process(error.to_string())),
                Ok(Some(status)) => {
                    if self.killer.killed.load(Ordering::SeqCst) {
                        return Ok(KILLED_EXIT_CODE);
                    }
                    return Ok(status.code().unwrap_or(KILLED_EXIT_CODE));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            }
        }
    }
}
