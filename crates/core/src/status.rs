//! T05 — `Core::status()`: what is connected.

use std::path::Path;

use serde_json::Value;

use crate::{Core, CoreError, Status, gws_env, scopes};

/// The fields of `gws auth status` JSON the app reads.
#[derive(Default)]
struct AuthStatus {
    user: Option<String>,
    scopes: Vec<String>,
    token_valid: bool,
}

impl Core {
    /// Reports gws availability (no download), the connected account, granted levels,
    /// the active config folder, key storage and foreign credentials.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use haulmer_googleworkspace_mcp_core::{Core, Paths};
    /// # let core = Core::system(Paths { home: "/h".into(), app_data: "/a".into() });
    /// let status = core.status().unwrap();
    /// println!("{:?}", status.account);
    /// ```
    pub fn status(&self) -> Result<Status, CoreError> {
        let settings = self.settings()?;
        let config_dir = settings.active_config_dir(&self.paths);
        let gws = self.find_gws()?;
        let gws_available = gws.is_some();
        // Without credentials in the folder, gws falls back to gcloud ADC: not our connection.
        let auth = match gws {
            Some(gws) if gws_env::credentials_exist(&config_dir) => self.auth_status(&gws)?,
            _ => AuthStatus::default(),
        };
        Ok(Status {
            gws_available,
            account: auth.user.filter(|user| !user.is_empty()),
            granted: scopes::selection_from_scopes(&auth.scopes),
            business_granted: scopes::business_selection_from_scopes(&auth.scopes),
            token_valid: auth.token_valid,
            config_dir_is_default: settings.is_default_config_dir(),
            key_storage: settings.key_storage,
            resolved_key_storage: settings.resolved_key_storage(),
            foreign_credentials: gws_env::foreign_credentials(&config_dir),
            config_dir,
        })
    }

    /// Runs `gws auth status` and parses its JSON. Missing, null or mistyped fields read as
    /// "not granted"; output that is not a JSON object is a [`CoreError::Parse`].
    ///
    /// # Example
    ///
    /// ```ignore
    /// let gws = self.ensure_gws()?;
    /// let auth = self.auth_status(&gws)?;
    /// println!("{:?} {}", auth.user, auth.token_valid);
    /// ```
    fn auth_status(&self, gws: &Path) -> Result<AuthStatus, CoreError> {
        let output = self
            .runner
            .run(&self.gws_command(gws, &["auth", "status"])?)?;
        if output.exit_code != 0 {
            return Err(CoreError::Gws {
                exit_code: output.exit_code,
                message: gws_env::gws_error_message(&output.stdout, &output.stderr),
            });
        }
        let json: Value = serde_json::from_str(&output.stdout)?;
        if !json.is_object() {
            return Err(CoreError::Parse(format!(
                "gws auth status: expected a JSON object, got {json}"
            )));
        }
        let scopes = json["scopes"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default();
        Ok(AuthStatus {
            user: json["user"].as_str().map(str::to_string),
            scopes: scopes
                .iter()
                .filter_map(|scope| scope.as_str().map(str::to_string))
                .collect(),
            token_valid: json["token_valid"].as_bool().unwrap_or(false),
        })
    }
}
