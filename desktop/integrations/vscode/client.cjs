"use strict";
const http = require("node:http");
const { createHash } = require("node:crypto");
const MAX_RESPONSE = 16 * 1024;
function localOrigin(value) {
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new Error(
      "Enter a local HTTP origin, for example http://127.0.0.1:4873.",
    );
  }
  if (
    url.protocol !== "http:" ||
    !["127.0.0.1", "[::1]"].includes(url.hostname) ||
    url.username ||
    url.password ||
    url.pathname !== "/" ||
    url.search ||
    url.hash
  )
    throw new Error(
      "Use an explicit loopback HTTP origin without credentials, path, query or fragment.",
    );
  return url.origin;
}
function capabilities(value) {
  if (
    !value ||
    value.schemaVersion !== 1 ||
    typeof value.workspaceId !== "string" ||
    !/^[a-zA-Z0-9_-]{1,128}$/.test(value.workspaceId) ||
    !["native-local", "monitored-v1", "unavailable"].includes(
      value.transport,
    ) ||
    typeof value.editing?.available !== "boolean" ||
    typeof value.preview?.available !== "boolean" ||
    !Number.isSafeInteger(value.editing.maxBytes) ||
    value.editing.maxBytes < 1 ||
    value.editing.maxBytes > 200 * 1024 ||
    value.preview.kind !== "single-file-html" ||
    (value.transport === "unavailable" &&
      (value.editing.available || value.preview.available))
  )
    throw new Error(
      "Verb returned an unsupported workspace contract. Disconnect and check the server version.",
    );
  return Object.freeze({
    workspaceId: value.workspaceId,
    transport: value.transport,
    editingAvailable: value.editing.available,
    previewAvailable: value.preview.available,
    maxFileBytes: value.editing.maxBytes,
  });
}
function readJSON(origin, token, path, limit, { timeoutMs = 5000 } = {}) {
  origin = localOrigin(origin);
  if (
    typeof token !== "string" ||
    !token ||
    token.length > 4096 ||
    /[^\x21-\x7e]/.test(token)
  )
    throw new Error("Enter a nonempty Verb access token without whitespace.");
  return new Promise((resolve, reject) => {
    let done = false;
    const finish = (error, value) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      error ? reject(error) : resolve(value);
    };
    const request = http.get(
      origin + path,
      {
        headers: { "X-Verb-Token": token, Origin: origin, Connection: "close" },
        agent: false,
      },
      (response) => {
        if (response.statusCode !== 200) {
          response.destroy();
          return finish(
            new Error(
              response.statusCode === 401 || response.statusCode === 403
                ? "Verb refused access. Check the token and host access settings."
                : "Verb did not return workspace details. Redirects are never followed.",
            ),
          );
        }
        let size = 0;
        const chunks = [];
        response.on("data", (chunk) => {
          size += chunk.length;
          if (size > limit) {
            response.destroy();
            finish(
              new Error("Workspace response exceeded the metadata limit."),
            );
          } else chunks.push(chunk);
        });
        response.on("end", () => {
          try {
            finish(null, JSON.parse(Buffer.concat(chunks).toString("utf8")));
          } catch {
            finish(
              new Error("Verb returned an unsupported workspace contract."),
            );
          }
        });
        response.on("error", () =>
          finish(
            new Error(
              "Workspace response was interrupted. Refresh explicitly.",
            ),
          ),
        );
      },
    );
    const timer = setTimeout(() => {
      request.destroy();
      finish(
        new Error(
          "Verb did not respond before the deadline. Refresh explicitly.",
        ),
      );
    }, timeoutMs);
    request.on("error", () =>
      finish(
        new Error(
          "Could not reach local Verb. Check its address and running state.",
        ),
      ),
    );
  });
}
function readWorkspace(origin, token, options) {
  return readJSON(
    origin,
    token,
    "/api/studio/workspace",
    MAX_RESPONSE,
    options,
  ).then(capabilities);
}
function relativePath(value) {
  if (
    typeof value !== "string" ||
    !value ||
    value.length > 4096 ||
    /[\\\x00-\x1f\x7f]/.test(value) ||
    value.split("/").some((part) => !part || part === "." || part === "..")
  )
    throw new Error(
      "Enter a project-relative file path without traversal or control characters.",
    );
  return value;
}
function document(value, id, path) {
  const file = value?.file;
  if (
    value?.workspaceId !== id ||
    !file ||
    file.path !== path ||
    typeof file.text !== "string" ||
    !Number.isSafeInteger(file.bytes) ||
    file.bytes < 0 ||
    file.bytes > 200 * 1024 ||
    Buffer.byteLength(file.text, "utf8") !== file.bytes ||
    typeof file.revision !== "string" ||
    !/^[a-f0-9]{64}$/.test(file.revision) ||
    createHash("sha256").update(file.text, "utf8").digest("hex") !==
      file.revision
  )
    throw new Error(
      "File identity or content revision did not match. No document opened.",
    );
  return Object.freeze({
    workspaceId: id,
    path,
    text: file.text,
    revision: file.revision,
    bytes: file.bytes,
  });
}
class WorkspaceConnection {
  #token;
  #origin;
  #id;
  #generation = 0;
  async connect(origin, token) {
    this.disconnect();
    const generation = this.#generation;
    origin = localOrigin(origin);
    const snapshot = await readWorkspace(origin, token);
    if (generation !== this.#generation)
      throw new Error("Connection was cancelled.");
    this.#origin = origin;
    this.#token = token;
    this.#id = snapshot.workspaceId;
    return snapshot;
  }
  async refresh() {
    if (!this.#token)
      throw new Error("Connect to a local Verb workspace first.");
    const generation = this.#generation;
    const snapshot = await readWorkspace(this.#origin, this.#token);
    if (generation !== this.#generation)
      throw new Error("Connection changed. Refresh explicitly.");
    if (snapshot.workspaceId !== this.#id) {
      this.disconnect();
      throw new Error(
        "Verb now identifies a different project. Disconnected; connect again deliberately.",
      );
    }
    return snapshot;
  }
  async readFile(path) {
    path = relativePath(path);
    const snapshot = await this.refresh();
    if (snapshot.transport === "unavailable")
      throw new Error("This Verb host has no connected file bridge.");
    const generation = this.#generation,
      id = this.#id;
    const value = await readJSON(
      this.#origin,
      this.#token,
      "/api/studio/file?path=" + encodeURIComponent(path),
      2 * 1024 * 1024,
    );
    if (generation !== this.#generation)
      throw new Error("Connection changed. No document opened.");
    return document(value, id, path);
  }
  disconnect() {
    ++this.#generation;
    this.#token = this.#origin = this.#id = undefined;
  }
}
module.exports = {
  localOrigin,
  capabilities,
  readWorkspace,
  relativePath,
  document,
  WorkspaceConnection,
};
