//! Directory-walk filtering — `validate_files` should skip generated /
//! vendored / cache directories so a stray bad SQL string in `target/` or
//! `.venv/` doesn't pollute results.

use std::fs;

use sqlshield::validate_files;

fn write_py(dir: &std::path::Path, query: &str) {
    fs::write(dir.join("file.py"), format!("q = \"{query}\"\n").as_bytes()).unwrap();
}

#[test]
fn skips_target_directory() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("schema.sql"),
        b"CREATE TABLE users (id INT);",
    )
    .unwrap();

    // Bad query inside target/ — would error if validated.
    let target = root.path().join("target");
    fs::create_dir(&target).unwrap();
    write_py(&target, "SELECT email FROM users");

    let errs = validate_files(root.path(), &root.path().join("schema.sql")).unwrap();
    assert!(errs.is_empty(), "target/ should be skipped; got: {errs:?}");
}

#[test]
fn skips_dot_git_directory() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("schema.sql"),
        b"CREATE TABLE users (id INT);",
    )
    .unwrap();

    let dotgit = root.path().join(".git");
    fs::create_dir(&dotgit).unwrap();
    write_py(&dotgit, "SELECT email FROM users");

    let errs = validate_files(root.path(), &root.path().join("schema.sql")).unwrap();
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn skips_node_modules_and_venv() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("schema.sql"),
        b"CREATE TABLE users (id INT);",
    )
    .unwrap();

    for name in [".venv", "venv", "node_modules", "__pycache__"] {
        let d = root.path().join(name);
        fs::create_dir(&d).unwrap();
        write_py(&d, "SELECT email FROM users");
    }

    let errs = validate_files(root.path(), &root.path().join("schema.sql")).unwrap();
    assert!(errs.is_empty(), "got: {errs:?}");
}

#[test]
fn does_not_skip_normal_directories() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("schema.sql"),
        b"CREATE TABLE users (id INT);",
    )
    .unwrap();

    let src = root.path().join("src");
    fs::create_dir(&src).unwrap();
    write_py(&src, "SELECT email FROM users");

    let errs = validate_files(root.path(), &root.path().join("schema.sql")).unwrap();
    assert!(
        errs.iter().any(|e| e.description.contains("`email`")),
        "src/ should be walked; got: {errs:?}"
    );
}
