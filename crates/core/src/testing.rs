//! Test doubles for the two ports, shared by every ticket's tests.
//! Available to unit tests and, through the `testing` feature, to integration tests.
//!
//! # Example
//!
//! ```
//! use std::sync::Arc;
//! use haulmer_googleworkspace_mcp_core::testing::{FakeHttp, FakeReply, FakeRunner};
//! use haulmer_googleworkspace_mcp_core::{Core, Paths};
//!
//! let home = tempfile::tempdir().unwrap();
//! let app_data = tempfile::tempdir().unwrap();
//! let runner = Arc::new(FakeRunner::new());
//! runner.on(&["--version"], FakeReply::ok("gws 0.22.5\n"));
//! let http = Arc::new(FakeHttp::new());
//! let paths = Paths { home: home.path().into(), app_data: app_data.path().into() };
//! let core = Core::new(paths, runner.clone(), http.clone());
//! // ... call `core`, then assert on `runner.calls()` and `http.requests()`.
//! ```

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use crate::{
    CommandSpec, CoreError, HttpClient, HttpResponse, KILLED_EXIT_CODE, OutputLine, ProcessKiller,
    ProcessOutput, ProcessRunner, RunningProcess,
};

/// Canned result of one fake process.
///
/// # Example
///
/// ```
/// use haulmer_googleworkspace_mcp_core::testing::FakeReply;
/// let failed = FakeReply::exit(2, r#"{"error":{"message":"no creds"}}"#, "");
/// let login = FakeReply::exit(0, "", "https://accounts.google.com/o?x\n").hang_until_killed();
/// assert!(login.hang_until_killed);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FakeReply {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    /// For `spawn`: after its lines, the process blocks until killed (a login nobody finishes).
    pub hang_until_killed: bool,
}

impl FakeReply {
    /// Exit 0 with this stdout.
    ///
    /// # Example
    ///
    /// ```
    /// assert_eq!(haulmer_googleworkspace_mcp_core::testing::FakeReply::ok("{}").exit_code, 0);
    /// ```
    pub fn ok(stdout: &str) -> Self {
        Self::exit(0, stdout, "")
    }

    /// Any exit code, stdout and stderr.
    ///
    /// # Example
    ///
    /// ```
    /// let reply = haulmer_googleworkspace_mcp_core::testing::FakeReply::exit(3, "", "validationError");
    /// assert_eq!(reply.stderr, "validationError");
    /// ```
    pub fn exit(exit_code: i32, stdout: &str, stderr: &str) -> Self {
        Self {
            exit_code,
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            hang_until_killed: false,
        }
    }

    /// Makes a spawned process block after its lines until it is killed.
    ///
    /// # Example
    ///
    /// ```
    /// let reply = haulmer_googleworkspace_mcp_core::testing::FakeReply::ok("").hang_until_killed();
    /// assert!(reply.hang_until_killed);
    /// ```
    pub fn hang_until_killed(mut self) -> Self {
        self.hang_until_killed = true;
        self
    }
}

/// How a command reached the fake runner.
///
/// # Example
///
/// ```
/// use haulmer_googleworkspace_mcp_core::testing::CallKind;
/// assert_ne!(CallKind::Run, CallKind::Spawn);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallKind {
    /// `ProcessRunner::run`.
    Run,
    /// `ProcessRunner::spawn`.
    Spawn,
}

/// One command the fake runner received, in call order.
///
/// # Example
///
/// ```
/// use haulmer_googleworkspace_mcp_core::testing::{CallKind, RecordedCall};
/// use haulmer_googleworkspace_mcp_core::CommandSpec;
/// let call = RecordedCall { kind: CallKind::Run, command: CommandSpec::new("gws", &["auth", "logout"], "/run") };
/// assert_eq!(call.command.args, ["auth", "logout"]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedCall {
    pub kind: CallKind,
    pub command: CommandSpec,
}

/// Computes a reply from the received command.
type Handler = Arc<dyn Fn(&CommandSpec) -> FakeReply + Send + Sync>;

/// Replies registered for one (program, args prefix).
struct Rule {
    program: Option<PathBuf>,
    prefix: Vec<String>,
    replies: VecDeque<Handler>,
}

/// Fake [`ProcessRunner`]: records every command and answers with canned replies.
///
/// Matching: among the rules whose args prefix matches (and whose program matches, if set),
/// the longest prefix wins; at equal length a program-specific rule wins. A rule with several
/// replies uses them in order and repeats the last one. No match -> `CoreError::Process`
/// (like a program that does not exist). `spawn` emits the stderr lines, then the stdout lines.
///
/// # Example
///
/// ```
/// use haulmer_googleworkspace_mcp_core::testing::{FakeReply, FakeRunner};
/// use haulmer_googleworkspace_mcp_core::{CommandSpec, ProcessRunner};
/// let runner = FakeRunner::new();
/// runner.on(&["auth", "status"], FakeReply::ok("{}"));
/// runner.run(&CommandSpec::new("gws", &["auth", "status"], "/run")).unwrap();
/// assert_eq!(runner.args(), [vec!["auth".to_string(), "status".to_string()]]);
/// ```
#[derive(Default)]
pub struct FakeRunner {
    rules: Mutex<Vec<Rule>>,
    calls: Mutex<Vec<RecordedCall>>,
    kills: Arc<AtomicUsize>,
}

impl FakeRunner {
    /// Creates a runner with no canned replies.
    ///
    /// # Example
    ///
    /// ```
    /// assert!(haulmer_googleworkspace_mcp_core::testing::FakeRunner::new().calls().is_empty());
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Replies to any program whose args start with `args_prefix`.
    ///
    /// # Example
    ///
    /// ```
    /// use haulmer_googleworkspace_mcp_core::testing::{FakeReply, FakeRunner};
    /// FakeRunner::new().on(&["auth", "logout"], FakeReply::ok(r#"{"status":"success"}"#));
    /// ```
    pub fn on(&self, args_prefix: &[&str], reply: FakeReply) -> &Self {
        self.add_rule(None, args_prefix, Arc::new(move |_| reply.clone()))
    }

    /// Replies only when the program equals `program` (e.g. `"gws"` on PATH vs a full path).
    ///
    /// # Example
    ///
    /// ```
    /// use haulmer_googleworkspace_mcp_core::testing::{FakeReply, FakeRunner};
    /// FakeRunner::new().on_program("gws", &["--version"], FakeReply::ok("gws 0.22.5"));
    /// ```
    pub fn on_program(
        &self,
        program: impl Into<PathBuf>,
        args_prefix: &[&str],
        reply: FakeReply,
    ) -> &Self {
        self.add_rule(
            Some(program.into()),
            args_prefix,
            Arc::new(move |_| reply.clone()),
        )
    }

    /// Replies with a closure: compute the reply or simulate gws side effects on disk.
    ///
    /// # Example
    ///
    /// ```
    /// use haulmer_googleworkspace_mcp_core::testing::{FakeReply, FakeRunner};
    /// FakeRunner::new().on_with(&["auth", "logout"], |command| {
    ///     let _ = std::fs::remove_file(command.cwd.join("credentials.enc"));
    ///     FakeReply::ok("{}")
    /// });
    /// ```
    pub fn on_with(
        &self,
        args_prefix: &[&str],
        handler: impl Fn(&CommandSpec) -> FakeReply + Send + Sync + 'static,
    ) -> &Self {
        self.add_rule(None, args_prefix, Arc::new(handler))
    }

    /// Every command received so far, in order.
    ///
    /// # Example
    ///
    /// ```
    /// assert!(haulmer_googleworkspace_mcp_core::testing::FakeRunner::new().calls().is_empty());
    /// ```
    pub fn calls(&self) -> Vec<RecordedCall> {
        self.calls.lock().unwrap().clone()
    }

    /// Just the args of every command received so far, in order.
    ///
    /// # Example
    ///
    /// ```
    /// assert!(haulmer_googleworkspace_mcp_core::testing::FakeRunner::new().args().is_empty());
    /// ```
    pub fn args(&self) -> Vec<Vec<String>> {
        self.calls()
            .into_iter()
            .map(|call| call.command.args)
            .collect()
    }

    /// How many times a spawned process was killed.
    ///
    /// # Example
    ///
    /// ```
    /// assert_eq!(haulmer_googleworkspace_mcp_core::testing::FakeRunner::new().kill_count(), 0);
    /// ```
    pub fn kill_count(&self) -> usize {
        self.kills.load(Ordering::SeqCst)
    }

    /// Appends a handler to the rule for (program, prefix), creating the rule if needed.
    fn add_rule(&self, program: Option<PathBuf>, args_prefix: &[&str], handler: Handler) -> &Self {
        let prefix: Vec<String> = args_prefix.iter().map(|arg| arg.to_string()).collect();
        let mut rules = self.rules.lock().unwrap();
        let existing = rules
            .iter_mut()
            .find(|rule| rule.program == program && rule.prefix == prefix);
        if let Some(rule) = existing {
            rule.replies.push_back(handler);
        } else {
            rules.push(Rule {
                program,
                prefix,
                replies: VecDeque::from([handler]),
            });
        }
        self
    }

    /// Records the call and computes the reply of the best matching rule.
    fn reply(&self, kind: CallKind, command: &CommandSpec) -> Result<FakeReply, CoreError> {
        self.calls.lock().unwrap().push(RecordedCall {
            kind,
            command: command.clone(),
        });
        let handler = {
            let mut rules = self.rules.lock().unwrap();
            let best = rules
                .iter_mut()
                .filter(|rule| command.args.starts_with(&rule.prefix))
                .filter(|rule| match &rule.program {
                    Some(program) => *program == command.program,
                    None => true,
                })
                .max_by_key(|rule| (rule.prefix.len(), rule.program.is_some()));
            match best {
                Some(rule) if rule.replies.len() > 1 => rule.replies.pop_front(),
                Some(rule) => rule.replies.front().cloned(),
                None => None,
            }
        };
        match handler {
            Some(handler) => Ok(handler(command)),
            None => Err(CoreError::Process(format!(
                "FakeRunner: no reply for {} {}",
                command.program.display(),
                command.args.join(" ")
            ))),
        }
    }
}

impl ProcessRunner for FakeRunner {
    /// Records the call and returns the canned output.
    ///
    /// # Example
    ///
    /// ```
    /// use haulmer_googleworkspace_mcp_core::testing::{FakeReply, FakeRunner};
    /// use haulmer_googleworkspace_mcp_core::{CommandSpec, ProcessRunner};
    /// let runner = FakeRunner::new();
    /// runner.on(&[], FakeReply::ok("hi"));
    /// assert_eq!(runner.run(&CommandSpec::new("x", &[], ".")).unwrap().stdout, "hi");
    /// ```
    fn run(&self, command: &CommandSpec) -> Result<ProcessOutput, CoreError> {
        let reply = self.reply(CallKind::Run, command)?;
        Ok(ProcessOutput {
            exit_code: reply.exit_code,
            stdout: reply.stdout,
            stderr: reply.stderr,
        })
    }

    /// Records the call and returns a fake process that replays the canned output as lines.
    ///
    /// # Example
    ///
    /// ```
    /// use haulmer_googleworkspace_mcp_core::testing::{FakeReply, FakeRunner};
    /// use haulmer_googleworkspace_mcp_core::{CommandSpec, ProcessRunner};
    /// let runner = FakeRunner::new();
    /// runner.on(&[], FakeReply::exit(1, "", ""));
    /// assert_eq!(runner.spawn(&CommandSpec::new("x", &[], ".")).unwrap().wait().unwrap(), 1);
    /// ```
    fn spawn(&self, command: &CommandSpec) -> Result<Box<dyn RunningProcess>, CoreError> {
        let reply = self.reply(CallKind::Spawn, command)?;
        let stderr = reply
            .stderr
            .lines()
            .map(|line| OutputLine::Stderr(line.to_string()));
        let stdout = reply
            .stdout
            .lines()
            .map(|line| OutputLine::Stdout(line.to_string()));
        Ok(Box::new(FakeProcess {
            lines: stderr.chain(stdout).collect(),
            exit_code: reply.exit_code,
            hang_until_killed: reply.hang_until_killed,
            killer: Arc::new(FakeKiller {
                killed: Mutex::new(false),
                wake: Condvar::new(),
                kills: Arc::clone(&self.kills),
            }),
        }))
    }
}

/// Kill switch shared by a fake process and its killers.
struct FakeKiller {
    killed: Mutex<bool>,
    wake: Condvar,
    kills: Arc<AtomicUsize>,
}

impl FakeKiller {
    /// Whether `kill` was called.
    fn is_killed(&self) -> bool {
        *self.killed.lock().unwrap()
    }
}

impl ProcessKiller for FakeKiller {
    /// Marks the process killed, counts the kill and wakes a blocked `next_line`.
    fn kill(&self) {
        *self.killed.lock().unwrap() = true;
        self.kills.fetch_add(1, Ordering::SeqCst);
        self.wake.notify_all();
    }
}

/// A fake running process: replays lines, optionally hangs until killed.
struct FakeProcess {
    lines: VecDeque<OutputLine>,
    exit_code: i32,
    hang_until_killed: bool,
    killer: Arc<FakeKiller>,
}

impl RunningProcess for FakeProcess {
    /// Next canned line; once they run out, blocks until killed if the reply hangs.
    fn next_line(&mut self) -> Option<OutputLine> {
        if self.killer.is_killed() {
            return None;
        }
        if let Some(line) = self.lines.pop_front() {
            return Some(line);
        }
        if self.hang_until_killed {
            let mut killed = self.killer.killed.lock().unwrap();
            while !*killed {
                killed = self.killer.wake.wait(killed).unwrap();
            }
        }
        None
    }

    /// A handle to this process's kill switch.
    fn killer(&self) -> Arc<dyn ProcessKiller> {
        self.killer.clone()
    }

    /// The canned exit code, or [`KILLED_EXIT_CODE`] if killed.
    fn wait(&mut self) -> Result<i32, CoreError> {
        if self.killer.is_killed() {
            Ok(KILLED_EXIT_CODE)
        } else {
            Ok(self.exit_code)
        }
    }
}

/// One request the fake HTTP client received.
///
/// # Example
///
/// ```
/// use haulmer_googleworkspace_mcp_core::testing::HttpRequest;
/// let request = HttpRequest::PostForm { url: "https://oauth2.googleapis.com/revoke".into(), form: vec![("token".into(), "t".into())] };
/// assert!(matches!(request, HttpRequest::PostForm { .. }));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpRequest {
    Get {
        url: String,
    },
    GetBearer {
        url: String,
        token: String,
    },
    RequestBearer {
        method: String,
        url: String,
        token: String,
        body: Option<Vec<u8>>,
        headers: Vec<(String, String)>,
    },
    PostForm {
        url: String,
        form: Vec<(String, String)>,
    },
}

/// Canned results for one URL.
type ResponseQueue = VecDeque<Result<HttpResponse, CoreError>>;

/// Fake [`HttpClient`]: canned responses per exact URL (for GET and POST), records every request.
/// Several responses for one URL are used in order, repeating the last. Unknown URL ->
/// `CoreError::Http` (like being offline).
///
/// # Example
///
/// ```
/// use haulmer_googleworkspace_mcp_core::testing::FakeHttp;
/// use haulmer_googleworkspace_mcp_core::{HttpClient, HttpResponse};
/// let http = FakeHttp::new();
/// http.on("https://oauth2.googleapis.com/revoke", HttpResponse::ok(""));
/// assert_eq!(http.post_form("https://oauth2.googleapis.com/revoke", &[("token", "t")]).unwrap().status, 200);
/// ```
#[derive(Default)]
pub struct FakeHttp {
    responses: Mutex<Vec<(String, ResponseQueue)>>,
    requests: Mutex<Vec<HttpRequest>>,
}

impl FakeHttp {
    /// Creates a client with no canned responses.
    ///
    /// # Example
    ///
    /// ```
    /// assert!(haulmer_googleworkspace_mcp_core::testing::FakeHttp::new().requests().is_empty());
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Answers `url` with `response`.
    ///
    /// # Example
    ///
    /// ```
    /// use haulmer_googleworkspace_mcp_core::{testing::FakeHttp, HttpResponse};
    /// FakeHttp::new().on("https://example.com/gws.tar.gz", HttpResponse::ok(vec![0u8; 4]));
    /// ```
    pub fn on(&self, url: &str, response: HttpResponse) -> &Self {
        self.add(url, Ok(response))
    }

    /// Answers `url` with an error (e.g. `CoreError::Http("offline")`).
    ///
    /// # Example
    ///
    /// ```
    /// use haulmer_googleworkspace_mcp_core::{testing::FakeHttp, CoreError};
    /// FakeHttp::new().on_error("https://oauth2.googleapis.com/revoke", CoreError::Http("offline".into()));
    /// ```
    pub fn on_error(&self, url: &str, error: CoreError) -> &Self {
        self.add(url, Err(error))
    }

    /// Every request received so far, in order.
    ///
    /// # Example
    ///
    /// ```
    /// assert!(haulmer_googleworkspace_mcp_core::testing::FakeHttp::new().requests().is_empty());
    /// ```
    pub fn requests(&self) -> Vec<HttpRequest> {
        self.requests.lock().unwrap().clone()
    }

    /// Queues a canned result for `url`.
    fn add(&self, url: &str, result: Result<HttpResponse, CoreError>) -> &Self {
        let mut responses = self.responses.lock().unwrap();
        let existing = responses.iter_mut().find(|(known, _)| known == url);
        if let Some((_, queue)) = existing {
            queue.push_back(result);
        } else {
            responses.push((url.to_string(), VecDeque::from([result])));
        }
        self
    }

    /// Records the request and returns the next canned result for its URL.
    fn respond(&self, request: HttpRequest) -> Result<HttpResponse, CoreError> {
        let url = match &request {
            HttpRequest::Get { url }
            | HttpRequest::GetBearer { url, .. }
            | HttpRequest::RequestBearer { url, .. }
            | HttpRequest::PostForm { url, .. } => url.clone(),
        };
        self.requests.lock().unwrap().push(request);
        let mut responses = self.responses.lock().unwrap();
        let queue = responses.iter_mut().find(|(known, _)| *known == url);
        match queue {
            Some((_, queue)) if queue.len() > 1 => queue.pop_front().unwrap(),
            Some((_, queue)) => queue.front().cloned().unwrap(),
            None => Err(CoreError::Http(format!("FakeHttp: no response for {url}"))),
        }
    }
}

impl HttpClient for FakeHttp {
    /// Records a GET and returns the canned result.
    ///
    /// # Example
    ///
    /// ```
    /// use haulmer_googleworkspace_mcp_core::{testing::FakeHttp, HttpClient};
    /// assert!(FakeHttp::new().get("https://unknown").is_err());
    /// ```
    fn get(&self, url: &str) -> Result<HttpResponse, CoreError> {
        self.respond(HttpRequest::Get {
            url: url.to_string(),
        })
    }

    fn get_bearer(&self, url: &str, token: &str) -> Result<HttpResponse, CoreError> {
        self.respond(HttpRequest::GetBearer {
            url: url.to_string(),
            token: token.to_string(),
        })
    }

    fn request_bearer(
        &self,
        method: &str,
        url: &str,
        token: &str,
        body: Option<&[u8]>,
        headers: &[(&str, &str)],
    ) -> Result<HttpResponse, CoreError> {
        self.respond(HttpRequest::RequestBearer {
            method: method.to_string(),
            url: url.to_string(),
            token: token.to_string(),
            body: body.map(<[u8]>::to_vec),
            headers: headers
                .iter()
                .map(|(name, value)| ((*name).into(), (*value).into()))
                .collect(),
        })
    }

    /// Records a form POST and returns the canned result.
    ///
    /// # Example
    ///
    /// ```
    /// use haulmer_googleworkspace_mcp_core::{testing::FakeHttp, HttpClient};
    /// assert!(FakeHttp::new().post_form("https://unknown", &[]).is_err());
    /// ```
    fn post_form(&self, url: &str, form: &[(&str, &str)]) -> Result<HttpResponse, CoreError> {
        self.respond(HttpRequest::PostForm {
            url: url.to_string(),
            form: form
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
        })
    }
}
