// Owns the terminal WebSocket, framing and reconnect.
// The callbacks consume stream events; transport owns neither DOM nor durable session records.
export function createTerminalTransport({
  token,
  onOpen,
  onClose,
  onControl,
  onOutput,
}) {
  let streamSocket = null;
  let streamReady = false;
  let socketReconnectTimer = null;
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

  // Credentials never go in the WebSocket URL, where proxy and access logs keep them. The page asks
  // for a one-use ticket with its normal credentials (header or session cookie) and opens the stream
  // with that; a ticket is useless once used or after 30 seconds.
  async function streamTicket() {
    const headers = { "Content-Type": "application/json" };
    if (token) headers["X-Verb-Token"] = token;
    const response = await fetch("/api/terminals/ticket", {
      method: "POST",
      headers,
      body: "{}",
      credentials: "same-origin",
    });
    if (!response.ok) throw new Error(`terminal ticket refused (${response.status})`);
    const { ticket } = await response.json();
    if (!ticket) throw new Error("terminal ticket missing");
    return ticket;
  }

  async function connectTerminalStream() {
    if (
      ticketPending ||
      (streamSocket &&
        (streamSocket.readyState === WebSocket.OPEN ||
          streamSocket.readyState === WebSocket.CONNECTING))
    ) {
      return;
    }
    clearTimeout(socketReconnectTimer);
    ticketPending = true;
    let ticket;
    try {
      ticket = await streamTicket();
    } catch (e) {
      console.warn("Verb terminal stream ticket:", e);
      scheduleStreamReconnect();
      return;
    } finally {
      ticketPending = false;
    }
    const protocol = location.protocol === "https:" ? "wss:" : "ws:";
    const wsUrl = `${protocol}//${location.host}/api/terminals/ws?ticket=${encodeURIComponent(ticket)}`;
    try {
      streamSocket = new WebSocket(wsUrl);
      streamSocket.binaryType = "arraybuffer";
    } catch (e) {
      scheduleStreamReconnect();
      return;
    }

    streamSocket.onopen = () => {
      streamReady = true;
      console.info("Verb terminal stream connected over WebSocket");
      onOpen();
    };

    streamSocket.onmessage = (event) => {
      if (typeof event.data === "string") {
        try {
          const msg = JSON.parse(event.data);
          onControl(msg);
        } catch (e) {
          console.error("Malformed control frame", e);
        }
      } else if (event.data instanceof ArrayBuffer) {
        onOutput(event.data);
      }
    };

    streamSocket.onclose = (event) => {
      streamReady = false;
      streamSocket = null;
      console.warn("Verb terminal stream closed:", event?.code, event?.reason);
      onClose();
      scheduleStreamReconnect();
    };

    streamSocket.onerror = (e) => {
      streamReady = false;
      console.warn("Verb terminal stream error:", e);
    };
  }

  function scheduleStreamReconnect() {
    clearTimeout(socketReconnectTimer);
    socketReconnectTimer = setTimeout(connectTerminalStream, 2000);
  }

  document.addEventListener("visibilitychange", () => {
    if (
      document.visibilityState === "visible" &&
      (!streamSocket || streamSocket.readyState !== WebSocket.OPEN)
    ) {
      connectTerminalStream();
    }
  });

  function sendStreamAttach(id, rows, cols) {
    if (!streamSocket || streamSocket.readyState !== WebSocket.OPEN) return;
    streamSocket.send(JSON.stringify({ type: "attach", id, rows, cols }));
  }

  function sendStreamDetach(id) {
    if (!streamSocket || streamSocket.readyState !== WebSocket.OPEN) return;
    streamSocket.send(JSON.stringify({ type: "detach", id }));
  }

  function sendStreamResize(id, rows, cols) {
    if (!streamSocket || streamSocket.readyState !== WebSocket.OPEN) return;
    streamSocket.send(JSON.stringify({ type: "resize", id, rows, cols }));
  }

  function sendStreamAck(id, bytes) {
    if (!streamSocket || streamSocket.readyState !== WebSocket.OPEN) return;
    streamSocket.send(JSON.stringify({ type: "ack", id, bytes }));
  }

  function sendStreamSignal(id, signal) {
    if (!streamSocket || streamSocket.readyState !== WebSocket.OPEN) return;
    streamSocket.send(JSON.stringify({ type: "signal", id, signal }));
  }

  function sendStreamInput(id, dataStr) {
    if (!streamSocket || streamSocket.readyState !== WebSocket.OPEN) {
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
  };
}
