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

  function connectTerminalStream() {
    if (
      streamSocket &&
      (streamSocket.readyState === WebSocket.OPEN ||
        streamSocket.readyState === WebSocket.CONNECTING)
    ) {
      return;
    }
    clearTimeout(socketReconnectTimer);
    const protocol = location.protocol === "https:" ? "wss:" : "ws:";
    const wsUrl = `${protocol}//${location.host}/api/terminals/ws?token=${encodeURIComponent(token || "")}`;
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
