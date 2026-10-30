export function connectionResultText(value) {
  if (value?.truncated && typeof value.text === "string")
    return `[Truncated service response]\n${value.text}`;
  const blocks = value?.content || value?.contents;
  const text = Array.isArray(blocks)
    ? blocks
        .filter((block) => typeof block.text === "string")
        .map((block) => block.text)
        .join("\n\n")
    : "";
  const structured = value?.structuredContent;
  return (
    [text, structured ? JSON.stringify(structured, null, 2) : ""]
      .filter(Boolean)
      .join("\n\n") || JSON.stringify(value, null, 2)
  );
}

// Project-scoped MCP connections. Results stay in this tab until explicitly attached to a draft.
export function validateConnectionNumbers(value) {
  const pending = [value];
  let count = 0;
  while (pending.length) {
    if (++count > 10000)
      throw new Error("Inputs are too complex for this request");
    const item = pending.pop();
    if (
      typeof item === "number" &&
      (!Number.isFinite(item) ||
        (Number.isInteger(item) && !Number.isSafeInteger(item)))
    )
      throw new Error(
        "An input number cannot be represented precisely. Use a supported number, or a string if the tool allows it",
      );
    if (item && typeof item === "object") pending.push(...Object.values(item));
  }
}

export function initConnections({ $, api, escapeHtml: esc, useInStudio }) {
  let timer = null;
  let selected = null,
    epoch = 0,
    busy = false,
    listSerial = 0,
    ticket = null,
    result = null,
    catalog = null;
  const say = (text) => {
    $("#connections-status").textContent = text;
  };
  const post = (body) => api("POST", "/api/connections", body);
  const choose = () => {
    const [kind, index] = $("#connection-item").value.split(":");
    return { kind, item: catalog?.[kind]?.[Number(index)] };
  };
  function invalidate() {
    ticket = null;
    $("#connection-review").hidden = true;
    $("#connection-run").disabled = true;
  }
  function controls() {
    $("#connection-connect").disabled = busy;
    for (const key of ["name", "url", "credential"])
      $(`#connection-${key}`).disabled = busy;
    $("#connection-item").disabled = busy;
    $("#connection-arguments").disabled = busy;
    $("#connection-use-json").disabled = busy || !inputFields;
    $("#connection-input-fields")
      .querySelectorAll("input,select")
      .forEach((field) => {
        field.disabled = busy;
      });
    $("#connection-review-call").disabled = busy || !choose().item;
    $("#connection-refresh").disabled = busy || !selected;
    $("#connection-disconnect").disabled = !selected;
    $("#connection-run").disabled = busy || !ticket;
  }
  function clear() {
    catalog = null;
    result = null;
    invalidate();
    $("#connection-item").replaceChildren();
    $("#connection-detail").hidden = true;
    $("#connection-result-wrap").hidden = true;
    $("#connection-result").textContent = "";
    controls();
  }
  let inputFields = null;
  function renderInputs(schema) {
    const properties = Object.entries(schema?.properties || {});
    inputFields =
      schema?.type === "object" &&
      properties.length <= 12 &&
      !["allOf", "anyOf", "oneOf", "$ref"].some((k) => schema[k]) &&
      properties.every(
        ([, p]) =>
          p &&
          ["string", "number", "integer", "boolean"].includes(p.type) &&
          !p.$ref &&
          (!p.enum ||
            (p.type === "string" &&
              Array.isArray(p.enum) &&
              p.enum.every((v) => typeof v === "string"))),
      )
        ? properties
        : null;
    const required = new Set(
      Array.isArray(schema?.required) ? schema.required : [],
    );
    $("#connection-input-fields").innerHTML = inputFields
      ? inputFields
          .map(([key, p], i) => {
            const label =
              p.title ||
              key.replaceAll("_", " ").replace(/^./, (c) => c.toUpperCase());
            const attrs = `id="connection-input-${i}" aria-label="${esc(label)}"`;
            let input;
            if (p.type === "boolean")
              input = `<input ${attrs} type="checkbox"${p.default === true ? " checked" : ""}>`;
            else if (p.enum)
              input = `<select ${attrs}><option value="">${required.has(key) ? "Choose a value" : "Not set"}</option>${p.enum.map((v) => `<option value="${esc(v)}"${p.default === v ? " selected" : ""}>${esc(v)}</option>`).join("")}</select>`;
            else
              input = `<input ${attrs} type="${p.type === "string" ? "text" : "number"}"${p.type === "integer" ? ' step="1"' : p.type === "number" ? ' step="any"' : ""} value="${esc(p.default == null ? "" : String(p.default))}">`;
            return `<label>${esc(label)}${required.has(key) ? "" : " (optional)"}${input}${p.description ? `<small class="muted">${esc(p.description)}</small>` : ""}</label>`;
          })
          .join("")
      : '<p class="muted">This tool has complex inputs. Edit its JSON using the input format above.</p>';
    $("#connection-use-json").checked = !inputFields;
    $("#connection-use-json").disabled = !inputFields;
    $("#connection-json-wrap").hidden = Boolean(inputFields);
  }
  function readInputs() {
    if (!inputFields || $("#connection-use-json").checked)
      return JSON.parse($("#connection-arguments").value);
    const required = new Set(
        Array.isArray(choose().item?.inputSchema?.required)
          ? choose().item.inputSchema.required
          : [],
      ),
      values = Object.create(null);
    for (const [i, [key, p]] of inputFields.entries()) {
      const field = $(`#connection-input-${i}`),
        value = field.value;
      if (p.type === "boolean") {
        values[key] = field.checked;
        continue;
      }
      if (value === "" && !required.has(key)) continue;
      if (p.type === "number" || p.type === "integer") {
        const number = Number(value);
        if (
          value === "" ||
          !Number.isFinite(number) ||
          (p.type === "integer" && !Number.isInteger(number))
        )
          throw new Error("Enter a valid number for " + key);
        values[key] = number;
      } else values[key] = value;
    }
    return values;
  }
  $("#connection-input-fields").addEventListener("input", invalidate);
  $("#connection-use-json").onchange = () => {
    const checkbox = $("#connection-use-json");
    if (checkbox.checked) {
      checkbox.checked = false;
      try {
        $("#connection-arguments").value = JSON.stringify(
          readInputs(),
          null,
          2,
        );
      } catch {
        say(
          "Finish the named number input, or correct the JSON draft before reviewing.",
        );
      }
      checkbox.checked = true;
    } else {
      try {
        const value = JSON.parse($("#connection-arguments").value),
          known = new Map(inputFields);
        if (
          !value ||
          Array.isArray(value) ||
          typeof value !== "object" ||
          Object.keys(value).some((key) => !known.has(key))
        )
          throw new Error();
        for (const [key, p] of inputFields) {
          if (
            value[key] !== undefined &&
            (p.type === "integer"
              ? !Number.isInteger(value[key])
              : p.type === "number"
                ? typeof value[key] !== "number"
                : typeof value[key] !== p.type)
          )
            throw new Error();
          if (
            p.enum &&
            value[key] !== undefined &&
            !p.enum.includes(value[key])
          )
            throw new Error();
          if (
            p.type === "string" &&
            value[key] === "" &&
            !choose().item.inputSchema.required?.includes(key)
          )
            throw new Error();
        }
        inputFields.forEach(([key, p], i) => {
          const field = $(`#connection-input-${i}`);
          if (p.type === "boolean") field.checked = value[key] === true;
          else field.value = value[key] ?? "";
        });
      } catch {
        checkbox.checked = true;
        say("These inputs need JSON mode. Your JSON draft has been kept.");
      }
    }
    $("#connection-json-wrap").hidden = !checkbox.checked;
    $("#connection-input-fields").hidden = checkbox.checked;
    invalidate();
  };

  function showItem() {
    invalidate();
    const { kind, item } = choose();
    $("#connection-arguments-wrap").hidden = kind !== "tools";
    $("#connection-schema").textContent = item
      ? JSON.stringify(
          kind === "tools" ? item.inputSchema : { uri: item.uri },
          null,
          2,
        )
      : "";
    $("#connection-description").textContent = item?.description || "";
    $("#connection-arguments").value = "{}";
    renderInputs(kind === "tools" ? item?.inputSchema : null);
    $("#connection-input-fields").hidden = false;
    controls();
  }
  async function list() {
    const current = epoch,
      request = ++listSerial;
    const data = await api("GET", "/api/connections");
    if (current !== epoch || request !== listSerial) return;
    $("#connections-runtime").textContent = data.location;
    $("#connections-list").innerHTML = data.connections.length
      ? data.connections
          .map(
            (c) =>
              `<button type="button" class="connection-row" data-connection="${esc(c.id)}" aria-pressed="${selected === c.id}"><span><strong>${esc(c.name)}</strong><small>${esc(c.url)}</small></span><span>${esc(c.status)}</span></button>`,
          )
          .join("")
      : '<p class="muted">No connections in this project. Connect a service to inspect its tools and resources.</p>';
    if (selected && !data.connections.some((c) => c.id === selected)) {
      const wasBusy = busy;
      selected = null;
      ++epoch;
      busy = false;
      clear();
      say(
        wasBusy
          ? "Connection disconnected. The in-flight outcome is unknown; local result and review were cleared."
          : "Connection disconnected. Local result and review were cleared.",
      );
    }
    return data;
  }
  async function load(id) {
    selected = id;
    const current = ++epoch;
    clear();
    busy = true;
    controls();
    say("Reading the server’s tool and resource catalog…");
    try {
      const data = await post({ op: "catalog", id });
      if (current !== epoch) return;
      catalog = data;
      $("#connection-detail").hidden = false;
      $("#connection-title").textContent = data.name;
      $("#connection-server").textContent = `${data.server} · ${data.url}`;
      $("#connection-item").innerHTML = ["tools", "resources"]
        .map(
          (kind) =>
            `<optgroup label="${kind === "tools" ? "Tools" : "Resources"}">${data[kind].map((item, i) => `<option value="${kind}:${i}">${esc(item.title || item.name)}</option>`).join("")}</optgroup>`,
        )
        .join("");
      showItem();
      say(
        data.tools.length || data.resources.length
          ? "Catalog loaded. Selecting an item does not execute it."
          : "Connected. This server exposes no tools or resources.",
      );
      await list();
    } catch (e) {
      if (current === epoch) say(e.message);
    } finally {
      if (current === epoch) {
        busy = false;
        controls();
      }
    }
  }
  $("#connection-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    if (busy) return;
    busy = true;
    controls();
    say("Connecting… no tools will run.");
    const input = {
      op: "connect",
      name: $("#connection-name").value.trim(),
      url: $("#connection-url").value.trim(),
      credential: $("#connection-credential").value,
    };
    $("#connection-credential").value = "";
    try {
      const c = await post(input);
      $("#connection-add").open = false;
      $("#connection-form").reset();
      busy = false;
      await load(c.id);
    } catch (e) {
      say(e.message);
    } finally {
      input.credential = "";
      busy = false;
      controls();
    }
  });
  $("#connections-list").addEventListener("click", (event) => {
    const row = event.target.closest("[data-connection]");
    if (row && !busy) void load(row.dataset.connection);
  });
  $("#connection-item").onchange = showItem;
  $("#connection-arguments").oninput = invalidate;
  $("#connection-refresh").onclick = () => {
    if (selected && !busy) void load(selected);
  };
  $("#connection-disconnect").onclick = async () => {
    const id = selected;
    if (!id) return;
    selected = null;
    ++epoch;
    busy = false;
    clear();
    say(
      "Disconnecting… pending reviews and visible results have been cleared.",
    );
    try {
      await post({ op: "disconnect", id });
      await list();
      say(
        "Disconnected. A call already received by the service may still finish there.",
      );
    } catch (e) {
      say(
        `Disconnect was not confirmed: ${e.message}. Check the connection before continuing.`,
      );
    }
  };
  $("#connection-review-call").onclick = async () => {
    if (busy) return;
    const { kind, item } = choose();
    if (!item) return;
    let argumentsValue = {};
    try {
      if (kind === "tools") argumentsValue = readInputs();
      validateConnectionNumbers(argumentsValue);
      if (
        kind === "tools" &&
        (!argumentsValue ||
          typeof argumentsValue !== "object" ||
          Array.isArray(argumentsValue))
      )
        throw new Error("Inputs must be a JSON object");
    } catch (error) {
      say(`${error.message}. Your input draft is kept.`);
      $("#connection-arguments").focus();
      return;
    }
    const current = epoch;
    busy = true;
    invalidate();
    controls();
    try {
      const review = await post({
        op: "review",
        id: selected,
        kind,
        name: item.name,
        uri: item.uri,
        arguments: argumentsValue,
      });
      if (current !== epoch) return;
      ticket = {
        id: review.review,
        source: `${catalog.name} · ${item.name || item.uri}`,
      };
      $("#connection-review-text").textContent = JSON.stringify(
        review.action,
        null,
        2,
      );
      $("#connection-review").hidden = false;
      say(
        "Review the destination and exact input. Approval runs this request once; server hints do not establish safety.",
      );
    } catch (e) {
      if (current === epoch) say(e.message);
    } finally {
      if (current === epoch) {
        busy = false;
        controls();
        if (ticket) $("#connection-run").focus();
      }
    }
  };
  $("#connection-run").onclick = async () => {
    if (busy || !ticket) return;
    const { id: review, source } = ticket,
      current = epoch;
    invalidate();
    busy = true;
    result = null;
    $("#connection-result-wrap").hidden = true;
    controls();
    say(
      "Request sent. Waiting for the service; it will not be retried automatically.",
    );
    try {
      const data = await post({ op: "execute", review });
      if (current !== epoch) return;
      say(
        data.outcome === "received"
          ? `Response received from ${catalog.name}. Review the result below.`
          : data.outcome === "not_sent"
            ? `Not sent: ${data.message}`
            : data.message,
      );
      if (data.result !== undefined) {
        result = { source, text: connectionResultText(data.result) };
        $("#connection-result").textContent = result.text;
        $("#connection-result-data").textContent = JSON.stringify(
          data.result,
          null,
          2,
        );
        if (data.result?.isError)
          say(
            "The service reported an error. Inspect its response; remote changes are not established.",
          );
        else if (data.result?.inputRequests)
          say(
            "The service needs additional input. This version does not answer those requests automatically.",
          );
        else if (data.result?.task)
          say(
            "The service created a remote task. Its completion has not been observed.",
          );
        $("#connection-result-wrap").hidden = false;
      }
    } catch (e) {
      if (current === epoch)
        say(
          `Outcome unknown: ${e.message}. Check the service before deciding whether to make a new request. Nothing was resent.`,
        );
    } finally {
      if (current === epoch) {
        busy = false;
        controls();
      }
    }
  };
  $("#connection-discard-review").onclick = () => {
    invalidate();
    say("Review discarded. Nothing was sent to the service.");
  };
  $("#connection-use").onclick = () => {
    if (result) useInStudio(result);
  };
  $("#connection-copy").onclick = async () => {
    if (!result) return;
    try {
      await navigator.clipboard.writeText(result.text);
      say("Result copied. It remains external, untrusted content.");
    } catch {
      say("Clipboard unavailable. Select and copy the result text.");
    }
  };
  return {
    onShowView(view) {
      clearInterval(timer);
      if (view !== "connections") return;
      void list().catch((e) => say(e.message));
      // Own connection metadata only: never poll remote tool/resource contents.
      timer = setInterval(() => {
        if (!document.hidden) void list().catch((e) => say(e.message));
      }, 5000);
    },
  };
}
