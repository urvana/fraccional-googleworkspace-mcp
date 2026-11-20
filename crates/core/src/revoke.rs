//! T06 — `Core::revoke_all()`: revoke at Google, then clear local credentials.

use std::path::Path;

use crate::gws_env::{credentials_exist, foreign_credentials, remove_marker};
use crate::{Core, CoreError, RevokeOutcome};

/// Google's OAuth token revocation endpoint.
///
/// # Example
///
/// ```
/// assert!(haulmer_googleworkspace_mcp_core::revoke::REVOKE_URL.starts_with("https://oauth2.googleapis.com/"));
/// ```
pub const REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";

impl Core {
    /// `gws auth export --unmasked` -> POST the refresh token to [`REVOKE_URL`] ->
    /// `gws auth logout` -> remove the ownership marker, all in the active config folder.
    /// If Google fails (or the token cannot be read), still clears local state and returns
    /// [`CoreError::RevokeFailed`]. Without credentials it only logs out. Never downloads
    /// gws when there is nothing to revoke: no gws and no credentials is a success.
    /// Credentials created by another app (no ownership marker) are only touched with
    /// `confirm_foreign`; without it nothing runs and the result is
    /// [`RevokeOutcome::NeedsForeignConfirmation`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use haulmer_googleworkspace_mcp_core::{Core, Paths, RevokeOutcome};
    /// # let core = Core::system(Paths { home: "/h".into(), app_data: "/a".into() });
    /// if core.revoke_all(false).unwrap() == RevokeOutcome::NeedsForeignConfirmation {
    ///     // ask the person, then:
    ///     core.revoke_all(true).unwrap();
    /// }
    /// ```
    pub fn revoke_all(&self, confirm_foreign: bool) -> Result<RevokeOutcome, CoreError> {
        let config_dir = self.settings()?.active_config_dir(&self.paths);
        if foreign_credentials(&config_dir) && !confirm_foreign {
            return Ok(RevokeOutcome::NeedsForeignConfirmation);
        }
        let gws = match self.find_gws()? {
            Some(gws) => gws,
            None => {
                if !credentials_exist(&config_dir) {
                    remove_marker(&config_dir)?;
                    return Ok(RevokeOutcome::Revoked);
                }
                self.ensure_gws()?
            }
        };
        self.revoke_and_logout(&gws)?;
        Ok(RevokeOutcome::Revoked)
    }

    /// The body of [`Core::revoke_all`] for a caller that already resolved gws: `save` (T07)
    /// calls it before signing in again with a reduced selection. Same contract: revokes at
    /// Google if credentials exist, always runs `gws auth logout` and removes the marker;
    /// a Google failure is [`CoreError::RevokeFailed`] returned after local state is cleared.
    /// A logout that cannot run is returned as is and leaves the marker in place.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // inside the crate, e.g. in `save`:
    /// let gws = self.ensure_gws()?;
    /// self.revoke_and_logout(&gws)?;
    /// ```
    pub(crate) fn revoke_and_logout(&self, gws: &Path) -> Result<(), CoreError> {
        let config_dir = self.settings()?.active_config_dir(&self.paths);
        let revoked = if credentials_exist(&config_dir) {
            self.revoke_at_google(gws)
        } else {
            Ok(())
        };
        self.runner
            .run(&self.gws_command(gws, &["auth", "logout"])?)?;
        remove_marker(&config_dir)?;
        revoked.map_err(CoreError::RevokeFailed)
    }

    /// Exports the refresh token and POSTs it to [`REVOKE_URL`]. `Err` says why Google
    /// did not confirm; it never contains the token.
    fn revoke_at_google(&self, gws: &Path) -> Result<(), String> {
        let command = self
            .gws_command(gws, &["auth", "export", "--unmasked"])
            .map_err(|error| error.to_string())?;
        let export = self
            .runner
            .run(&command)
            .map_err(|error| error.to_string())?;
        let json: serde_json::Value = serde_json::from_str(&export.stdout).unwrap_or_default();
        let Some(token) = json["refresh_token"]
            .as_str()
            .filter(|_| export.exit_code == 0)
        else {
            return Err(format!(
                "could not read the refresh token (gws exit {})",
                export.exit_code
            ));
        };
        match self.http.post_form(REVOKE_URL, &[("token", token)]) {
            Ok(response) if response.status == 200 => Ok(()),
            Ok(response) => Err(format!("Google answered HTTP {}", response.status)),
            Err(error) => Err(error.to_string()),
        }
    }
}
