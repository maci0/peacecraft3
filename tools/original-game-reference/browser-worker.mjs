import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createInterface} from 'node:readline';
import {pathToFileURL} from 'node:url';
import {createBrowserAdapter, isConsoleError} from './browser-adapter.mjs';

const maximumMetadata = 2 * 1024 * 1024;
const respond = message => {
  const line = JSON.stringify(message);
  if (Buffer.byteLength(line) > maximumMetadata) throw Error('Adapter response exceeds metadata bound');
  process.stdout.write(line + '\n');
};
const config = JSON.parse(await readFile(process.argv[2], 'utf8'));
const {chromium} = await import(pathToFileURL(config.playwrightModule).href);
const manifest = JSON.parse(await readFile(config.manifest, 'utf8'));
const origin = new URL(config.url).origin;
const requests = [], errors = [], sockets = [], messages = [];
let browser;
let stage = 'launch';
try {
  browser = await chromium.launch({executablePath: config.chromium,
    args: ['--no-sandbox', '--enable-unsafe-swiftshader']});
  const [width, height] = config.extent;
  const context = await browser.newContext({viewport: {width, height}, deviceScaleFactor: 1, serviceWorkers: 'block'});
  await context.routeWebSocket('**/*', socket => {
    append(sockets, socket.url());
    socket.close();
  });
  const page = await context.newPage();
  page.setDefaultTimeout(60000);
  const append = (list, value) => {
    if (list.length >= 1024 || JSON.stringify(value).length > 16384)
      throw Error('Browser observation exceeds bounded collection policy');
    list.push(value);
  };
  await page.route('**/*', async route => {
    try {
      const request = route.request(), url = new URL(request.url());
      const name = url.pathname.slice(1);
      const expected = Object.hasOwn(manifest.files, name) && manifest.files[name];
      if (request.method() !== 'GET' || request.postDataBuffer() || url.origin !== origin ||
          !expected || !Number.isSafeInteger(expected.bytes) || expected.bytes < 0 ||
          expected.bytes > 72 * 1024 * 1024 || !/^[a-f0-9]{64}$/.test(expected.sha256)) {
        append(errors, `Blocked request outside code manifest: ${request.method()} ${url.pathname}`);
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
  await page.waitForFunction(() => globalThis.peacecraft3?.menu_state && globalThis.peacecraft3Settings);
  const revision = await page.evaluate(() => peacecraft3.asset_revision());
  stage = 'import';
  await page.locator('#iso').setInputFiles(config.iso);
  await page.waitForFunction(() => document.querySelectorAll('#iso-candidates input').length > 0 && !document.querySelector('#iso').disabled);
  if (await page.locator('#iso-candidates input').count() !== 1)
    throw Error('Comparison import requires one caller-prepared source closure; ambiguous candidates refused');
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
  // Only the host page is positioned: game layout and framebuffer pixels are unchanged.
  await page.evaluate(([w, h]) => {
    const stage = document.querySelector('#stage');
    Object.assign(stage.style, {position: 'fixed', left: '0', top: '0', zIndex: '2147483647',
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
  const adapter = createBrowserAdapter({page, cdp: await page.context().newCDPSession(page), width, height});
  stage = 'focus';
  await adapter.focus();
  if (errors.length || sockets.length) throw Error('Browser startup had errors or sockets; inspect adapter error response');
  respond({ok: true, data: {state: await adapter.state(), extent: config.extent,
    dpr: await page.evaluate(() => devicePixelRatio), rng: 'uncontrolled', requests}});
  stage = 'replay';
  for await (const line of createInterface({input: process.stdin, crlfDelay: Infinity})) {
    if (Buffer.byteLength(line) > maximumMetadata) throw Error('Adapter request exceeds metadata bound');
    const request = JSON.parse(line);
    const startedNs = process.hrtime.bigint().toString();
    let data;
    if (request.op === 'input') data = await adapter.input(request.event);
    else if (request.op === 'capture') data = await adapter.capture(request.path);
    else if (request.op === 'summary') data = {state: await adapter.state(), requests, errors, sockets, messages};
    else throw Error('Unsupported adapter operation');
    if (errors.length || sockets.length) throw Error('Browser observed an error or socket during replay');
    respond({id: request.id, ok: true, data: {...(data ?? {}), started_ns: startedNs,
      finished_ns: process.hrtime.bigint().toString()}});
  }
} catch (error) {
  const message = String(error.message ?? error), code = String(error.code ?? 'BROWSER_REPLAY_FAILED');
  // Eight bounded records per collection fit the metadata ceiling even with UTF-8 expansion.
  const diagnostics = Object.entries({errors, sockets, requests, messages});
  respond({ok: false, error: {message: message.slice(0, 4000), code: code.slice(0, 128), stage,
    message_truncated: message.length > 4000, code_truncated: code.length > 128,
    ...Object.fromEntries(diagnostics.map(([name, values]) => [name, values.slice(-8)])),
    omitted: Object.fromEntries(diagnostics.map(([name, values]) => [name, Math.max(0, values.length - 8)]))}});
  process.exitCode = 2;
} finally {
  await browser?.close();
}
