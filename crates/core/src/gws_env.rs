//! T04 — the environment and working folder of every gws call, plus credential helpers.

use std::path::Path;

use crate::oauth_client::CLIENT_ID;
use crate::{CommandSpec, Core, CoreError};

/// Ownership marker written into the config folder after a successful fraccional-googleworkspace-mcp login.
///
/// # Example
///
/// ```
/// assert_eq!(haulmer_googleworkspace_mcp_core::gws_env::MARKER_FILE, "fraccional-googleworkspace-mcp.json");
/// ```
pub const MARKER_FILE: &str = "fraccional-googleworkspace-mcp.json";

/// Variables removed from every gws child: they would bypass the config folder's credentials
/// (gws falls back to gcloud ADC, a credentials file or a raw token).
const REMOVED_ENV: [&str; 3] = [
    "GOOGLE_APPLICATION_CREDENTIALS",
    "GOOGLE_WORKSPACE_CLI_CREDENTIALS_FILE",
    "GOOGLE_WORKSPACE_CLI_TOKEN",
];

impl Core {
    /// Builds the command for one gws call: `program args...` with the gws env
    /// (config folder, key backend, client id/secret, `NO_COLOR=1`, removed credential
    /// overrides) and cwd `<app_data>/run` (created if missing).
    ///
    /// # Example
    ///
    /// ```
    /// # use haulmer_googleworkspace_mcp_core::{Core, Paths};
    /// # let app_data = tempfile::tempdir().unwrap();
    /// # let core = Core::system(Paths { home: "/h".into(), app_data: app_data.path().into() });
    /// let command = core.gws_command("gws".as_ref(), &["auth", "status"]).unwrap();
    /// assert_eq!(command.env["GOOGLE_WORKSPACE_CLI_KEYRING_BACKEND"], "keyring");
    /// assert!(command.cwd.ends_with("run"));
    /// ```
    pub fn gws_command(&self, program: &Path, args: &[&str]) -> Result<CommandSpec, CoreError> {
        let settings = self.settings()?;
        let cwd = self.paths.app_data.join("run");
        std::fs::create_dir_all(&cwd)?;

        let mut command = CommandSpec::new(program, args, cwd);
        // The folder always comes from the settings, never from the parent environment:
        // `status` reads credentials from `active_config_dir`, so gws must use that same folder.
        if settings.config_dir.is_none() {
            command
                .env_remove
                .insert("GOOGLE_WORKSPACE_CLI_CONFIG_DIR".to_string());
        }
        let mut set = |name: &str, value: &str| {
            command.env.insert(name.to_string(), value.to_string());
        };
        if let Some(dir) = &settings.config_dir {
            set("GOOGLE_WORKSPACE_CLI_CONFIG_DIR", &dir.to_string_lossy());
        }
        set(
            "GOOGLE_WORKSPACE_CLI_KEYRING_BACKEND",
            settings.resolved_key_storage().backend(),
        );
        set("GOOGLE_WORKSPACE_CLI_CLIENT_ID", CLIENT_ID);
        set("NO_COLOR", "1");
        command
            .env_remove
            .extend(REMOVED_ENV.iter().map(|name| name.to_string()));
        Ok(command)
    }
}

/// Whether gws credentials exist in `config_dir` (`credentials.enc` or `credentials.json`).
///
/// # Example
///
/// ```
/// use haulmer_googleworkspace_mcp_core::gws_env::credentials_exist;
/// assert!(!credentials_exist("/nonexistent".as_ref()));
/// ```
pub fn credentials_exist(config_dir: &Path) -> bool {
    ["credentials.enc", "credentials.json"]
        .iter()
        .any(|name| config_dir.join(name).is_file())
}

/// Whether `config_dir` holds credentials not created by fraccional-googleworkspace-mcp (no marker).
///
/// # Example
///
/// ```
/// use haulmer_googleworkspace_mcp_core::gws_env::foreign_credentials;
/// assert!(!foreign_credentials("/nonexistent".as_ref()));
/// ```
pub fn foreign_credentials(config_dir: &Path) -> bool {
    credentials_exist(config_dir) && !config_dir.join(MARKER_FILE).is_file()
}

/// Writes the ownership marker [`MARKER_FILE`] into `config_dir` (created if missing).
///
/// # Example
///
/// ```
/// let dir = tempfile::tempdir().unwrap();
/// haulmer_googleworkspace_mcp_core::gws_env::write_marker(dir.path()).unwrap();
/// assert!(dir.path().join(haulmer_googleworkspace_mcp_core::gws_env::MARKER_FILE).is_file());
/// ```
pub fn write_marker(config_dir: &Path) -> Result<(), CoreError> {
    std::fs::create_dir_all(config_dir)?;
    std::fs::write(
        config_dir.join(MARKER_FILE),
        "{\"created_by\":\"fraccional-googleworkspace-mcp\"}\n",
    )?;
    Ok(())
}

/// Removes the ownership marker from `config_dir` (no error if it is absent).
///
/// # Example
///
/// ```
/// let dir = tempfile::tempdir().unwrap();
/// haulmer_googleworkspace_mcp_core::gws_env::remove_marker(dir.path()).unwrap();
/// ```
pub fn remove_marker(config_dir: &Path) -> Result<(), CoreError> {
    match std::fs::remove_file(config_dir.join(MARKER_FILE)) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

/// The message of a failed gws call: `error.message` from gws's JSON error on `stdout`
/// (`{"error":{"message":"..."}}`), else `fallback` trimmed.
///
/// # Example
///
/// ```ignore
/// let stdout = r#"{"error":{"code":401,"message":"No credentials"}}"#;
/// assert_eq!(gws_error_message(stdout, "ignored"), "No credentials");
/// assert_eq!(gws_error_message("not json", " stderr text\n"), "stderr text");
/// ```
pub(crate) fn gws_error_message(stdout: &str, fallback: &str) -> String {
    let json: serde_json::Value = serde_json::from_str(stdout).unwrap_or_default();
    match json["error"]["message"].as_str() {
        Some(message) => message.to_string(),
        None => fallback.trim().to_string(),
    }
}
