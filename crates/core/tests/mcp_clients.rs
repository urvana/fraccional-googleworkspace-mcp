//! `mcp_clients::install_prompt`: the Spanish text a person pastes into an Agent.

use fraccional_googleworkspace_mcp_core::CoreError;
use fraccional_googleworkspace_mcp_core::mcp_clients::{check_exe_path, install_prompt};

const MAC_EXE: &str = "/Applications/fraccional-googleworkspace-mcp.app/Contents/MacOS/fraccional-googleworkspace-mcp";
const WINDOWS_EXE: &str =
    r"C:\Users\ana\AppData\Local\fraccional-googleworkspace-mcp\fraccional-googleworkspace-mcp.exe";

#[test]
fn an_installed_mac_app_gets_both_commands_and_no_warning() {
    let prompt = install_prompt(MAC_EXE.as_ref()).unwrap();

    assert_eq!(prompt.warning, None);
    assert!(prompt.text.contains(&format!(
        "claude mcp add --scope user fraccional-googleworkspace-mcp -- \"{MAC_EXE}\" mcp"
    )));
    assert!(prompt.text.contains(&format!(
        "codex mcp add fraccional-googleworkspace-mcp -- \"{MAC_EXE}\" mcp"
    )));
    assert!(
        prompt
            .text
            .contains("claude mcp get fraccional-googleworkspace-mcp")
    );
    assert!(
        prompt
            .text
            .contains("codex mcp get fraccional-googleworkspace-mcp")
    );
}

#[test]
fn the_skills_command_pins_the_tag_and_lists_the_service_skills() {
    let prompt = install_prompt(MAC_EXE.as_ref()).unwrap();

    let expected = "npx -y skills add https://github.com/googleworkspace/cli/tree/v0.22.5 -g -y -s \
         gws-shared gws-drive gws-sheets gws-gmail gws-calendar gws-admin-reports gws-docs \
         gws-slides gws-tasks gws-people gws-chat gws-classroom gws-forms gws-keep gws-meet \
         gws-events gws-modelarmor gws-workflow gws-script";
    assert!(prompt.text.contains(expected), "{}", prompt.text);
    assert!(prompt.text.contains("Abrir skills › Descargar .zip"));
}

#[test]
fn a_translocated_path_is_an_error() {
    let exe = "/private/var/folders/x/T/AppTranslocation/AB12/d/fraccional-googleworkspace-mcp.app/Contents/MacOS/fraccional-googleworkspace-mcp";

    let error = install_prompt(exe.as_ref()).unwrap_err();

    assert!(matches!(&error, CoreError::UnstableExe(message) if message.contains("Aplicaciones")));
    assert!(error.to_string().contains("carpeta temporal"));
}

#[test]
fn a_path_on_a_mounted_volume_warns_about_the_disk_image() {
    let exe = "/Volumes/fraccional-googleworkspace-mcp/fraccional-googleworkspace-mcp.app/Contents/MacOS/fraccional-googleworkspace-mcp";

    let prompt = install_prompt(exe.as_ref()).unwrap();

    let warning = prompt.warning.unwrap();
    assert!(warning.contains(".dmg"));
    assert!(warning.contains("Aplicaciones"));
    assert!(prompt.text.contains(exe));
}

#[test]
fn a_windows_path_is_raw_in_the_commands() {
    let prompt = install_prompt(WINDOWS_EXE.as_ref()).unwrap();

    assert!(prompt.text.contains(&format!(
        "codex mcp add fraccional-googleworkspace-mcp -- \"{WINDOWS_EXE}\" mcp"
    )));
}

#[test]
fn claude_desktop_is_pointed_at_the_mcp_extension_instead_of_a_json_block() {
    let prompt = install_prompt(MAC_EXE.as_ref()).unwrap();

    assert!(prompt.text.contains(
        "- Claude Desktop: en fraccional-googleworkspace-mcp abre \"Conectar un agente…\" y pulsa \"Instalar MCP\"."
    ));
    assert!(!prompt.text.contains("Edit Config"), "{}", prompt.text);
    assert!(!prompt.text.contains("mcpServers"), "{}", prompt.text);
    assert!(!prompt.text.contains("{desktop_json}"), "{}", prompt.text);
}

#[test]
fn check_exe_path_accepts_an_installed_path_without_a_warning() {
    assert_eq!(check_exe_path(MAC_EXE.as_ref()), Ok(None));
    assert_eq!(check_exe_path(WINDOWS_EXE.as_ref()), Ok(None));
}

#[test]
fn check_exe_path_rejects_a_translocated_path() {
    let exe = "/private/var/folders/x/T/AppTranslocation/AB12/d/fraccional-googleworkspace-mcp.app/Contents/MacOS/fraccional-googleworkspace-mcp";

    let error = check_exe_path(exe.as_ref()).unwrap_err();

    assert!(matches!(&error, CoreError::UnstableExe(message) if message.contains("Aplicaciones")));
}

#[test]
fn check_exe_path_warns_about_a_mounted_volume() {
    let exe = "/Volumes/fraccional-googleworkspace-mcp/fraccional-googleworkspace-mcp.app/Contents/MacOS/fraccional-googleworkspace-mcp";

    let warning = check_exe_path(exe.as_ref()).unwrap().unwrap();

    assert!(warning.contains(".dmg"));
}

#[test]
fn the_text_carries_the_rules_the_fallbacks_and_the_web_chat_warning() {
    let prompt = install_prompt(MAC_EXE.as_ref()).unwrap();

    for needle in [
        "pídeme confirmación antes de ejecutarlo, a menos que te diga lo contrario",
        "ya existe un servidor llamado \"fraccional-googleworkspace-mcp\"",
        "No modifiques otros servidores ni otros archivos",
        "Instalar MCP",
        "Settings > MCP servers > Add server",
        "STDIO",
        "claude.ai o chatgpt.com",
        "no funciona ahí",
        "/mcp",
        "usa la misma configuración que Codex",
        "Hermes, Cursor",
    ] {
        assert!(prompt.text.contains(needle), "missing {needle:?}");
    }
}
