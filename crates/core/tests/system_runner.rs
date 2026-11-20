//! The real process runner against real programs (Unix shell).
#![cfg(unix)]

use haulmer_googleworkspace_mcp_core::{
    CommandSpec, CoreError, KILLED_EXIT_CODE, OutputLine, ProcessRunner, SystemRunner,
};

fn sh(script: &str, cwd: &std::path::Path) -> CommandSpec {
    CommandSpec::new("sh", &["-c", script], cwd)
}

#[test]
fn run_applies_env_removals_and_cwd_and_captures_both_streams_and_the_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().canonicalize().unwrap();
    let mut command = sh(
        r#"echo "$HC_SET|${HOME:+home}|${HC_GONE:-gone}|$(pwd)"; echo oops >&2; exit 3"#,
        &cwd,
    );
    command.env.insert("HC_SET".into(), "set".into());
    command.env.insert("HC_GONE".into(), "still here".into());
    command.env_remove.insert("HC_GONE".into());

    let output = SystemRunner.run(&command).unwrap();

    assert_eq!(output.stdout, format!("set|home|gone|{}\n", cwd.display()));
    assert_eq!(output.stderr, "oops\n");
    assert_eq!(output.exit_code, 3);
}

#[test]
fn run_closes_stdin_so_a_reader_does_not_block() {
    let dir = tempfile::tempdir().unwrap();

    let output = SystemRunner
        .run(&CommandSpec::new("cat", &[], dir.path()))
        .unwrap();

    assert_eq!((output.exit_code, output.stdout.as_str()), (0, ""));
}

#[test]
fn run_of_a_missing_program_is_a_process_error() {
    let dir = tempfile::tempdir().unwrap();

    let result = SystemRunner.run(&CommandSpec::new("hc-no-such-program", &[], dir.path()));

    assert!(matches!(result, Err(CoreError::Process(_))));
}

#[test]
fn spawned_process_streams_lines_from_both_streams_then_reports_the_exit_code() {
    let dir = tempfile::tempdir().unwrap();
    let mut process = SystemRunner
        .spawn(&sh(
            "echo to-err >&2; sleep 0.1; echo to-out; exit 4",
            dir.path(),
        ))
        .unwrap();

    let lines: Vec<OutputLine> = std::iter::from_fn(|| process.next_line()).collect();

    assert_eq!(
        lines,
        [
            OutputLine::Stderr("to-err".into()),
            OutputLine::Stdout("to-out".into())
        ]
    );
    assert_eq!(process.wait().unwrap(), 4);
}

#[test]
fn killing_a_spawned_process_ends_its_output_and_reports_killed() {
    let dir = tempfile::tempdir().unwrap();
    let mut process = SystemRunner
        .spawn(&sh("echo waiting >&2; exec sleep 30", dir.path()))
        .unwrap();

    let first = process.next_line();
    process.killer().kill();
    let after_kill = process.next_line();

    assert_eq!(first, Some(OutputLine::Stderr("waiting".into())));
    assert_eq!(after_kill, None);
    assert_eq!(process.wait().unwrap(), KILLED_EXIT_CODE);
}

#[test]
fn killing_a_process_group_closes_descendant_output() {
    let dir = tempfile::tempdir().unwrap();
    let mut process = SystemRunner
        .spawn(&sh("sleep 30 & wait", dir.path()))
        .unwrap();

    process.killer().kill();

    assert_eq!(process.next_line(), None);
    assert_eq!(process.wait().unwrap(), KILLED_EXIT_CODE);
}
