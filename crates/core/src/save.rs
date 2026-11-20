//! T07 — `Core::save()` and `Core::cancel_login()`: connect, add and remove services.

use std::path::Path;

use crate::{
    BusinessSelection, Core, CoreError, OutputLine, SaveOutcome, Selection, gws_env, scopes,
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
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use haulmer_googleworkspace_mcp_core::{scopes, Core, Paths};
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
                self.revoke_all(false)?;
            }
            return Ok(SaveOutcome::Connected(self.status()?));
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
            self.revoke_and_logout(&gws)?;
            // The old access is gone: a sign-in that does not finish leaves nothing connected.
            self.login(&gws, &selection, &business_selection, on_auth_url)
                .map_err(|_| CoreError::AccessRemoved)?;
        } else {
            self.login(&gws, &selection, &business_selection, on_auth_url)?;
        }
        gws_env::write_marker(&config_dir)?;
        Ok(SaveOutcome::Connected(self.status()?))
    }

    /// Kills the running login, if any; the blocked `save` returns [`CoreError::Cancelled`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use haulmer_googleworkspace_mcp_core::{Core, Paths};
    /// # let core = Core::system(Paths { home: "/h".into(), app_data: "/a".into() });
    /// core.cancel_login().unwrap();
    /// ```
    pub fn cancel_login(&self) -> Result<(), CoreError> {
        let killer = self.active_login.lock().unwrap().take();
        if let Some(killer) = killer {
            killer.kill();
        }
        Ok(())
    }

    /// Runs `gws auth login --scopes <urls>`, forwards the first sign-in URL and waits.
    /// [`Core::cancel_login`] taking the killer while it runs means it was cancelled.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let gws = self.ensure_gws()?;
    /// self.login(&gws, &selection, &|url| println!("open {url}"))?;
    /// ```
    fn login(
        &self,
        gws: &Path,
        selection: &Selection,
        business_selection: &BusinessSelection,
        on_auth_url: &dyn Fn(&str),
    ) -> Result<(), CoreError> {
        let mut requested_scopes = scopes::scopes_for(selection);
        requested_scopes.extend(scopes::business_scopes_for(business_selection));
        let scopes = requested_scopes.join(",");
        let command = self.gws_command(gws, &["auth", "login", "--scopes", &scopes])?;
        let mut process = self.runner.spawn(&command)?;
        *self.active_login.lock().unwrap() = Some(process.killer());

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
        let exit_code = process.wait();
        let cancelled = self.active_login.lock().unwrap().take().is_none();
        let exit_code = exit_code?;
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
