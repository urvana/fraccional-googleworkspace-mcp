//! T06 — `Core::revoke_all()`: revoke at Google, then clear local credentials.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use fraccional_googleworkspace_mcp_core::gws_env::{MARKER_FILE, write_marker};
use fraccional_googleworkspace_mcp_core::revoke::REVOKE_URL;
use fraccional_googleworkspace_mcp_core::testing::{FakeHttp, FakeReply, FakeRunner, HttpRequest};
use fraccional_googleworkspace_mcp_core::{Core, CoreError, HttpResponse, Paths, RevokeOutcome};
use tempfile::TempDir;

const TOKEN: &str = "1//refresh-token-secret";

/// Temp home + app data, a custom gws folder holding fraccional-googleworkspace-mcp credentials,
/// and a pinned `gws` on PATH whose logout deletes the credentials.
struct Fixture {
    _home: TempDir,
    _app_data: TempDir,
    config_dir: PathBuf,
    runner: Arc<FakeRunner>,
    http: Arc<FakeHttp>,
    /// How many HTTP requests had been made when `gws auth logout` ran.
    http_at_logout: Arc<Mutex<Option<usize>>>,
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
    let http_at_logout = Arc::new(Mutex::new(None));
    let (seen, http_seen) = (http_at_logout.clone(), http.clone());
    runner.on_with(&["auth", "logout"], move |command| {
        *seen.lock().unwrap() = Some(http_seen.requests().len());
        let dir = PathBuf::from(&command.env["GOOGLE_WORKSPACE_CLI_CONFIG_DIR"]);
        let _ = std::fs::remove_file(dir.join("credentials.enc"));
        FakeReply::ok(r#"{"status":"success"}"#)
    });
    Fixture {
        _home: home,
        _app_data: app_data,
        config_dir,
        runner,
        http,
        http_at_logout,
        core,
    }
}

impl Fixture {
    /// Simulates a fraccional-googleworkspace-mcp login: credentials plus the ownership marker.
    fn sign_in(&self) {
        let export = format!(
            r#"{{"type":"authorized_user","client_id":"id","client_secret":"secret","refresh_token":"{TOKEN}"}}"#
        );
        self.sign_in_with_export(FakeReply::ok(&export));
    }

    /// Like [`Fixture::sign_in`], with `gws auth export --unmasked` answering `export`.
    fn sign_in_with_export(&self, export: FakeReply) {
        self.runner.on(&["auth", "export", "--unmasked"], export);
        std::fs::create_dir_all(&self.config_dir).unwrap();
        std::fs::write(self.config_dir.join("credentials.enc"), "x").unwrap();
        write_marker(&self.config_dir).unwrap();
    }

    /// The gws calls after the version probe.
    fn gws_calls(&self) -> Vec<Vec<String>> {
        self.runner
            .args()
            .into_iter()
            .filter(|args| args != &["--version"])
            .collect()
    }

    fn signed_out(&self) -> bool {
        !self.config_dir.join("credentials.enc").exists()
            && !self.config_dir.join(MARKER_FILE).exists()
    }
}

fn revoke_post() -> HttpRequest {
    HttpRequest::PostForm {
        url: REVOKE_URL.into(),
        form: vec![("token".into(), TOKEN.into())],
    }
}

#[test]
fn exports_revokes_the_refresh_token_then_logs_out() {
    let f = fixture();
    f.sign_in();
    f.http.on(REVOKE_URL, HttpResponse::ok("{}"));

    assert_eq!(f.core.revoke_all(false).unwrap(), RevokeOutcome::Revoked);

    assert_eq!(
        f.gws_calls(),
        [vec!["auth", "export", "--unmasked"], vec!["auth", "logout"]]
    );
    assert_eq!(f.http.requests(), [revoke_post()]);
    assert_eq!(
        *f.http_at_logout.lock().unwrap(),
        Some(1),
        "revoke before logout"
    );
    assert!(f.signed_out());
}

#[test]
fn google_rejecting_the_revoke_still_logs_out_and_reports_it() {
    let f = fixture();
    f.sign_in();
    f.http.on(
        REVOKE_URL,
        HttpResponse::new(400, r#"{"error":"invalid_token"}"#),
    );

    let error = f.core.revoke_all(false).unwrap_err();

    assert!(matches!(error, CoreError::RevokeFailed(_)), "{error:?}");
    assert!(!error.to_string().contains(TOKEN));
    assert_eq!(f.gws_calls().last().unwrap(), &["auth", "logout"]);
    assert!(f.signed_out());
}

#[test]
fn google_unreachable_still_logs_out_and_reports_it() {
    let f = fixture();
    f.sign_in();
    f.http
        .on_error(REVOKE_URL, CoreError::Http("offline".into()));

    let error = f.core.revoke_all(false).unwrap_err();

    assert!(matches!(error, CoreError::RevokeFailed(_)), "{error:?}");
    assert!(error.to_string().contains("local credentials were cleared"));
    assert_eq!(f.gws_calls().last().unwrap(), &["auth", "logout"]);
    assert!(f.signed_out());
}

#[test]
fn without_credentials_only_logs_out() {
    let f = fixture();

    assert_eq!(f.core.revoke_all(false).unwrap(), RevokeOutcome::Revoked);

    assert_eq!(f.gws_calls(), [vec!["auth", "logout"]]);
    assert!(f.http.requests().is_empty());
}

#[test]
fn unreadable_credentials_still_log_out_and_report_it() {
    let f = fixture();
    f.sign_in_with_export(FakeReply::exit(
        1,
        "",
        "Error: failed to decrypt credentials",
    ));

    let error = f.core.revoke_all(false).unwrap_err();

    assert!(matches!(error, CoreError::RevokeFailed(_)), "{error:?}");
    assert!(f.http.requests().is_empty());
    assert!(f.signed_out());
}

#[test]
fn without_gws_and_credentials_does_nothing_and_downloads_nothing() {
    let (home, app_data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let http = Arc::new(FakeHttp::new());
    let paths = Paths {
        home: home.path().into(),
        app_data: app_data.path().into(),
    };
    // No reply for `--version`: gws cannot start.
    let core = Core::new(paths, Arc::new(FakeRunner::new()), http.clone());

    assert_eq!(core.revoke_all(false).unwrap(), RevokeOutcome::Revoked);

    assert!(http.requests().is_empty());
}

#[test]
fn every_gws_call_uses_the_custom_folder() {
    let f = fixture();
    f.sign_in();
    f.http.on(REVOKE_URL, HttpResponse::ok("{}"));

    assert_eq!(f.core.revoke_all(false).unwrap(), RevokeOutcome::Revoked);

    let gws_calls: Vec<_> = f
        .runner
        .calls()
        .into_iter()
        .filter(|call| call.command.args != ["--version"])
        .collect();
    assert_eq!(gws_calls.len(), 2);
    for call in gws_calls {
        assert_eq!(
            call.command.env["GOOGLE_WORKSPACE_CLI_CONFIG_DIR"],
            f.config_dir.to_str().unwrap()
        );
    }
}

impl Fixture {
    /// Credentials created by another app: no ownership marker.
    fn foreign_sign_in(&self) {
        self.sign_in();
        std::fs::remove_file(self.config_dir.join(MARKER_FILE)).unwrap();
    }
}

#[test]
fn foreign_credentials_without_confirmation_ask_first_and_run_nothing() {
    let f = fixture();
    f.foreign_sign_in();

    let outcome = f.core.revoke_all(false).unwrap();

    assert_eq!(outcome, RevokeOutcome::NeedsForeignConfirmation);
    assert!(f.runner.calls().is_empty());
    assert!(f.http.requests().is_empty());
    assert!(f.config_dir.join("credentials.enc").exists());
}

#[test]
fn confirmed_foreign_credentials_are_revoked_and_logged_out() {
    let f = fixture();
    f.foreign_sign_in();
    f.http.on(REVOKE_URL, HttpResponse::ok("{}"));

    let outcome = f.core.revoke_all(true).unwrap();

    assert_eq!(outcome, RevokeOutcome::Revoked);
    assert_eq!(f.http.requests(), [revoke_post()]);
    assert!(f.signed_out());
}
