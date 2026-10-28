// Rendered browser pixels are data; website code never runs in the Verb page.
export function initStudioBrowser({ $, api, beforeOpen }) {
  const panel = $('#studio-browser'), stage = $('#studio-browser-stage');
  const status = $('#studio-browser-status'), address = $('#studio-browser-address');
  let view = null, busy = false, serial = 0, timer = null, tabKey = null, sitesKey = null, recoverStatus = false;
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
      ? `Browser on this Verb host · ${mine ? 'You have control' : 'Another client has control'} · ${view.session?.slice(0, 8) ?? ''}`
      : 'Fresh session · no saved logins imported';
    for (const button of panel.querySelectorAll('[data-browser-op], #studio-browser-open, #studio-browser-send-text, #studio-browser-viewport'))
      button.disabled = busy || !mine;
    $('#studio-browser-start').disabled = busy;
    const tabs = $('#studio-browser-tabs');
    const nextTabKey = JSON.stringify([view?.tabs ?? [], view?.active]);
    if (tabKey !== nextTabKey) {
      tabKey = nextTabKey; tabs.replaceChildren();
      for (const tab of view?.tabs ?? []) {
      const button = document.createElement('button'); button.type = 'button';
      button.className = 'studio-browser-tab'; button.textContent = tab.title || 'New tab';
      button.title = tab.url; button.setAttribute('aria-pressed', String(tab.id === view.active));
        button.onclick = () => command('tab', { tab: tab.id }); tabs.append(button);
      }
    }
    for (const button of tabs.querySelectorAll('button')) button.disabled = busy || !mine;
    const current = view?.tabs?.find((tab) => tab.id === view.active);
    $('#studio-browser-current-url').textContent = current?.url || 'No page open';
    if (document.activeElement !== address && current?.url && current.url !== 'about:blank') address.value = current.url;
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
          if (!controls() || busy) return;
          const rect = image.getBoundingClientRect(), size = view.viewport;
          command('click', { x: Math.floor((event.clientX - rect.left) / rect.width * size.width), y: Math.floor((event.clientY - rect.top) / rect.height * size.height) });
        });
        image.addEventListener('keydown', (event) => {
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
      empty.textContent = 'Open a real page beside your work. Start a fresh browser, then allow the site you want to visit.'; stage.append(empty);
    }
  }
  async function command(op, extra = {}) {
    if (busy && op !== 'stop') { announce('Browser is busy. Your action was not sent; try again when it finishes.'); return false; }
    const request = ++serial; busy = true;
    if (op !== 'capture') { render(); announce(op === 'stop' ? 'Closing browser…' : 'Working…'); }
    try {
      const result = await api('POST', '/api/browser', { op, client, session: view?.session, generation: view?.generation, ...extra });
      if (request !== serial || panel.hidden) return false;
      if (result.error) { if (result.running === false) view = result; throw new Error(result.error); }
      if (view?.active !== result.active || ['open', 'back', 'forward', 'reload'].includes(op)) $('#studio-browser-text').textContent = '';
      recoverStatus = false;
      view = result; if (result.text != null) $('#studio-browser-text').textContent = result.text;
      if (!result.running) $('#studio-browser-text').textContent = '';
      announce(result.running ? (result.blocked?.length ? 'Some page resources need site access. Review sites waiting for access below.' : 'Page view updated. Site access lasts until you stop this session.') : op === 'status' ? 'Ready to start. No browser is running.' : 'Browser stopped. Session logins and site access were discarded.');
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
    if (!panel.hidden && view?.running) timer = setTimeout(() => command(recoverStatus ? 'status' : 'capture'), 1800);
  }
  async function show() {
    beforeOpen(); panel.hidden = false; $('#studio-preview-content').hidden = true;
    $('#studio-show-browser').setAttribute('aria-pressed', 'true');
    $('#studio-show-document').setAttribute('aria-pressed', 'false');
    $('#studio-editor-status').textContent = '';
    try {
      const capability = await api('GET', '/api/browser');
      if (panel.hidden) return;
      $('#studio-browser-start').disabled = !capability.available;
      announce(capability.reason);
      if (capability.available) await command('status');
    } catch (error) { announce(error.message); $('#studio-browser-start').disabled = true; }
    address.focus();
  }
  function hide({ restore = true } = {}) {
    if (panel.hidden) return;
    panel.hidden = true; ++serial; busy = false; clearTimeout(timer);
    stage.replaceChildren(); $('#studio-browser-text').textContent = ''; $('#studio-browser-errors').textContent = '';
    $('#studio-show-browser').setAttribute('aria-pressed', 'false');
    if (restore) $('#studio-show-document').setAttribute('aria-pressed', String($('#view-studio').dataset.canvasMode === 'document'));
    if (restore) $('#studio-preview-content').hidden = false;
    if (view?.running) void api('POST', '/api/browser', { op: 'stop', client, session: view?.session }).catch(() => {});
    view = null;
  }
  $('#studio-show-browser').onclick = () => panel.hidden ? show() : hide();
  $('#studio-browser-start').onclick = () => command('start');
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
