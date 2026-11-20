//! T05 — `Core::status()`: what is connected.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use haulmer_googleworkspace_mcp_core::gws_env::write_marker;
use haulmer_googleworkspace_mcp_core::testing::{FakeHttp, FakeReply, FakeRunner};
use haulmer_googleworkspace_mcp_core::{
    Core, CoreError, KeyStorage, Level, Paths, Selection, Service,
};
use tempfile::TempDir;

struct Setup {
    home: TempDir,
    _app_data: TempDir,
    runner: Arc<FakeRunner>,
    core: Core,
}

fn setup() -> Setup {
    let (home, app_data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let runner = Arc::new(FakeRunner::new());
    let paths = Paths {
        home: home.path().into(),
        app_data: app_data.path().into(),
    };
    let core = Core::new(paths, runner.clone(), Arc::new(FakeHttp::new()));
    Setup {
        home,
        _app_data: app_data,
        runner,
        core,
    }
}

/// gws 0.22.5 on PATH.
fn with_gws(s: &Setup) {
    s.runner
        .on_program("gws", &["--version"], FakeReply::ok("gws 0.22.5\n"));
}

/// Writes `credentials.enc` (and optionally the marker) into `dir`.
fn credentials_in(dir: &Path, ours: bool) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("credentials.enc"), b"x").unwrap();
    if ours {
        write_marker(dir).unwrap();
    }
}

fn default_dir(s: &Setup) -> PathBuf {
    s.home.path().join(".config").join("gws")
}

const CONNECTED: &str = r#"{
  "auth_method": "oauth2",
  "user": "ana@haulmer.com",
  "scopes": [
    "openid",
    "https://www.googleapis.com/auth/userinfo.email",
    "https://www.googleapis.com/auth/gmail.readonly",
    "https://www.googleapis.com/auth/calendar.readonly"
  ],
  "token_valid": true,
  "encrypted_credentials_exists": true
}"#;

#[test]
fn connected_account_reports_email_and_granted_levels() {
    let s = setup();
    with_gws(&s);
    credentials_in(&default_dir(&s), true);
    s.runner.on(&["auth", "status"], FakeReply::ok(CONNECTED));

    let status = s.core.status().unwrap();

    assert!(status.gws_available);
    assert_eq!(status.account.as_deref(), Some("ana@haulmer.com"));
    assert_eq!(
        status.granted,
        Selection::from_levels([
            (Service::Gmail, Level::Read),
            (Service::Calendar, Level::Read)
        ])
    );
    assert!(status.token_valid);
    assert_eq!(status.config_dir, default_dir(&s));
    assert!(status.config_dir_is_default);
    assert!(!status.foreign_credentials);
}

#[test]
fn missing_gws_is_reported_without_download_or_auth_call() {
    let s = setup();
    let http = Arc::new(FakeHttp::new());
    let core = Core::new(s.core.paths().clone(), s.runner.clone(), http.clone());
    credentials_in(&default_dir(&s), false);

    let status = core.status().unwrap();

    assert!(!status.gws_available);
    assert_eq!(status.account, None);
    assert!(status.granted.is_empty());
    assert!(!status.token_valid);
    assert!(status.foreign_credentials);
    assert!(http.requests().is_empty());
    assert!(
        s.runner
            .args()
            .iter()
            .all(|args| args != &["auth", "status"])
    );
}

#[test]
fn no_credentials_in_active_folder_is_not_connected_even_with_adc_fallback() {
    let s = setup();
    with_gws(&s);
    // gws found gcloud ADC and reported its account; the config folder holds no credentials.
    s.runner.on(&["auth", "status"], FakeReply::ok(CONNECTED));

    let status = s.core.status().unwrap();

    assert!(status.gws_available);
    assert_eq!(status.account, None);
    assert!(status.granted.is_empty());
    assert!(!status.token_valid);
    assert!(!status.foreign_credentials);
}

#[test]
fn custom_folder_status_call_carries_config_dir_and_file_backend() {
    let s = setup();
    with_gws(&s);
    let custom = s.home.path().join("haulmer-gws");
    s.core.set_config_dir(Some(custom.clone())).unwrap();
    credentials_in(&custom, true);
    s.runner.on(&["auth", "status"], FakeReply::ok(CONNECTED));

    let status = s.core.status().unwrap();

    let call = s
        .runner
        .calls()
        .into_iter()
        .find(|call| call.command.args == ["auth", "status"])
        .expect("gws auth status was called");
    let env = |name: &str| call.command.env.get(name).map(String::as_str);
    assert_eq!(
        env("GOOGLE_WORKSPACE_CLI_CONFIG_DIR"),
        Some(custom.to_str().unwrap())
    );
    assert_eq!(env("GOOGLE_WORKSPACE_CLI_KEYRING_BACKEND"), Some("file"));
    assert_eq!(status.config_dir, custom);
    assert!(!status.config_dir_is_default);
    assert_eq!(status.key_storage, KeyStorage::Auto);
    assert_eq!(status.resolved_key_storage, KeyStorage::File);
    assert_eq!(status.account.as_deref(), Some("ana@haulmer.com"));
}

#[test]
fn revoked_token_with_missing_or_null_fields_is_not_connected() {
    let s = setup();
    with_gws(&s);
    credentials_in(&default_dir(&s), true);
    s.runner.on(
        &["auth", "status"],
        FakeReply::ok(
            r#"{"auth_method":"oauth2","user":null,"scopes":null,"token_valid":false,"token_error":"Token has been expired or revoked."}"#,
        ),
    );

    let status = s.core.status().unwrap();

    assert_eq!(status.account, None);
    assert!(status.granted.is_empty());
    assert!(!status.token_valid);
}

#[test]
fn unreadable_auth_status_output_is_a_parse_error() {
    let s = setup();
    with_gws(&s);
    credentials_in(&default_dir(&s), true);
    s.runner.on(
        &["auth", "status"],
        FakeReply::ok("gws: something went wrong"),
    );

    assert!(matches!(s.core.status(), Err(CoreError::Parse(_))));
}

#[test]
fn failing_auth_status_is_a_gws_error_with_its_message() {
    let s = setup();
    with_gws(&s);
    credentials_in(&default_dir(&s), true);
    s.runner.on(
        &["auth", "status"],
        FakeReply::exit(
            1,
            r#"{"error":{"code":1,"message":"keyring unavailable"}}"#,
            "",
        ),
    );

    assert_eq!(
        s.core.status(),
        Err(CoreError::Gws {
            exit_code: 1,
            message: "keyring unavailable".into()
        })
    );
}
