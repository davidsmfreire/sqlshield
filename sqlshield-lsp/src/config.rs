//! Locate and load `.sqlshield.toml` for LSP server configuration.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::Deserialize;
use sqlshield::Dialect;

pub const CONFIG_FILE_NAME: &str = ".sqlshield.toml";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    schema: Option<PathBuf>,
    dialect: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ServerConfig {
    pub schema_path: Option<PathBuf>,
    pub dialect: Dialect,
}

/// Walk upward from `start_dir` looking for `.sqlshield.toml`, parse it, and
/// return the loaded config. Returns defaults if no config is found.
/// Paths inside the config are resolved relative to the config's directory.
pub fn discover(start_dir: &Path) -> Result<ServerConfig, String> {
    let Some((config_dir, path)) = find_config_file(start_dir) else {
        return Ok(ServerConfig::default());
    };

    let source = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let raw: RawConfig =
        toml::from_str(&source).map_err(|e| format!("invalid {}: {e}", path.display()))?;

    let schema_path = raw.schema.map(|p| {
        if p.is_absolute() {
            p
        } else {
            config_dir.join(p)
        }
    });
    let dialect = raw
        .dialect
        .as_deref()
        .map(Dialect::from_str)
        .transpose()?
        .unwrap_or_default();

    Ok(ServerConfig {
        schema_path,
        dialect,
    })
}

fn find_config_file(start: &Path) -> Option<(PathBuf, PathBuf)> {
    let mut current = Some(start.to_path_buf());
    while let Some(dir) = current {
        let candidate = dir.join(CONFIG_FILE_NAME);
        if candidate.is_file() {
            return Some((dir, candidate));
        }
        current = dir.parent().map(|p| p.to_path_buf());
    }
    None
}
