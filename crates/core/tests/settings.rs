//! T04 — persisted settings and their resolution.

use std::path::PathBuf;
use std::sync::Arc;

use haulmer_googleworkspace_mcp_core::testing::{FakeHttp, FakeRunner};
use haulmer_googleworkspace_mcp_core::{Core, KeyStorage, Paths, Settings};
use tempfile::TempDir;

/// A core over a temp home and temp app data.
fn core_in(home: &TempDir, app_data: &TempDir) -> Core {
    let paths = Paths {
        home: home.path().into(),
        app_data: app_data.path().into(),
    };
    Core::new(
        paths,
        Arc::new(FakeRunner::new()),
        Arc::new(FakeHttp::new()),
    )
}

#[test]
fn settings_default_when_nothing_was_saved() {
    let (home, app_data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    assert_eq!(
        core_in(&home, &app_data).settings().unwrap(),
        Settings::default()
    );
}

#[test]
fn settings_persist_across_core_instances() {
    let (home, app_data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let custom = PathBuf::from("/somewhere/haulmer-gws");
    let core = core_in(&home, &app_data);
    core.set_config_dir(Some(custom.clone())).unwrap();
    core.set_key_storage(KeyStorage::Keyring).unwrap();

    let reopened = core_in(&home, &app_data).settings().unwrap();
    assert_eq!(
        reopened,
        Settings {
            config_dir: Some(custom),
            key_storage: KeyStorage::Keyring
        }
    );
}

#[test]
fn resetting_the_config_dir_keeps_the_key_storage() {
    let (home, app_data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let core = core_in(&home, &app_data);
    core.set_key_storage(KeyStorage::File).unwrap();
    core.set_config_dir(Some("/x".into())).unwrap();
    core.set_config_dir(None).unwrap();

    let settings = core_in(&home, &app_data).settings().unwrap();
    assert_eq!(settings.config_dir, None);
    assert_eq!(settings.key_storage, KeyStorage::File);
}

#[test]
fn settings_are_created_even_if_app_data_does_not_exist_yet() {
    let home = TempDir::new().unwrap();
    let parent = TempDir::new().unwrap();
    let paths = Paths {
        home: home.path().into(),
        app_data: parent.path().join("nested/app"),
    };
    let core = Core::new(
        paths.clone(),
        Arc::new(FakeRunner::new()),
        Arc::new(FakeHttp::new()),
    );
    core.set_key_storage(KeyStorage::File).unwrap();
    let reopened = Core::new(
        paths,
        Arc::new(FakeRunner::new()),
        Arc::new(FakeHttp::new()),
    );
    assert_eq!(reopened.settings().unwrap().key_storage, KeyStorage::File);
}
