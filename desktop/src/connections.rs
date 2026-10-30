//! Owner-controlled, transient project MCP connections. No credentials or tool content are persisted.
use crate::mcp_http::{BoundedHttp, MAX_RESPONSE};
use rmcp::{
    model::{CallToolRequestParams, ReadResourceRequestParams},
    service::RunningService,
    transport::{
        common::client_side_sse::NeverRetry,
        streamable_http_client::StreamableHttpClientTransportConfig, StreamableHttpClientTransport,
    },
    RoleClient, ServiceExt,
};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
use tokio::runtime::Runtime;
const DEADLINE: Duration = Duration::from_secs(25);
const REVIEW_TTL: Duration = Duration::from_secs(90);
static DNS_ACTIVE: AtomicUsize = AtomicUsize::new(0);
#[derive(Default, Clone)]
pub(crate) struct Connections(Arc<Inner>);
#[derive(Default)]
struct Inner {
    state: Mutex<State>,
    runtime: OnceLock<Result<Runtime, String>>,
}
#[derive(Default)]
struct State {
    connections: HashMap<String, Arc<Connection>>,
    pending: HashSet<String>,
    reviews: HashMap<String, Review>,
}
struct Connection {
    name: String,
    url: String,
    credential: String,
    service: RunningService<RoleClient, rmcp::model::ClientConfig>,
    auth_required: Arc<AtomicBool>,
    revoked: AtomicBool,
    busy: AtomicBool,
    catalog: Mutex<Catalog>,
}
#[derive(Default)]
struct Catalog {
    generation: u64,
    tools: Vec<Value>,
    resources: Vec<Value>,
    server: String,
}
struct Review {
    id: String,
    generation: u64,
    expires: Instant,
    action: Value,
}
struct Busy(Arc<Connection>);
impl Drop for Busy {
    fn drop(&mut self) {
        self.0.busy.store(false, Ordering::SeqCst);
    }
}
struct Pending {
    inner: Arc<Inner>,
    id: String,
}
impl Drop for Pending {
    fn drop(&mut self) {
        if let Ok(mut s) = self.inner.state.lock() {
            s.pending.remove(&self.id);
        }
    }
}
fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            let blocked = matches!(a, 0 | 10 | 127)
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192 && (b == 168 || b == 0 || (b == 88 && c == 99)))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113)
                || a >= 224;
            !blocked
        }
        IpAddr::V6(ip) => {
            let s = ip.segments();
            (0x2000..=0x3fff).contains(&s[0])
                && s[0] != 0x2002
                && s[0] != 0x3fff
                && !(s[0] == 0x2001 && (s[1] == 0x0db8 || s[1] < 0x0200))
        }
    }
}
fn endpoint(raw: &str, origin: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(raw).map_err(|_| "Enter a valid HTTPS MCP endpoint.")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || raw.len() > 2048
    {
        return Err(
            "Use an HTTPS endpoint without credentials, query parameters or fragments.".into(),
        );
    }
    if let Ok(own) = reqwest::Url::parse(origin) {
        if url.host_str().map(|h| h.trim_end_matches('.'))
            == own.host_str().map(|h| h.trim_end_matches('.'))
        {
            return Err("Verb cannot connect to its own host as an MCP service.".into());
        }
    }
    let host = url
        .host_str()
        .unwrap_or("")
        .trim_matches(['[', ']'])
        .trim_end_matches('.');
    if host.eq_ignore_ascii_case("localhost")
        || host.ends_with(".localhost")
        || host.ends_with(".local")
        || host.parse::<IpAddr>().is_ok_and(|ip| !public_ip(ip))
    {
        return Err("Local, private and metadata endpoints are not supported.".into());
    }
    Ok(url)
}
// Matches the existing synthetic-boundary contract: this code cannot compile into a release build.
#[cfg(feature = "synthetic-boundary")]
fn synthetic_endpoint(raw: &str) -> bool {
    std::env::var("VERB_SYNTHETIC_MCP_ENDPOINT").is_ok_and(|allowed| allowed == raw)
        && reqwest::Url::parse(raw).is_ok_and(|u| {
            u.scheme() == "http"
                && u.host_str() == Some("127.0.0.1")
                && u.port().is_some()
                && u.path() == "/mcp"
                && u.username().is_empty()
                && u.password().is_none()
                && u.query().is_none()
                && u.fragment().is_none()
        })
}
#[cfg(not(feature = "synthetic-boundary"))]
fn synthetic_endpoint(_: &str) -> bool {
    false
}
fn resolve(url: &reqwest::Url) -> Result<Vec<SocketAddr>, String> {
    let mut active = DNS_ACTIVE.load(Ordering::Relaxed);
    loop {
        if active >= 4 {
            return Err(
                "DNS capacity is busy. Try again after the pending lookup finishes.".into(),
            );
        }
        match DNS_ACTIVE.compare_exchange_weak(
            active,
            active + 1,
            Ordering::SeqCst,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(current) => active = current,
        }
    }
    let host = url
        .host_str()
        .unwrap_or("")
        .trim_matches(['[', ']'])
        .to_owned();
    let port = url.port_or_known_default().unwrap_or(443);
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    if std::thread::Builder::new()
        .name("mcp-dns".into())
        .spawn(move || {
            let result = (host.as_str(), port)
                .to_socket_addrs()
                .map(|a| a.take(17).collect::<Vec<_>>());
            let _ = send.send(result);
            DNS_ACTIVE.fetch_sub(1, Ordering::SeqCst);
        })
        .is_err()
    {
        DNS_ACTIVE.fetch_sub(1, Ordering::SeqCst);
        return Err("DNS worker unavailable.".into());
    }
    let addresses = receive
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| "Service DNS lookup timed out.")?
        .map_err(|_| "Service DNS lookup failed.")?;
    if addresses.is_empty() || addresses.len() > 16 || addresses.iter().any(|a| !public_ip(a.ip()))
    {
        return Err("Service DNS must resolve only to public addresses.".into());
    }
    Ok(addresses)
}
fn clean(s: &str, credential: &str) -> String {
    let s = crate::transcript::redact(s);
    if credential.is_empty() {
        s
    } else {
        s.replace(credential, "[redacted service credential]")
    }
}
fn scrub(v: &mut Value, credential: &str) {
    match v {
        Value::String(s) => *s = clean(s, credential),
        Value::Array(a) => a.iter_mut().for_each(|v| scrub(v, credential)),
        Value::Object(o) => {
            let old = std::mem::take(o);
            for (k, mut v) in old {
                scrub(&mut v, credential);
                o.insert(clean(&k, credential), v);
            }
        }
        _ => {}
    }
}
async fn catalog_pages(c: &Connection, resources: bool) -> Result<Value, &'static str> {
    let mut cursor = None;
    let mut cursors = HashSet::new();
    let mut identities = HashSet::new();
    let mut items = Vec::new();
    for _ in 0..4 {
        let params = cursor
            .map(|cursor| rmcp::model::PaginatedRequestParams::default().with_cursor(Some(cursor)));
        let (page, next) = if resources {
            let page = c
                .service
                .list_resources(params)
                .await
                .map_err(|_| "Could not read resources. Check authentication and refresh.")?;
            (
                serde_json::to_value(page.resources).map_err(|_| "Invalid resource catalog.")?,
                page.next_cursor,
            )
        } else {
            let page = c
                .service
                .list_tools(params)
                .await
                .map_err(|_| "Could not read tools. Check authentication and refresh.")?;
            (
                serde_json::to_value(page.tools).map_err(|_| "Invalid tool catalog.")?,
                page.next_cursor,
            )
        };
        for item in page.as_array().ok_or("Invalid catalog page.")? {
            let key = item[if resources { "uri" } else { "name" }]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or("Empty catalog identifier.")?;
            if !identities.insert(key.to_owned()) {
                return Err(
                    "Catalog repeats an identifier. Restrict the service toolset or refresh.",
                );
            }
            items.push(item.clone());
            if items.len() > 200 {
                return Err("Catalog exceeds 200 items. Restrict the service toolset.");
            }
        }
        if next.is_none() {
            return Ok(json!(items));
        }
        if !cursors.insert(next.clone()) {
            return Err("Catalog pagination repeated a cursor.");
        }
        cursor = next;
    }
    Err("Catalog exceeds four pages. Restrict the service toolset.")
}

impl Connections {
    fn state(&self) -> Result<std::sync::MutexGuard<'_, State>, String> {
        self.0
            .state
            .lock()
            .map_err(|_| "Connections state unavailable.".into())
    }
    fn runtime(&self) -> Result<&Runtime, String> {
        self.0
            .runtime
            .get_or_init(|| {
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                    .map_err(|_| "MCP runtime unavailable.".into())
            })
            .as_ref()
            .map_err(Clone::clone)
    }
    fn connection(&self, id: &str) -> Result<Arc<Connection>, String> {
        self.state()?
            .connections
            .get(id)
            .cloned()
            .ok_or_else(|| "Connection changed or disconnected. Inspect it again.".into())
    }
    fn acquire(c: Arc<Connection>) -> Result<Busy, String> {
        if c.revoked.load(Ordering::SeqCst)
            || c.service.is_transport_closed()
            || c.auth_required.load(Ordering::SeqCst)
            || c.busy
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
        {
            return Err("Connection unavailable or busy. Nothing was queued.".into());
        }
        Ok(Busy(c))
    }
    pub(crate) fn status(&self) -> Result<Value, String> {
        let s = self.state()?;
        let mut connections = Vec::with_capacity(s.connections.len());
        for (id, c) in &s.connections {
            let status = if c.auth_required.load(Ordering::SeqCst) {
                "Sign-in required"
            } else if c.service.is_closed() || c.service.is_transport_closed() {
                "Unavailable"
            } else {
                "Connected"
            };
            connections.push(json!({"id":id,"name":c.name,"url":c.url,"status":status}));
        }
        connections.sort_by_key(|v| v["name"].as_str().unwrap_or("").to_owned());
        Ok(
            json!({"connections":connections,"pendingConnections":s.pending.len(),"activeRequests":s.connections.values().filter(|c| c.busy.load(Ordering::SeqCst)).count(),"pendingReviews":s.reviews.values().filter(|r|r.expires>Instant::now()).count(),"location":"Remote MCP services · credentials stay in memory on this Verb host · owner access only"}),
        )
    }
    pub(crate) fn command(&self, input: Value, origin: &str) -> Result<Value, String> {
        match input["op"].as_str().unwrap_or("") {
            "connect" => self.connect(input, origin),
            "disconnect" => {
                let id = input["id"].as_str().ok_or("Choose a connection.")?;
                let mut s = self.state()?;
                let c = s
                    .connections
                    .remove(id)
                    .ok_or("Connection already disconnected.")?;
                c.revoked.store(true, Ordering::SeqCst);
                s.reviews.retain(|_, r| r.id != id);
                c.service.cancellation_token().cancel();
                Ok(json!({"disconnected":true}))
            }
            "catalog" => self.catalog(input["id"].as_str().ok_or("Choose a connection.")?),
            "review" => self.review(input),
            "execute" => self
                .execute(
                    input["review"]
                        .as_str()
                        .ok_or("Review this request before running it.")?,
                )
                .or_else(|message| Ok(json!({"outcome":"not_sent","message":message}))),
            _ => Err("Unknown connection operation.".into()),
        }
    }
    fn connect(&self, input: Value, origin: &str) -> Result<Value, String> {
        let name = input["name"].as_str().unwrap_or("").trim().to_owned();
        if name.is_empty() || name.len() > 120 || name.chars().any(char::is_control) {
            return Err("Give the connection a short name.".into());
        }
        let raw = input["url"].as_str().unwrap_or("");
        let fixture = synthetic_endpoint(raw);
        let url = if fixture {
            reqwest::Url::parse(raw).map_err(|_| "Invalid fixture endpoint.")?
        } else {
            endpoint(raw, origin)?
        };
        let credential = input["credential"].as_str().unwrap_or("").to_owned();
        if credential.len() > 4096
            || credential
                .chars()
                .any(|c| c.is_control() || c.is_whitespace())
            || credential.starts_with("verb_at_")
        {
            return Err(
                "Use only this service’s access token, without spaces or line breaks.".into(),
            );
        }
        let id = crate::web::secure_token()?;
        {
            let mut s = self.state()?;
            if s.connections.len() + s.pending.len() >= 4 {
                return Err(
                    "Disconnect an existing service before adding another (maximum four).".into(),
                );
            }
            s.pending.insert(id.clone());
        }
        let _pending = Pending {
            inner: self.0.clone(),
            id: id.clone(),
        };
        let addresses = if fixture {
            vec![SocketAddr::from((
                [127, 0, 0, 1],
                url.port().ok_or("Missing fixture port.")?,
            ))]
        } else {
            resolve(&url)?
        };
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = reqwest::Client::builder()
            .https_only(!fixture)
            .retry(reqwest::retry::never())
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(DEADLINE)
            .resolve_to_addrs(url.host_str().unwrap_or(""), &addresses)
            .build()
            .map_err(|_| "MCP HTTPS client unavailable.")?;
        self.connect_client(id, name, url.to_string(), credential, client)
    }
    fn connect_client(
        &self,
        id: String,
        name: String,
        url: String,
        credential: String,
        client: reqwest::Client,
    ) -> Result<Value, String> {
        let auth_required = Arc::new(AtomicBool::new(false));
        let http = BoundedHttp {
            client,
            endpoint: url.clone().into(),
            auth_required: auth_required.clone(),
        };
        let mut config = StreamableHttpClientTransportConfig::with_uri(url.clone())
            .max_concurrent_requests(1)
            .max_sse_event_size(MAX_RESPONSE)
            .reinit_on_expired_session(false);
        config.retry_config = Arc::new(NeverRetry::default());
        if !credential.is_empty() {
            config = config.auth_header(credential.clone());
        }
        let service=self.runtime()?.block_on(async { let transport = StreamableHttpClientTransport::with_client(http, config); tokio::time::timeout(DEADLINE, rmcp::model::ClientConfig::new(Default::default(), rmcp::model::Implementation::new("verb", env!("CARGO_PKG_VERSION"))).serve(transport)).await }).map_err(|_|"MCP connection timed out; no tools were run.")?.map_err(|_|if auth_required.load(Ordering::SeqCst){"Service authentication is required. OAuth sign-in is not supported yet; use a service-specific token if offered."}else{"MCP handshake failed. Check the endpoint, protocol support and service availability."})?;
        let c = Connection {
            name,
            url,
            credential,
            service,
            auth_required,
            revoked: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            catalog: Mutex::default(),
        };
        self.state()?.connections.insert(id.clone(), Arc::new(c));
        Ok(json!({"id":id}))
    }
    fn catalog(&self, id: &str) -> Result<Value, String> {
        let c = self.connection(id)?;
        let _busy = Self::acquire(c.clone())?;
        {
            let mut cat = c.catalog.lock().map_err(|_| "Catalog unavailable.")?;
            cat.generation = cat.generation.wrapping_add(1);
            cat.tools.clear();
            cat.resources.clear();
        }
        self.state()?.reviews.retain(|_, r| r.id != id);
        let (tools, resources) = self
            .runtime()?
            .block_on(async {
                tokio::time::timeout(DEADLINE, async {
                    let info = c
                        .service
                        .peer_info()
                        .ok_or("MCP server identity unavailable.")?;
                    let tools = if info.capabilities.tools.is_some() {
                        catalog_pages(&c, false).await?
                    } else {
                        json!([])
                    };
                    let resources = if info.capabilities.resources.is_some() {
                        catalog_pages(&c, true).await?
                    } else {
                        json!([])
                    };
                    Ok::<_, &str>((tools, resources))
                })
                .await
            })
            .map_err(|_| "Catalog request timed out.")??;
        if c.revoked.load(Ordering::SeqCst) {
            return Err("Connection was disconnected; content discarded.".into());
        }
        let mut tools = tools.as_array().cloned().ok_or("Invalid tools.")?;
        let mut resources = resources.as_array().cloned().ok_or("Invalid resources.")?;
        if serde_json::to_vec(&(&tools, &resources))
            .map_err(|_| "Invalid catalog.")?
            .len()
            > 512 * 1024
        {
            return Err("Combined catalog exceeds 512 KB. Restrict the service toolset.".into());
        }
        for item in tools.iter_mut().chain(resources.iter_mut()) {
            for field in ["name", "uri"] {
                if let Some(v) = item.get(field).and_then(Value::as_str) {
                    if clean(v, &c.credential) != v {
                        return Err(
                            "Catalog identifiers contain sensitive data; connection refused."
                                .into(),
                        );
                    }
                }
            }
            scrub(item, &c.credential);
        }
        let server = c
            .service
            .peer_info()
            .and_then(|p| p.server_info.as_ref().map(|i| i.name.clone()))
            .unwrap_or_else(|| "MCP service".into());
        let mut cat = c.catalog.lock().map_err(|_| "Catalog unavailable.")?;
        cat.tools = tools;
        cat.resources = resources;
        cat.server = clean(&server, &c.credential);
        Ok(
            json!({"id":id,"name":c.name,"url":c.url,"server":cat.server,"tools":cat.tools,"resources":cat.resources}),
        )
    }
    fn review(&self, input: Value) -> Result<Value, String> {
        let id = input["id"].as_str().ok_or("Choose a connection.")?;
        let c = self.connection(id)?;
        if c.auth_required.load(Ordering::SeqCst) {
            return Err("Service sign-in required. Disconnect and connect again with a service-specific token.".into());
        }
        if c.revoked.load(Ordering::SeqCst)
            || c.busy.load(Ordering::SeqCst)
            || c.service.is_closed()
        {
            return Err(
                "Connection busy or unavailable. Inspect it before reviewing a request.".into(),
            );
        }
        let cat = c.catalog.lock().map_err(|_| "Catalog unavailable.")?;
        let action = match input["kind"].as_str() {
            Some("tools") => {
                let name = input["name"].as_str().ok_or("Choose a tool.")?;
                if !cat.tools.iter().any(|t| t["name"] == name) {
                    return Err("Tool is not in the inspected catalog.".into());
                }
                if !input["arguments"].is_object() {
                    return Err("Tool arguments must be a JSON object.".into());
                }
                json!({"destination":c.url,"kind":"tools","name":name,"arguments":input["arguments"]})
            }
            Some("resources") => {
                let uri = input["uri"].as_str().ok_or("Choose a resource.")?;
                if !cat.resources.iter().any(|r| r["uri"] == uri) {
                    return Err("Resource is not in the inspected catalog.".into());
                }
                json!({"destination":c.url,"kind":"resources","uri":uri})
            }
            _ => return Err("Choose a catalog tool or resource.".into()),
        };
        let review = crate::web::secure_token()?;
        let mut s = self.state()?;
        if c.revoked.load(Ordering::SeqCst) || !s.connections.contains_key(id) {
            return Err("Connection was disconnected.".into());
        }
        s.reviews.retain(|_, r| r.expires > Instant::now());
        if s.reviews.len() >= 16 {
            return Err("Too many pending reviews. Wait for expiry or disconnect.".into());
        }
        s.reviews.insert(
            review.clone(),
            Review {
                id: id.into(),
                generation: cat.generation,
                expires: Instant::now() + REVIEW_TTL,
                action: action.clone(),
            },
        );
        Ok(json!({"review":review,"action":action,"expiresInSeconds":90}))
    }
    fn execute(&self, ticket: &str) -> Result<Value, String> {
        let r = self
            .state()?
            .reviews
            .remove(ticket)
            .ok_or("Review expired, changed or already used. Inspect and review again.")?;
        if r.expires <= Instant::now() {
            return Err("Review expired. Review the request again.".into());
        }
        let c = self.connection(&r.id)?;
        let _busy = Self::acquire(c.clone())?;
        if c.catalog
            .lock()
            .map_err(|_| "Catalog unavailable.")?
            .generation
            != r.generation
        {
            return Err("Catalog changed. Inspect and review again.".into());
        }
        let outcome = self.runtime()?.block_on(async {
            tokio::time::timeout(DEADLINE, async {
                if r.action["kind"] == "tools" {
                    c.service
                        .call_tool_once(
                            CallToolRequestParams::new(
                                r.action["name"].as_str().unwrap_or("").to_owned(),
                            )
                            .with_arguments(
                                r.action["arguments"]
                                    .as_object()
                                    .cloned()
                                    .unwrap_or_default(),
                            ),
                        )
                        .await
                        .map(|v| {
                            serde_json::to_value(rmcp::model::ServerResult::from(v))
                                .unwrap_or(Value::Null)
                        })
                } else {
                    c.service
                        .read_resource_once(ReadResourceRequestParams::new(
                            r.action["uri"].as_str().unwrap_or(""),
                        ))
                        .await
                        .map(|v| {
                            serde_json::to_value(rmcp::model::ServerResult::from(v))
                                .unwrap_or(Value::Null)
                        })
                }
            })
            .await
        });
        if c.revoked.load(Ordering::SeqCst) {
            return Ok(
                json!({"outcome":"unknown","message":"Disconnected while a request was in flight. Its outcome is unknown; content was discarded. Check the service before retrying."}),
            );
        }
        match outcome {
            Ok(Ok(mut result)) => {
                scrub(&mut result, &c.credential);
                let Ok(raw) = serde_json::to_string(&result) else {
                    return Ok(
                        json!({"outcome":"unknown","message":"A service response arrived but could not be displayed. Nothing was retried."}),
                    );
                };
                if raw.len() > 64 * 1024 {
                    let mut end = 64 * 1024;
                    while !raw.is_char_boundary(end) {
                        end -= 1;
                    }
                    result = json!({"truncated":true,"text":&raw[..end]});
                }
                Ok(
                    json!({"outcome":"received","message":"Service response received. Inspect its result; receipt alone does not prove the requested change succeeded.","result":result}),
                )
            }
            _ => Ok(
                json!({"outcome":"unknown","message":"No complete service response was confirmed. Outcome unknown; nothing was retried. Check the service before making a new request."}),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use tiny_http::{Header, Response, Server};
    struct Fixture {
        url: String,
        calls: Arc<AtomicUsize>,
        last: Arc<Mutex<Value>>,
        stop: Arc<AtomicBool>,
        server: Arc<Server>,
        thread: Option<std::thread::JoinHandle<()>>,
        deny_auth: Arc<AtomicBool>,
    }
    impl Fixture {
        fn new() -> Self {
            let server = Arc::new(Server::http("127.0.0.1:0").unwrap());
            let url = format!("http://{}/mcp", server.server_addr());
            let calls = Arc::new(AtomicUsize::new(0));
            let last = Arc::new(Mutex::new(Value::Null));
            let stop = Arc::new(AtomicBool::new(false));
            let deny_auth = Arc::new(AtomicBool::new(false));
            let denied = deny_auth.clone();
            let (s, n, l, done) = (server.clone(), calls.clone(), last.clone(), stop.clone());
            let thread = std::thread::spawn(move || {
                while !done.load(Ordering::SeqCst) {
                    let Ok(Some(mut request)) = s.recv_timeout(Duration::from_millis(100)) else {
                        continue;
                    };
                    if denied.load(Ordering::SeqCst) {
                        let _ = request.respond(Response::empty(401));
                        continue;
                    }
                    if request.method().as_str() != "POST" {
                        let _ = request.respond(Response::empty(405));
                        continue;
                    }
                    let input: Value = match serde_json::from_reader(request.as_reader()) {
                        Ok(v) => v,
                        Err(_) => {
                            let _ = request.respond(Response::empty(400));
                            continue;
                        }
                    };
                    let id = input["id"].clone();
                    let method = input["method"].as_str().unwrap_or("");
                    if method.starts_with("notifications/") {
                        let _ = request.respond(Response::empty(202));
                        continue;
                    }
                    let result = match method {
                        "initialize" => {
                            json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{},"resources":{}},"serverInfo":{"name":"Protocol fixture","version":"1"}})
                        }
                        "tools/list" => {
                            if input["params"]["cursor"].is_string() {
                                json!({"tools":[{"name":"other","inputSchema":{"type":"object"}}]})
                            } else {
                                json!({"tools":[{"name":"record","description":"Records one explicitly approved call","inputSchema":{"type":"object","properties":{"mode":{"type":"string"},"text":{"type":"string"}}}}],"nextCursor":"page-two"})
                            }
                        }
                        "resources/list" => {
                            json!({"resources":[{"name":"Fixture reference","uri":"memo://fixture","mimeType":"text/plain"}]})
                        }
                        "resources/read" => {
                            json!({"contents":[{"uri":"memo://fixture","mimeType":"text/plain","text":"Fixture reference content"}]})
                        }
                        "tools/call" => {
                            n.fetch_add(1, Ordering::SeqCst);
                            *l.lock().unwrap() = input["params"]["arguments"].clone();
                            match input["params"]["arguments"]["mode"].as_str().unwrap_or("") {
                                "drop" => {
                                    let _ = request.respond(Response::empty(503));
                                    continue;
                                }
                                "delay" => std::thread::sleep(Duration::from_millis(350)),
                                _ => {}
                            }
                            let text = if input["params"]["arguments"]["mode"] == "oversize" {
                                "x".repeat(MAX_RESPONSE + 1)
                            } else {
                                input["params"]["arguments"]["text"]
                                    .as_str()
                                    .unwrap_or("Recorded")
                                    .to_owned()
                            };
                            json!({"content":[{"type":"text","text":text}],"structuredContent":{"fixture-service-secret":"fixture-service-secret"},"isError":false})
                        }
                        _ => {
                            let _=request.respond(Response::from_string(json!({"jsonrpc":"2.0","id":"server-error","error":{"code":-32601,"message":"Method not found"}}).to_string()).with_status_code(400).with_header(Header::from_bytes("content-type","application/json").unwrap()));
                            continue;
                        }
                    };
                    let raw = json!({"jsonrpc":"2.0","id":id,"result":result}).to_string();
                    let (body, kind) = if input["params"]["arguments"]["mode"] == "sse" {
                        (
                            format!("event: message\ndata: {raw}\n\n"),
                            "text/event-stream",
                        )
                    } else {
                        (raw, "application/json")
                    };
                    let _ = request.respond(
                        Response::from_string(body)
                            .with_header(Header::from_bytes("content-type", kind).unwrap())
                            .with_header(
                                Header::from_bytes("mcp-session-id", "fixture-session").unwrap(),
                            ),
                    );
                }
            });
            Self {
                url,
                calls,
                last,
                stop,
                server,
                thread: Some(thread),
                deny_auth,
            }
        }
        fn connect(&self, m: &Connections) -> String {
            let _ = rustls::crypto::ring::default_provider().install_default();
            // Test-only direct constructor: production connect always validates and pins public HTTPS.
            let client = reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .retry(reqwest::retry::never())
                .timeout(DEADLINE)
                .build()
                .unwrap();
            let c = m
                .connect_client(
                    crate::web::secure_token().unwrap(),
                    "Fixture".into(),
                    self.url.clone(),
                    "fixture-service-secret".into(),
                    client,
                )
                .unwrap();
            let id = c["id"].as_str().unwrap().to_owned();
            m.catalog(&id).unwrap();
            id
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::SeqCst);
            self.server.unblock();
            if let Some(t) = self.thread.take() {
                t.join().unwrap();
            }
        }
    }
    fn review(m: &Connections, id: &str, mode: &str) -> String {
        m.command(json!({"op":"review","id":id,"kind":"tools","name":"record","arguments":{"mode":mode,"text":"Approved input"}}),"").unwrap()["review"].as_str().unwrap().into()
    }
    fn run(m: &Connections, ticket: &str) -> Value {
        m.command(
            json!({"op":"execute","review":ticket,"arguments":{"text":"Unreviewed override"}}),
            "",
        )
        .unwrap()
    }
    #[test]
    fn endpoint_rejects_private_self_and_credential_urls() {
        for url in [
            "http://example.com/mcp",
            "https://127.0.0.1/mcp",
            "https://0x7f000001/mcp",
            "https://[::1]/mcp",
            "https://[::ffff:127.0.0.1]/mcp",
            "https://169.254.169.254/mcp",
            "https://user:secret@example.com/mcp",
            "https://example.com/mcp?token=secret",
            "https://example.com/mcp#secret",
            "https://verb.example/mcp",
            "https://localhost./mcp",
        ] {
            assert!(endpoint(url, "https://verb.example").is_err(), "{url}");
        }
        assert!(endpoint("https://mcp.example.com/mcp", "https://verb.example").is_ok());
        for ip in [
            "10.0.0.1",
            "172.16.2.1",
            "192.168.1.1",
            "100.64.0.1",
            "198.18.0.1",
            "192.0.2.1",
            "203.0.113.1",
            "224.0.0.1",
            "fc00::1",
            "fe80::1",
            "3fff::1",
            "2001:db8::1",
        ] {
            assert!(!public_ip(ip.parse().unwrap()), "{ip}");
        }
        assert!(public_ip("1.1.1.1".parse().unwrap()));
        assert!(public_ip("2606:4700:4700::1111".parse().unwrap()));
    }
    #[test]
    fn sdk_discovers_reads_and_executes_exactly_once() {
        let f = Fixture::new();
        let m = Connections::default();
        let id = f.connect(&m);
        let r = m
            .command(
                json!({"op":"review","id":id,"kind":"resources","uri":"memo://fixture"}),
                "",
            )
            .unwrap();
        assert!(run(&m, r["review"].as_str().unwrap())["result"]
            .to_string()
            .contains("Fixture reference content"));
        for mode in ["json", "sse"] {
            let ticket = review(&m, &id, mode);
            let result = run(&m, &ticket);
            assert_eq!(result["outcome"], "received");
            assert!(result.to_string().contains("Approved input"));
            assert!(!result.to_string().contains("fixture-service-secret"));
            assert_eq!(run(&m, &ticket)["outcome"], "not_sent");
        }
        assert_eq!(f.calls.load(Ordering::SeqCst), 2);
        assert_eq!(f.last.lock().unwrap()["text"], "Approved input");
    }
    #[test]
    fn refresh_and_expiry_invalidate_reviews_without_dispatch() {
        let f = Fixture::new();
        let m = Connections::default();
        let id = f.connect(&m);
        let ticket = review(&m, &id, "json");
        m.catalog(&id).unwrap();
        assert_eq!(run(&m, &ticket)["outcome"], "not_sent");
        let ticket = review(&m, &id, "json");
        m.state().unwrap().reviews.get_mut(&ticket).unwrap().expires =
            Instant::now() - Duration::from_secs(1);
        assert_eq!(run(&m, &ticket)["outcome"], "not_sent");
        assert_eq!(f.calls.load(Ordering::SeqCst), 0);
    }
    #[test]
    fn duplicate_concurrent_approval_cannot_duplicate_a_call() {
        let f = Fixture::new();
        let m = Connections::default();
        let id = f.connect(&m);
        let ticket = review(&m, &id, "delay");
        let other = m.clone();
        let copy = ticket.clone();
        let t = std::thread::spawn(move || run(&other, &copy));
        let result = run(&m, &ticket);
        let second = t.join().unwrap();
        assert!(result["outcome"] == "not_sent" || second["outcome"] == "not_sent");
        assert_eq!(f.calls.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn failed_and_oversized_responses_are_unknown_and_never_replayed() {
        let f = Fixture::new();
        let m = Connections::default();
        let id = f.connect(&m);
        for mode in ["drop", "oversize"] {
            let ticket = review(&m, &id, mode);
            assert_eq!(run(&m, &ticket)["outcome"], "unknown");
            assert_eq!(run(&m, &ticket)["outcome"], "not_sent");
        }
        assert_eq!(f.calls.load(Ordering::SeqCst), 2);
    }
    #[test]
    fn disconnect_revokes_reviews_and_discards_inflight_content() {
        let f = Fixture::new();
        let m = Connections::default();
        let id = f.connect(&m);
        let ticket = review(&m, &id, "delay");
        let unused = review(&m, &id, "json");
        let other = m.clone();
        let t = std::thread::spawn(move || run(&other, &ticket));
        let until = Instant::now() + Duration::from_secs(3);
        while f.calls.load(Ordering::SeqCst) == 0 {
            assert!(Instant::now() < until);
            std::thread::sleep(Duration::from_millis(5));
        }
        m.command(json!({"op":"disconnect","id":id}), "").unwrap();
        let result = t.join().unwrap();
        assert_eq!(result["outcome"], "unknown");
        assert!(result.get("result").is_none());
        assert_eq!(run(&m, &unused)["outcome"], "not_sent");
        assert_eq!(m.status().unwrap()["connections"], json!([]));
        assert_eq!(
            Connections::default().status().unwrap()["connections"],
            json!([])
        );
    }
    #[test]
    fn service_authentication_failure_revokes_further_tool_admission() {
        let f = Fixture::new();
        let m = Connections::default();
        let id = f.connect(&m);
        let ticket = review(&m, &id, "json");
        f.deny_auth.store(true, Ordering::SeqCst);
        assert_eq!(run(&m, &ticket)["outcome"], "unknown");
        assert_eq!(
            m.status().unwrap()["connections"][0]["status"],
            "Sign-in required"
        );
        assert!(m
            .command(
                json!({"op":"review","id":id,"kind":"tools","name":"record","arguments":{}}),
                ""
            )
            .unwrap_err()
            .contains("sign-in"));
        assert_eq!(f.calls.load(Ordering::SeqCst), 0);
        assert!(!m
            .status()
            .unwrap()
            .to_string()
            .contains("fixture-service-secret"));
    }
}
