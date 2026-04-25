//! Zed extension entry point. Wraps the `sqlshield-lsp` binary so SQL
//! diagnostics for embedded queries (and standalone .sql files) light up
//! inside Zed.
//!
//! Binary discovery: prefer `sqlshield-lsp` already on PATH (the user
//! ran `cargo install sqlshield-lsp`); otherwise fall back to a
//! GitHub-release download keyed by the current platform/arch.
//!
//! Settings: forwarded to the LSP via `language_server_workspace_configuration`,
//! merged with `.sqlshield.toml` server-side (editor wins per-field).

use std::fs;

use zed::settings::LspSettings;
use zed_extension_api::{
    self as zed,
    serde_json::{self, Value},
    LanguageServerId, Result, Worktree,
};

const GITHUB_REPO: &str = "davidsmfreire/sqlshield";
const TAG_PREFIX: &str = "sqlshield-lsp-v";

struct SqlshieldExtension {
    cached_binary_path: Option<String>,
}

impl SqlshieldExtension {
    fn language_server_binary_path(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<String> {
        // Honor a user-provided absolute path first (Zed LSP setting
        // `binary.path` under `sqlshield`). Lets self-built binaries
        // win over both the cached download and PATH lookup.
        if let Ok(settings) = LspSettings::for_worktree("sqlshield", worktree) {
            if let Some(binary) = settings.binary {
                if let Some(path) = binary.path {
                    return Ok(path);
                }
            }
        }

        if let Some(path) = worktree.which("sqlshield-lsp") {
            return Ok(path);
        }

        if let Some(path) = &self.cached_binary_path {
            if fs::metadata(path).is_ok_and(|stat| stat.is_file()) {
                return Ok(path.clone());
            }
        }

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let release = zed::latest_github_release(
            GITHUB_REPO,
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )?;

        // release-plz tags every workspace crate at the same SHA, but
        // `latest_github_release` returns whichever release is most recent.
        // Reject anything that isn't an `sqlshield-lsp-v*` tag; users who
        // hit this should bump the LSP via `cargo install sqlshield-lsp`.
        if !release.version.starts_with(TAG_PREFIX) && !release.version.starts_with("v") {
            return Err(format!(
                "latest GitHub release ({}) is not an sqlshield-lsp release; \
                 install the binary with `cargo install sqlshield-lsp` and \
                 ensure it is on PATH.",
                release.version
            ));
        }

        let (platform, arch) = zed::current_platform();
        let target = match (platform, arch) {
            (zed::Os::Linux, zed::Architecture::X8664) => "x86_64-unknown-linux-gnu",
            (zed::Os::Linux, zed::Architecture::Aarch64) => "aarch64-unknown-linux-gnu",
            (zed::Os::Mac, zed::Architecture::X8664) => "x86_64-apple-darwin",
            (zed::Os::Mac, zed::Architecture::Aarch64) => "aarch64-apple-darwin",
            (zed::Os::Windows, zed::Architecture::X8664) => "x86_64-pc-windows-msvc",
            (zed::Os::Windows, zed::Architecture::Aarch64) => "aarch64-pc-windows-msvc",
            _ => {
                return Err(format!(
                    "unsupported platform/arch: {platform:?}/{arch:?}; \
                     install the binary with `cargo install sqlshield-lsp`."
                ));
            }
        };

        let (asset_name, download_kind) = match platform {
            zed::Os::Windows => (
                format!("sqlshield-lsp-{target}.zip"),
                zed::DownloadedFileType::Zip,
            ),
            _ => (
                format!("sqlshield-lsp-{target}.tar.gz"),
                zed::DownloadedFileType::GzipTar,
            ),
        };

        let asset = release
            .assets
            .iter()
            .find(|a| a.name == asset_name)
            .ok_or_else(|| {
                format!(
                    "no GitHub release asset matching {asset_name:?}; \
                     install with `cargo install sqlshield-lsp`."
                )
            })?;

        let version_dir = format!("sqlshield-lsp-{}", release.version);
        let binary_name = match platform {
            zed::Os::Windows => "sqlshield-lsp.exe",
            _ => "sqlshield-lsp",
        };
        let binary_path = format!("{version_dir}/{binary_name}");

        if !fs::metadata(&binary_path).is_ok_and(|stat| stat.is_file()) {
            zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Downloading,
            );

            zed::download_file(&asset.download_url, &version_dir, download_kind)
                .map_err(|e| format!("failed to download {asset_name}: {e}"))?;

            // Drop older version directories to keep the extension cache
            // small after upgrades.
            if let Ok(entries) = fs::read_dir(".") {
                for entry in entries.flatten() {
                    if entry.file_name().to_str() != Some(&version_dir) {
                        fs::remove_dir_all(entry.path()).ok();
                    }
                }
            }
        }

        zed::make_file_executable(&binary_path).ok();

        self.cached_binary_path = Some(binary_path.clone());
        Ok(binary_path)
    }
}

impl zed::Extension for SqlshieldExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<zed::Command> {
        Ok(zed::Command {
            command: self.language_server_binary_path(language_server_id, worktree)?,
            args: Vec::new(),
            env: Default::default(),
        })
    }

    fn language_server_workspace_configuration(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> Result<Option<Value>> {
        // Pass through the user's `lsp.sqlshield.settings` block (Zed's
        // standard LSP-settings shape) to the server. Server merges with
        // `.sqlshield.toml`, editor wins per-field. See
        // sqlshield-lsp/src/config.rs::EditorSettings.
        let settings = LspSettings::for_worktree("sqlshield", worktree)
            .ok()
            .and_then(|s| s.settings)
            .unwrap_or_else(|| serde_json::json!({}));

        Ok(Some(serde_json::json!({ "sqlshield": settings })))
    }
}

zed::register_extension!(SqlshieldExtension);
