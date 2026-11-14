//! Bounded HTTP adapter for the official MCP transport. No redirects, unsolicited subscriptions or error-body logging.
use futures::{stream::BoxStream, StreamExt};
use reqwest::{
    header::{HeaderName, HeaderValue},
    Client, RequestBuilder,
};
use rmcp::{
    model::{ClientJsonRpcMessage, ClientRequest, ErrorData, ServerJsonRpcMessage},
    transport::streamable_http_client::{
        SseError, StreamableHttpClient, StreamableHttpError, StreamableHttpPostResponse,
    },
};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
pub(crate) const MAX_RESPONSE: usize = 256 * 1024;
#[derive(Clone)]
pub(crate) struct BoundedHttp {
    pub client: Client,
    pub endpoint: Arc<str>,
    pub auth_required: Arc<AtomicBool>,
}
#[derive(Debug)]
pub(crate) struct HttpError(pub &'static str);
impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for HttpError {}
type Error = StreamableHttpError<HttpError>;
fn fail(s: &'static str) -> Error {
    StreamableHttpError::Client(HttpError(s))
}
impl BoundedHttp {
    fn request(
        &self,
        uri: &str,
        method: reqwest::Method,
        session: Option<Arc<str>>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<RequestBuilder, Error> {
        if uri != self.endpoint.as_ref() || headers.len() > 32 {
            return Err(fail("MCP destination or header limit changed"));
        }
        let mut request = self.client.request(method, uri);
        for (name, value) in headers {
            if matches!(
                name.as_str(),
                "authorization"
                    | "proxy-authorization"
                    | "cookie"
                    | "host"
                    | "origin"
                    | "content-length"
                    | "transfer-encoding"
                    | "connection"
                    | "accept"
                    | "content-type"
            ) || value.as_bytes().len() > 4096
            {
                return Err(fail("Reserved or oversized connector header"));
            }
            request = request.header(name, value);
        }
        if let Some(session) = session {
            if session.len() > 2048 {
                return Err(fail("MCP session identifier too large"));
            }
            request = request.header("mcp-session-id", session.as_ref());
        }
        if let Some(auth) = auth {
            request = request.bearer_auth(auth);
        }
        Ok(request)
    }
    async fn post(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session: Option<Arc<str>>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<StreamableHttpPostResponse, Error> {
        let had_session = session.is_some();
        let mut response = self
            .request(&uri, reqwest::Method::POST, session, auth, headers)?
            .header("accept", "application/json, text/event-stream")
            .json(&message)
            .send()
            .await
            .map_err(|_| fail("MCP network request failed"))?;
        let status = response.status();
        if matches!(status.as_u16(), 401 | 403) {
            self.auth_required.store(true, Ordering::SeqCst);
            return Err(fail("Service authentication required"));
        }
        if status.as_u16() == 404 && had_session {
            return Err(StreamableHttpError::SessionExpired);
        }
        if matches!(status.as_u16(), 202 | 204) {
            return Ok(StreamableHttpPostResponse::Accepted);
        }
        if status.is_redirection() {
            return Err(fail("MCP redirects are refused"));
        }
        if response
            .content_length()
            .is_some_and(|n| n > MAX_RESPONSE as u64)
        {
            return Err(fail("MCP response exceeds its byte limit"));
        }
        let session = response
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        if session.as_ref().is_some_and(|s| s.len() > 2048) {
            return Err(fail("MCP session identifier too large"));
        }
        // Delegate version fallback to the SDK lifecycle. Some older middleware replies
        // to server/discover with a plain 4xx or an uncorrelated JSON-RPC error.
        // This applies only to sessionless discovery, never tool calls or authentication.
        if status.is_client_error() && !had_session {
            if let ClientJsonRpcMessage::Request(request) = &message {
                if matches!(request.request, ClientRequest::DiscoverRequest(_)) {
                    let mut body = Vec::new();
                    while let Some(chunk) = response
                        .chunk()
                        .await
                        .map_err(|_| fail("Discovery response could not be read"))?
                    {
                        if body.len() + chunk.len() > MAX_RESPONSE {
                            return Err(fail("Discovery response exceeds its byte limit"));
                        }
                        body.extend_from_slice(&chunk);
                    }
                    let error = match serde_json::from_slice::<ServerJsonRpcMessage>(&body) {
                        Ok(ServerJsonRpcMessage::Error(error)) => error.error,
                        _ => ErrorData::invalid_request(
                            "Server discovery was refused by legacy middleware",
                            None,
                        ),
                    };
                    return Ok(StreamableHttpPostResponse::Json(
                        ServerJsonRpcMessage::error(error, Some(request.id.clone())),
                        None,
                    ));
                }
            }
        }
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        if status.is_success() && content_type.starts_with("text/event-stream") {
            // The total raw-byte limit also bounds an unterminated SSE event before parsing.
            let limited = futures::stream::try_unfold(
                (response, 0usize),
                |(mut response, total)| async move {
                    match response
                        .chunk()
                        .await
                        .map_err(|_| std::io::Error::other("MCP stream failed"))?
                    {
                        None => Ok(None),
                        Some(chunk) if total + chunk.len() <= MAX_RESPONSE => {
                            let size = total + chunk.len();
                            Ok(Some((chunk, (response, size))))
                        }
                        Some(_) => Err(std::io::Error::other("MCP stream exceeds its byte limit")),
                    }
                },
            );
            return Ok(StreamableHttpPostResponse::Sse(
                sse_stream::SseStream::from_bytes_stream(limited).boxed(),
                session,
            ));
        }
        if !content_type.starts_with("application/json") {
            return Err(fail("MCP server did not return JSON or SSE"));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| fail("MCP response could not be read"))?
        {
            if body.len() + chunk.len() > MAX_RESPONSE {
                return Err(fail("MCP response exceeds its byte limit"));
            }
            body.extend_from_slice(&chunk);
        }
        let parsed: ServerJsonRpcMessage = serde_json::from_slice(&body)
            .map_err(|_| fail("MCP response was not valid protocol JSON"))?;
        if !status.is_success() && !matches!(parsed, ServerJsonRpcMessage::Error(_)) {
            return Err(fail("MCP server refused the request"));
        }
        Ok(StreamableHttpPostResponse::Json(parsed, session))
    }
}
impl StreamableHttpClient for BoundedHttp {
    type Error = HttpError;
    async fn post_message(
        &self,
        uri: Arc<str>,
        message: ClientJsonRpcMessage,
        session: Option<Arc<str>>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<StreamableHttpPostResponse, Error> {
        self.post(uri, message, session, auth, headers).await
    }
    async fn delete_session(
        &self,
        uri: Arc<str>,
        session: Arc<str>,
        auth: Option<String>,
        headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<(), Error> {
        let response = self
            .request(&uri, reqwest::Method::DELETE, Some(session), auth, headers)?
            .send()
            .await
            .map_err(|_| fail("MCP disconnect could not be confirmed"))?;
        if response.status().is_success() || response.status().as_u16() == 405 {
            Ok(())
        } else {
            Err(fail("MCP disconnect was refused"))
        }
    }
    async fn get_stream(
        &self,
        _uri: Arc<str>,
        _session: Option<Arc<str>>,
        _last: Option<String>,
        _auth: Option<String>,
        _headers: HashMap<HeaderName, HeaderValue>,
    ) -> Result<BoxStream<'static, Result<sse_stream::Sse, SseError>>, Error> {
        Err(StreamableHttpError::ServerDoesNotSupportSse)
    }
}
