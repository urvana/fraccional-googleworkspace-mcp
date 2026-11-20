//! The MCP extension (`.mcpb`): the file Claude Desktop installs to register
//! `fraccional-googleworkspace-mcp mcp` as an MCP server without editing any JSON by hand.
//!
//! The bundle is thin. It does not ship the server binary: its manifest points at the installed
//! app, which `src-tauri` passes in as `exe`.

use std::io::{Cursor, Write};
use std::path::Path;

use serde_json::json;

use crate::CoreError;
use crate::mcp_clients::check_exe_path;

/// Spanish markdown for the "details" page of the extension in Claude Desktop.
const LONG_DESCRIPTION: &str = include_str!("mcp_extension_long_description.es.md");

/// The `.mcpb` file, plus a warning to show when the app path may not last.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::mcp_extension::McpExtension;
/// let extension = McpExtension { bytes: vec![0x50, 0x4b], warning: None };
/// assert_eq!(extension.bytes.len(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct McpExtension {
    /// The ZIP bytes of the `.mcpb`.
    pub bytes: Vec<u8>,
    /// A Spanish warning for the person (see [`check_exe_path`]); the extension is still usable.
    pub warning: Option<String>,
}

/// Builds the `.mcpb`: a ZIP with `manifest.json`, `icon.png` and `README.md`.
///
/// `exe` is checked with [`check_exe_path`] first. A translocated path is an error. A `/Volumes/`
/// path gives a warning.
///
/// The manifest uses `exe` as given. Never canonicalize it: on Windows that returns `\\?\`
/// paths. The manifest drops a final `.exe`, in any case. MCPB clients add `.exe` on Windows by
/// themselves, and Windows finds `name.exe` from `name` anyway.
///
/// `icon_png` is the content of `icon.png`.
///
/// # Example
///
/// ```
/// use fraccional_googleworkspace_mcp_core::mcp_extension::mcp_extension;
/// let exe = "/Applications/fraccional-googleworkspace-mcp.app/Contents/MacOS/fraccional-googleworkspace-mcp";
/// let extension = mcp_extension(exe.as_ref(), b"png bytes").unwrap();
/// assert!(extension.bytes.starts_with(b"PK"));
/// assert!(extension.warning.is_none());
/// ```
pub fn mcp_extension(exe: &Path, icon_png: &[u8]) -> Result<McpExtension, CoreError> {
    let warning = check_exe_path(exe)?;

    let exe = exe.to_string_lossy();
    let mut command = exe.as_ref();
    if command.len() >= 4 && command.is_char_boundary(command.len() - 4) {
        let (stem, suffix) = command.split_at(command.len() - 4);
        if suffix.eq_ignore_ascii_case(".exe") {
            command = stem;
        }
    }

    let manifest = serde_json::to_string_pretty(&json!({
        "manifest_version": "0.3",
        "name": "fraccional-googleworkspace-mcp",
        "display_name": "fraccional-googleworkspace-mcp",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Gmail, Google Calendar, Drive, Google Chat, Sheets, Forms y más de tu cuenta @fraccional.com, desde Claude.",
        "long_description": LONG_DESCRIPTION,
        "author": {"name": "Fraccional"},
        "homepage": "https://github.com/urvana/fraccional-googleworkspace-mcp",
        "support": "https://github.com/urvana/fraccional-googleworkspace-mcp/issues",
        "icon": "icon.png",
        "icons": [
            {"src": "icon.png", "size": "512x512", "theme": "light"},
            {"src": "icon.png", "size": "512x512", "theme": "dark"},
        ],
        "keywords": [
            "google workspace", "gmail", "correo", "email", "google calendar", "calendario",
            "agenda", "google drive", "archivos", "google chat", "google sheets", "planillas",
            "google forms", "formularios", "google docs", "google slides", "google meet",
            "google tasks", "contactos", "fraccional"
        ],
        "privacy_policies": ["https://policies.google.com/privacy"],
        "server": {
            "type": "binary",
            "entry_point": "README.md",
            "mcp_config": {"command": command, "args": ["mcp"]},
        },
        "tools": [
            {
                "name": "gws",
                "description": "Ejecuta comandos de Google Workspace (gws) con tu cuenta: correos, eventos, archivos, planillas y más.",
            },
            {
                "name": "youtube_channel",
                "description": "Lee el canal de YouTube de la cuenta conectada.",
            },
            {
                "name": "business_request",
                "description": "Llama las APIs oficiales de los conectores empresariales concedidos.",
            },
            {
                "name": "status",
                "description": "Muestra la cuenta conectada y qué servicios puede usar, con Lectura o Escritura.",
            },
            {
                "name": "open_fraccional_googleworkspace_mcp",
                "description": "Abre la configuración de fraccional-googleworkspace-mcp para conectar la cuenta o cambiar permisos.",
            },
        ],
        "compatibility": {"platforms": ["darwin", "win32"]},
    }))?;

    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    for (name, content) in [
        ("manifest.json", manifest.as_bytes()),
        ("icon.png", icon_png),
        (
            "README.md",
            "Esta extensión no incluye el servidor: usa la app fraccional-googleworkspace-mcp instalada en tu equipo.\n"
                .as_bytes(),
        ),
    ] {
        archive.start_file(name, options).map_err(|error| {
            CoreError::Io(format!("could not create the MCP extension ZIP: {error}"))
        })?;
        archive.write_all(content)?;
    }
    let bytes = archive
        .finish()
        .map(|cursor| cursor.into_inner())
        .map_err(|error| {
            CoreError::Io(format!("could not finish the MCP extension ZIP: {error}"))
        })?;
    Ok(McpExtension { bytes, warning })
}
