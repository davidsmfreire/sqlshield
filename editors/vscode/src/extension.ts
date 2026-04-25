// Thin VS Code wrapper around the `sqlshield-lsp` server binary. The
// extension's only job is to spawn the server, wire up the LSP transport,
// forward configuration to the server, and expose a "Restart Language
// Server" command. All diagnostic logic lives in the Rust crate.

import * as fs from "fs";
import * as path from "path";
import { ExtensionContext, window, workspace, commands } from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export async function activate(context: ExtensionContext): Promise<void> {
  context.subscriptions.push(
    commands.registerCommand("sqlshield.restartServer", async () => {
      await stop();
      await start(context);
    }),
    workspace.onDidChangeConfiguration(async (e) => {
      // Server-relevant settings that require a restart (the binary path
      // can't be hot-swapped). Per-doc settings (schema/dialect) are
      // instead pushed to the running server via didChangeConfiguration,
      // which the LSP client wires up automatically.
      if (e.affectsConfiguration("sqlshield.serverPath")) {
        await stop();
        await start(context);
      }
    }),
  );

  await start(context);
}

export async function deactivate(): Promise<void> {
  await stop();
}

async function start(context: ExtensionContext): Promise<void> {
  const serverPath = resolveServerPath(context);

  const serverOptions: ServerOptions = {
    run: { command: serverPath, transport: TransportKind.stdio },
    debug: { command: serverPath, transport: TransportKind.stdio },
  };

  // Diagnostics are published for any document the server cares about; the
  // server itself decides whether the file extension is one it handles.
  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "sql" },
      { scheme: "file", language: "python" },
      { scheme: "file", language: "rust" },
      { scheme: "file", language: "go" },
      { scheme: "file", language: "javascript" },
      { scheme: "file", language: "javascriptreact" },
      { scheme: "file", language: "typescript" },
      { scheme: "file", language: "typescriptreact" },
    ],
    initializationOptions: () => readSqlshieldSettings(),
    synchronize: {
      configurationSection: "sqlshield",
      fileEvents: workspace.createFileSystemWatcher("**/.sqlshield.toml"),
    },
    outputChannelName: "sqlshield",
  };

  client = new LanguageClient(
    "sqlshield",
    "sqlshield",
    serverOptions,
    clientOptions,
  );

  try {
    await client.start();
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    window.showErrorMessage(
      `sqlshield: failed to start language server (${serverPath}): ${msg}. ` +
        `Install with \`cargo install sqlshield-lsp\` or set 'sqlshield.serverPath'.`,
    );
    client = undefined;
  }
}

async function stop(): Promise<void> {
  if (!client) return;
  try {
    await client.stop();
  } catch {
    // The server may already be gone — swallow shutdown errors so a
    // restart-after-crash isn't blocked.
  }
  client = undefined;
}

/// Resolve the LSP binary in this order:
///   1. `sqlshield.serverPath` setting (explicit override).
///   2. Bundled binary inside the VSIX (shipped by platform-specific
///      builds in CI).
///   3. `sqlshield-lsp` on PATH (lets users who `cargo install` keep
///      working without bundled binaries).
function resolveServerPath(context: ExtensionContext): string {
  const override = workspace
    .getConfiguration("sqlshield")
    .get<string>("serverPath", "")
    .trim();
  if (override) {
    return override;
  }

  const exe = process.platform === "win32" ? "sqlshield-lsp.exe" : "sqlshield-lsp";
  const bundled = path.join(context.extensionPath, "server", exe);
  if (fs.existsSync(bundled)) {
    return bundled;
  }

  return "sqlshield-lsp";
}

/// The settings we forward to the LSP server. Keep in sync with the
/// `EditorSettings` struct in `sqlshield-lsp/src/config.rs`.
function readSqlshieldSettings(): { schema?: string; dialect?: string } {
  const cfg = workspace.getConfiguration("sqlshield");
  return {
    schema: cfg.get<string>("schema", "") || undefined,
    dialect: cfg.get<string>("dialect", "") || undefined,
  };
}
