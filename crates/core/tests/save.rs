//! T07 — `Core::save()` and `Core::cancel_login()`: connect, add and remove services.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use fraccional_googleworkspace_mcp_core::gws_env::MARKER_FILE;
use fraccional_googleworkspace_mcp_core::revoke::REVOKE_URL;
use fraccional_googleworkspace_mcp_core::testing::{CallKind, FakeHttp, FakeReply, FakeRunner};
use fraccional_googleworkspace_mcp_core::{
    BusinessConnector, BusinessSelection, CommandSpec, Core, CoreError, HttpResponse, Level,
    OutputLine, Paths, ProcessKiller, ProcessOutput, ProcessRunner, RunningProcess, SaveOutcome,
    Selection, Service,
};
use tempfile::TempDir;

const URL: &str = "https://accounts.google.com/o/oauth2/auth?client_id=x&scope=y";
const LOGIN_OK: &str = r#"{"status":"success","account":"ana@fraccional.com"}"#;
const GMAIL_READ: &str = "https://www.googleapis.com/auth/gmail.readonly";
const CALENDAR_READ: &str = "https://www.googleapis.com/auth/calendar.readonly";
const YOUTUBE_READ: &str = "https://www.googleapis.com/auth/youtube.readonly";

/// Temp home + app data, a custom gws folder, and a pinned `gws` on PATH. Login writes
/// credentials and makes `auth status` report the scopes it was asked for.
struct Fixture {
    _home: TempDir,
    _app_data: TempDir,
    config_dir: PathBuf,
    runner: Arc<FakeRunner>,
    http: Arc<FakeHttp>,
    core: Core,
}

fn fixture() -> Fixture {
    let (home, app_data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let config_dir = home.path().join("fraccional-gws");
    let runner = Arc::new(FakeRunner::new());
    let http = Arc::new(FakeHttp::new());
    let paths = Paths {
        home: home.path().into(),
        app_data: app_data.path().into(),
    };
    let core = Core::new(paths, runner.clone(), http.clone());
    core.set_config_dir(Some(config_dir.clone())).unwrap();

    runner.on_program("gws", &["--version"], FakeReply::ok("gws 0.22.5\n"));
    let granted = Arc::new(Mutex::new(Vec::<String>::new()));
    let reported = granted.clone();
    runner.on_with(&["auth", "status"], move |_| {
        let scopes = serde_json::to_string(&*reported.lock().unwrap()).unwrap();
        FakeReply::ok(&format!(
            r#"{{"user":"ana@fraccional.com","token_valid":true,"scopes":{scopes}}}"#
        ))
    });
    let (logged_in, folder) = (granted.clone(), config_dir.clone());
    runner.on_with(&["auth", "login"], move |command| {
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("credentials.enc"), "x").unwrap();
        *logged_in.lock().unwrap() = command.args[3].split(',').map(String::from).collect();
        FakeReply::exit(0, LOGIN_OK, &format!("Open this URL:\n\n  {URL}\n"))
    });
    let (logged_out, folder) = (granted, config_dir.clone());
    runner.on_with(&["auth", "logout"], move |_| {
        let _ = std::fs::remove_file(folder.join("credentials.enc"));
        logged_out.lock().unwrap().clear();
        FakeReply::ok(r#"{"status":"success"}"#)
    });
    runner.on(
        &["auth", "export", "--unmasked"],
        FakeReply::ok(r#"{"refresh_token":"1//old"}"#),
    );
    http.on(REVOKE_URL, HttpResponse::ok("{}"));
    Fixture {
        _home: home,
        _app_data: app_data,
        config_dir,
        runner,
        http,
        core,
    }
}

impl Fixture {
    /// The gws calls after the version probes and status reads.
    fn changing_calls(&self) -> Vec<Vec<String>> {
        self.runner
            .args()
            .into_iter()
            .filter(|args| args != &["--version"] && args != &["auth", "status"])
            .collect()
    }

    fn save(&self, selection: &Selection) -> SaveOutcome {
        self.core.save(selection, false, &|_| {}).unwrap()
    }

    fn save_with_business(
        &self,
        selection: &Selection,
        business_selection: &BusinessSelection,
    ) -> SaveOutcome {
        self.core
            .save_with_business(selection, business_selection, false, &|_| {})
            .unwrap()
    }
}

fn args(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| arg.to_string()).collect()
}

fn login(scopes: &[&str]) -> Vec<String> {
    vec![
        "auth".into(),
        "login".into(),
        "--scopes".into(),
        scopes.join(","),
    ]
}

#[test]
fn first_save_logs_in_once_with_the_selected_scopes_and_marks_the_folder() {
    let f = fixture();

    let outcome = f.save(&Selection::from_levels([(Service::Gmail, Level::Read)]));

    assert_eq!(f.changing_calls(), [login(&[GMAIL_READ])]);
    let SaveOutcome::Connected(status) = outcome else {
        panic!("expected Connected, got {outcome:?}");
    };
    assert_eq!(status.account.as_deref(), Some("ana@fraccional.com"));
    assert!(status.granted.contains(Service::Gmail, Level::Read));
    assert!(f.config_dir.join(MARKER_FILE).is_file());
    let login_call = f
        .runner
        .calls()
        .into_iter()
        .find(|c| c.command.args.get(1).is_some_and(|a| a == "login"));
    assert_eq!(login_call.unwrap().kind, CallKind::Spawn);
}

#[test]
fn save_requests_shared_oauth_scope_for_youtube() {
    let f = fixture();
    let business = BusinessSelection::from_levels([(BusinessConnector::Youtube, Level::Read)]);

    let outcome = f.save_with_business(&Selection::new(), &business);

    assert!(matches!(outcome, SaveOutcome::Connected(_)));
    assert_eq!(f.changing_calls(), [login(&[YOUTUBE_READ])]);
}

#[test]
fn adding_a_service_logs_in_once_with_the_full_new_set_and_revokes_nothing() {
    let f = fixture();
    f.save(&Selection::from_levels([(Service::Gmail, Level::Read)]));

    f.save(&Selection::from_levels([
        (Service::Gmail, Level::Read),
        (Service::Calendar, Level::Read),
    ]));

    assert_eq!(
        f.changing_calls(),
        [login(&[GMAIL_READ]), login(&[GMAIL_READ, CALENDAR_READ])]
    );
    assert!(f.http.requests().is_empty());
}

#[test]
fn removing_a_granted_level_revokes_and_logs_out_before_signing_in_with_the_reduced_set() {
    let f = fixture();
    f.save(&Selection::from_levels([
        (Service::Gmail, Level::Read),
        (Service::Calendar, Level::Read),
    ]));

    let outcome = f.save(&Selection::from_levels([(Service::Calendar, Level::Read)]));

    assert_eq!(
        f.changing_calls()[1..],
        [
            args(&["auth", "export", "--unmasked"]),
            args(&["auth", "logout"]),
            login(&[CALENDAR_READ]),
        ]
    );
    assert_eq!(f.http.requests().len(), 1, "one revoke at Google");
    let SaveOutcome::Connected(status) = outcome else {
        panic!("expected Connected");
    };
    assert_eq!(
        status.granted,
        Selection::from_levels([(Service::Calendar, Level::Read)])
    );
    assert!(f.config_dir.join(MARKER_FILE).is_file());
}

#[test]
fn empty_selection_revokes_everything_and_does_not_sign_in() {
    let f = fixture();
    f.save(&Selection::from_levels([(Service::Gmail, Level::Read)]));

    let outcome = f.save(&Selection::new());

    assert_eq!(
        f.changing_calls()[1..],
        [
            args(&["auth", "export", "--unmasked"]),
            args(&["auth", "logout"])
        ]
    );
    assert_eq!(f.http.requests().len(), 1);
    let SaveOutcome::Connected(status) = outcome else {
        panic!("expected Connected");
    };
    assert!(status.granted.is_empty());
    assert!(!f.config_dir.join(MARKER_FILE).exists());
}

impl Fixture {
    /// Credentials some other gws setup created (no ownership marker).
    fn foreign_credentials(&self) {
        std::fs::create_dir_all(&self.config_dir).unwrap();
        std::fs::write(self.config_dir.join("credentials.enc"), "theirs").unwrap();
    }
}

#[test]
fn foreign_credentials_without_confirmation_ask_first_and_change_nothing() {
    let f = fixture();
    f.foreign_credentials();

    let outcome = f.save(&Selection::from_levels([(Service::Gmail, Level::Read)]));

    assert_eq!(outcome, SaveOutcome::NeedsReplaceConfirmation);
    assert!(f.changing_calls().is_empty());
    assert!(f.http.requests().is_empty());
    let credentials = std::fs::read_to_string(f.config_dir.join("credentials.enc")).unwrap();
    assert_eq!(credentials, "theirs");
    assert!(!f.config_dir.join(MARKER_FILE).exists());
}

#[test]
fn confirmed_replace_signs_in_over_foreign_credentials_without_revoking_them() {
    let f = fixture();
    f.foreign_credentials();

    let outcome = f
        .core
        .save(
            &Selection::from_levels([(Service::Gmail, Level::Read)]),
            true,
            &|_| {},
        )
        .unwrap();

    assert!(matches!(outcome, SaveOutcome::Connected(_)));
    assert_eq!(f.changing_calls(), [login(&[GMAIL_READ])]);
    assert!(f.http.requests().is_empty());
    assert!(f.config_dir.join(MARKER_FILE).is_file());
}

/// Saves Gmail Read and returns the URLs passed to `on_auth_url`.
fn forwarded_urls(f: &Fixture) -> Vec<String> {
    let urls = Mutex::new(Vec::new());
    f.core
        .save(
            &Selection::from_levels([(Service::Gmail, Level::Read)]),
            false,
            &|url| urls.lock().unwrap().push(url.to_string()),
        )
        .unwrap();
    urls.into_inner().unwrap()
}

#[test]
fn the_sign_in_url_printed_on_stderr_is_forwarded_once() {
    let f = fixture();
    assert_eq!(forwarded_urls(&f), [URL]);
}

#[test]
fn the_sign_in_url_printed_on_stdout_is_forwarded() {
    let f = fixture();
    f.runner.on(
        &["auth", "login", "--scopes"],
        FakeReply::exit(
            0,
            &format!("Open this URL in your browser to authenticate:\n\n  {URL}\n{LOGIN_OK}\n"),
            "Using keyring backend: file\n",
        ),
    );
    assert_eq!(forwarded_urls(&f), [URL]);
}

#[test]
fn a_failed_login_reports_the_gws_error_message_and_leaves_no_marker() {
    let f = fixture();
    f.runner.on(
        &["auth", "login", "--scopes"],
        FakeReply::exit(
            2,
            r#"{"error":{"code":401,"message":"Access blocked: admin policy","reason":"authError"}}"#,
            "error[auth]: Access blocked\n",
        ),
    );

    let error = f
        .core
        .save(
            &Selection::from_levels([(Service::Gmail, Level::Read)]),
            false,
            &|_| {},
        )
        .unwrap_err();

    assert_eq!(
        error,
        CoreError::Gws {
            exit_code: 2,
            message: "Access blocked: admin policy".into()
        }
    );
    assert!(!f.config_dir.join(MARKER_FILE).exists());
}

#[test]
fn cancel_kills_the_waiting_login_and_save_reports_it_was_cancelled() {
    let f = fixture();
    f.runner.on(
        &["auth", "login", "--scopes"],
        FakeReply::exit(0, "", &format!("  {URL}\n")).hang_until_killed(),
    );
    let (url_seen, url_received) = std::sync::mpsc::channel();

    let result = std::thread::scope(|scope| {
        let save = scope.spawn(|| {
            f.core.save(
                &Selection::from_levels([(Service::Gmail, Level::Read)]),
                false,
                &|_| url_seen.send(()).unwrap(),
            )
        });
        url_received.recv().unwrap();
        f.core.cancel_login().unwrap();
        save.join().unwrap()
    });

    assert_eq!(result, Err(CoreError::Cancelled));
    assert_eq!(f.runner.kill_count(), 1);
    assert!(!f.config_dir.join(MARKER_FILE).exists());
}

#[test]
fn cancel_without_a_running_login_does_nothing() {
    let f = fixture();
    f.core.cancel_login().unwrap();
    assert_eq!(f.runner.kill_count(), 0);
    assert!(matches!(
        f.save(&Selection::from_levels([(Service::Gmail, Level::Read)])),
        SaveOutcome::Connected(_)
    ));
}

#[test]
fn cancellation_at_save_admission_prevents_login_without_poisoning_future_saves() {
    let f = fixture();
    f.core.cancel_login().unwrap();
    let selection = Selection::from_levels([(Service::Gmail, Level::Read)]);
    let result = f.core.save_with_business_on_start(
        &selection,
        &BusinessSelection::new(),
        false,
        &|| f.core.cancel_login().unwrap(),
        &|_| {},
    );
    assert_eq!(result, Err(CoreError::Cancelled));
    assert!(f.changing_calls().is_empty());
    assert!(matches!(f.save(&selection), SaveOutcome::Connected(_)));
}

#[test]
fn the_login_runs_with_the_custom_folder() {
    let f = fixture();

    f.save(&Selection::from_levels([(Service::Gmail, Level::Read)]));

    let login_call = f
        .runner
        .calls()
        .into_iter()
        .find(|call| call.command.args.get(1).is_some_and(|arg| arg == "login"))
        .unwrap();
    assert_eq!(
        login_call.command.env["GOOGLE_WORKSPACE_CLI_CONFIG_DIR"],
        f.config_dir.to_str().unwrap()
    );
    assert_eq!(login_call.command.env["NO_COLOR"], "1");
}

/// Connects Gmail + Calendar, then makes every later login answer `reply`.
fn connected_then_login_replies(reply: FakeReply) -> Fixture {
    let f = fixture();
    f.save(&Selection::from_levels([
        (Service::Gmail, Level::Read),
        (Service::Calendar, Level::Read),
    ]));
    f.runner.on(&["auth", "login", "--scopes"], reply);
    f
}

#[test]
fn a_failed_login_after_removing_a_level_reports_that_access_was_removed() {
    let f = connected_then_login_replies(FakeReply::exit(
        2,
        r#"{"error":{"code":401,"message":"Access blocked"}}"#,
        "",
    ));

    let result = f.core.save(
        &Selection::from_levels([(Service::Gmail, Level::Read)]),
        false,
        &|_| {},
    );

    assert_eq!(result, Err(CoreError::AccessRemoved));
    assert_eq!(f.http.requests().len(), 1, "the old token was revoked");
    assert!(!f.config_dir.join(MARKER_FILE).exists());
}

#[test]
fn a_cancelled_login_after_removing_a_level_reports_that_access_was_removed() {
    let f = connected_then_login_replies(
        FakeReply::exit(0, "", &format!("  {URL}\n")).hang_until_killed(),
    );
    let (url_seen, url_received) = std::sync::mpsc::channel();

    let result = std::thread::scope(|scope| {
        let save = scope.spawn(|| {
            f.core.save(
                &Selection::from_levels([(Service::Gmail, Level::Read)]),
                false,
                &|_| url_seen.send(()).unwrap(),
            )
        });
        url_received.recv().unwrap();
        f.core.cancel_login().unwrap();
        save.join().unwrap()
    });

    assert_eq!(result, Err(CoreError::AccessRemoved));
}

// Hold a fake command at a known boundary without relying on scheduling or sleeps.
fn gate(
    f: &Fixture,
    prefix: &[&str],
    reply: FakeReply,
    consume_existing: bool,
) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
    let (started, observed) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let released = Mutex::new(released);
    f.runner.on_with(prefix, move |_| {
        started.send(()).unwrap();
        released
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        reply.clone()
    });
    if consume_existing {
        f.runner.run(&CommandSpec::new("gws", prefix, ".")).unwrap();
    }
    (observed, release)
}

fn wait(receiver: &mpsc::Receiver<()>) {
    receiver.recv_timeout(Duration::from_secs(5)).unwrap();
}

#[test]
fn access_changes_reject_overlap_in_both_directions_and_cancel_only_save() {
    let f = fixture();
    f.runner.on(
        &["auth", "login", "--scopes", GMAIL_READ],
        FakeReply::exit(0, "", &format!("  {URL}\n")).hang_until_killed(),
    );
    let (seen, observed) = mpsc::channel();
    std::thread::scope(|scope| {
        let save = scope.spawn(|| {
            f.core.save_with_business_identified(
                &Selection::from_levels([(Service::Gmail, Level::Read)]),
                &BusinessSelection::new(),
                false,
                Some("current"),
                &|| {},
                &|_| seen.send(()).unwrap(),
            )
        });
        wait(&observed);
        assert_eq!(
            f.core.save(&Selection::new(), false, &|_| {}),
            Err(CoreError::AccessInProgress)
        );
        let started = std::cell::Cell::new(false);
        assert_eq!(
            f.core.save_with_business_on_start(
                &Selection::new(),
                &BusinessSelection::new(),
                false,
                &|| started.set(true),
                &|_| {},
            ),
            Err(CoreError::AccessInProgress)
        );
        assert!(!started.get());
        assert_eq!(f.core.revoke_all(false), Err(CoreError::AccessInProgress));
        f.core.cancel_login_for("old").unwrap();
        assert_eq!(f.runner.kill_count(), 0);
        f.core.cancel_login_for("current").unwrap();
        assert_eq!(save.join().unwrap(), Err(CoreError::Cancelled));
    });

    f.save(&Selection::from_levels([(Service::Calendar, Level::Read)]));
    let (started, release) = gate(
        &f,
        &["auth", "export", "--unmasked"],
        FakeReply::ok(r#"{"refresh_token":"old"}"#),
        true,
    );
    std::thread::scope(|scope| {
        let revoke = scope.spawn(|| f.core.revoke_all(false));
        wait(&started);
        assert_eq!(
            f.core.save(&Selection::new(), false, &|_| {}),
            Err(CoreError::AccessInProgress)
        );
        assert_eq!(f.core.revoke_all(false), Err(CoreError::AccessInProgress));
        f.core.cancel_login().unwrap();
        release.send(()).unwrap();
        assert!(revoke.join().unwrap().is_ok());
    });
    assert_eq!(f.runner.kill_count(), 1);
    assert!(f.core.revoke_all(false).is_ok());
}

#[test]
fn cancellation_before_revocation_leaves_access_untouched() {
    for remove in [false, true] {
        let f = fixture();
        f.save(&Selection::from_levels([
            (Service::Gmail, Level::Read),
            (Service::Calendar, Level::Read),
        ]));
        let (started, release) = gate(
            &f,
            &["auth", "status"],
            FakeReply::ok(&format!(
                r#"{{"scopes":["{GMAIL_READ}","{CALENDAR_READ}"]}}"#
            )),
            true,
        );
        let before = f.changing_calls();
        std::thread::scope(|scope| {
            let save = scope.spawn(|| {
                let selection = if remove {
                    Selection::from_levels([(Service::Gmail, Level::Read)])
                } else {
                    Selection::from_levels([
                        (Service::Gmail, Level::Read),
                        (Service::Calendar, Level::Read),
                    ])
                };
                f.core.save(&selection, false, &|_| {})
            });
            wait(&started);
            f.core.cancel_login().unwrap();
            release.send(()).unwrap();
            assert_eq!(save.join().unwrap(), Err(CoreError::Cancelled));
        });
        assert_eq!(f.changing_calls(), before);
        assert!(f.http.requests().is_empty());
        assert!(f.config_dir.join(MARKER_FILE).is_file());
    }
}

#[test]
fn cancelling_empty_save_while_finding_gws_does_not_start_revocation() {
    struct PausedRunner {
        inner: Arc<FakeRunner>,
        started: mpsc::Sender<()>,
        release: Mutex<mpsc::Receiver<()>>,
    }

    impl ProcessRunner for PausedRunner {
        fn run(&self, command: &CommandSpec) -> Result<ProcessOutput, CoreError> {
            if command.args == ["--version"] {
                self.started.send(()).unwrap();
                self.release
                    .lock()
                    .unwrap()
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap();
            }
            self.inner.run(command)
        }

        fn spawn(&self, command: &CommandSpec) -> Result<Box<dyn RunningProcess>, CoreError> {
            self.inner.spawn(command)
        }
    }

    let f = fixture();
    f.save(&Selection::from_levels([(Service::Gmail, Level::Read)]));
    let (started, observed) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let core = Core::new(
        f.core.paths().clone(),
        Arc::new(PausedRunner {
            inner: f.runner.clone(),
            started,
            release: Mutex::new(released),
        }),
        f.http.clone(),
    );
    let before = f.changing_calls();
    std::thread::scope(|scope| {
        let save = scope.spawn(|| core.save(&Selection::new(), false, &|_| {}));
        wait(&observed);
        core.cancel_login().unwrap();
        release.send(()).unwrap();
        assert_eq!(save.join().unwrap(), Err(CoreError::Cancelled));
    });
    assert_eq!(f.changing_calls(), before);
    assert!(f.config_dir.join(MARKER_FILE).is_file());
    assert!(f.http.requests().is_empty());
}

#[test]
fn cancellation_during_revocation_finishes_it_and_preserves_errors() {
    for (empty, failed) in [(false, false), (false, true), (true, false), (true, true)] {
        let f = fixture();
        f.save(&Selection::from_levels([
            (Service::Gmail, Level::Read),
            (Service::Calendar, Level::Read),
        ]));
        let reply = if failed {
            FakeReply::exit(1, "", "export failed")
        } else {
            FakeReply::ok(r#"{"refresh_token":"old"}"#)
        };
        let (started, release) = gate(&f, &["auth", "export", "--unmasked"], reply, true);
        let before = f.changing_calls().len();
        std::thread::scope(|scope| {
            let save = scope.spawn(|| {
                let selection = if empty {
                    Selection::new()
                } else {
                    Selection::from_levels([(Service::Gmail, Level::Read)])
                };
                f.core.save(&selection, false, &|_| {})
            });
            wait(&started);
            f.core.cancel_login().unwrap();
            release.send(()).unwrap();
            let result = save.join().unwrap();
            if failed {
                assert!(
                    matches!(result, Err(CoreError::RevokeFailed(_))),
                    "{result:?}"
                );
            } else {
                assert_eq!(result, Err(CoreError::AccessRemoved));
            }
        });
        assert_eq!(
            &f.changing_calls()[before..],
            [
                args(&["auth", "export", "--unmasked"]),
                args(&["auth", "logout"])
            ]
        );
        assert!(!f.config_dir.join(MARKER_FILE).exists());
    }
}

#[test]
fn cancellation_racing_process_start_kills_the_new_process() {
    let f = fixture();
    let (started, release) = gate(
        &f,
        &["auth", "login", "--scopes", GMAIL_READ],
        FakeReply::ok("").hang_until_killed(),
        false,
    );
    std::thread::scope(|scope| {
        let save = scope.spawn(|| {
            f.core.save(
                &Selection::from_levels([(Service::Gmail, Level::Read)]),
                false,
                &|_| {},
            )
        });
        wait(&started);
        f.core.cancel_login().unwrap();
        release.send(()).unwrap();
        assert_eq!(save.join().unwrap(), Err(CoreError::Cancelled));
    });
    assert_eq!(f.runner.kill_count(), 1);
    assert!(f.core.revoke_all(false).is_ok());
}

#[test]
fn access_stays_busy_until_a_concurrent_cancellation_finishes() {
    struct PausedKiller {
        started: mpsc::Sender<()>,
        release: Mutex<mpsc::Receiver<()>>,
    }
    impl ProcessKiller for PausedKiller {
        fn kill(&self) {
            self.started.send(()).unwrap();
            self.release
                .lock()
                .unwrap()
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
        }
    }
    struct PausedProcess {
        killer: Arc<PausedKiller>,
        finish: mpsc::Receiver<()>,
        url_sent: bool,
    }
    impl RunningProcess for PausedProcess {
        fn next_line(&mut self) -> Option<OutputLine> {
            if !self.url_sent {
                self.url_sent = true;
                return Some(OutputLine::Stderr(URL.into()));
            }
            self.finish.recv_timeout(Duration::from_secs(5)).unwrap();
            None
        }
        fn killer(&self) -> Arc<dyn ProcessKiller> {
            self.killer.clone()
        }
        fn wait(&mut self) -> Result<i32, CoreError> {
            Ok(0)
        }
    }
    struct PausedRunner {
        inner: Arc<FakeRunner>,
        killer: Arc<PausedKiller>,
        finish: Mutex<Option<mpsc::Receiver<()>>>,
    }
    impl ProcessRunner for PausedRunner {
        fn run(&self, command: &CommandSpec) -> Result<ProcessOutput, CoreError> {
            self.inner.run(command)
        }
        fn spawn(&self, _: &CommandSpec) -> Result<Box<dyn RunningProcess>, CoreError> {
            Ok(Box::new(PausedProcess {
                killer: self.killer.clone(),
                finish: self.finish.lock().unwrap().take().unwrap(),
                url_sent: false,
            }))
        }
    }

    let f = fixture();
    let (kill_started, kill_observed) = mpsc::channel();
    let (release_kill, kill_released) = mpsc::channel();
    let (finish_process, finished) = mpsc::channel();
    let (url_seen, url_observed) = mpsc::channel();
    let core = Core::new(
        f.core.paths().clone(),
        Arc::new(PausedRunner {
            inner: f.runner.clone(),
            killer: Arc::new(PausedKiller {
                started: kill_started,
                release: Mutex::new(kill_released),
            }),
            finish: Mutex::new(Some(finished)),
        }),
        f.http.clone(),
    );
    std::thread::scope(|scope| {
        let save = scope.spawn(|| {
            core.save(
                &Selection::from_levels([(Service::Gmail, Level::Read)]),
                false,
                &|_| url_seen.send(()).unwrap(),
            )
        });
        wait(&url_observed);
        let cancel = scope.spawn(|| core.cancel_login());
        wait(&kill_observed);
        finish_process.send(()).unwrap();
        assert!(save.join().unwrap().is_ok());
        assert_eq!(core.revoke_all(false), Err(CoreError::AccessInProgress));
        release_kill.send(()).unwrap();
        cancel.join().unwrap().unwrap();
    });
    assert!(core.revoke_all(false).is_ok());
}

#[test]
fn panic_in_login_callback_kills_process_and_releases_access() {
    let f = fixture();
    f.runner.on(
        &["auth", "login", "--scopes", GMAIL_READ],
        FakeReply::exit(0, "", &format!("{URL}\n")).hang_until_killed(),
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.core.save(
            &Selection::from_levels([(Service::Gmail, Level::Read)]),
            false,
            &|_| panic!("callback"),
        )
    }));
    assert!(result.is_err());
    assert_eq!(f.runner.kill_count(), 1);
    assert!(f.core.revoke_all(false).is_ok());
}
