extern crate sqlshield;
use sqlshield::validation::SqlValidationError;
use std::path::PathBuf;

#[test]
fn test_sqlshield_acceptance_python() {
    let validation_errors = sqlshield::validate_files(
        &PathBuf::from("./tests/languages/main.py"),
        &PathBuf::from("./tests/schema.sql"),
    );

    let expected_validation_errors = vec![
        SqlValidationError {
            location: "./tests/languages/main.py:7".to_string(),
            description: "Column `email` not found in table `users`".to_string(),
        },
        SqlValidationError {
            location: "./tests/languages/main.py:13".to_string(),
            description: "Table `admin` not found in schema nor subqueries".to_string(),
        },
        SqlValidationError {
            location: "./tests/languages/main.py:21".to_string(),
            description: "Table `admin` not found in schema nor subqueries".to_string(),
        },
        SqlValidationError {
            location: "./tests/languages/main.py:28".to_string(),
            description: "Table `admin` not found in schema nor subqueries".to_string(),
        },
        SqlValidationError {
            location: "./tests/languages/main.py:43".to_string(),
            description: "Column `name` not found in table `receipt`".to_string(),
        },
        SqlValidationError {
            location: "./tests/languages/main.py:43".to_string(),
            description: "Column `content` not found in table `users`".to_string(),
        },
        SqlValidationError {
            location: "./tests/languages/main.py:61".to_string(),
            description: "Table `admin` not found in schema nor subqueries".to_string(),
        },
        SqlValidationError {
            location: "./tests/languages/main.py:71".to_string(),
            description: "Column `id` not found in table `sub`".to_string(),
        },
        SqlValidationError {
            location: "./tests/languages/main.py:71".to_string(),
            description: "Column `content` not found in table `users`".to_string(),
        },
    ];
    assert_eq!(validation_errors, expected_validation_errors);
}

#[test]
fn test_sqlshield_acceptance_rust() {
    let validation_errors = sqlshield::validate_files(
        &PathBuf::from("./tests/languages/main.rs"),
        &PathBuf::from("./tests/schema.sql"),
    );

    let expected_validation_errors = vec![
        SqlValidationError {
            location: "./tests/languages/main.rs:10".to_string(),
            description: "Column `email` not found in table `users`".to_string(),
        },
        SqlValidationError {
            location: "./tests/languages/main.rs:16".to_string(),
            description: "Table `admin` not found in schema nor subqueries".to_string(),
        },
        SqlValidationError {
            location: "./tests/languages/main.rs:28".to_string(),
            description: "Table `admin` not found in schema nor subqueries".to_string(),
        },
        SqlValidationError {
            location: "./tests/languages/main.rs:38".to_string(),
            description: "Table `admin` not found in schema nor subqueries".to_string(),
        },
        SqlValidationError {
            location: "./tests/languages/main.rs:54".to_string(),
            description: "Column `name` not found in table `receipt`".to_string(),
        },
        SqlValidationError {
            location: "./tests/languages/main.rs:54".to_string(),
            description: "Column `content` not found in table `users`".to_string(),
        },
        SqlValidationError {
            location: "./tests/languages/main.rs:72".to_string(),
            description: "Table `admin` not found in schema nor subqueries".to_string(),
        },
        SqlValidationError {
            location: "./tests/languages/main.rs:82".to_string(),
            description: "Column `id` not found in table `sub`".to_string(),
        },
        SqlValidationError {
            location: "./tests/languages/main.rs:82".to_string(),
            description: "Column `content` not found in table `users`".to_string(),
        },
    ];
    assert_eq!(validation_errors, expected_validation_errors);
}
