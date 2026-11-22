//! T07 — `Core::save()` and `Core::cancel_login()`: connect, add and remove services.

use std::path::Path;

use crate::{
    AccessOperation, BusinessSelection, CancelKillGuard, Core, CoreError, OutputLine, SaveOutcome,
    Selection, gws_env, scopes,
};

impl Core {
    /// Applies the selection: may download gws, asks for replace confirmation on foreign
    /// credentials, revokes first when a granted level is removed, then runs the gws login.
    /// An empty selection revokes everything (like [`Core::revoke_all`]) and signs in to nothing.
    /// A foreign token is never revoked: with `confirm_replace` the login overwrites it.
    /// Calls `on_auth_url` with the Google sign-in URL (the caller opens the browser).
    /// Blocks until the login ends or [`Core::cancel_login`] is called ([`CoreError::Cancelled`]).
    /// If the old access was revoked first and the login is then cancelled or fails, returns
    /// [`CoreError::AccessRemoved`] (nothing is connected any more).
    /// Returns [`CoreError::TooManyScopes`] before changing anything when the selection needs
    /// more than [`scopes::MAX_LOGIN_SCOPES`] scopes.
    /// Returns [`CoreError::AccessInProgress`] if this Core is already saving or revoking.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use fraccional_googleworkspace_mcp_core::{scopes, Core, Paths};
    /// # let core = Core::system(Paths { home: "/h".into(), app_data: "/a".into() });
    /// let outcome = core.save(&scopes::default_selection(), false, &|url| println!("open {url}"));
    /// ```
    pub fn save(
        &self,
        selection: &Selection,
        confirm_replace: bool,
        on_auth_url: &dyn Fn(&str),
    ) -> Result<SaveOutcome, CoreError> {
        self.save_with_business(
            selection,
            &BusinessSelection::new(),
            confirm_replace,
            on_auth_url,
        )
    }

    /// Applies Workspace and business connector access in one shared OAuth login.
    pub fn save_with_business(
        &self,
        selection: &Selection,
        business_selection: &BusinessSelection,
        confirm_replace: bool,
        on_auth_url: &dyn Fn(&str),
    ) -> Result<SaveOutcome, CoreError> {
        self.save_with_business_on_start(
            selection,
            business_selection,
            confirm_replace,
            &|| {},
            on_auth_url,
        )
    }

    /// Like [`Core::save_with_business`], calling `on_start` only after reserving access.
    pub fn save_with_business_on_start(
        &self,
        selection: &Selection,
        business_selection: &BusinessSelection,
        confirm_replace: bool,
        on_start: &dyn Fn(),
        on_auth_url: &dyn Fn(&str),
    ) -> Result<SaveOutcome, CoreError> {
        self.save_with_business_identified(
            selection,
            business_selection,
            confirm_replace,
            None,
            on_start,
            on_auth_url,
        )
    }

    /// Like [`Core::save_with_business_on_start`], with an optional cancellation identifier.
    pub fn save_with_business_identified(
        &self,
        selection: &Selection,
        business_selection: &BusinessSelection,
        confirm_replace: bool,
        id: Option<&str>,
        on_start: &dyn Fn(),
        on_auth_url: &dyn Fn(&str),
    ) -> Result<SaveOutcome, CoreError> {
        let _operation = self.begin_access(AccessOperation::Save {
            id: id.map(str::to_owned),
            cancelled: false,
            killer: None,
            cancel_in_flight: false,
            finished: false,
        })?;
        on_start();
        self.check_cancelled()?;
        let config_dir = self.settings()?.active_config_dir(&self.paths);
        let foreign = gws_env::foreign_credentials(&config_dir);
        if foreign && !confirm_replace {
            return Ok(SaveOutcome::NeedsReplaceConfirmation);
        }
        let selection = scopes::normalize(selection);
        let business_selection = scopes::business_normalize(business_selection);
        if selection.is_empty() && business_selection.is_empty() {
            if !foreign {
                // Own credentials only (checked above), so no confirmation is involved.
                let had_credentials = gws_env::credentials_exist(&config_dir);
                self.check_cancelled()?;
                self.revoke_all_inner(false, true)?;
                if self.check_cancelled().is_err() {
                    return Err(if had_credentials {
                        CoreError::AccessRemoved
                    } else {
                        CoreError::Cancelled
                    });
                }
            }
            return Ok(SaveOutcome::Connected(self.status()?));
        }
        // Checked before any revocation: a refused login must leave the old access in place.
        let requested_scopes = scopes::login_scopes(&selection, &business_selection);
        if requested_scopes.len() > scopes::MAX_LOGIN_SCOPES {
            return Err(CoreError::TooManyScopes {
                requested: requested_scopes.len(),
                max: scopes::MAX_LOGIN_SCOPES,
            });
        }
        let gws = self.ensure_gws()?;
        let (granted, business_granted) = if foreign {
            (Selection::new(), BusinessSelection::new())
        } else {
            let status = self.status()?;
            (status.granted, status.business_granted)
        };
        let removes_a_level = granted
            .iter()
            .any(|level| !selection.contains(level.service, level.level));
        let removes_business_level = business_granted
            .iter()
            .any(|level| !business_selection.contains(level.connector, level.level));
        if removes_a_level || removes_business_level {
            self.check_cancelled()?;
            self.revoke_and_logout(&gws, true)?;
            // The old access is gone: a sign-in that does not finish leaves nothing connected.
            self.login(&gws, &requested_scopes, on_auth_url)
                .map_err(|_| CoreError::AccessRemoved)?;
        } else {
            self.login(&gws, &requested_scopes, on_auth_url)?;
        }
        gws_env::write_marker(&config_dir)?;
        Ok(SaveOutcome::Connected(self.status()?))
    }

    /// Cancels an active save, killing its login if running. An active revocation is not stopped.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use fraccional_googleworkspace_mcp_core::{Core, Paths};
    /// # let core = Core::system(Paths { home: "/h".into(), app_data: "/a".into() });
    /// core.cancel_login().unwrap();
    /// ```
    pub fn cancel_login(&self) -> Result<(), CoreError> {
        self.cancel_login_matching(None)
    }

    /// Cancels only the save with this identifier, if it is still active.
    pub fn cancel_login_for(&self, id: &str) -> Result<(), CoreError> {
        self.cancel_login_matching(Some(id))
    }

    fn cancel_login_matching(&self, id: Option<&str>) -> Result<(), CoreError> {
        let killer = {
            let mut active = self
                .access_operation
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            match active.as_mut() {
                Some(AccessOperation::Save {
                    id: active_id,
                    cancelled,
                    killer,
                    cancel_in_flight,
                    ..
                }) if id.is_none_or(|id| active_id.as_deref() == Some(id)) => {
                    *cancelled = true;
                    let killer = killer.take();
                    if killer.is_some() {
                        *cancel_in_flight = true;
                    }
                    killer
                }
                _ => None,
            }
        };
        if let Some(killer) = killer {
            let guard = CancelKillGuard { core: self, killer };
            guard.killer.kill();
        }
        Ok(())
    }

    /// Runs `gws auth login --scopes <urls>`, forwards the first sign-in URL and waits.
    /// Registers the killer even if cancellation raced with process startup.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let gws = self.ensure_gws()?;
    /// let scopes = scopes::login_scopes(&selection, &business_selection);
    /// self.login(&gws, &scopes, &|url| println!("open {url}"))?;
    /// ```
    fn login(
        &self,
        gws: &Path,
        requested_scopes: &[String],
        on_auth_url: &dyn Fn(&str),
    ) -> Result<(), CoreError> {
        let scopes = requested_scopes.join(",");
        let command = self.gws_command(gws, &["auth", "login", "--scopes", &scopes])?;
        // This check commits startup; later cancellation kills the process after spawn returns.
        self.check_cancelled()?;
        let mut process = self.runner.spawn(&command)?;
        let killer = process.killer();
        let cancelled = {
            let mut active = self
                .access_operation
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            match active.as_mut() {
                Some(AccessOperation::Save {
                    cancelled,
                    killer: slot,
                    ..
                }) => {
                    *slot = Some(killer.clone());
                    *cancelled
                }
                _ => unreachable!("login requires an active save"),
            }
        };
        if cancelled {
            self.cancel_login()?;
        }

        let (mut stdout, mut url_sent) = (String::new(), false);
        while let Some(line) = process.next_line() {
            let text = match line {
                OutputLine::Stdout(text) => {
                    stdout.push_str(&text);
                    stdout.push('\n');
                    text
                }
                OutputLine::Stderr(text) => text,
            };
            if !url_sent && let Some(url) = auth_url(&text) {
                url_sent = true;
                on_auth_url(url);
            }
        }
        let exit_code = process.wait()?;
        let cancelled = {
            let mut active = self
                .access_operation
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            match active.as_mut() {
                Some(AccessOperation::Save {
                    cancelled, killer, ..
                }) => {
                    killer.take();
                    *cancelled
                }
                _ => unreachable!("login requires an active save"),
            }
        };
        if exit_code == 0 {
            return Ok(());
        }
        if cancelled {
            return Err(CoreError::Cancelled);
        }
        Err(CoreError::Gws {
            exit_code,
            message: gws_env::gws_error_message(&stdout, &stdout),
        })
    }
}

/// The Google sign-in URL in a gws output line, without the surrounding text.
///
/// # Example
///
/// ```ignore
/// let line = "  https://accounts.google.com/o/oauth2/auth?x=1 (open it)";
/// assert_eq!(auth_url(line), Some("https://accounts.google.com/o/oauth2/auth?x=1"));
/// assert_eq!(auth_url("Waiting..."), None);
/// ```
fn auth_url(line: &str) -> Option<&str> {
    let start = line.find("https://accounts.google.com/")?;
    line[start..].split_whitespace().next()
}
