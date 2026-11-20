//! MCP server (`mcp::serve`): the `gws` tool, the diagnosis of its failures and
//! `open_fraccional_googleworkspace_mcp`.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use fraccional_googleworkspace_mcp_core::testing::{FakeHttp, FakeReply, FakeRunner};
use fraccional_googleworkspace_mcp_core::{Core, CoreError, Paths, mcp};
use serde_json::{Value, json};
use tempfile::TempDir;

/// A core on temp dirs with a fake runner; gws 0.22.5 is on PATH.
fn setup() -> (TempDir, TempDir, Arc<FakeRunner>, Core) {
    let (home, app_data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let runner = Arc::new(FakeRunner::new());
    runner.on_program("gws", &["--version"], FakeReply::ok("gws 0.22.5\n"));
    let paths = Paths {
        home: home.path().into(),
        app_data: app_data.path().into(),
    };
    let core = Core::new(paths, runner.clone(), Arc::new(FakeHttp::new()));
    (home, app_data, runner, core)
}

/// Sends each request as one line and returns the reply lines parsed. The app opens fine.
fn serve(core: &Core, requests: &[Value]) -> Vec<Value> {
    serve_opening(core, &|| Ok(()), requests)
}

/// Like [`serve`], with `open_app` as the way to open the desktop app.
fn serve_opening(
    core: &Core,
    open_app: &dyn Fn() -> Result<(), CoreError>,
    requests: &[Value],
) -> Vec<Value> {
    let input: String = requests
        .iter()
        .map(|request| format!("{request}\n"))
        .collect();
    let mut output = Vec::new();
    mcp::serve(core, open_app, input.as_bytes(), &mut output).unwrap();
    String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn call(id: u32, args: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": {"name": "gws", "arguments": {"args": args}}})
}

#[test]
fn initialize_and_list_tools() {
    let (_home, _app_data, _runner, core) = setup();
    let replies = serve(
        &core,
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": {"protocolVersion": "2025-03-26"}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "nope"}),
        ],
    );
    assert_eq!(replies.len(), 3); // the notification got no reply
    assert_eq!(replies[0]["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(
        replies[0]["result"]["serverInfo"]["name"],
        "fraccional-googleworkspace-mcp"
    );
    assert_eq!(
        replies[0]["result"]["serverInfo"]["icons"][0]["src"],
        "https://api.fraccional.app/storage/v1/object/public/assets/mediakit/isotipo-white-transparent.svg"
    );
    let tools = &replies[1]["result"]["tools"];
    assert_eq!(tools[0]["name"], "gws");
    assert_eq!(tools[0]["inputSchema"]["required"], json!(["args"]));
    assert!(
        tools[0]["description"]
            .as_str()
            .unwrap()
            .contains("open_fraccional_googleworkspace_mcp")
    );
    assert_eq!(tools[1]["name"], "youtube_channel");
    assert!(
        tools[1]["description"]
            .as_str()
            .unwrap()
            .contains("YouTube")
    );
    assert_eq!(tools[2]["name"], "business_request");
    assert_eq!(tools[3]["name"], "open_fraccional_googleworkspace_mcp");
    assert_eq!(
        tools[3]["inputSchema"],
        json!({"type": "object", "properties": {}})
    );
    assert!(
        tools[3]["description"]
            .as_str()
            .unwrap()
            .contains("settings window")
    );
    assert_eq!(tools[4]["name"], "status");
    assert_eq!(
        tools[4]["inputSchema"],
        json!({"type": "object", "properties": {}})
    );
    let status_description = tools[4]["description"].as_str().unwrap();
    assert!(
        status_description.contains("before `gws`"),
        "{status_description}"
    );
    assert!(
        status_description.contains("Read-only"),
        "{status_description}"
    );
    assert_eq!(tools.as_array().unwrap().len(), 5);
    assert_eq!(replies[2]["error"]["code"], -32601);
}

#[test]
fn youtube_tool_is_exposed_and_reports_missing_connection() {
    let (_home, _app_data, _runner, core) = setup();
    let request = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "youtube_channel", "arguments": {}}});

    let replies = serve(&core, &[request]);

    assert_eq!(replies[0]["result"]["isError"], true);
    assert!(
        replies[0]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("no reply for")
    );
}

#[test]
fn bad_json_is_a_parse_error() {
    let (_home, _app_data, _runner, core) = setup();
    let mut output = Vec::new();
    mcp::serve(&core, &|| Ok(()), "{nope\n".as_bytes(), &mut output).unwrap();
    let reply: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(reply["error"]["code"], -32700);
}

#[test]
fn tool_call_runs_gws_and_returns_stdout() {
    let (_home, _app_data, runner, core) = setup();
    runner.on(&["gmail", "+triage"], FakeReply::ok("{\"unread\":3}"));
    let replies = serve(&core, &[call(1, json!(["gmail", "+triage"]))]);
    assert_eq!(replies[0]["result"]["content"][0]["text"], "{\"unread\":3}");
    assert_eq!(replies[0]["result"]["isError"], false);
    assert!(
        runner
            .args()
            .contains(&vec!["gmail".to_string(), "+triage".to_string()])
    );
}

#[test]
fn tool_call_refuses_auth() {
    let (_home, _app_data, runner, core) = setup();
    let replies = serve(&core, &[call(1, json!(["auth", "export", "--unmasked"]))]);
    assert_eq!(replies[0]["result"]["isError"], true);
    assert_eq!(
        replies[0]["result"]["content"][0]["text"],
        "Los comandos auth están desactivados en MCP. Conecta la cuenta desde la app fraccional-googleworkspace-mcp."
    );
    assert!(runner.calls().is_empty());
}

/// What the agent is told in every diagnosis: both ways to reach the settings window.
fn assert_points_to_settings(text: &str) {
    assert!(text.contains("barra de menú"), "{text}");
    assert!(text.contains("bandeja del sistema"), "{text}");
    assert!(text.contains("\"Abrir configuración\""), "{text}");
    assert!(
        text.contains("open_fraccional_googleworkspace_mcp"),
        "{text}"
    );
}

/// The text and `isError` flag of the first reply.
fn tool_text(replies: &[Value]) -> (String, bool) {
    let result = &replies[0]["result"];
    (
        result["content"][0]["text"].as_str().unwrap().to_string(),
        result["isError"].as_bool().unwrap(),
    )
}

/// Gives the account in the default gws folder and answers `gws auth status` with `scopes`.
fn connected(home: &TempDir, runner: &FakeRunner, scopes: &[&str], token_valid: bool) {
    let dir = home.path().join(".config/gws");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("credentials.enc"), b"x").unwrap();
    let scopes: Vec<String> = scopes
        .iter()
        .map(|scope| {
            if scope.starts_with("https://") {
                (*scope).to_string()
            } else {
                format!("https://www.googleapis.com/auth/{scope}")
            }
        })
        .collect();
    let status =
        json!({"user": "ana@fraccional.com", "scopes": scopes, "token_valid": token_valid});
    runner.on(&["auth", "status"], FakeReply::ok(&status.to_string()));
}

#[test]
fn a_missing_gws_says_so() {
    let (home, app_data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let runner = Arc::new(FakeRunner::new());
    let paths = Paths {
        home: home.path().into(),
        app_data: app_data.path().into(),
    };
    let core = Core::new(paths, runner.clone(), Arc::new(FakeHttp::new()));

    let (text, is_error) = tool_text(&serve(&core, &[call(1, json!(["drive", "files"]))]));

    assert!(is_error);
    assert!(text.starts_with("gws no está instalado."), "{text}");
    assert!(text.contains("\"Conectar\""), "{text}");
    assert_points_to_settings(&text);
    assert!(runner.args().iter().all(|args| args != &["drive", "files"]));
}

#[test]
fn gws_vanishing_during_a_failed_call_is_diagnosed_as_missing() {
    let (_home, _app_data, _setup_runner, core) = setup();
    // `setup` answers `--version`; the first lookup finds gws, later ones do not.
    let lookups = AtomicUsize::new(0);
    let runner = FakeRunner::new();
    runner.on_with(&["--version"], move |_| {
        if lookups.fetch_add(1, Ordering::SeqCst) == 0 {
            FakeReply::ok("gws 0.22.5\n")
        } else {
            FakeReply::exit(127, "", "not found")
        }
    });
    runner.on(&["drive"], FakeReply::exit(2, "", "boom"));
    let core = Core::new(
        core.paths().clone(),
        Arc::new(runner),
        Arc::new(FakeHttp::new()),
    );

    let (text, is_error) = tool_text(&serve(&core, &[call(1, json!(["drive", "files"]))]));

    assert!(is_error);
    assert!(text.starts_with("gws no está instalado."), "{text}");
}

#[test]
fn a_failure_without_an_account_asks_to_connect() {
    let (_home, _app_data, runner, core) = setup();
    runner.on(&["drive"], FakeReply::exit(2, "", "no credentials"));

    let (text, is_error) = tool_text(&serve(&core, &[call(1, json!(["drive", "files"]))]));

    assert!(is_error);
    assert!(
        text.starts_with("No hay una cuenta de Google conectada."),
        "{text}"
    );
    assert!(text.contains("\"Conectar\""), "{text}");
    assert_points_to_settings(&text);
}

#[test]
fn a_failure_with_an_expired_session_asks_to_save_again() {
    let (home, _app_data, runner, core) = setup();
    connected(&home, &runner, &["gmail.readonly"], false);
    runner.on(&["gmail"], FakeReply::exit(1, "", "invalid_grant"));

    let (text, is_error) = tool_text(&serve(&core, &[call(1, json!(["gmail", "+triage"]))]));

    assert!(is_error);
    assert!(text.starts_with("La sesión de Google expiró."), "{text}");
    assert!(text.contains("\"Guardar\""), "{text}");
    assert_points_to_settings(&text);
}

#[test]
fn a_failure_on_a_service_not_granted_names_the_permission() {
    let (home, _app_data, runner, core) = setup();
    connected(&home, &runner, &["gmail.readonly"], true);
    runner.on(
        &["drive"],
        FakeReply::exit(1, "", "403 insufficient scopes"),
    );

    let (text, is_error) = tool_text(&serve(&core, &[call(1, json!(["drive", "files", "list"]))]));

    assert!(is_error);
    assert!(text.starts_with("Falta el permiso de Drive."), "{text}");
    assert!(text.contains("\"Guardar\""), "{text}");
    assert!(!text.contains("403"), "{text}");
    assert_points_to_settings(&text);
}

#[test]
fn a_failure_on_a_read_only_service_returns_the_output_plus_a_write_hint() {
    let (home, _app_data, runner, core) = setup();
    connected(&home, &runner, &["gmail.readonly"], true);
    runner.on(
        &["gmail"],
        FakeReply::exit(1, "", "403 insufficient scopes"),
    );

    let (text, is_error) = tool_text(&serve(&core, &[call(1, json!(["gmail", "send"]))]));

    assert!(is_error);
    assert!(text.starts_with("403 insufficient scopes"), "{text}");
    assert!(text.contains("\"Escritura\""), "{text}");
    assert!(text.contains("Gmail"), "{text}");
    assert_points_to_settings(&text);
}

#[test]
fn a_failure_on_a_service_with_write_access_returns_the_raw_output() {
    let (home, _app_data, runner, core) = setup();
    connected(
        &home,
        &runner,
        &[
            "gmail.readonly",
            "https://mail.google.com/",
            "gmail.settings.basic",
            "gmail.settings.sharing",
        ],
        true,
    );
    runner.on(&["gmail"], FakeReply::exit(1, "", "400 bad request"));

    let (text, is_error) = tool_text(&serve(&core, &[call(1, json!(["gmail", "send"]))]));

    assert!(is_error);
    assert_eq!(text, "400 bad request");
}

#[test]
fn a_failure_outside_any_service_returns_the_raw_output() {
    let (home, _app_data, runner, core) = setup();
    connected(&home, &runner, &["gmail.readonly"], true);
    runner.on(&["schema"], FakeReply::exit(1, "", "unknown method"));

    let (text, is_error) = tool_text(&serve(&core, &[call(1, json!(["schema", "nope"]))]));

    assert!(is_error);
    assert_eq!(text, "unknown method");
}

#[test]
fn when_the_status_itself_fails_the_raw_output_is_returned() {
    let (home, _app_data, runner, core) = setup();
    let dir = home.path().join(".config/gws");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("credentials.enc"), b"x").unwrap();
    runner.on(
        &["auth", "status"],
        FakeReply::exit(1, "", "keyring locked"),
    );
    runner.on(&["gmail"], FakeReply::exit(1, "", "boom"));

    let (text, is_error) = tool_text(&serve(&core, &[call(1, json!(["gmail", "+triage"]))]));

    assert!(is_error);
    assert_eq!(text, "boom");
}

#[test]
fn a_successful_call_is_never_diagnosed() {
    let (_home, _app_data, runner, core) = setup();
    runner.on(&["drive"], FakeReply::ok("{}"));

    serve(&core, &[call(1, json!(["drive", "files", "list"]))]);

    assert!(runner.args().iter().all(|args| args != &["auth", "status"]));
}

fn open_call(id: u32) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": {"name": "open_fraccional_googleworkspace_mcp", "arguments": {}}})
}

#[test]
fn open_fraccional_googleworkspace_mcp_opens_the_app_and_says_what_to_tell_the_person() {
    let (_home, _app_data, _runner, core) = setup();
    let opened = AtomicUsize::new(0);
    let open_app = || {
        opened.fetch_add(1, Ordering::SeqCst);
        Ok(())
    };

    let (text, is_error) = tool_text(&serve_opening(&core, &open_app, &[open_call(1)]));

    assert!(!is_error);
    assert_eq!(
        text,
        "Pedí abrir fraccional-googleworkspace-mcp en su configuración. Si no aparece, pídele a la persona que lo \
         abra desde el ícono de fraccional-googleworkspace-mcp (barra de menús en macOS, bandeja del sistema en \
         Windows) › \"Abrir configuración\"."
    );
    assert_eq!(opened.load(Ordering::SeqCst), 1);
}

#[test]
fn open_fraccional_googleworkspace_mcp_reports_when_the_app_cannot_open() {
    let (_home, _app_data, _runner, core) = setup();
    let open_app = || Err(CoreError::Process("no window".into()));

    let (text, is_error) = tool_text(&serve_opening(&core, &open_app, &[open_call(1)]));

    assert!(is_error);
    assert!(text.contains("no window"), "{text}");
}

#[test]
fn an_unknown_tool_is_an_invalid_params_error() {
    let (_home, _app_data, _runner, core) = setup();
    let request = json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": {"name": "nope", "arguments": {}}});

    let replies = serve(&core, &[request]);

    assert_eq!(replies[0]["error"]["code"], -32602);
}

/// A `tools/call` request for the `status` tool.
///
/// # Example
///
/// ```ignore
/// let replies = serve(&core, &[status_call(1)]);
/// ```
fn status_call(id: u32) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": {"name": "status", "arguments": {}}})
}

#[test]
fn status_lists_the_account_and_the_levels_per_service() {
    let (home, _app_data, runner, core) = setup();
    connected(
        &home,
        &runner,
        &["gmail.readonly", "calendar.readonly", "calendar"],
        true,
    );

    let (text, is_error) = tool_text(&serve(&core, &[status_call(1)]));

    assert!(!is_error);
    let expected_head = "Cuenta: ana@fraccional.com\n\
        Sesión: válida\n\
        gws: instalado\n\
        Con permiso: Gmail (Lectura), Calendar (Lectura y Escritura)\n\
        Sin permiso: Chat, Meet, Drive, ";
    assert!(text.starts_with(expected_head), "{text}");
    assert!(
        !text.contains("Sin permiso: Chat, Meet, Drive, Gmail"),
        "{text}"
    );
    assert!(text.contains(
        "\nPara activar más servicios: pídele a la persona que abra fraccional-googleworkspace-mcp"
    ));
    assert!(
        text.contains("\"Abrir configuración\", active el servicio y pulse \"Guardar\"."),
        "{text}"
    );
    assert!(
        text.ends_with("También puedes llamar a open_fraccional_googleworkspace_mcp."),
        "{text}"
    );
    assert!(!text.contains("Conectores de negocio"), "{text}");
    assert_points_to_settings(&text);
}

#[test]
fn status_without_an_account_says_so_and_asks_to_connect() {
    let (_home, _app_data, _runner, core) = setup();

    let (text, is_error) = tool_text(&serve(&core, &[status_call(1)]));

    assert!(!is_error);
    assert!(text.starts_with("Cuenta: ninguna conectada\n"), "{text}");
    assert!(!text.contains("Sesión:"), "{text}");
    assert!(text.contains("gws: instalado"), "{text}");
    assert!(text.contains("Con permiso: ninguno"), "{text}");
    assert!(
        text.contains("active el servicio y pulse \"Conectar\"."),
        "{text}"
    );
    assert!(!text.contains("\"Guardar\""), "{text}");
}

#[test]
fn status_with_an_expired_session_says_so() {
    let (home, _app_data, runner, core) = setup();
    connected(&home, &runner, &["gmail.readonly"], false);

    let (text, is_error) = tool_text(&serve(&core, &[status_call(1)]));

    assert!(!is_error);
    assert!(
        text.contains("Cuenta: ana@fraccional.com\nSesión: expirada\n"),
        "{text}"
    );
    assert!(text.contains("Con permiso: Gmail (Lectura)\n"), "{text}");
}

#[test]
fn status_with_gws_missing_says_so() {
    let (home, app_data) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let paths = Paths {
        home: home.path().into(),
        app_data: app_data.path().into(),
    };
    let core = Core::new(
        paths,
        Arc::new(FakeRunner::new()),
        Arc::new(FakeHttp::new()),
    );

    let (text, is_error) = tool_text(&serve(&core, &[status_call(1)]));

    assert!(!is_error);
    assert!(text.contains("gws: no instalado"), "{text}");
    assert!(text.starts_with("Cuenta: ninguna conectada"), "{text}");
}

#[test]
fn status_names_the_business_connectors_when_granted() {
    let (home, _app_data, runner, core) = setup();
    connected(
        &home,
        &runner,
        &["gmail.readonly", "youtube.readonly"],
        true,
    );

    let (text, _) = tool_text(&serve(&core, &[status_call(1)]));

    assert!(
        text.contains("\nConectores de negocio: YouTube (Lectura)\n"),
        "{text}"
    );
}

#[test]
fn status_that_fails_is_an_error_result_with_the_error_text() {
    let (home, _app_data, runner, core) = setup();
    let dir = home.path().join(".config/gws");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("credentials.enc"), b"x").unwrap();
    runner.on(
        &["auth", "status"],
        FakeReply::exit(1, "", "keyring locked"),
    );

    let (text, is_error) = tool_text(&serve(&core, &[status_call(1)]));

    assert!(is_error);
    assert!(text.contains("keyring locked"), "{text}");
}
