#![cfg(not(target_arch = "wasm32"))]

use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

fn invoke(args: &[&str]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_peacecraft3"))
        .args(args)
        .current_dir(env!("CARGO_TARGET_TMPDIR"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("app CLI must spawn");
    let started = Instant::now();
    while child
        .try_wait()
        .expect("app status must be readable")
        .is_none()
    {
        // Process liveness guard, not a game performance gate.
        if started.elapsed() >= Duration::from_secs(5) {
            let _ = child.kill();
            let output = child
                .wait_with_output()
                .expect("timed-out app must be reaped");
            panic!("CLI did not refuse promptly for {args:?}: {output:?}");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    child
        .wait_with_output()
        .expect("app output must be readable")
}

#[test]
fn unknown_commands_and_unsupported_metadata_refuse_without_stdout() {
    for args in [
        vec!["typo"],
        vec!["--help"],
        vec!["-h"],
        vec!["--version"],
        vec!["schema"],
        vec!["--format", "json", "schema"],
        vec!["launching", "sources"],
        vec!["viewing", "synthetic.mdx"],
        vec!["--tile", "Zzzz"],
        vec!["quote\"newline\nΩ"],
    ] {
        let output = invoke(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.starts_with("unknown command "), "{args:?}: {error}");
        assert!(error.contains("usage: peacecraft3"), "{args:?}: {error}");
        assert_eq!(error.lines().count(), 2, "{args:?}: {error}");
    }
}

#[test]
fn documented_subcommand_usage_and_parser_statuses_stay_unchanged() {
    for (args, code, message) in [
        (vec!["launch"], 2, "peacecraft3 launch —"),
        (vec!["launch", "--help"], 2, "unknown launch command"),
        (
            vec!["launch", "plan"],
            2,
            "usage: peacecraft3 launch plan <mode>",
        ),
        (vec!["view"], 1, "view requires an explicit MODEL path"),
        (vec!["view", "--help"], 1, "unknown viewer option"),
        (vec!["view", "chosen.mdx", "--tile"], 1, "--tile requires"),
    ] {
        let output = invoke(&args);
        assert_eq!(output.status.code(), Some(code), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
        assert!(String::from_utf8(output.stderr).unwrap().contains(message));
    }
}
