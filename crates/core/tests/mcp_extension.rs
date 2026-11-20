//! `mcp_extension::mcp_extension`: the `.mcpb` that Claude Desktop installs.

use std::io::{Cursor, Read};

use haulmer_googleworkspace_mcp_core::CoreError;
use haulmer_googleworkspace_mcp_core::mcp_extension::mcp_extension;
use serde_json::{Value, json};

const MAC_EXE: &str = "/Applications/fraccional-googleworkspace-mcp.app/Contents/MacOS/fraccional-googleworkspace-mcp";
const WINDOWS_EXE: &str =
    r"C:\Users\ana\AppData\Local\fraccional-googleworkspace-mcp\fraccional-googleworkspace-mcp.exe";
const ICON: &[u8] = b"\x89PNG fake icon bytes";

/// The names and the bytes of each entry of a ZIP.
fn unzip(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index).unwrap();
        let mut content = Vec::new();
        file.read_to_end(&mut content).unwrap();
        entries.push((file.name().unwrap().to_string(), content));
    }
    entries
}

/// The parsed `manifest.json` of the extension built for `exe`.
fn manifest(exe: &str) -> Value {
    let extension = mcp_extension(exe.as_ref(), ICON).unwrap();
    let entries = unzip(&extension.bytes);
    let (_, content) = entries
        .iter()
        .find(|(name, _)| name == "manifest.json")
        .unwrap();
    serde_json::from_slice(content).unwrap()
}

#[test]
fn the_archive_has_the_manifest_the_icon_and_the_readme() {
    let extension = mcp_extension(MAC_EXE.as_ref(), ICON).unwrap();

    let entries = unzip(&extension.bytes);

    let mut names: Vec<&str> = entries.iter().map(|(name, _)| name.as_str()).collect();
    names.sort();
    assert_eq!(names, ["README.md", "icon.png", "manifest.json"]);
    assert_eq!(extension.warning, None);
}

#[test]
fn the_icon_is_the_given_bytes() {
    let extension = mcp_extension(MAC_EXE.as_ref(), ICON).unwrap();

    let entries = unzip(&extension.bytes);

    let (_, icon) = entries.iter().find(|(name, _)| name == "icon.png").unwrap();
    assert_eq!(icon, ICON);
}

#[test]
fn the_manifest_points_at_the_installed_app() {
    let manifest = manifest(MAC_EXE);

    assert_eq!(manifest["manifest_version"], "0.3");
    assert_eq!(manifest["name"], "fraccional-googleworkspace-mcp");
    assert_eq!(manifest["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(manifest["icon"], "icon.png");
    assert_eq!(
        manifest["icons"],
        json!([
            {"src": "icon.png", "size": "512x512", "theme": "light"},
            {"src": "icon.png", "size": "512x512", "theme": "dark"}
        ])
    );
    assert_eq!(manifest["server"]["type"], "binary");
    assert_eq!(manifest["server"]["entry_point"], "README.md");
    assert_eq!(manifest["server"]["mcp_config"]["command"], MAC_EXE);
    assert_eq!(manifest["server"]["mcp_config"]["args"], json!(["mcp"]));
    assert_eq!(manifest["author"], json!({"name": "Haulmer"}));
    assert_eq!(
        manifest["compatibility"]["platforms"],
        json!(["darwin", "win32"])
    );
}

#[test]
fn the_manifest_lists_the_tools_in_spanish() {
    let manifest = manifest(MAC_EXE);

    let names: Vec<&str> = manifest["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        [
            "gws",
            "youtube_channel",
            "status",
            "open_fraccional_googleworkspace_mcp"
        ]
    );
    assert!(
        manifest["tools"][2]["description"]
            .as_str()
            .unwrap()
            .starts_with("Muestra la cuenta conectada")
    );
}

#[test]
fn the_long_description_names_the_domain_and_links_the_gws_cli() {
    let manifest = manifest(MAC_EXE);

    let long = manifest["long_description"].as_str().unwrap();
    assert!(long.contains("@haulmer.com"));
    assert!(
        long.contains("[github.com/googleworkspace/cli](https://github.com/googleworkspace/cli)")
    );
    assert!(long.starts_with("Conecta Claude con tu Google Workspace de Haulmer"));
    assert!(long.contains("\n\n**Qué le puedes pedir**\n- "));
    assert!(long.ends_with("fijada en la v0.22.5."));
}

#[test]
fn a_windows_exe_suffix_is_stripped_from_the_command() {
    let manifest = manifest(WINDOWS_EXE);

    assert_eq!(
        manifest["server"]["mcp_config"]["command"],
        r"C:\Users\ana\AppData\Local\fraccional-googleworkspace-mcp\fraccional-googleworkspace-mcp"
    );
}

#[test]
fn the_exe_suffix_is_stripped_in_any_case() {
    let manifest = manifest(r"C:\Apps\Haulmer-Connect.EXE");

    assert_eq!(
        manifest["server"]["mcp_config"]["command"],
        r"C:\Apps\Haulmer-Connect"
    );
}

#[test]
fn a_translocated_path_is_an_error() {
    let exe = "/private/var/folders/x/T/AppTranslocation/AB12/d/fraccional-googleworkspace-mcp.app/Contents/MacOS/fraccional-googleworkspace-mcp";

    let result = mcp_extension(exe.as_ref(), ICON);

    assert!(matches!(result, Err(CoreError::UnstableExe(_))));
}

#[test]
fn a_path_on_a_mounted_volume_builds_the_extension_with_a_warning() {
    let exe = "/Volumes/fraccional-googleworkspace-mcp/fraccional-googleworkspace-mcp.app/Contents/MacOS/fraccional-googleworkspace-mcp";

    let extension = mcp_extension(exe.as_ref(), ICON).unwrap();

    assert!(extension.warning.unwrap().contains(".dmg"));
    assert!(!extension.bytes.is_empty());
}
