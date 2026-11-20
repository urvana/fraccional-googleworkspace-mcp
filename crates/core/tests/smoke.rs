//! Smoke test: the core builds from injected ports and temp dirs, like the Tauri layer builds it.

use std::sync::Arc;

use haulmer_googleworkspace_mcp_core::testing::{FakeHttp, FakeRunner};
use haulmer_googleworkspace_mcp_core::{Core, Paths};

#[test]
fn core_builds_with_fakes_and_temp_dirs_and_is_shareable_across_threads() {
    let home = tempfile::tempdir().unwrap();
    let app_data = tempfile::tempdir().unwrap();
    let paths = Paths {
        home: home.path().to_path_buf(),
        app_data: app_data.path().to_path_buf(),
    };

    let core = Arc::new(Core::new(
        paths.clone(),
        Arc::new(FakeRunner::new()),
        Arc::new(FakeHttp::new()),
    ));

    let from_thread = {
        let core = Arc::clone(&core);
        std::thread::spawn(move || core.paths().clone())
            .join()
            .unwrap()
    };
    assert_eq!(from_thread, paths);
}
