# tiny_http 0.12.0, vendored with one patch

Upstream: https://github.com/tiny-http/tiny-http, version 0.12.0 from crates.io, MIT OR Apache-2.0
(license files kept alongside). Tests, benches and examples are not vendored.

**The patch** is `src/util/equal_reader.rs`, `impl Drop for EqualReader`. Upstream drains every
unread request-body byte when a request is dropped, allocating `vec![0; remaining]` for the length
the client *declared*, on whichever thread dropped the request. In `verb web` that is the thread
that also pumps every hosted terminal, so an unauthenticated local request could:

* freeze the host: `Content-Length: 5000` and no body blocks until the client closes;
* abort it: `Content-Length: 1000000000000000` fails the allocation and kills the process, with no
  chance to close hosted sessions' records.

The patched drop leaves an unread body unread. That connection can no longer be reused (the next
request parsed on it fails and closes it), which costs nothing for a local API whose normal
requests are small JSON bodies that Verb reads in full. `desktop/tests/web_integration.rs` covers
both attacks.

To update: copy the new upstream release here, re-apply the patch, and keep this file current.

**Patch 2 (2026-10-04, Phase 1 real terminals)**: `Request::upgrade_tcp` and connection disarming in
`src/util/refined_tcp_stream.rs`, `src/client.rs`, and `src/request.rs`. Upstream `upgrade` only
yields a trait-erased `Box<dyn ReadWrite + Send>`, which prevents setting socket flags (such as
`TCP_NODELAY` and nonblocking mode) or wrapping the underlying stream with `tungstenite` for
WebSocket terminal streaming. `upgrade_tcp` provides the raw `TcpStream` while preserving HTTP
protocol upgrade semantics and disarming automatic connection shutdown upon request drop.
