//! Server configuration. Two sources, merged per-field with editor wins:
//!   1. Editor settings — pushed via `initializationOptions` and
//!      `workspace/didChangeConfiguration`. Drives the VS Code
//!      `sqlshield.*` settings UI.
//!   2. `.sqlshield.toml` — discovered by walking up from the workspace
//!      root. Used as a fallback for any field the editor leaves blank.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use serde::Deserialize;
use sqlshield::Dialect;

pub const CONFIG_FILE_NAME: &str = ".sqlshield.toml";

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTomlConfig {
    schema: Option<PathBuf>,
    dialect: Option<String>,
}

/// Editor-supplied settings. Mirrors the `sqlshield.*` keys declared in
/// the VS Code extension's `package.json`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct EditorSettings {
    pub schema: Option<String>,
    pub dialect: Option<String>,
}

impl EditorSettings {
    /// Parse from the loose JSON value VS Code sends. Empty strings are
    /// treated as "not set" so the user can clear a field in the UI and
    /// fall back to `.sqlshield.toml`.
    pub fn from_value(value: &serde_json::Value) -> Self {
        if value.is_null() {
            return Self::default();
        }
        let mut s: Self = serde_json::from_value(value.clone()).unwrap_or_default();
        if matches!(s.schema.as_deref(), Some("")) {
            s.schema = None;
        }
        if matches!(s.dialect.as_deref(), Some("")) {
            s.dialect = None;
        }
        s
    }

    /// Pick the `sqlshield` section out of a `workspace/didChangeConfiguration`
    /// payload, which may either nest under the section name or arrive
    /// flat (depending on client behavior).
    pub fn from_change_notification(value: &serde_json::Value) -> Self {
        if let Some(nested) = value.get("sqlshield") {
            return Self::from_value(nested);
        }
        Self::from_value(value)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ServerConfig {
    pub schema_path: Option<PathBuf>,
    pub dialect: Dialect,
}

/// Resolve the effective server config from both sources. Editor settings
/// win per-field; `.sqlshield.toml` fills any gaps.
pub fn resolve(workspace_root: &Path, editor: &EditorSettings) -> Result<ServerConfig, String> {
    let toml_cfg = discover_toml(workspace_root)?;

    let dialect = match editor.dialect.as_deref() {
        Some(d) => Dialect::from_str(d)?,
        None => toml_cfg.dialect,
    };

    let schema_path = match editor.schema.as_deref() {
        Some(s) => {
            let p = PathBuf::from(s);
            Some(if p.is_absolute() {
                p
            } else {
                workspace_root.join(p)
            })
        }
        None => toml_cfg.schema_path,
    };

    Ok(ServerConfig {
        schema_path,
        dialect,
    })
}

/// Walk upward from `start_dir` looking for `.sqlshield.toml`, parse it,
/// and return the loaded config. Returns defaults if no config is found.
/// Paths inside the config are resolved relative to the config's directory.
pub fn discover_toml(start_dir: &Path) -> Result<ServerConfig, String> {
    let Some((config_dir, path)) = find_config_file(start_dir) else {
        return Ok(ServerConfig::default());
    };

    let source = std::fs::read_to_string(&path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let raw: RawTomlConfig =
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn editor_empty_strings_become_none() {
        let s = EditorSettings::from_value(&json!({ "schema": "", "dialect": "" }));
        assert!(s.schema.is_none());
        assert!(s.dialect.is_none());
    }

    #[test]
    fn editor_nested_under_section_name() {
        let s = EditorSettings::from_change_notification(
            &json!({ "sqlshield": { "schema": "s.sql", "dialect": "postgres" } }),
        );
        assert_eq!(s.schema.as_deref(), Some("s.sql"));
        assert_eq!(s.dialect.as_deref(), Some("postgres"));
    }

    #[test]
    fn editor_flat_settings() {
        let s = EditorSettings::from_change_notification(
            &json!({ "schema": "s.sql", "dialect": "mysql" }),
        );
        assert_eq!(s.schema.as_deref(), Some("s.sql"));
        assert_eq!(s.dialect.as_deref(), Some("mysql"));
    }

    #[test]
    fn editor_unknown_keys_ignored() {
        // Forward-compat: don't fail if user has stale or future settings.
        let s = EditorSettings::from_value(&json!({ "schema": "s.sql", "future_key": 42 }));
        assert_eq!(s.schema.as_deref(), Some("s.sql"));
    }
}
