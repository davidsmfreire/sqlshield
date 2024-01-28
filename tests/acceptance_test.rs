extern crate sqlshield;
use sqlshield::SqlValidationError;
use std::path::PathBuf;

#[test]
fn test_sqlshield_acceptance() {
    let validation_errors = sqlshield::validate(
        &PathBuf::from("./tests/main.py"),
        &PathBuf::from("./tests/schema.sql"),
    );

    let expected_validation_errors = vec![
        SqlValidationError {
            location: "./tests/main.py:7".to_string(),
            description: "Column `email` not found in table `users`".to_string(),
        },
        SqlValidationError {
            location: "./tests/main.py:13".to_string(),
            description: "Table `admin` not found in schema".to_string(),
        },
        SqlValidationError {
            location: "./tests/main.py:21".to_string(),
            description: "Table `admin` not found in schema".to_string(),
        },
        SqlValidationError {
            location: "./tests/main.py:28".to_string(),
            description: "Table `admin` not found in schema".to_string(),
        },
    ];
    assert_eq!(validation_errors, expected_validation_errors);
}
