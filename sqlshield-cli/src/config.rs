//! Loading `.sqlshield.toml` from the current working directory.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::Deserialize;
use sqlshield::Dialect;

pub const CONFIG_FILE_NAME: &str = ".sqlshield.toml";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawConfig {
    pub schema: Option<PathBuf>,
    pub directory: Option<PathBuf>,
    pub dialect: Option<String>,
}

#[derive(Debug, Default)]
pub struct Config {
    pub schema: Option<PathBuf>,
    pub directory: Option<PathBuf>,
    pub dialect: Option<Dialect>,
}

/// Load `.sqlshield.toml` from `dir`. Returns an empty config (`Ok(None)`)
/// if no file is present. Only malformed files error; missing ones are
/// just defaults.
pub fn load_from(dir: &Path) -> Result<Option<Config>, String> {
    let path = dir.join(CONFIG_FILE_NAME);
    if !path.is_file() {
        return Ok(None);
    }

    let source = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;

    let raw: RawConfig =
        toml::from_str(&source).map_err(|e| format!("invalid {}: {e}", path.display()))?;

    let dialect = raw.dialect.as_deref().map(Dialect::from_str).transpose()?;

    Ok(Some(Config {
        schema: raw.schema,
        directory: raw.directory,
        dialect,
    }))
}
