use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = "")]
struct Args {
    /// Directory. Defaults to "." (current)
    #[arg(short, long, value_hint = clap::ValueHint::DirPath)]
    directory: Option<std::path::PathBuf>,

    /// Schema file. Defaults to "schema.sql"
    #[arg(short, long, value_hint = clap::ValueHint::DirPath)]
    schema: Option<std::path::PathBuf>,
}

fn main() {
    let args = Args::parse();

    let directory = args.directory.unwrap_or(std::path::PathBuf::from("."));
    let schema = args
        .schema
        .unwrap_or(std::path::PathBuf::from("schema.sql"));

    let validation_errors = match sqlshield::validate_files(&directory, &schema) {
        Ok(errors) => errors,
        Err(err) => {
            eprintln!("sqlshield: {err}");
            std::process::exit(1);
        }
    };

    for error in &validation_errors {
        println!("{error}");
    }

    if !validation_errors.is_empty() {
        std::process::exit(1);
    }
}
