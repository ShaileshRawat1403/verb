// Owns the terminal WebSocket, framing and reconnect.
// The callbacks consume stream events; transport owns neither DOM nor durable session records.
// Close code the host sends when the stream's sign-in has ended (see SIGNED_OUT in stream.rs).
export const SIGNED_OUT_CODE = 4401;
// Back-off between reconnects after any other close, in seconds, with jitter. It starts again from
// the beginning once a stream has stayed open for a minute.
export const RECONNECT_DELAYS = [1, 2, 4, 8, 16, 30];
export const STABLE_MS = 60_000;

// Delay before reconnect attempt `attempt` (0-based): the step, plus up to 20% jitter.
export function reconnectDelay(attempt, random = Math.random) {
  const step = RECONNECT_DELAYS[Math.min(attempt, RECONNECT_DELAYS.length - 1)] * 1000;
  return Math.round(step + step * 0.2 * random());
}

export function createTerminalTransport({
  token,
  onOpen,
  onClose,
  onControl,
  onOutput,
  onSignedOut = () => {},
  // Injected by tests; the page uses the browser's own.
  WebSocketImpl = globalThis.WebSocket,
  fetchImpl = (...args) => globalThis.fetch(...args),
  setTimer = (fn, ms) => setTimeout(fn, ms),
  clearTimer = (id) => clearTimeout(id),
  now = () => Date.now(),
  random = Math.random,
  log = console,
  loc = globalThis.location,
  doc = globalThis.document,
}) {
  let streamSocket = null;
  let streamReady = false;
  let socketReconnectTimer = null;
  let attempt = 0;
  let openedAt = 0;
  let signedOut = false;
  let lastClose = null;
  const textEncoder = new TextEncoder();
  const idBytesMap = new Map();

  function getIdBytes(id) {
    let bytes = idBytesMap.get(id);
    if (!bytes) {
      bytes = textEncoder.encode(id);
      idBytesMap.set(id, bytes);
    }
    return bytes;
  }

  let ticketPending = false;

  // The first close is logged, then only a change, never every retry.
  function noteClose(what) {
    if (what === lastClose) return;
    lastClose = what;
    log.warn("Verb terminal stream closed:", what);
  }

  // The sign-in has ended: reconnecting cannot help, so stop and ask for a reload once.
  function endSignedOut() {
    if (signedOut) return;
    signedOut = true;
    clearTimer(socketReconnectTimer);
    noteClose(`${SIGNED_OUT_CODE} sign-in expired`);
    onSignedOut();
  }

  // Credentials never go in the WebSocket URL, where proxy and access logs keep them. The page asks
  // for a one-use ticket with its normal credentials (header or session cookie) and opens the stream
  // with that; a ticket is useless once used or after 30 seconds.
  async function streamTicket() {
    const headers = { "Content-Type": "application/json" };
    if (token) headers["X-Verb-Token"] = token;
    const response = await fetchImpl("/api/terminals/ticket", {
      method: "POST",
      headers,
      body: "{}",
      credentials: "same-origin",
    });
    if (response.status === 401 || response.status === 403) {
      throw Object.assign(new Error(`terminal ticket refused (${response.status})`), { signedOut: true });
    }
    if (!response.ok) throw new Error(`terminal ticket refused (${response.status})`);
    const { ticket } = await response.json();
    if (!ticket) throw new Error("terminal ticket missing");
    return ticket;
  }

  async function connectTerminalStream() {
    if (
      signedOut ||
      ticketPending ||
      (streamSocket &&
        (streamSocket.readyState === WebSocketImpl.OPEN ||
          streamSocket.readyState === WebSocketImpl.CONNECTING))
    ) {
      return;
    }
    clearTimer(socketReconnectTimer);
    ticketPending = true;
    let ticket;
    try {
      ticket = await streamTicket();
    } catch (e) {
      if (e.signedOut) {
        endSignedOut();
        return;
      }
      noteClose(`ticket: ${e.message}`);
      scheduleStreamReconnect();
      return;
    } finally {
      ticketPending = false;
    }
    const protocol = loc.protocol === "https:" ? "wss:" : "ws:";
    const wsUrl = `${protocol}//${loc.host}/api/terminals/ws?ticket=${encodeURIComponent(ticket)}`;
    let socket;
    try {
      socket = new WebSocketImpl(wsUrl);
      socket.binaryType = "arraybuffer";
    } catch (e) {
      scheduleStreamReconnect();
      return;
    }
    streamSocket = socket;

    socket.onopen = () => {
      streamReady = true;
      openedAt = now();
      if (lastClose === null) log.info("Verb terminal stream connected over WebSocket");
      onOpen();
    };

    socket.onmessage = (event) => {
      if (typeof event.data === "string") {
        try {
          const msg = JSON.parse(event.data);
          onControl(msg);
        } catch (e) {
          log.error("Malformed control frame", e);
        }
      } else if (event.data instanceof ArrayBuffer) {
        onOutput(event.data);
      }
    };

    socket.onclose = (event) => {
      if (streamSocket === socket) streamSocket = null;
      const wasOpen = streamReady;
      streamReady = false;
      onClose();
      if (event?.code === SIGNED_OUT_CODE) {
        endSignedOut();
        return;
      }
      // A stream that stayed up for a minute was healthy: start the back-off again.
      if (wasOpen && now() - openedAt >= STABLE_MS) {
        attempt = 0;
        lastClose = null;
      }
      noteClose(`${event?.code ?? ""} ${event?.reason ?? ""}`.trim());
      scheduleStreamReconnect();
    };

    socket.onerror = () => {
      streamReady = false;
    };
  }

  function scheduleStreamReconnect() {
    if (signedOut) return;
    clearTimer(socketReconnectTimer);
    const delay = reconnectDelay(attempt, random);
    attempt++;
    socketReconnectTimer = setTimer(connectTerminalStream, delay);
  }

  doc?.addEventListener("visibilitychange", () => {
    if (
      doc.visibilityState === "visible" &&
      (!streamSocket || streamSocket.readyState !== WebSocketImpl.OPEN)
    ) {
      connectTerminalStream();
    }
  });

  function sendStreamAttach(id, rows, cols) {
    if (!streamSocket || streamSocket.readyState !== WebSocketImpl.OPEN) return;
    streamSocket.send(JSON.stringify({ type: "attach", id, rows, cols }));
  }

  function sendStreamDetach(id) {
    if (!streamSocket || streamSocket.readyState !== WebSocketImpl.OPEN) return;
    streamSocket.send(JSON.stringify({ type: "detach", id }));
  }

  function sendStreamResize(id, rows, cols) {
    if (!streamSocket || streamSocket.readyState !== WebSocketImpl.OPEN) return;
    streamSocket.send(JSON.stringify({ type: "resize", id, rows, cols }));
  }

  function sendStreamAck(id, bytes) {
    if (!streamSocket || streamSocket.readyState !== WebSocketImpl.OPEN) return;
    streamSocket.send(JSON.stringify({ type: "ack", id, bytes }));
  }

  function sendStreamSignal(id, signal) {
    if (!streamSocket || streamSocket.readyState !== WebSocketImpl.OPEN) return;
    streamSocket.send(JSON.stringify({ type: "signal", id, signal }));
  }

  function sendStreamInput(id, dataStr) {
    if (!streamSocket || streamSocket.readyState !== WebSocketImpl.OPEN) {
      return false;
    }
    const idBytes = getIdBytes(id);
    const dataBytes = textEncoder.encode(dataStr);
    const frame = new Uint8Array(1 + idBytes.length + dataBytes.length);
    frame[0] = idBytes.length;
    frame.set(idBytes, 1);
    frame.set(dataBytes, 1 + idBytes.length);
    streamSocket.send(frame.buffer);
    return true;
  }

  return {
    connectTerminalStream,
    sendStreamAttach,
    sendStreamDetach,
    sendStreamResize,
    sendStreamAck,
    sendStreamSignal,
    sendStreamInput,
    get ready() {
      return streamReady;
    },
    get signedOut() {
      return signedOut;
    },
  };
}
