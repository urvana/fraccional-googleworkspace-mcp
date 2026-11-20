//! Ports: the only ways the core reaches the outside world (processes and HTTP).
//! Real adapters: [`crate::SystemRunner`], [`crate::SystemHttp`]. Test fakes: `crate::testing`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use crate::CoreError;

/// Exit code reported by [`RunningProcess::wait`] when the process was killed (no exit code).
pub const KILLED_EXIT_CODE: i32 = -1;

/// A program invocation: program, arguments, environment changes and working folder.
/// The child inherits the parent environment, plus `env`, minus `env_remove` (removal wins
/// when a name is in both). Stdin is always closed.
///
/// # Example
///
/// ```
/// use haulmer_googleworkspace_mcp_core::CommandSpec;
/// let mut command = CommandSpec::new("gws", &["auth", "status"], "/tmp/run");
/// command.env.insert("NO_COLOR".into(), "1".into());
/// command.env_remove.insert("GOOGLE_APPLICATION_CREDENTIALS".into());
/// assert_eq!(command.args, ["auth", "status"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    /// Program name (looked up on PATH) or full path.
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Variables set for the child.
    pub env: BTreeMap<String, String>,
    /// Variables removed from the inherited environment.
    pub env_remove: BTreeSet<String>,
    /// Working folder of the child.
    pub cwd: PathBuf,
}

impl CommandSpec {
    /// Builds a command with no environment changes.
    ///
    /// # Example
    ///
    /// ```
    /// use haulmer_googleworkspace_mcp_core::CommandSpec;
    /// let command = CommandSpec::new("gws", &["--version"], "/tmp");
    /// assert!(command.env.is_empty());
    /// ```
    pub fn new(program: impl Into<PathBuf>, args: &[&str], cwd: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: args.iter().map(|arg| arg.to_string()).collect(),
            env: BTreeMap::new(),
            env_remove: BTreeSet::new(),
            cwd: cwd.into(),
        }
    }
}

/// Output of a process run to completion.
///
/// # Example
///
/// ```
/// use haulmer_googleworkspace_mcp_core::ProcessOutput;
/// let output = ProcessOutput { exit_code: 0, stdout: "gws 0.22.5\n".into(), stderr: String::new() };
/// assert_eq!(output.stdout.trim(), "gws 0.22.5");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProcessOutput {
    /// Exit code; [`KILLED_EXIT_CODE`] if the process ended without one.
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// One line read from a running process, tagged with its stream (no trailing newline).
///
/// # Example
///
/// ```
/// use haulmer_googleworkspace_mcp_core::OutputLine;
/// let line = OutputLine::Stderr("  https://accounts.google.com/o/oauth2/auth?...".into());
/// assert!(matches!(line, OutputLine::Stderr(_)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputLine {
    Stdout(String),
    Stderr(String),
}

/// Runs external programs (gws). Implemented by [`crate::SystemRunner`] and `testing::FakeRunner`.
///
/// # Example
///
/// ```
/// use haulmer_googleworkspace_mcp_core::{CommandSpec, ProcessRunner};
/// fn version(runner: &dyn ProcessRunner) -> String {
///     let output = runner.run(&CommandSpec::new("gws", &["--version"], ".")).unwrap();
///     output.stdout.trim().to_string()
/// }
/// ```
pub trait ProcessRunner: Send + Sync {
    /// Runs the command to completion and captures its output.
    /// `Err` only when the program cannot be started or awaited; a non-zero exit is `Ok`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use haulmer_googleworkspace_mcp_core::{CommandSpec, ProcessRunner, SystemRunner};
    /// let output = SystemRunner.run(&CommandSpec::new("gws", &["auth", "status"], ".")).unwrap();
    /// println!("{}", output.exit_code);
    /// ```
    fn run(&self, command: &CommandSpec) -> Result<ProcessOutput, CoreError>;

    /// Starts a long-running command (gws login) whose output is read line by line.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use haulmer_googleworkspace_mcp_core::{CommandSpec, ProcessRunner, SystemRunner};
    /// let mut login = SystemRunner.spawn(&CommandSpec::new("gws", &["auth", "login"], ".")).unwrap();
    /// while let Some(line) = login.next_line() {
    ///     println!("{line:?}");
    /// }
    /// let exit_code = login.wait().unwrap();
    /// ```
    fn spawn(&self, command: &CommandSpec) -> Result<Box<dyn RunningProcess>, CoreError>;
}

/// A process started by [`ProcessRunner::spawn`].
///
/// # Example
///
/// ```no_run
/// use haulmer_googleworkspace_mcp_core::{CommandSpec, OutputLine, ProcessRunner, SystemRunner};
/// let mut process = SystemRunner.spawn(&CommandSpec::new("gws", &["auth", "login"], ".")).unwrap();
/// let killer = process.killer(); // hand this to another thread to cancel
/// while let Some(OutputLine::Stderr(line) | OutputLine::Stdout(line)) = process.next_line() {
///     if line.contains("https://accounts.google.com/") { killer.kill(); }
/// }
/// ```
pub trait RunningProcess: Send {
    /// Blocks for the next line from stdout or stderr; `None` once both streams are closed
    /// (the process ended or was killed).
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use haulmer_googleworkspace_mcp_core::{CommandSpec, ProcessRunner, SystemRunner};
    /// # let mut process = SystemRunner.spawn(&CommandSpec::new("gws", &[], ".")).unwrap();
    /// let first = process.next_line();
    /// ```
    fn next_line(&mut self) -> Option<crate::OutputLine>;

    /// A handle that kills this process from any thread (used by `Core::cancel_login`).
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use haulmer_googleworkspace_mcp_core::{CommandSpec, ProcessRunner, SystemRunner};
    /// # let process = SystemRunner.spawn(&CommandSpec::new("gws", &[], ".")).unwrap();
    /// let killer = process.killer();
    /// std::thread::spawn(move || killer.kill());
    /// ```
    fn killer(&self) -> Arc<dyn ProcessKiller>;

    /// Waits for the process to end; returns its exit code, or [`KILLED_EXIT_CODE`] if killed.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use haulmer_googleworkspace_mcp_core::{CommandSpec, ProcessRunner, SystemRunner};
    /// # let mut process = SystemRunner.spawn(&CommandSpec::new("gws", &[], ".")).unwrap();
    /// let exit_code = process.wait().unwrap();
    /// ```
    fn wait(&mut self) -> Result<i32, CoreError>;
}

/// Kills a running process. Killing twice, or after the process ended, is harmless.
///
/// # Example
///
/// ```no_run
/// # use haulmer_googleworkspace_mcp_core::{CommandSpec, ProcessRunner, SystemRunner};
/// # let process = SystemRunner.spawn(&CommandSpec::new("gws", &[], ".")).unwrap();
/// process.killer().kill();
/// ```
pub trait ProcessKiller: Send + Sync {
    /// Kills the process (best effort).
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use haulmer_googleworkspace_mcp_core::{CommandSpec, ProcessRunner, SystemRunner};
    /// # let process = SystemRunner.spawn(&CommandSpec::new("gws", &[], ".")).unwrap();
    /// process.killer().kill();
    /// ```
    fn kill(&self);
}

/// An HTTP response: status code and raw body.
///
/// # Example
///
/// ```
/// use haulmer_googleworkspace_mcp_core::HttpResponse;
/// let response = HttpResponse::ok("hello");
/// assert_eq!((response.status, response.body.as_slice()), (200, b"hello".as_slice()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Builds a response with any status.
    ///
    /// # Example
    ///
    /// ```
    /// use haulmer_googleworkspace_mcp_core::HttpResponse;
    /// assert_eq!(HttpResponse::new(400, "bad").status, 400);
    /// ```
    pub fn new(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            body: body.into(),
        }
    }

    /// Builds a `200 OK` response.
    ///
    /// # Example
    ///
    /// ```
    /// use haulmer_googleworkspace_mcp_core::HttpResponse;
    /// assert_eq!(HttpResponse::ok(vec![1, 2]).status, 200);
    /// ```
    pub fn ok(body: impl Into<Vec<u8>>) -> Self {
        Self::new(200, body)
    }
}

/// Minimal HTTP client (gws download, checksum file, skills tarball, revoke).
/// Any HTTP status is `Ok`; `Err` ([`CoreError::Http`]) only when no response arrived.
/// Redirects are followed (GitHub release assets redirect).
///
/// # Example
///
/// ```
/// use haulmer_googleworkspace_mcp_core::HttpClient;
/// fn revoke(http: &dyn HttpClient, token: &str) -> bool {
///     let response = http.post_form("https://oauth2.googleapis.com/revoke", &[("token", token)]);
///     matches!(response, Ok(response) if response.status == 200)
/// }
/// ```
pub trait HttpClient: Send + Sync {
    /// GETs a URL and returns the whole body.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use haulmer_googleworkspace_mcp_core::{HttpClient, SystemHttp};
    /// let body = SystemHttp::new().get("https://example.com").unwrap().body;
    /// ```
    fn get(&self, url: &str) -> Result<HttpResponse, CoreError>;

    /// GETs a URL with an OAuth bearer token without placing the token in the URL.
    fn get_bearer(&self, url: &str, token: &str) -> Result<HttpResponse, CoreError>;

    /// Sends a JSON request with an OAuth bearer token.
    fn request_bearer(
        &self,
        method: &str,
        url: &str,
        token: &str,
        body: Option<&[u8]>,
        headers: &[(&str, &str)],
    ) -> Result<HttpResponse, CoreError> {
        let _ = (method, url, token, body, headers);
        Err(CoreError::NotImplemented("authenticated JSON requests"))
    }

    /// POSTs an `application/x-www-form-urlencoded` body.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use haulmer_googleworkspace_mcp_core::{HttpClient, SystemHttp};
    /// let response = SystemHttp::new().post_form("https://oauth2.googleapis.com/revoke", &[("token", "1//abc")]).unwrap();
    /// ```
    fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<HttpResponse, CoreError>;
}

/// GETs `url` and returns the body of a 2xx response. No response or any other status is
/// a [`CoreError::Download`] that names the URL.
///
/// # Example
///
/// ```ignore
/// let archive = download(self.http.as_ref(), SKILLS_TARBALL_URL)?;
/// // Err(Download("https://...: HTTP 404")) when GitHub answers 404
/// ```
pub(crate) fn download(http: &dyn HttpClient, url: &str) -> Result<Vec<u8>, CoreError> {
    let response = http
        .get(url)
        .map_err(|error| CoreError::Download(format!("{url}: {error}")))?;
    if !(200..300).contains(&response.status) {
        return Err(CoreError::Download(format!(
            "{url}: HTTP {}",
            response.status
        )));
    }
    Ok(response.body)
}
