// Thin VS Code wrapper around the `sqlshield-lsp` server binary. The
// extension's only job is to spawn the server, wire up the LSP transport,
// and expose a "Restart Language Server" command. All diagnostic logic lives
// in the Rust crate.

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
  );

  await start(context);
}

export async function deactivate(): Promise<void> {
  await stop();
}

async function start(_context: ExtensionContext): Promise<void> {
  const config = workspace.getConfiguration("sqlshield");
  const serverPath = config.get<string>("serverPath") ?? "sqlshield-lsp";

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
    ],
    synchronize: {
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
