//! System notifications for the end of `save` and `revoke_all`, so the person sees the result
//! even when the window is hidden behind the browser that just finished the Google sign-in.

use fraccional_googleworkspace_mcp_core::{CoreError, RevokeOutcome, SaveOutcome};
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

/// Title of every notification: the app name.
const TITLE: &str = "fraccional-googleworkspace-mcp";

/// Text of the notification for the end of `save`; `None` when the window already asks the person
/// something (a replace confirmation) or the person cancelled the sign-in.
///
/// `removed_everything` is true when the saved selection had no service at all.
///
/// # Example
///
/// ```ignore
/// assert_eq!(save_text(&Err(CoreError::Cancelled), false), None);
/// ```
pub(crate) fn save_text(
    result: &Result<SaveOutcome, CoreError>,
    removed_everything: bool,
) -> Option<String> {
    match result {
        Ok(SaveOutcome::Connected(status)) => match &status.account {
            Some(account) => Some(format!("Conectado como {account}")),
            None if removed_everything => Some("Se eliminó todo el acceso".into()),
            // The login ended but no account is stored: never report it as a removal.
            None => Some("No se encontró la cuenta. Vuelve a intentarlo.".into()),
        },
        Ok(SaveOutcome::NeedsReplaceConfirmation) => None,
        Err(CoreError::Cancelled | CoreError::AccessInProgress) => None,
        Err(error) => Some(format!("No se pudo guardar: {error}")),
    }
}

/// Text of the notification for the end of `revoke_all`; `None` when the window asks for a
/// confirmation first.
///
/// # Example
///
/// ```ignore
/// assert_eq!(revoke_text(&Ok(RevokeOutcome::Revoked)).as_deref(), Some("Se revocó todo el acceso"));
/// ```
pub(crate) fn revoke_text(result: &Result<RevokeOutcome, CoreError>) -> Option<String> {
    match result {
        Ok(RevokeOutcome::Revoked) => Some("Se revocó todo el acceso".into()),
        Ok(RevokeOutcome::NeedsForeignConfirmation) => None,
        Err(CoreError::AccessInProgress) => None,
        Err(error) => Some(format!("No se pudo revocar: {error}")),
    }
}

/// Shows `text` as a system notification. A failure (for example, the system denied the
/// permission) is logged and never breaks the command that called it.
///
/// # Example
///
/// ```ignore
/// if let Some(text) = save_text(&result, false) {
///     notify(&app, &text);
/// }
/// ```
pub(crate) fn notify(app: &AppHandle, text: &str) {
    if let Err(error) = app.notification().builder().title(TITLE).body(text).show() {
        eprintln!("Could not show notification: {error}");
    }
}

#[cfg(test)]
mod tests {
    use fraccional_googleworkspace_mcp_core::{
        BusinessSelection, CoreError, KeyStorage, RevokeOutcome, SaveOutcome, Selection, Status,
    };

    use super::{revoke_text, save_text};

    fn connected(account: Option<&str>) -> Result<SaveOutcome, CoreError> {
        Ok(SaveOutcome::Connected(Status {
            gws_available: true,
            account: account.map(str::to_owned),
            granted: Selection::new(),
            business_granted: BusinessSelection::new(),
            token_valid: account.is_some(),
            config_dir: "/tmp/gws".into(),
            config_dir_is_default: true,
            key_storage: KeyStorage::Auto,
            resolved_key_storage: KeyStorage::Keyring,
            foreign_credentials: false,
        }))
    }

    #[test]
    fn save_with_account_names_it() {
        let text = save_text(&connected(Some("ana@fraccional.com")), false);

        assert_eq!(text.as_deref(), Some("Conectado como ana@fraccional.com"));
    }

    #[test]
    fn save_without_account_and_empty_selection_reports_removal() {
        let text = save_text(&connected(None), true);

        assert_eq!(text.as_deref(), Some("Se eliminó todo el acceso"));
    }

    #[test]
    fn save_without_account_and_selection_reports_error_not_removal() {
        let text = save_text(&connected(None), false);

        assert_eq!(
            text.as_deref(),
            Some("No se encontró la cuenta. Vuelve a intentarlo.")
        );
    }

    #[test]
    fn save_stays_silent_when_the_person_decides() {
        assert_eq!(
            save_text(&Ok(SaveOutcome::NeedsReplaceConfirmation), false),
            None
        );
        assert_eq!(save_text(&Err(CoreError::Cancelled), false), None);
        assert_eq!(save_text(&Err(CoreError::AccessInProgress), false), None);
    }

    #[test]
    fn save_failure_includes_the_error() {
        let text = save_text(&Err(CoreError::Io("disk full".into())), false);

        assert_eq!(
            text.as_deref(),
            Some("No se pudo guardar: file error: disk full")
        );
    }

    #[test]
    fn revoke_reports_each_outcome() {
        assert_eq!(
            revoke_text(&Ok(RevokeOutcome::Revoked)).as_deref(),
            Some("Se revocó todo el acceso")
        );
        assert_eq!(
            revoke_text(&Ok(RevokeOutcome::NeedsForeignConfirmation)),
            None
        );
        assert_eq!(
            revoke_text(&Err(CoreError::Io("offline".into()))).as_deref(),
            Some("No se pudo revocar: file error: offline")
        );
        assert_eq!(revoke_text(&Err(CoreError::AccessInProgress)), None);
    }
}
