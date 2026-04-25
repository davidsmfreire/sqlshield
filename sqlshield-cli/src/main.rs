mod config;

use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, ValueEnum};
use serde::Serialize;
use sqlshield::{schema::TablesAndColumns, Dialect, SqlShieldError};

const EXIT_VALIDATION_ERRORS: u8 = 1;
const EXIT_CONFIG_ERROR: u8 = 2;

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = "")]
struct Args {
    /// Directory. Defaults to "." (current). Ignored in --stdin mode.
    #[arg(short, long, value_hint = clap::ValueHint::DirPath)]
    directory: Option<PathBuf>,

    /// Schema file. Defaults to "schema.sql".
    #[arg(short, long, value_hint = clap::ValueHint::FilePath)]
    schema: Option<PathBuf>,

    /// Live database URL — read schema directly from a running DB
    /// (postgres / sqlite). Mutually exclusive with --schema.
    /// Examples: `postgres://user:pass@localhost/mydb`,
    /// `sqlite:///abs/path/to/db.sqlite`, `./relative.sqlite`.
    #[arg(long, conflicts_with = "schema")]
    db_url: Option<String>,

    /// SQL dialect to parse with (generic, postgres, mysql, sqlite, mssql,
    /// snowflake, bigquery, redshift, clickhouse, duckdb, hive, ansi).
    #[arg(long)]
    dialect: Option<Dialect>,

    /// Output format.
    #[arg(long, value_enum, default_value = "text")]
    format: OutputFormat,

    /// Read a single SQL query from stdin and validate it. Useful for
    /// editor integrations. Ignores --directory.
    #[arg(long, conflicts_with = "directory")]
    stdin: bool,
}

#[derive(Serialize)]
struct JsonErrorReport<'a> {
    location: &'a str,
    description: &'a str,
}

#[derive(Serialize)]
struct JsonStdinReport<'a> {
    description: &'a str,
}

fn main() -> ExitCode {
    let args = Args::parse();

    // Layer: CLI flags > .sqlshield.toml > defaults.
    let file_config = match config::load_from(std::path::Path::new(".")) {
        Ok(cfg) => cfg.unwrap_or_default(),
        Err(err) => {
            eprintln!("sqlshield: {err}");
            return ExitCode::from(EXIT_CONFIG_ERROR);
        }
    };

    let dialect = args.dialect.or(file_config.dialect).unwrap_or_default();

    let db_url = args.db_url.clone().or_else(|| file_config.db_url.clone());

    if args.stdin {
        let schema_path = args
            .schema
            .clone()
            .or_else(|| file_config.schema.clone())
            .unwrap_or_else(|| PathBuf::from("schema.sql"));
        return run_stdin(&schema_path, dialect, args.format);
    }

    let directory = args
        .directory
        .clone()
        .or(file_config.directory)
        .unwrap_or_else(|| PathBuf::from("."));

    // Resolve the schema either via live introspection or from a file.
    let schema: TablesAndColumns = match db_url.as_deref() {
        Some(url) => match introspect_schema(url) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("sqlshield: {err}");
                return ExitCode::from(EXIT_CONFIG_ERROR);
            }
        },
        None => {
            let schema_path = args
                .schema
                .clone()
                .or(file_config.schema)
                .unwrap_or_else(|| PathBuf::from("schema.sql"));
            match sqlshield::schema::load_schema_from_file(&schema_path, dialect) {
                Ok(s) => s,
                Err(err) => {
                    eprintln!("sqlshield: {err}");
                    return ExitCode::from(EXIT_CONFIG_ERROR);
                }
            }
        }
    };

    let validation_errors = sqlshield::validate_files_with_schema(&directory, &schema, dialect);

    match args.format {
        OutputFormat::Text => {
            for error in &validation_errors {
                println!("{error}");
            }
        }
        OutputFormat::Json => {
            let reports: Vec<JsonErrorReport<'_>> = validation_errors
                .iter()
                .map(|e| JsonErrorReport {
                    location: &e.location,
                    description: &e.description,
                })
                .collect();
            match serde_json::to_string_pretty(&reports) {
                Ok(s) => println!("{s}"),
                Err(err) => {
                    eprintln!("sqlshield: failed to serialize JSON: {err}");
                    return ExitCode::from(EXIT_CONFIG_ERROR);
                }
            }
        }
    }

    if validation_errors.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(EXIT_VALIDATION_ERRORS)
    }
}

#[cfg(feature = "introspect")]
fn introspect_schema(url: &str) -> Result<TablesAndColumns, String> {
    sqlshield_introspect::introspect(url).map_err(|e| e.to_string())
}

#[cfg(not(feature = "introspect"))]
fn introspect_schema(_url: &str) -> Result<TablesAndColumns, String> {
    Err("--db-url requires the `introspect` feature (rebuild with --features introspect)".into())
}

fn run_stdin(schema_path: &std::path::Path, dialect: Dialect, format: OutputFormat) -> ExitCode {
    let mut query = String::new();
    if let Err(err) = std::io::stdin().read_to_string(&mut query) {
        eprintln!("sqlshield: failed to read stdin: {err}");
        return ExitCode::from(EXIT_CONFIG_ERROR);
    }

    let schema_source = match std::fs::read_to_string(schema_path) {
        Ok(s) => s,
        Err(err) => {
            eprintln!(
                "sqlshield: failed to read schema {}: {err}",
                schema_path.display()
            );
            return ExitCode::from(EXIT_CONFIG_ERROR);
        }
    };

    match sqlshield::validate_query_with_dialect(&query, &schema_source, dialect) {
        Ok(errors) => {
            match format {
                OutputFormat::Text => {
                    for err in &errors {
                        println!("{err}");
                    }
                }
                OutputFormat::Json => {
                    let reports: Vec<JsonStdinReport<'_>> = errors
                        .iter()
                        .map(|e| JsonStdinReport { description: e })
                        .collect();
                    match serde_json::to_string_pretty(&reports) {
                        Ok(s) => println!("{s}"),
                        Err(err) => {
                            eprintln!("sqlshield: failed to serialize JSON: {err}");
                            return ExitCode::from(EXIT_CONFIG_ERROR);
                        }
                    }
                }
            }
            if errors.is_empty() {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(EXIT_VALIDATION_ERRORS)
            }
        }
        Err(err @ SqlShieldError::SqlParse(_)) => {
            eprintln!("sqlshield: {err}");
            ExitCode::from(EXIT_VALIDATION_ERRORS)
        }
        Err(err) => {
            eprintln!("sqlshield: {err}");
            ExitCode::from(EXIT_CONFIG_ERROR)
        }
    }
}
