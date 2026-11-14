"use strict";
const { randomUUID } = require("node:crypto");
const {
  WorkspaceConnection,
  localOrigin,
  relativePath,
} = require("./client.cjs");
function activate(context, vscode = require("vscode")) {
  const connection = new WorkspaceConnection();
  const output = vscode.window.createOutputChannel("Verb workspace");
  const snapshots = new Map();
  const changed = new vscode.EventEmitter();
  const provider = {
    onDidChange: changed.event,
    provideTextDocumentContent(uri) {
      return (
        snapshots.get(uri.toString())?.text ??
        "Verb snapshot cleared. Open the file again explicitly."
      );
    },
  };
  function clearSnapshots() {
    const uris = [...snapshots.values()].map((value) => value.uri);
    snapshots.clear();
    for (const uri of uris) changed.fire(uri);
  }
  let busy = false;
  let generation = 0;
  function show(snapshot) {
    output.clear();
    output.appendLine("Verb connection · read-only integration preview");
    output.appendLine("Observed now; refresh to check again.");
    output.appendLine(`Verb project: ${snapshot.workspaceId}`);
    output.appendLine(`Transport: ${snapshot.transport}`);
    output.appendLine(
      `Studio text editing: ${snapshot.editingAvailable ? "available" : "unavailable"}`,
    );
    output.appendLine(
      `Saved HTML preview: ${snapshot.previewAvailable ? "available" : "unavailable"}`,
    );
    output.appendLine(`Text file limit: ${snapshot.maxFileBytes} bytes`);
    output.appendLine(
      "These are Verb server capabilities. This extension opens read-only snapshots; it does not edit, execute or preview apps.",
    );
    output.appendLine(
      "The IDE folder has not been matched to this project. No file/terminal authority is delegated.",
    );
    output.show(true);
  }
  async function action(work) {
    if (busy)
      return vscode.window.showInformationMessage(
        "A Verb connection check is already running.",
      );
    if (!vscode.workspace.isTrusted)
      return vscode.window.showErrorMessage(
        "Trust this IDE workspace before connecting to Verb.",
      );
    busy = true;
    const attempt = generation;
    output.clear();
    output.appendLine(
      "Checking Verb… Previous observations are no longer current.",
    );
    try {
      const snapshot = await work(attempt);
      if (snapshot && attempt === generation) show(snapshot);
    } catch (error) {
      if (attempt === generation) {
        output.clear();
        output.appendLine(
          "Verb connection is unconfirmed. Refresh or reconnect explicitly.",
        );
        vscode.window.showErrorMessage(error.message);
      }
    } finally {
      busy = false;
    }
  }
  context.subscriptions.push(
    output,
    changed,
    vscode.workspace.registerTextDocumentContentProvider(
      "verb-snapshot",
      provider,
    ),
    vscode.commands.registerCommand("verb.connect", () =>
      action(async () => {
        clearSnapshots();
        connection.disconnect();
        const pending = generation;
        const origin = await vscode.window.showInputBox({
          title: "Connect to local Verb",
          prompt:
            "Address on the IDE extension host. This may differ from your browser device.",
          placeHolder: "http://127.0.0.1:4873",
          ignoreFocusOut: true,
          validateInput: (value) => {
            try {
              localOrigin(value);
              return undefined;
            } catch (error) {
              return error.message;
            }
          },
        });
        if (!origin || pending !== generation) return;
        const token = await vscode.window.showInputBox({
          title: "Verb access token",
          password: true,
          prompt:
            "Use a read-scoped token. Held in memory until disconnect or IDE reload; never saved.",
          ignoreFocusOut: true,
        });
        if (!token || pending !== generation) return;
        return connection.connect(origin, token);
      }),
    ),
    vscode.commands.registerCommand("verb.inspect", () =>
      action(() => connection.refresh()),
    ),
    vscode.commands.registerCommand("verb.openSnapshot", () =>
      action(async (attempt) => {
        if (snapshots.size >= 8)
          throw new Error(
            "Eight snapshots are open. Disconnect to clear them before opening more.",
          );
        const path = await vscode.window.showInputBox({
          title: "Open a Verb file snapshot",
          prompt:
            "Project-relative path. Reads only this file; never saves or runs its content.",
          validateInput: (value) => {
            try {
              relativePath(value);
              return undefined;
            } catch (error) {
              return error.message;
            }
          },
        });
        if (!path || attempt !== generation) return;
        const file = await connection.readFile(path);
        if (attempt !== generation) return;
        const uri = vscode.Uri.from({
          scheme: "verb-snapshot",
          authority: randomUUID(),
          path: "/" + file.path,
        });
        snapshots.set(uri.toString(), { text: file.text, uri });
        const document = await vscode.workspace.openTextDocument(uri);
        if (attempt !== generation) return;
        await vscode.window.showTextDocument(document, { preview: false });
        if (attempt !== generation) return;
        output.clear();
        output.appendLine(
          "Read-only saved-file snapshot · " + file.revision.slice(0, 12),
        );
        output.appendLine(
          "This is a snapshot, not a live editor. Reopen to read a newer revision. Nothing was saved or executed.",
        );
      }),
    ),
    vscode.commands.registerCommand("verb.disconnect", () => {
      ++generation;
      connection.disconnect();
      clearSnapshots();
      output.clear();
      output.appendLine("Disconnected. In-memory token cleared.");
    }),
    {
      dispose() {
        ++generation;
        connection.disconnect();
        clearSnapshots();
      },
    },
  );
}
module.exports = { activate };
