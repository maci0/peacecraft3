// PeaceCraft browser backend node driver for jev-harness.
//
// Drives the REAL PeaceCraft III WASM app in headless muted Chromium through
// the existing tools/original-game-reference/browser-adapter.mjs capture/input
// path (checkedGeometry, physicalPoint, physicalKey, focus, capture with
// actual clipped canvas, no None->(0,0) fallback, no OS desktop input/audio).
//
// Protocol (line-delimited JSON on stdin/stdout, one response per request):
//   request:  {"id":N,"op":"capture"|"input"|"wait"|"summary"|"close", ...}
//   response: {"id":N,"ok":true,"data":{...}} or {"id":N,"ok":false,"error":{...}}
//
// Startup config is the JSON file named by argv[2]:
//   {"url":"http://127.0.0.1:PORT/index.html","manifest":path,"iso":path|null,
//    "playwrightModule":path,"chromium":path,"extent":[w,h],"edition":"roc"|"tft"|null,
//    "importMode":"iso"|"none"|"assets"}
//
//  - importMode "iso" (default when iso set): browser-worker.mjs-compatible
//    ISO import flow (one candidate, edition select, demo-start, Main menu).
//  - importMode "assets": serve web/ statically, load launcher GUI without
//    archives (menu_state available, screen not Main). This is the smallest
//    ACTUAL PeaceCraft surface usable without a source closure.
//  - importMode "none": just load url, wait for #launcher-canvas.
//
// Headless/muted isolation is enforced before launch: chromium is launched
// with --headless=new, --mute-audio, --no-sandbox, --enable-unsafe-swiftshader,
// --disable-dev-shm-usage. DISPLAY/WAYLAND_DISPLAY are NOT cleared here (the
// node process inherits the caller's); the Python backend verifies isolation
// before spawning this driver and passes only the config file path.
//
// No real keys/API keys/image uploads are used. No new frameworks or CLI.

import {readFile, writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createInterface} from 'node:readline';
import {pathToFileURL} from 'node:url';
import {createBrowserAdapter, isConsoleError} from '../../original-game-reference/browser-adapter.mjs';

const MAX_META = 2 * 1024 * 1024;
const respond = (msg) => {
  const line = JSON.stringify(msg);
  if (Buffer.byteLength(line) > MAX_META) throw Error('response exceeds metadata bound');
  process.stdout.write(line + '\n');
};

const config = JSON.parse(await readFile(process.argv[2], 'utf8'));
const {chromium} = await import(pathToFileURL(config.playwrightModule).href);
const manifest = config.manifest ? JSON.parse(await readFile(config.manifest, 'utf8')) : null;
const origin = new URL(config.url).origin;
const [width, height] = config.extent;
const requests = [], errors = [], sockets = [], messages = [];
let browser;
let stage = 'launch';
let currentId = null;

const append = (list, value) => {
  if (list.length >= 1024 || JSON.stringify(value).length > 16384)
    throw Error('observation exceeds bounded collection policy');
  list.push(value);
};

try {
  browser = await chromium.launch({
    executablePath: config.chromium,
    headless: true,
    args: ['--no-sandbox', '--mute-audio', '--enable-unsafe-swiftshader', '--disable-dev-shm-usage'],
  });
  const context = await browser.newContext({
    viewport: {width, height},
    deviceScaleFactor: 1,
    serviceWorkers: 'block',
  });
  await context.routeWebSocket('**/*', socket => { append(sockets, socket.url()); socket.close(); });
  const page = await context.newPage();
  page.setDefaultTimeout(60000);

  // Restricted routing: only hash-checked manifest files or, for assets mode,
  // static web/ files under origin. Everything else aborted.
  await page.route('**/*', async route => {
    try {
      const request = route.request(), url = new URL(request.url());
      if (request.method() !== 'GET' || request.postDataBuffer() || url.origin !== origin) {
        append(errors, `Blocked request: ${request.method()} ${url.pathname}`);
        await route.abort('blockedbyclient');
        return;
      }
      if (manifest?.files) {
        const name = url.pathname.slice(1);
        const expected = Object.hasOwn(manifest.files, name) && manifest.files[name];
        if (!expected || !Number.isSafeInteger(expected.bytes) || expected.bytes < 0 ||
            expected.bytes > 72 * 1024 * 1024 || !/^[a-f0-9]{64}$/.test(expected.sha256)) {
          append(errors, `Blocked request outside manifest: ${url.pathname}`);
          await route.abort('blockedbyclient');
          return;
        }
        const response = await route.fetch({timeout: 60000, maxRedirects: 0});
        const body = await response.body();
        if (response.status() !== 200 || body.length !== expected.bytes ||
            createHash('sha256').update(body).digest('hex') !== expected.sha256)
          throw Error(`Code identity mismatch: ${name}`);
        append(requests, {path: name, method: 'GET', bytes: body.length, sha256: expected.sha256});
        await route.fulfill({response, body});
        return;
      }
      // Assets mode: fetch from the local static server (loopback only).
      await route.continue();
    } catch (error) {
      append(errors, error.message);
      await route.abort('failed').catch(() => {});
    }
  });

  page.on('pageerror', error => append(errors, error.message));
  page.on('websocket', socket => append(sockets, socket.url()));
  page.on('console', message => {
    const type = message.type(), text = message.text().slice(0, 4000);
    append(messages, {type, text});
    if (isConsoleError(type, text)) append(errors, text);
  });

  stage = 'page';
  await page.goto(config.url, {waitUntil: 'domcontentloaded'});

  const mode = config.importMode ?? (config.iso ? 'iso' : 'none');
  if (mode === 'iso') {
    await page.waitForFunction(() => globalThis.peacecraft3?.menu_state && globalThis.peacecraft3Settings);
    const revision = await page.evaluate(() => peacecraft3.asset_revision());
    stage = 'import';
    await page.locator('#iso').setInputFiles(config.iso);
    await page.waitForFunction(() => document.querySelectorAll('#iso-candidates input').length > 0 && !document.querySelector('#iso').disabled);
    if (await page.locator('#iso-candidates input').count() !== 1)
      throw Error('ISO import requires one candidate; ambiguous candidates refused');
    await page.locator('#iso-candidates input').check();
    await page.locator('#iso-import').click();
    await page.waitForFunction(value => peacecraft3.asset_revision() === value + 1 && !document.querySelector('#iso').disabled, revision);
    if (!(await page.evaluate(() => peacecraft3.local_menu_editions())).includes(config.edition))
      throw Error('Requested edition is not admitted by the imported source');
    await page.locator('#demo-edition').selectOption(config.edition);
    stage = 'entry-click';
    await page.locator('#demo-start').click({timeout: 120000});
    stage = 'entry-readiness';
    await page.waitForFunction(() => {
      const raw = peacecraft3.menu_state();
      if (!raw) return false;
      const state = JSON.parse(raw);
      return state.menuVisible && !state.transitionActive && state.screen === 'Main';
    }, undefined, {timeout: 120000});
    stage = 'capture-geometry';
    await page.evaluate(([w, h]) => {
      const stageEl = document.querySelector('#stage');
      Object.assign(stageEl.style, {position: 'fixed', left: '0', top: '0', zIndex: '2147483647',
        width: w + 'px', height: h + 'px', minWidth: w + 'px', maxWidth: w + 'px',
        minHeight: h + 'px', maxHeight: h + 'px', margin: '0', padding: '0', border: '0',
        overflow: 'hidden', transform: 'none', boxSizing: 'content-box'});
      window.scrollTo(0, 0);
      document.querySelector('#launcher-canvas').style.outline = 'none';
      peacecraft3Settings.resumeParentSizing();
    }, config.extent);
    await page.waitForFunction(([w, h]) => {
      const canvas = document.querySelector('#launcher-canvas');
      return canvas.width === w && canvas.height === h && canvas.clientWidth === w && canvas.clientHeight === h;
    }, config.extent);
  } else {
    // assets / none: wait for canvas + menu_state bridge
    await page.waitForFunction(() => {
      const c = document.querySelector('#launcher-canvas');
      return c && globalThis.peacecraft3?.menu_state;
    }, undefined, {timeout: 60000});
    stage = 'canvas-ready';
    // Position host page stage at viewport origin like the ISO path.
    await page.evaluate(([w, h]) => {
      const stageEl = document.querySelector('#stage');
      if (stageEl) {
        Object.assign(stageEl.style, {position: 'fixed', left: '0', top: '0', zIndex: '2147483647',
          width: w + 'px', height: h + 'px', minWidth: w + 'px', maxWidth: w + 'px',
          minHeight: h + 'px', maxHeight: h + 'px', margin: '0', padding: '0', border: '0',
          overflow: 'hidden', transform: 'none', boxSizing: 'content-box'});
      }
      window.scrollTo(0, 0);
      const canvas = document.querySelector('#launcher-canvas');
      if (canvas) canvas.style.outline = 'none';
      if (globalThis.peacecraft3Settings?.resumeParentSizing) peacecraft3Settings.resumeParentSizing();
    }, config.extent);
    await page.waitForFunction(([w, h]) => {
      const canvas = document.querySelector('#launcher-canvas');
      if (!canvas) return false;
      // Canvas bitmap/client must match extent for geometry check; if the app
      // sizes differently, checkedGeometry will refuse at capture time.
      return canvas.width > 0 && canvas.height > 0;
    }, config.extent);
  }

  const adapter = createBrowserAdapter({page, cdp: await page.context().newCDPSession(page), width, height});
  stage = 'focus';
  await adapter.focus();
  if (errors.length || sockets.length)
    throw Error('Browser startup had errors or sockets; inspect error response');
  respond({ok: true, data: {
    state: await adapter.state(),
    extent: config.extent,
    dpr: await page.evaluate(() => devicePixelRatio),
    importMode: mode,
    requests,
  }});

  stage = 'replay';
  for await (const line of createInterface({input: process.stdin, crlfDelay: Infinity})) {
    if (Buffer.byteLength(line) > MAX_META) throw Error('request exceeds metadata bound');
    const request = JSON.parse(line);
    currentId = request.id;
    const startedNs = process.hrtime.bigint().toString();
    let data;
    if (request.op === 'input') {
      // Single action -> validated pointer/key event sequence.
      const ev = request.event;
      if (ev.kind === 'click') {
        // Click = move + down + up (explicit move first, per adapter rule).
        data = await adapter.input({kind: 'pointer', phase: 'move', x: ev.x, y: ev.y});
        data = await adapter.input({kind: 'pointer', phase: 'down', x: ev.x, y: ev.y, button: ev.button ?? 'left'});
        data = await adapter.input({kind: 'pointer', phase: 'up', x: ev.x, y: ev.y, button: ev.button ?? 'left'});
      } else if (ev.kind === 'key') {
        data = await adapter.input({kind: 'key', phase: 'down', key: ev.key});
        data = await adapter.input({kind: 'key', phase: 'up', key: ev.key});
      } else if (ev.kind === 'wait') {
        await new Promise(r => setTimeout(r, Math.min(ev.wait_ms, 2000)));
        data = {wait_ms: ev.wait_ms};
      } else throw Error('Unsupported action kind');
    } else if (request.op === 'capture') {
      data = await adapter.capture(request.path);
    } else if (request.op === 'summary') {
      data = {state: await adapter.state(), requests, errors, sockets, messages};
    } else if (request.op === 'close') {
      respond({id: request.id, ok: true, data: {closed: true,
        finished_ns: process.hrtime.bigint().toString()}});
      break;
    } else throw Error('Unsupported adapter operation');
    if (errors.length || sockets.length)
      throw Error('Browser observed an error or socket during replay');
    respond({id: request.id, ok: true, data: {...(data ?? {}), started_ns: startedNs,
      finished_ns: process.hrtime.bigint().toString()}});
  }
} catch (error) {
  const message = String(error.message ?? error), code = String(error.code ?? 'JEV_BROWSER_FAILED');
  const diagnostics = Object.entries({errors, sockets, requests, messages});
  respond({...(currentId === null ? {} : {id: currentId}), ok: false, error: {message: message.slice(0, 4000), code: code.slice(0, 128), stage,
    message_truncated: message.length > 4000, code_truncated: code.length > 128,
    ...Object.fromEntries(diagnostics.map(([name, values]) => [name, values.slice(-8)])),
    omitted: Object.fromEntries(diagnostics.map(([name, values]) => [name, Math.max(0, values.length - 8)]))}});
  process.exitCode = 2;
} finally {
  await browser?.close().catch(() => {});
}
