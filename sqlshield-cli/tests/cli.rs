//! End-to-end CLI tests — invoke the compiled binary to exercise flags.

use std::fs;
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
fn config_file_supplies_schema_path() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("my-schema.sql"),
        b"CREATE TABLE widgets (id INT);",
    )
    .unwrap();
    fs::write(
        dir.path().join(".sqlshield.toml"),
        br#"schema = "my-schema.sql""#,
    )
    .unwrap();
    // No source files to scan → no validation errors, exit 0.
    let output = Command::new(cli_bin())
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cli_flag_overrides_config_file() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".sqlshield.toml"),
        br#"schema = "never-resolved.sql""#,
    )
    .unwrap();
    let real_schema = dir.path().join("real.sql");
    fs::write(&real_schema, b"CREATE TABLE t (id INT);").unwrap();

    let output = Command::new(cli_bin())
        .current_dir(dir.path())
        .args(["--schema"])
        .arg(&real_schema)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn malformed_config_file_returns_exit_two() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join(".sqlshield.toml"),
        b"this is ::: not toml {{{",
    )
    .unwrap();
    let output = Command::new(cli_bin())
        .current_dir(dir.path())
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
