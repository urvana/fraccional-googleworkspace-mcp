//! T04 — the environment and working folder of every gws call, plus credential helpers.

use std::path::Path;
use std::sync::Arc;

use haulmer_googleworkspace_mcp_core::gws_env::{
    MARKER_FILE, credentials_exist, foreign_credentials, remove_marker, write_marker,
};
use haulmer_googleworkspace_mcp_core::oauth_client::{CLIENT_ID, CLIENT_SECRET};
use haulmer_googleworkspace_mcp_core::testing::{FakeHttp, FakeRunner};
use haulmer_googleworkspace_mcp_core::{CommandSpec, Core, KeyStorage, Paths};
use tempfile::TempDir;

/// Temp home + temp app data and a core over them.
struct Fixture {
    home: TempDir,
    app_data: TempDir,
    core: Core,
}

fn fixture() -> Fixture {
    let (home, app_data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let paths = Paths {
        home: home.path().into(),
        app_data: app_data.path().into(),
    };
    let core = Core::new(
        paths,
        Arc::new(FakeRunner::new()),
        Arc::new(FakeHttp::new()),
    );
    Fixture {
        home,
        app_data,
        core,
    }
}

fn status_command(core: &Core) -> CommandSpec {
    core.gws_command(Path::new("gws"), &["auth", "status"])
        .unwrap()
}

fn env<'a>(command: &'a CommandSpec, name: &str) -> Option<&'a str> {
    command.env.get(name).map(String::as_str)
}

#[test]
fn custom_folder_passes_config_dir_and_file_backend() {
    let f = fixture();
    let custom = f.home.path().join("haulmer-gws");
    f.core.set_config_dir(Some(custom.clone())).unwrap();

    let command = status_command(&f.core);
    assert_eq!(
        env(&command, "GOOGLE_WORKSPACE_CLI_CONFIG_DIR"),
        Some(custom.to_str().unwrap())
    );
    assert_eq!(
        env(&command, "GOOGLE_WORKSPACE_CLI_KEYRING_BACKEND"),
        Some("file")
    );
}

#[test]
fn default_folder_passes_no_config_dir_and_keyring_backend() {
    let f = fixture();
    let command = status_command(&f.core);
    assert_eq!(env(&command, "GOOGLE_WORKSPACE_CLI_CONFIG_DIR"), None);
    assert_eq!(
        env(&command, "GOOGLE_WORKSPACE_CLI_KEYRING_BACKEND"),
        Some("keyring")
    );
}

#[test]
fn default_folder_removes_an_inherited_config_dir() {
    // An exported GOOGLE_WORKSPACE_CLI_CONFIG_DIR would make gws log in to a folder that
    // `status` never reads, so the app would report no account after a successful login.
    let f = fixture();
    let command = status_command(&f.core);
    assert!(
        command
            .env_remove
            .contains("GOOGLE_WORKSPACE_CLI_CONFIG_DIR")
    );
}

#[test]
fn custom_folder_does_not_remove_config_dir() {
    let f = fixture();
    f.core
        .set_config_dir(Some(f.home.path().join("haulmer-gws")))
        .unwrap();
    let command = status_command(&f.core);
    assert!(
        !command
            .env_remove
            .contains("GOOGLE_WORKSPACE_CLI_CONFIG_DIR")
    );
}

#[test]
fn explicit_key_storage_overrides_auto() {
    let f = fixture();
    f.core.set_key_storage(KeyStorage::File).unwrap();
    assert_eq!(
        env(
            &status_command(&f.core),
            "GOOGLE_WORKSPACE_CLI_KEYRING_BACKEND"
        ),
        Some("file")
    );

    f.core
        .set_config_dir(Some(f.home.path().join("custom")))
        .unwrap();
    f.core.set_key_storage(KeyStorage::Keyring).unwrap();
    assert_eq!(
        env(
            &status_command(&f.core),
            "GOOGLE_WORKSPACE_CLI_KEYRING_BACKEND"
        ),
        Some("keyring")
    );
}

#[test]
fn every_command_carries_client_no_color_and_removes_credential_overrides() {
    let f = fixture();
    let command = f
        .core
        .gws_command(Path::new("/opt/gws"), &["auth", "login", "--scopes", "x"])
        .unwrap();

    assert_eq!(command.program, Path::new("/opt/gws"));
    assert_eq!(command.args, ["auth", "login", "--scopes", "x"]);
    assert_eq!(
        env(&command, "GOOGLE_WORKSPACE_CLI_CLIENT_ID"),
        Some(CLIENT_ID)
    );
    assert_eq!(
        env(&command, "GOOGLE_WORKSPACE_CLI_CLIENT_SECRET"),
        Some(CLIENT_SECRET)
    );
    assert_eq!(env(&command, "NO_COLOR"), Some("1"));
    for removed in [
        "GOOGLE_APPLICATION_CREDENTIALS",
        "GOOGLE_WORKSPACE_CLI_CREDENTIALS_FILE",
        "GOOGLE_WORKSPACE_CLI_TOKEN",
    ] {
        assert!(
            command.env_remove.contains(removed),
            "{removed} not removed"
        );
    }
}

#[test]
fn commands_run_in_an_app_controlled_folder_that_exists() {
    let f = fixture();
    let command = status_command(&f.core);
    assert_eq!(command.cwd, f.app_data.path().join("run"));
    assert!(command.cwd.is_dir());
}

#[test]
fn credentials_exist_with_either_credentials_file() {
    let dir = TempDir::new().unwrap();
    assert!(!credentials_exist(dir.path()));
    std::fs::write(dir.path().join("credentials.enc"), b"x").unwrap();
    assert!(credentials_exist(dir.path()));

    let plain = TempDir::new().unwrap();
    std::fs::write(plain.path().join("credentials.json"), b"{}").unwrap();
    assert!(credentials_exist(plain.path()));
}

#[test]
fn foreign_credentials_are_credentials_without_our_marker() {
    let dir = TempDir::new().unwrap();
    assert!(!foreign_credentials(dir.path()), "no credentials");

    std::fs::write(dir.path().join("credentials.enc"), b"x").unwrap();
    assert!(foreign_credentials(dir.path()), "credentials, no marker");

    write_marker(dir.path()).unwrap();
    assert!(dir.path().join(MARKER_FILE).is_file());
    assert!(!foreign_credentials(dir.path()), "credentials with marker");

    remove_marker(dir.path()).unwrap();
    assert!(foreign_credentials(dir.path()), "marker removed");
}

#[test]
fn marker_can_be_written_into_a_missing_folder_and_removed_twice() {
    let parent = TempDir::new().unwrap();
    let dir = parent.path().join("new/gws");
    write_marker(&dir).unwrap();
    assert!(dir.join(MARKER_FILE).is_file());
    remove_marker(&dir).unwrap();
    remove_marker(&dir).unwrap();
    assert!(!dir.join(MARKER_FILE).exists());
}
