//! Process-level checks of the `session_server` CLI: argument handling, stdout and stderr
//! separation, exit codes, JSON error envelopes, identity retries and the serve line.
#![cfg(not(target_arch = "wasm32"))]

use std::{
    io::{BufRead, BufReader},
    net::TcpListener,
    path::PathBuf,
    process::{Command, Output, Stdio},
};

use serde_json::Value;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_session_server"))
}

fn run(args: &[&str]) -> Output {
    bin().args(args).output().expect("run session_server")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("utf-8 stdout")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("utf-8 stderr")
}

/// The one JSON envelope a structured failure writes as the last stderr line.
fn envelope(output: &Output) -> Value {
    let text = stderr(output);
    assert_eq!(
        text.lines().count(),
        1,
        "structured stderr is one line: {text:?}"
    );
    let value: Value = serde_json::from_str(text.trim_end()).expect("stderr is JSON");
    value["error"].clone()
}

#[test]
fn schema_is_offline_json_that_declares_every_command_and_failure_kind() {
    let output = run(&["schema"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stderr(&output), "");
    let schema: Value = serde_json::from_str(&stdout(&output)).expect("schema is JSON");
    assert_eq!(schema["clispec"], "0.3");
    let names: Vec<&str> = schema["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|command| command["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["schema", "identity", "check-map", "serve"]);
    for command in schema["commands"].as_array().unwrap() {
        assert!(command.get("stdout_schema").is_some(), "{command}");
        assert!(command.get("effects").is_some(), "{command}");
    }
    let kinds: Vec<(&str, i64)> = schema["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|error| {
            (
                error["kind"].as_str().unwrap(),
                error["exit_code"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        kinds,
        [("usage", 2), ("runtime_failed", 1), ("unsupported_map", 3)]
    );
}

#[test]
fn help_mentions_the_schema_command() {
    let output = run(&["--help"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout(&output).contains("schema"), "{}", stdout(&output));
    assert_eq!(stderr(&output), "");
}

#[test]
fn an_unknown_option_is_a_usage_error_in_text_on_stderr() {
    let output = run(&["--bogus"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
    assert!(
        stderr(&output).starts_with("error: unknown option --bogus\n"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn a_usage_error_honors_json_given_before_it() {
    let output = run(&["--format", "json", "--bogus"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
    assert_eq!(envelope(&output)["kind"], "usage");
}

#[test]
fn identity_is_the_same_on_every_retry_in_both_formats() {
    let text = run(&["identity"]);
    let again = run(&["identity"]);
    let json = run(&["identity", "--format", "json"]);
    for output in [&text, &again, &json] {
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(stderr(output), "");
    }
    let hex = stdout(&text);
    assert_eq!(hex, stdout(&again));
    let identity = hex
        .strip_prefix("session identity ")
        .and_then(|rest| rest.strip_suffix('\n'))
        .expect("text line");
    assert_eq!(identity.len(), 64);
    let value: Value = serde_json::from_str(&stdout(&json)).unwrap();
    assert_eq!(value["identity"], identity);
}

#[test]
fn the_legacy_print_identity_flag_is_the_identity_command() {
    assert_eq!(
        stdout(&run(&["--print-identity"])),
        stdout(&run(&["identity"]))
    );
}

#[test]
fn identity_refuses_extra_arguments_before_running() {
    let output = run(&["identity", "127.0.0.1:1"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
}

#[test]
fn a_missing_map_is_a_runtime_failure_with_the_path_in_the_envelope() {
    let output = run(&[
        "check-map",
        "--map",
        "/nonexistent/map.w3x",
        "--format",
        "json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    let error = envelope(&output);
    assert_eq!(error["kind"], "runtime_failed");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("/nonexistent/map.w3x")
    );
}

#[test]
fn bytes_that_are_not_a_map_are_a_runtime_failure() {
    let path = std::env::temp_dir().join(format!("pcx-session-cli-{}.w3x", std::process::id()));
    std::fs::write(&path, b"not a map archive").unwrap();
    let output = run(&[
        "check-map",
        "--map",
        path.to_str().unwrap(),
        "--format",
        "json",
    ]);
    std::fs::remove_file(&path).ok();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(envelope(&output)["kind"], "runtime_failed");
}

#[test]
fn serve_prints_its_listening_line_on_stdout_and_nothing_on_stderr() {
    let mut child = bin()
        .args(["127.0.0.1:0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .expect("listening line");
    child.kill().ok();
    let output = child.wait_with_output().expect("collect output");
    assert!(
        line.starts_with("PeaceCraft session listening on ws://127.0.0.1:")
            && line.ends_with("; local movement simulation, not a complete Warcraft match\n"),
        "{line}"
    );
    assert_eq!(
        stderr(&output),
        "",
        "diagnostics stayed off stdout and stderr was empty"
    );
}

#[test]
fn serve_in_json_prints_one_listening_object() {
    let mut child = bin()
        .args(["127.0.0.1:0", "--format", "json"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn serve");
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .expect("listening object");
    child.kill().ok();
    child.wait().ok();
    let value: Value = serde_json::from_str(&line).expect("JSON line");
    assert!(
        value["listening"]
            .as_str()
            .unwrap()
            .starts_with("ws://127.0.0.1:")
    );
}

#[test]
fn an_address_already_in_use_is_a_runtime_failure() {
    let held = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = held.local_addr().unwrap().to_string();
    let output = run(&[&address, "--format", "json"]);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(stdout(&output), "");
    assert_eq!(envelope(&output)["kind"], "runtime_failed");
}

/// Set PCX_SESSION_MAP to a selected local map file. Maps are refused until the production
/// startup is wired, so the process must exit 3 and name each reason.
#[test]
#[ignore = "requires PCX_SESSION_MAP selected local map file"]
fn a_real_map_exits_three_with_its_reasons() {
    let path = PathBuf::from(std::env::var_os("PCX_SESSION_MAP").expect("PCX_SESSION_MAP"));
    let output = run(&[
        "check-map",
        "--map",
        path.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(output.status.code(), Some(3), "{}", stderr(&output));
    assert_eq!(stdout(&output), "");
    let error = envelope(&output);
    assert_eq!(error["kind"], "unsupported_map");
    let reasons = error["details"]["reasons"].as_array().unwrap();
    assert!(
        reasons
            .iter()
            .any(|r| r.as_str().unwrap().contains("owner-to-player"))
    );
    assert!(
        reasons
            .iter()
            .any(|r| r.as_str().unwrap().contains("unit movement"))
    );
}
