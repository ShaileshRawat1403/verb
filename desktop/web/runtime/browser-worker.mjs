// Private NDJSON control worker with an authenticated loopback-only egress proxy. No personal profile.
import { chromium } from 'playwright-core';
import { createInterface } from 'node:readline';
import { randomUUID } from 'node:crypto';
import { createServer, request as httpRequest } from 'node:http';
import { connect } from 'node:net';
import { browserOrigin } from './browser-origin.mjs';

const forbidden = new URL(process.argv[2]).origin;
let proxy = null, proxySockets = new Set();
let browser, context, pages = new Map(), active, controller = null, generation = 0;
let grants = new Set(), blocked = new Set(), revision = 0;
const pageState = new WeakMap();
function stateFor(page) {
  if (!pageState.has(page)) pageState.set(page, { errors: [], navigationError: null });
  return pageState.get(page);
}
const text = (value, max = 2000) => String(value ?? '').slice(0, max);
const originOf = (value) => browserOrigin(value, forbidden);
function failure(error, page = pages.get(active)) {
  if (!page) return;
  const state = stateFor(page);
  state.errors.push(text(error?.message ?? error, 500));
  state.errors = state.errors.slice(-12);
}
async function navigate(page, action) {
  const state = stateFor(page);
  state.navigationError = null; state.errors = [];
  try {
    const response = await action();
    if (response && response.status() >= 400) throw new Error(`HTTP ${response.status()} from the page or its access boundary`);
  } catch (error) {
    state.navigationError = text(error?.message ?? error, 500);
    failure(error, page);
  }
}
// The proxy checks destinations before connecting. Route hooks alone do not cover every redirect.
async function startProxy() {
  const password = randomUUID(), authorization = `Basic ${Buffer.from(`verb:${password}`).toString('base64')}`;
  function allowed(value) {
    try {
      const origin = originOf(value);
      if (grants.has(origin)) return true;
      // Background Chromium connections are denied silently, not presented as page dependencies.
    } catch { /* Invalid protocols or Verb itself. */ }
    return false;
  }
  proxy = createServer((request, response) => {
    if (request.headers['proxy-authorization'] !== authorization) {
      response.writeHead(407, { 'Proxy-Authenticate': 'Basic realm="Verb browser"' }); response.end(); return;
    }
    if (!allowed(request.url)) { response.writeHead(403); response.end('Site access has not been granted in Verb.'); return; }
    const url = new URL(request.url);
    if (url.protocol !== 'http:') { response.writeHead(400); response.end(); return; }
    const headers = { ...request.headers, host: url.host };
    delete headers['proxy-authorization']; delete headers['proxy-connection']; headers.connection = 'close';
    const upstream = httpRequest(url, { method: request.method, headers, timeout: 15000 }, (result) => {
      result.on('error', () => response.destroy());
      response.writeHead(result.statusCode, result.headers); result.pipe(response);
    });
    upstream.on('timeout', () => upstream.destroy());
    upstream.on('error', () => { if (response.destroyed) return; if (!response.headersSent) response.writeHead(502); response.end(); });
    response.on('error', () => upstream.destroy());
    response.on('close', () => upstream.destroy());
    request.on('aborted', () => upstream.destroy());
    request.pipe(upstream);
  });
  proxy.on('connection', (socket) => {
    proxySockets.add(socket);
    // Reset/aborted browser connections are request failures, never worker failures.
    socket.on('error', () => socket.destroy());
    socket.on('close', () => proxySockets.delete(socket));
  });
  proxy.on('connect', (request, socket, head) => {
    if (request.headers['proxy-authorization'] !== authorization) {
      socket.end('HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic realm="Verb browser"\r\n\r\n'); return;
    }
    if (!allowed(`https://${request.url}`)) { socket.end('HTTP/1.1 403 Forbidden\r\n\r\n'); return; }
    const url = new URL(`https://${request.url}`);
    const upstream = connect(Number(url.port || 443), url.hostname.replace(/^\[|\]$/g, ''));
    proxySockets.add(upstream); upstream.on('close', () => proxySockets.delete(upstream));
    upstream.setTimeout(30000, () => upstream.destroy());
    upstream.once('connect', () => { if (socket.destroyed) { upstream.destroy(); return; } socket.write('HTTP/1.1 200 Connection Established\r\n\r\n'); if (head.length) upstream.write(head); socket.pipe(upstream); upstream.pipe(socket); });
    upstream.on('error', () => socket.destroy()); socket.on('error', () => upstream.destroy()); socket.on('close', () => upstream.destroy());
  });
  await new Promise((ok, fail) => { proxy.once('error', fail); proxy.listen(0, '127.0.0.1', ok); });
  return { server: `http://127.0.0.1:${proxy.address().port}`, username: 'verb', password };
}

async function addPage() {
  if (pages.size >= 4) throw new Error('Four browser tabs are open. Close a tab first.');
  const page = await context.newPage();
  page.setDefaultTimeout(4000);
  page.setDefaultNavigationTimeout(10000);
  const id = randomUUID(); pages.set(id, page); active = id;
  page.on('pageerror', (error) => failure(error, page));
  page.on('console', (m) => { if (m.type() === 'error') failure(m.text(), page); });
  page.on('dialog', (d) => { failure('A page dialog was dismissed.', page); void d.dismiss(); });
  page.on('popup', (p) => { failure('A popup was blocked. Open its address explicitly.', page); void p.close(); });
  page.on('download', (d) => { failure('Downloads are disabled in this first browser slice.', page); void d.cancel(); });
  return page;
}
async function start(client) {
  if (browser) return;
  sessionId = randomUUID();
  const proxyConfig = await startProxy();
  browser = await chromium.launch({ executablePath: process.env.VERB_BROWSER_EXECUTABLE,
    proxy: proxyConfig, args: ['--proxy-bypass-list=<-loopback>', '--force-webrtc-ip-handling-policy=disable_non_proxied_udp'],
    headless: true, chromiumSandbox: true, timeout: 15000,
    env: { PATH: process.env.PATH, TMPDIR: process.env.TMPDIR },
  });
  context = await browser.newContext({ viewport: { width: 1100, height: 720 },
    acceptDownloads: false, serviceWorkers: 'block', permissions: [] });
  context.on('request', (request) => {
    // Unlike route hooks, request events include every page redirect. Browser background
    // traffic does not belong to this context and must not ask the person for site access.
    try {
      const origin = originOf(request.url());
      if (!grants.has(origin) && blocked.size < 32) blocked.add(origin);
    } catch { /* Unsupported destinations are never eligible for a grant. */ }
  });
  await context.route('**/*', async (route) => {
    try {
      const origin = originOf(route.request().url());
      if (grants.has(origin)) { await route.continue(); return; }
      if (blocked.size < 32) blocked.add(origin);
    } catch { /* Unknown protocols and Verb are always refused. */ }
    await route.abort('blockedbyclient');
  });
  await context.routeWebSocket('**/*', (route) => {
    // WebSockets are intentionally unavailable until their revocation model is implemented.
    failure('A WebSocket connection was blocked.'); route.close();
  });
  controller = client; generation += 1;
  await addPage();
}
async function stop() {
  const old = browser; browser = null; context = null; active = null;
  pages.clear(); controller = null; generation += 1; grants.clear(); blocked.clear();
  await old?.close();
  for (const socket of proxySockets) socket.destroy();
  proxySockets.clear();
  if (proxy) { proxy.closeAllConnections(); await new Promise((resolve) => proxy.close(resolve)); proxy = null; }
}
async function view(capture = false) {
  const tabs = [];
  for (const [id, page] of pages) {
    if (page.isClosed()) { pages.delete(id); continue; }
    tabs.push({ id, url: page.url(), title: text(await page.title().catch(() => ''), 100) });
  }
  const page = pages.get(active);
  let image = null;
  if (capture && page && !page.isClosed()) {
    const png = await page.screenshot({ type: 'jpeg', quality: 70, timeout: 5000 });
    if (png.length > 1500000) throw new Error('The rendered page is too large to display.');
    image = png.toString('base64');
  }
  return { running: Boolean(browser?.isConnected()), controller, generation, session: sessionId,
    revision: ++revision, active, tabs, grants: [...grants], blocked: [...blocked],
    errors: page ? stateFor(page).errors : [], navigationError: page ? stateFor(page).navigationError : null,
    viewport: page?.viewportSize(), image };
}
let sessionId = randomUUID();
async function handle(input) {
  if (!input || typeof input.client !== 'string' || !/^[a-zA-Z0-9_-]{8,64}$/.test(input.client))
    throw new Error('A browser client identity is required.');
  if (input.op === 'start') { try { await start(input.client); } catch (error) { await stop(); throw error; } return view(true); }
  if (input.op === 'status') return view(false);
  if (input.op === 'stop') { await stop(); return view(false); }
  if (!browser?.isConnected()) throw new Error('The browser stopped. Start a fresh session; no action was retried.');
  if (input.op === 'take') { controller = input.client; generation += 1; return view(true); }
  if (input.op === 'capture') return view(true); // Viewing does not transfer input control.
  if (input.client !== controller || input.generation !== generation)
    throw new Error('Browser control changed. Take control before acting.');
  let page = pages.get(active);
  switch (input.op) {
    case 'capture': break;
    case 'open': {
      if (typeof input.url !== 'string' || input.url.length > 2000) throw new Error('Address is too long.');
      const origin = originOf(input.url);
      if (!grants.has(origin)) {
        if (input.allow !== true) throw new Error('Allow this site before opening it.');
        if (grants.size >= 32) throw new Error('Site limit reached. Stop and start a fresh session.');
        grants.add(origin); blocked.delete(origin);
      }
      await navigate(page, () => page.goto(input.url, { waitUntil: 'domcontentloaded' }));
      break;
    }
    case 'grant': {
      const origin = originOf(input.url);
      if (grants.size >= 32) throw new Error('Site limit reached.');
      grants.add(origin); blocked.delete(origin); break;
    }
    case 'back': await navigate(page, () => page.goBack({ waitUntil: 'domcontentloaded' })); break;
    case 'forward': await navigate(page, () => page.goForward({ waitUntil: 'domcontentloaded' })); break;
    case 'reload': await navigate(page, () => page.reload({ waitUntil: 'domcontentloaded' })); break;
    case 'tab': if (!pages.has(input.tab)) throw new Error('Tab no longer exists.'); active = input.tab; break;
    case 'new': await addPage(); break;
    case 'close': await page.close(); pages.delete(active); active = pages.keys().next().value;
      if (!active) await addPage(); break;
    case 'viewport': {
      const width = Number(input.width), height = Number(input.height);
      if (![390, 768, 1100].includes(width) || height !== 720) throw new Error('Unsupported viewport.');
      await page.setViewportSize({ width, height }); break;
    }
    case 'click': {
      const { width, height } = page.viewportSize();
      if (!Number.isFinite(input.x) || !Number.isFinite(input.y) || input.x < 0 || input.y < 0
          || input.x >= width || input.y >= height) throw new Error('Click is outside the page.');
      await page.mouse.click(input.x, input.y); break;
    }
    case 'type':
      if (typeof input.text !== 'string' || input.text.length > 2000) throw new Error('Text is too long.');
      await page.keyboard.insertText(input.text); break;
    case 'key':
      if (!['Enter', 'Tab', 'Shift+Tab', 'Escape', 'Backspace', 'ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight', 'Space', 'Home', 'End', 'Delete', 'ControlOrMeta+A'].includes(input.key))
        throw new Error('Unsupported key.');
      await page.keyboard.press(input.key); break;
    case 'scroll':
      if (!Number.isFinite(input.dy) || Math.abs(input.dy) > 2000) throw new Error('Scroll is out of range.');
      await page.mouse.wheel(0, input.dy); break;
    case 'inspect': {
      const content = await page.locator('body').innerText({ timeout: 3000 }).catch(() => '');
      return { ...await view(true), text: content.slice(0, 12000) };
    }
    default: throw new Error('Unsupported browser operation.');
  }
  return view(true);
}
const rl = createInterface({ input: process.stdin, crlfDelay: Infinity });
let shutdownTask;
function shutdown() { return shutdownTask ??= stop().catch(() => {}); }
process.on('SIGTERM', () => { void shutdown().finally(() => process.exit(0)); });
process.on('SIGINT', () => { void shutdown().finally(() => process.exit(0)); });
try {
  for await (const line of rl) {
    if (Buffer.byteLength(line) > 3500) { console.log(JSON.stringify({ error: 'Browser command too large.' })); continue; }
    try { console.log(JSON.stringify(await handle(JSON.parse(line)))); }
    catch (e) { console.log(JSON.stringify({ error: text(e.message, 600), running: Boolean(browser?.isConnected()) })); }
  }
} finally { await shutdown(); }
