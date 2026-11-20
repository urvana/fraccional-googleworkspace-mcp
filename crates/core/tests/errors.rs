//! How errors reach the UI: their display message as a JSON string.

use haulmer_googleworkspace_mcp_core::CoreError;

#[test]
fn access_removed_has_a_distinct_message_telling_the_person_to_save_again() {
    assert_eq!(
        serde_json::to_value(CoreError::AccessRemoved).unwrap(),
        "Access was removed, but the new Google sign-in did not finish. Save again to reconnect."
    );
}

#[test]
fn a_missing_mcp_extension_app_tells_the_person_to_use_the_prompt() {
    assert_eq!(
        serde_json::to_value(CoreError::NoMcpExtensionApp).unwrap(),
        "No encontré una app que abra extensiones MCP. Usa el prompt de abajo."
    );
}
