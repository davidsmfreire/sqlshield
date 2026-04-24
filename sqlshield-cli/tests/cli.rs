//! End-to-end CLI tests — invoke the compiled binary to exercise flags.

use std::io::Write;
use std::process::{Command, Stdio};

fn cli_bin() -> std::path::PathBuf {
    env!("CARGO_BIN_EXE_sqlshield").into()
}

#[test]
fn stdin_valid_query_exits_zero() {
    let mut child = Command::new(cli_bin())
        .args([
            "--stdin",
            "--schema",
            "./tests/schema.sql",
            "--format",
            "json",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"SELECT id, name FROM users")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "stdout: {:?}", output.stdout);
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.trim(), "[]");
}

#[test]
fn stdin_invalid_query_exits_one() {
    let mut child = Command::new(cli_bin())
        .args([
            "--stdin",
            "--schema",
            "./tests/schema.sql",
            "--format",
            "json",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"SELECT email FROM users")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("email"));
}

#[test]
fn missing_schema_exits_two() {
    let output = Command::new(cli_bin())
        .args(["--schema", "/definitely/nope/schema.sql"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn json_format_produces_valid_json_on_directory_scan() {
    let output = Command::new(cli_bin())
        .args([
            "--directory",
            "./tests/languages",
            "--schema",
            "./tests/schema.sql",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1)); // validation errors present
    let stdout = String::from_utf8(output.stdout).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let arr = parsed.as_array().unwrap();
    assert!(!arr.is_empty());
    for item in arr {
        assert!(item.get("location").is_some());
        assert!(item.get("description").is_some());
    }
}
