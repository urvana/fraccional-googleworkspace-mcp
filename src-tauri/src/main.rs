// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Binary entry point: `fraccional-googleworkspace-mcp mcp` serves MCP on stdin/stdout, anything else opens
/// the app ([`fraccional_googleworkspace_mcp_lib::run`]).
///
/// # Example
///
/// ```text
/// $ pnpm tauri dev
/// $ fraccional-googleworkspace-mcp mcp
/// ```
#[derive(Debug, PartialEq, Eq)]
enum LaunchMode {
    Mcp,
    Desktop,
}

fn launch_mode(mut args: impl Iterator<Item = String>) -> LaunchMode {
    if args.nth(1).as_deref() == Some("mcp") {
        LaunchMode::Mcp
    } else {
        LaunchMode::Desktop
    }
}

fn main() {
    if launch_mode(std::env::args()) == LaunchMode::Mcp {
        if let Err(error) = fraccional_googleworkspace_mcp_lib::run_mcp() {
            eprintln!("fraccional-googleworkspace-mcp mcp: {error}");
            std::process::exit(1);
        }
    } else {
        fraccional_googleworkspace_mcp_lib::run()
    }
}

#[cfg(test)]
mod tests {
    use super::{LaunchMode, launch_mode};

    #[test]
    fn mcp_argument_selects_mcp_before_desktop_startup() {
        let args = ["fraccional-googleworkspace-mcp", "mcp"]
            .into_iter()
            .map(String::from);

        assert_eq!(launch_mode(args), LaunchMode::Mcp);
    }

    #[test]
    fn other_arguments_select_desktop_startup() {
        let args = ["fraccional-googleworkspace-mcp", "--show"]
            .into_iter()
            .map(String::from);

        assert_eq!(launch_mode(args), LaunchMode::Desktop);
    }
}
