// Rendered browser pixels are data; website code never runs in the Verb page.
export function initStudioBrowser({ $, api, beforeOpen }) {
  const panel = $('#studio-browser'), stage = $('#studio-browser-stage');
  const status = $('#studio-browser-status'), address = $('#studio-browser-address');
  let view = null, busy = false, serial = 0, timer = null, tabKey = null, sitesKey = null, recoverStatus = false, availability = null, visibility = 0, addressDirty = false, captureWait = null, runtimeLocation = 'this Verb host', pairingTimer = null, pairingBusy = false;
  const announce = (text) => { if (status.textContent !== text) status.textContent = text; };
  let client;
  try { client = sessionStorage.getItem('verb.browser.client'); } catch {}
  if (!client) {
    client = `browser-${crypto.randomUUID()}`;
    try { sessionStorage.setItem('verb.browser.client', client); } catch {}
  }
  const controls = () => Boolean(view?.running && view.controller === client);
  function render() {
    const running = Boolean(view?.running), mine = controls();
    $('#studio-browser-start').hidden = running;
    $('#studio-browser-take').hidden = !running || mine;
    $('#studio-browser-stop').hidden = !running;
    $('#studio-browser-controls').hidden = !running;
    $('#studio-browser-identity').textContent = running
      ? `Browser on ${runtimeLocation} · ${mine ? 'You have control' : 'Another client has control'} · ${view.session?.slice(0, 8) ?? ''}`
      : availability === false ? 'Browser unavailable on this host' : 'Fresh session · no saved logins imported';
    for (const button of panel.querySelectorAll('[data-browser-op], #studio-browser-open, #studio-browser-send-text, #studio-browser-viewport'))
      button.disabled = busy || !mine;
    $('#studio-browser-start').disabled = busy || availability !== true;
    $('#studio-browser-viewport').value = String(view?.viewport?.width ?? 1100);
    const tabs = $('#studio-browser-tabs');
    const nextTabKey = JSON.stringify([view?.tabs ?? [], view?.active]);
    if (tabKey !== nextTabKey) {
      tabKey = nextTabKey; tabs.replaceChildren();
      for (const tab of view?.tabs ?? []) {
      const button = document.createElement('button'); button.type = 'button';
      button.className = 'studio-browser-tab'; button.textContent = tab.title || 'Blank page';
      button.title = tab.url; button.setAttribute('aria-pressed', String(tab.id === view.active));
        button.onclick = () => command('tab', { tab: tab.id }); tabs.append(button);
      }
    }
    for (const button of tabs.querySelectorAll('button')) button.disabled = busy || !mine;
    const current = view?.tabs?.find((tab) => tab.id === view.active);
    $('#studio-browser-current-url').textContent = current?.url || 'No page open';
    if (!addressDirty && document.activeElement !== address) address.value = current?.url === 'about:blank' ? '' : current?.url ?? '';
    const grants = $('#studio-browser-sites');
    const nextSitesKey = JSON.stringify(view?.blocked ?? []);
    if (sitesKey !== nextSitesKey) {
      sitesKey = nextSitesKey; grants.replaceChildren();
      for (const origin of view?.blocked ?? []) {
      const row = document.createElement('div'), text = document.createElement('span'); text.textContent = origin;
      const allow = document.createElement('button'); allow.type = 'button'; allow.className = 'text-button';
      allow.textContent = 'Allow site';
        allow.onclick = () => command('grant', { url: origin }); row.append(text, allow); grants.append(row);
      }
    }
    for (const button of grants.querySelectorAll('button')) button.disabled = busy || !mine;
    $('#studio-browser-sites-wrap').hidden = !view?.blocked?.length;
    $('#studio-browser-errors').textContent = (view?.errors ?? []).join('\n');
    if (view?.image) {
      let image = stage.querySelector('img');
      if (!image) {
        stage.replaceChildren(); image = document.createElement('img');
        image.alt = 'Rendered browser page. Use Inspect page for readable text.'; image.draggable = false;
        image.tabIndex = 0; image.setAttribute('aria-label', 'Browser page: click to interact. Open Type & navigate the page for remote keyboard controls.');
        image.addEventListener('click', (event) => {
          if (!controls()) return;
          const rect = image.getBoundingClientRect(), size = view.viewport;
          command('click', { x: Math.floor((event.clientX - rect.left) / rect.width * size.width), y: Math.floor((event.clientY - rect.top) / rect.height * size.height) });
        });
        image.addEventListener('keydown', (event) => {
          if (!controls()) return;
          if (['Tab', 'Enter', 'Escape', 'ArrowDown', 'ArrowUp', 'ArrowLeft', 'ArrowRight', 'Backspace', ' '].includes(event.key) && !event.ctrlKey && !event.metaKey) {
            // Escape always leaves the browser surface; Tab outside this image uses normal UI focus.
            if (event.key === 'Escape') { address.focus(); return; }
            if (event.key === 'Tab') return;
            event.preventDefault(); command('key', { key: event.key === ' ' ? 'Space' : event.key });
          }
        });
        stage.append(image);
      }
      image.src = `data:image/jpeg;base64,${view.image}`;
      image.style.width = `min(100%, ${view.viewport.width}px)`;
      image.style.cursor = mine ? 'crosshair' : 'default';
    } else if (!running) {
      stage.replaceChildren(); const empty = document.createElement('div'); empty.className = 'studio-browser-empty';
      empty.textContent = availability === false
        ? 'Connect your Mac browser to open websites beside your work. Saved HTML files can still use App preview.'
        : 'Open a real page beside your work. Start a fresh browser, then allow the site you want to visit.'; stage.append(empty);
    }
  }
  async function command(op, extra = {}) {
    if (busy && captureWait && !['capture', 'status', 'stop'].includes(op)) {
      const session = view?.session, generation = view?.generation;
      announce('Updating the page… your action will follow this refresh.');
      const refreshed = await captureWait;
      if (!refreshed) return false;
      if (panel.hidden || !view?.running || view.session !== session || view.generation !== generation) {
        if (!panel.hidden) announce('Browser session or control changed. Your action was not sent.');
        return false;
      }
    }
    if (busy && op !== 'stop') { announce('Browser is busy. Your action was not sent; try again when it finishes.'); return false; }
    clearTimeout(timer);
    const request = ++serial; busy = true;
    if (op !== 'capture') { render(); announce(op === 'stop' ? 'Closing browser…' : 'Working…'); }
    try {
      const result = await api('POST', '/api/browser', { op, client, session: view?.session, generation: view?.generation, ...extra });
      if (request !== serial || panel.hidden) {
        // Closing while Start is in flight must not leave an unseen runtime behind.
        // Use only the returned session identity: a stale stop cannot affect a replacement.
        if (op === 'start' && panel.hidden && result.running && result.session)
          void api('POST', '/api/browser', { op: 'stop', client, session: result.session }).catch(() => {});
        return false;
      }
      if (result.error) { if (result.running === false) view = result; throw new Error(result.error); }
      if (view?.active !== result.active || view?.tabs?.find(tab => tab.id === view.active)?.url !== result.tabs?.find(tab => tab.id === result.active)?.url || ['open', 'back', 'forward', 'reload'].includes(op)) $('#studio-browser-text').textContent = '';
      if (op === 'open') addressDirty = false;
      recoverStatus = false;
      view = result; if (result.text != null) $('#studio-browser-text').textContent = result.text;
      if (!result.running) $('#studio-browser-text').textContent = '';
      announce(result.running ? (result.navigationError ? `Page could not finish loading: ${result.navigationError}. Check the address or reload; nothing was retried.` : result.blocked?.length ? 'Some page resources need site access. Review sites waiting for access below.' : 'Page view updated. Site access lasts until you stop this session.') : op === 'status' ? 'Ready to start. No browser is running.' : 'Browser stopped. Session logins and site access were discarded.');
      return true;
    } catch (error) {
      if (request === serial) {
        announce(error.message);
        if ([401, 403, 409].includes(error.status)) { view = null; $('#studio-browser-text').textContent = ''; }
        else recoverStatus = true; // Read status before another snapshot; never replay the failed action.
      }
      return false;
    } finally {
      if (request === serial) { busy = false; render(); schedule(); }
    }
  }
  function schedule() {
    clearTimeout(timer);
    if (!panel.hidden && view?.running) timer = setTimeout(() => {
      const pending = command(recoverStatus ? 'status' : 'capture');
      captureWait = pending;
      void pending.finally(() => { if (captureWait === pending) captureWait = null; });
    }, 1800);
  }
  async function show() {
    const opened = ++visibility;
    availability = null;
    beforeOpen(); panel.hidden = false; $('#studio-preview-content').hidden = true;
    $('#studio-show-browser').setAttribute('aria-pressed', 'true');
    $('#studio-show-document').setAttribute('aria-pressed', 'false');
    $('#studio-editor-status').textContent = '';
    render();
    announce('Checking browser availability…');
    try {
      const capability = await api('GET', '/api/browser');
      if (panel.hidden || opened !== visibility) return;
      availability = capability.available === true;
      runtimeLocation = capability.location === 'Paired Mac companion' ? 'your paired Mac' : 'this Verb host';
      render();
      announce(capability.reason);
      if (capability.available) await command('status');
    } catch (error) {
      if (panel.hidden || opened !== visibility) return;
      availability = false; render();
      announce(`${error.message} Saved HTML app previews are separate and remain available from an HTML file.`);
    }
    if (!panel.hidden && opened === visibility) {
      await pairingStatus(opened);
      if (availability) address.focus();
    }
  }
  function clearPairCode() {
    clearTimeout(pairingTimer); $('#studio-browser-pair-code').value = ''; $('#studio-browser-code-wrap').hidden = true;
  }
  async function pairingStatus(opened = visibility) {
    try {
      const result = await api('GET', '/api/browser/pairing');
      if (panel.hidden || opened !== visibility) return;
      $('#studio-browser-pairing').hidden = false;
      $('#studio-browser-pair-status').textContent = result.connected ? 'Mac companion connected. Localhost refers to that Mac.' : result.pairing ? 'Waiting for the Mac to use its pairing code.' : 'No Mac companion connected.';
      $('#studio-browser-pair').disabled = pairingBusy || Boolean(result.connected);
      $('#studio-browser-revoke').disabled = pairingBusy || (!result.connected && !result.pairing);
      $('#studio-browser-companion-command').textContent = `node desktop/web/runtime/browser-companion.mjs --server ${location.origin} --chromium /absolute/path/to/chromium${location.protocol === 'https:' ? ' --cloudflare-access' : ''}`;
      if (result.connected || !result.pairing) clearPairCode();
    } catch { if (opened === visibility) $('#studio-browser-pairing').hidden = true; }
  }
  $('#studio-browser-pair').onclick = async () => {
    if (pairingBusy) return;
    pairingBusy = true; $('#studio-browser-pair').disabled = true; $('#studio-browser-revoke').disabled = true;
    const opened = visibility;
    try {
      const result = await api('POST', '/api/browser/pairing', { op: 'pair' });
      if (panel.hidden || opened !== visibility) return;
      clearPairCode(); $('#studio-browser-pair-code').value = result.code; $('#studio-browser-code-wrap').hidden = false;
      pairingTimer = setTimeout(clearPairCode, 120000); await pairingStatus(opened);
    } catch (error) { announce(error.message); }
    finally { pairingBusy = false; void pairingStatus(); }
  };
  $('#studio-browser-revoke').onclick = async () => {
    if (pairingBusy) return;
    pairingBusy = true; $('#studio-browser-pair').disabled = true; $('#studio-browser-revoke').disabled = true;
    try { await api('POST', '/api/browser/pairing', { op: 'revoke' }); clearPairCode(); ++serial; busy = false; clearTimeout(timer); view = null; await show(); }
    catch (error) { announce(error.message); }
    finally { pairingBusy = false; void pairingStatus(); }
  };
  $('#studio-browser-check').onclick = () => show();
  $('#studio-browser-copy-code').onclick = async () => {
    try { await navigator.clipboard.writeText($('#studio-browser-pair-code').value); announce('Pairing code copied. It expires in two minutes.'); }
    catch { announce('Clipboard unavailable. Select and copy the pairing code manually.'); }
  };
  function hide({ restore = true } = {}) {
    if (panel.hidden) return;
    clearPairCode(); panel.hidden = true; ++visibility; ++serial; busy = false; availability = null; clearTimeout(timer);
    stage.replaceChildren(); $('#studio-browser-text').textContent = ''; $('#studio-browser-errors').textContent = '';
    $('#studio-show-browser').setAttribute('aria-pressed', 'false');
    if (restore) $('#studio-show-document').setAttribute('aria-pressed', String($('#view-studio').dataset.canvasMode === 'document'));
    if (restore) $('#studio-preview-content').hidden = false;
    if (view?.running) void api('POST', '/api/browser', { op: 'stop', client, session: view?.session }).catch(() => {});
    view = null;
  }
  $('#studio-show-browser').onclick = () => panel.hidden ? show() : hide();
  $('#studio-browser-start').onclick = () => { if (availability === true) command('start'); };
  address.addEventListener('input', () => { addressDirty = true; });
  $('#studio-browser-stop').onclick = () => command('stop');
  $('#studio-browser-take').onclick = () => command('take');
  $('#studio-browser-form').onsubmit = (event) => { event.preventDefault(); command('open', { url: address.value.trim(), allow: true }); };
  async function typeIntoPage() {
    const input = $('#studio-browser-type'), value = input.value;
    if (!value) return;
    // Keep the draft on failure or an uncertain outcome. Never resend it automatically.
    const sent = await command('type', { text: value });
    if (sent && input.value === value) input.value = '';
  }
  $('#studio-browser-send-text').onclick = typeIntoPage;
  $('#studio-browser-type').addEventListener('keydown', (event) => {
    if (event.key === 'Enter' && !event.isComposing) {
      event.preventDefault(); void typeIntoPage();
    }
  });
  $('#studio-browser-viewport').onchange = (event) => command('viewport', { width: Number(event.target.value), height: 720 });
  panel.addEventListener('keydown', (event) => {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'l') {
      event.preventDefault(); address.focus(); address.select();
    }
  });
  panel.addEventListener('click', (event) => {
    const button = event.target.closest('[data-browser-op]');
    if (button) command(button.dataset.browserOp, button.dataset.browserKey ? { key: button.dataset.browserKey } : button.dataset.browserDy ? { dy: Number(button.dataset.browserDy) } : {});
  });
  return { hide };
}
