//! Behaviour of the shared test doubles every ticket relies on.

use std::sync::Arc;
use std::time::Duration;

use haulmer_googleworkspace_mcp_core::testing::{
    CallKind, FakeHttp, FakeReply, FakeRunner, HttpRequest,
};
use haulmer_googleworkspace_mcp_core::{
    CommandSpec, CoreError, HttpClient, HttpResponse, KILLED_EXIT_CODE, OutputLine, ProcessRunner,
};

fn gws(args: &[&str]) -> CommandSpec {
    let mut command = CommandSpec::new("/app/bin/gws", args, "/app/run");
    command.env.insert("NO_COLOR".into(), "1".into());
    command
}

#[test]
fn runner_returns_the_canned_reply_and_records_the_whole_command() {
    let runner = FakeRunner::new();
    runner.on(
        &["auth", "status"],
        FakeReply::ok(r#"{"user":"ana@haulmer.com"}"#),
    );

    let output = runner.run(&gws(&["auth", "status"])).unwrap();

    assert_eq!(output.exit_code, 0);
    assert_eq!(output.stdout, r#"{"user":"ana@haulmer.com"}"#);
    let calls = runner.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].kind, CallKind::Run);
    assert_eq!(calls[0].command, gws(&["auth", "status"]));
}

#[test]
fn runner_picks_the_longest_matching_prefix_and_uses_queued_replies_in_order() {
    let runner = FakeRunner::new();
    runner.on(&["auth"], FakeReply::exit(5, "", "generic"));
    runner.on(&["auth", "status"], FakeReply::ok("first"));
    runner.on(&["auth", "status"], FakeReply::ok("then always"));

    let replies: Vec<String> = (0..3)
        .map(|_| runner.run(&gws(&["auth", "status"])).unwrap().stdout)
        .collect();
    let other = runner.run(&gws(&["auth", "logout"])).unwrap();

    assert_eq!(replies, ["first", "then always", "then always"]);
    assert_eq!((other.exit_code, other.stderr.as_str()), (5, "generic"));
}

#[test]
fn runner_without_a_reply_fails_like_a_missing_program_and_still_records_the_call() {
    let runner = FakeRunner::new();

    let result = runner.run(&gws(&["--version"]));

    assert!(matches!(result, Err(CoreError::Process(_))));
    assert_eq!(runner.args(), [vec!["--version".to_string()]]);
}

#[test]
fn runner_reply_for_one_program_does_not_answer_another_program() {
    let runner = FakeRunner::new();
    runner.on_program("gws", &["--version"], FakeReply::ok("gws 0.22.5"));

    let on_path = runner.run(&CommandSpec::new("gws", &["--version"], "/run"));
    let in_app_data = runner.run(&gws(&["--version"]));

    assert_eq!(on_path.unwrap().stdout, "gws 0.22.5");
    assert!(in_app_data.is_err());
}

#[test]
fn runner_handler_can_compute_the_reply_and_touch_the_filesystem() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("credentials.enc");
    let runner = FakeRunner::new();
    let written = marker.clone();
    runner.on_with(&["auth", "login"], move |command| {
        std::fs::write(&written, "x").unwrap();
        FakeReply::ok(&format!("cwd={}", command.cwd.display()))
    });

    let output = runner
        .run(&gws(&["auth", "login", "--scopes", "a"]))
        .unwrap();

    assert_eq!(output.stdout, "cwd=/app/run");
    assert!(marker.exists());
}

#[test]
fn spawned_process_yields_stderr_then_stdout_lines_and_the_exit_code() {
    let runner = FakeRunner::new();
    runner.on(
        &["auth", "login"],
        FakeReply::exit(
            2,
            "{\"error\":{}}\n",
            "Open this URL\n\n  https://accounts.google.com/x\n",
        ),
    );

    let mut process = runner.spawn(&gws(&["auth", "login"])).unwrap();
    let lines: Vec<OutputLine> = std::iter::from_fn(|| process.next_line()).collect();

    assert_eq!(
        lines,
        [
            OutputLine::Stderr("Open this URL".into()),
            OutputLine::Stderr("".into()),
            OutputLine::Stderr("  https://accounts.google.com/x".into()),
            OutputLine::Stdout("{\"error\":{}}".into()),
        ]
    );
    assert_eq!(process.wait().unwrap(), 2);
    assert_eq!(runner.calls()[0].kind, CallKind::Spawn);
}

#[test]
fn hanging_process_blocks_until_killed_from_another_thread() {
    let runner = Arc::new(FakeRunner::new());
    runner.on(
        &["auth", "login"],
        FakeReply::exit(0, "", "https://accounts.google.com/x\n").hang_until_killed(),
    );
    let mut process = runner.spawn(&gws(&["auth", "login"])).unwrap();
    let killer = process.killer();

    let first = process.next_line();
    let canceller = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        killer.kill();
    });
    let after_kill = process.next_line();
    canceller.join().unwrap();

    assert_eq!(
        first,
        Some(OutputLine::Stderr("https://accounts.google.com/x".into()))
    );
    assert_eq!(after_kill, None);
    assert_eq!(process.wait().unwrap(), KILLED_EXIT_CODE);
    assert_eq!(runner.kill_count(), 1);
}

#[test]
fn http_returns_canned_responses_and_records_requests() {
    let http = FakeHttp::new();
    http.on(
        "https://example.com/a.sha256",
        HttpResponse::ok("abc  a.tar.gz\r\n"),
    );
    http.on(
        "https://oauth2.googleapis.com/revoke",
        HttpResponse::new(400, "invalid_token"),
    );

    let get = http.get("https://example.com/a.sha256").unwrap();
    let post = http
        .post_form("https://oauth2.googleapis.com/revoke", &[("token", "1//t")])
        .unwrap();

    assert_eq!(get.body, b"abc  a.tar.gz\r\n");
    assert_eq!(post.status, 400);
    assert_eq!(
        http.requests(),
        [
            HttpRequest::Get {
                url: "https://example.com/a.sha256".into()
            },
            HttpRequest::PostForm {
                url: "https://oauth2.googleapis.com/revoke".into(),
                form: vec![("token".into(), "1//t".into())],
            },
        ]
    );
}

#[test]
fn http_fails_for_unknown_urls_and_for_canned_errors() {
    let http = FakeHttp::new();
    http.on_error(
        "https://oauth2.googleapis.com/revoke",
        CoreError::Http("offline".into()),
    );

    let unknown = http.get("https://example.com/missing");
    let offline = http.post_form("https://oauth2.googleapis.com/revoke", &[]);

    assert!(matches!(unknown, Err(CoreError::Http(_))));
    assert_eq!(offline, Err(CoreError::Http("offline".into())));
    assert_eq!(http.requests().len(), 2);
}
