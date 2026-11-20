//! The Install prompt: the Spanish text a person pastes into an Agent (Claude Code, Codex, ...)
//! so that the Agent registers `fraccional-googleworkspace-mcp mcp` as a local MCP server by itself.

use std::path::Path;

use serde::Serialize;

use crate::CoreError;
use crate::skills::skills_add_command;

/// The text to copy, plus a warning to show when the path may not last.
///
/// # Example
///
/// ```
/// use haulmer_googleworkspace_mcp_core::mcp_clients::InstallPrompt;
/// let prompt = InstallPrompt { text: "...".into(), warning: None };
/// assert_eq!(serde_json::to_string(&prompt).unwrap(), r#"{"text":"...","warning":null}"#);
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct InstallPrompt {
    /// The prompt, in Spanish.
    pub text: String,
    /// A Spanish warning for the person; the prompt is still usable.
    pub warning: Option<String>,
}

/// The Spanish Install prompt with `exe` as the MCP server command, and the core service skills
/// ([`skills_add_command`]) as an optional last step.
///
/// The path is checked with [`check_exe_path`]: it fails for a translocated path and warns for
/// `/Volumes/`. It uses `exe` as given. Never canonicalize it: on Windows that returns `\\?\`
/// paths.
///
/// # Example
///
/// ```
/// use haulmer_googleworkspace_mcp_core::mcp_clients::install_prompt;
/// let installed = "/Applications/fraccional-googleworkspace-mcp.app/Contents/MacOS/fraccional-googleworkspace-mcp";
/// let prompt = install_prompt(installed.as_ref()).unwrap();
/// assert!(prompt.text.contains(&format!("-- \"{installed}\" mcp")));
/// assert!(prompt.warning.is_none());
///
/// let translocated = "/private/var/folders/x/T/AppTranslocation/AB12/d/fraccional-googleworkspace-mcp.app/Contents/MacOS/fraccional-googleworkspace-mcp";
/// assert!(install_prompt(translocated.as_ref()).is_err());
/// ```
pub fn install_prompt(exe: &Path) -> Result<InstallPrompt, CoreError> {
    let warning = check_exe_path(exe)?;
    let exe = exe.to_string_lossy();
    let text = format!(
        include_str!("install_prompt.es.txt"),
        exe = exe,
        skills_command = skills_add_command(),
    );
    Ok(InstallPrompt { text, warning })
}

/// Checks that the app path will last, for the Install prompt and the MCP extension.
///
/// Apple has no supported API to detect app translocation. So this function reads the path text.
/// It fails for a translocated path. It returns a warning for `/Volumes/`, which is the mounted
/// disk image or a working external drive. The warning is `None` for any other path.
///
/// # Example
///
/// ```
/// use haulmer_googleworkspace_mcp_core::mcp_clients::check_exe_path;
/// let installed = "/Applications/fraccional-googleworkspace-mcp.app/Contents/MacOS/fraccional-googleworkspace-mcp";
/// assert_eq!(check_exe_path(installed.as_ref()), Ok(None));
///
/// let mounted = "/Volumes/fraccional-googleworkspace-mcp/fraccional-googleworkspace-mcp.app/Contents/MacOS/fraccional-googleworkspace-mcp";
/// assert!(check_exe_path(mounted.as_ref()).unwrap().is_some());
///
/// let translocated = "/private/var/folders/x/T/AppTranslocation/AB12/d/fraccional-googleworkspace-mcp.app/Contents/MacOS/fraccional-googleworkspace-mcp";
/// assert!(check_exe_path(translocated.as_ref()).is_err());
/// ```
pub fn check_exe_path(exe: &Path) -> Result<Option<String>, CoreError> {
    let exe = exe.to_string_lossy();
    if exe.contains("/AppTranslocation/") {
        return Err(CoreError::UnstableExe(
            "macOS está ejecutando fraccional-googleworkspace-mcp desde una carpeta temporal. Muévela a \
             Aplicaciones, ábrela de nuevo y vuelve a abrir esta ventana."
                .to_string(),
        ));
    }
    let mut warning = None;
    if exe.starts_with("/Volumes/") {
        warning = Some(
            "Si abriste fraccional-googleworkspace-mcp desde la imagen de disco (.dmg), muévela a Aplicaciones \
             y ábrela de nuevo: esta ruta deja de existir al expulsar el disco."
                .to_string(),
        );
    }
    Ok(warning)
}
