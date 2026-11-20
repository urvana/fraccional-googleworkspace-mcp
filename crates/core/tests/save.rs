//! T07 — `Core::save()` and `Core::cancel_login()`: connect, add and remove services.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use haulmer_googleworkspace_mcp_core::gws_env::MARKER_FILE;
use haulmer_googleworkspace_mcp_core::revoke::REVOKE_URL;
use haulmer_googleworkspace_mcp_core::testing::{CallKind, FakeHttp, FakeReply, FakeRunner};
use haulmer_googleworkspace_mcp_core::{
    BusinessConnector, BusinessSelection, Core, CoreError, HttpResponse, Level, Paths, SaveOutcome,
    Selection, Service,
};
use tempfile::TempDir;

const URL: &str = "https://accounts.google.com/o/oauth2/auth?client_id=x&scope=y";
const LOGIN_OK: &str = r#"{"status":"success","account":"ana@haulmer.com"}"#;
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
    let config_dir = home.path().join("haulmer-gws");
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
            r#"{{"user":"ana@haulmer.com","token_valid":true,"scopes":{scopes}}}"#
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
    assert_eq!(status.account.as_deref(), Some("ana@haulmer.com"));
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
