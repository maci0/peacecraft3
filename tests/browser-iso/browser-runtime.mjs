import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { fileURLToPath, pathToFileURL } from 'node:url';
import assert from 'node:assert/strict';
import { wavFixture } from './fixtures.mjs';
const root = fileURLToPath(new URL('../../', import.meta.url));
const playwrightPath = process.env.PCX_PLAYWRIGHT_MODULE ?? `${root}.scratch/browser-check/node_modules/playwright-core/index.mjs`;
const { chromium } = await import(pathToFileURL(playwrightPath));
const allowed = new Set([
  '/tests/browser-iso/runtime.html', '/tests/browser-iso/runtime-worker.mjs', '/tests/browser-iso/fixtures.mjs',
  '/web/iso.js', '/web/iso/import.js', '/web/iso/audio.js', '/web/menu-audio.js',
  '/web/pkg/peacecraft3.js', '/web/pkg/peacecraft3_bg.wasm',
  // Existing audio module's ordinary static presentation sources, before replacement.
  '/web/assets/menu-music.mp3', '/web/assets/menu-click.wav', '/tests/browser-iso/assets/menu-music.mp3', '/tests/browser-iso/assets/menu-click.wav', '/favicon.ico',
]);
const received = [];
const server = createServer(async (req, res) => {
  received.push({ method: req.method, path: req.url });
  if (req.method !== 'GET' || !allowed.has(req.url)) { res.writeHead(403).end(); return; }
  try {
    if (req.url.endsWith('/assets/menu-music.mp3') || req.url.endsWith('/assets/menu-click.wav')) {
      res.writeHead(200, { 'Content-Type': 'audio/wav' }).end(wavFixture()); return;
    }
    const data = await readFile(`${root}${req.url.slice(1)}`);
    const type = req.url.endsWith('.wasm') ? 'application/wasm' : req.url.endsWith('.html') ? 'text/html' : req.url.endsWith('.mp3') ? 'audio/mpeg' : req.url.endsWith('.wav') ? 'audio/wav' : 'text/javascript';
    res.writeHead(200, { 'Content-Type': type }).end(data);
  } catch { res.writeHead(404).end(); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const origin = `http://127.0.0.1:${server.address().port}`;
const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: true, args: ['--no-sandbox'] });
try {
  const context = await browser.newContext();
  const page = await context.newPage(); const requests = [], sockets = [];
  context.on('request', request => requests.push({ url: request.url(), method: request.method(), hasBody: request.postDataBuffer() !== null }));
  page.on('websocket', socket => sockets.push(socket.url()));
  await page.addInitScript(() => {
    const create = URL.createObjectURL.bind(URL), revoke = URL.revokeObjectURL.bind(URL);
    window.audioUrlProof = { created: [], revoked: [], active: new Set() };
    URL.createObjectURL = blob => { const url = create(blob); audioUrlProof.created.push(url); audioUrlProof.active.add(url); return url; };
    URL.revokeObjectURL = url => { audioUrlProof.revoked.push(url); audioUrlProof.active.delete(url); revoke(url); };
  });
  await page.goto(`${origin}/tests/browser-iso/runtime.html`);
  const result = await page.evaluate(() => window.isoRuntimeResult);
  assert.equal(result.ok, true, JSON.stringify(result));
  const audio = await page.evaluate(async result => {
    const source = { music: { blob: new Blob([new Uint8Array(result.audio.music)]), path: result.audio.musicPath, provenance: result.result.provenance },
      click: { ...result.audio.click, blob: new Blob([new Uint8Array(result.audio.clickBytes)]) } };
    const api = globalThis.peacecraft3Audio;
    document.querySelector('#sound').click(); // Preserve mute across source replacement.
    await api.setLocalAssets(source); const muted = api.status();
    api.pauseForIntro(); await api.setLocalAssets(source); const intro = api.status();
    api.resumeAfterIntro(); document.querySelector('#sound').click();
    document.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true })); api.click();
    await new Promise(resolve => setTimeout(resolve, 100)); const playing = api.status();
    await api.setLocalAssets(source); const replaced = api.status();
    const beforeUnsupported = api.status().localUrlCount;
    let unsupported; try { await api.setLocalAssets({ click: { ...source.click, blob: new Blob(['unknown codec']) } }); } catch (error) { unsupported = String(error); }
    const afterUnsupported = api.status().localUrlCount;
    api.resetLocalAssets(); const reset = api.status();
    return { muted, intro, playing, replaced, beforeUnsupported, afterUnsupported, unsupported, reset, urls: { created: audioUrlProof.created.length, revoked: audioUrlProof.revoked.length, active: audioUrlProof.active.size } };
  }, result);
  assert.equal(audio.muted.muted, true); assert.equal(audio.muted.localUrlCount, 2);
  assert.equal(audio.intro.introActive, true); assert.equal(audio.intro.musicPaused, true);
  assert.equal(audio.playing.localSources.click.path, 'Synthetic\\Audio\\click.wav');
  assert.equal(audio.playing.clickVolume, 37 / 127); assert.ok(audio.playing.clicks >= 1);
  assert.equal(audio.replaced.localUrlCount, 2); assert.match(audio.unsupported, /UNSUPPORTED_LOCAL_AUDIO/);
  assert.equal(audio.afterUnsupported, audio.beforeUnsupported);
  assert.equal(audio.reset.localUrlCount, 0); assert.equal(audio.reset.musicPaused, true); assert.equal(audio.reset.clickPaused, true);
  assert.equal(audio.urls.active, 0); assert.equal(audio.urls.revoked, audio.urls.created);
  await page.waitForTimeout(100);
  const unexpected = requests.filter(r => !r.url.startsWith('blob:') && (!r.url.startsWith(origin) || !allowed.has(new URL(r.url).pathname) || r.method !== 'GET' || r.hasBody));
  assert.deepEqual(unexpected, []); assert.deepEqual(sockets, []);
  assert.ok(received.every(r => r.method === 'GET' && allowed.has(r.path)));
  console.log(JSON.stringify({ ok: true, actualWasmChecks: result.checks, selftest: result.selftest, revision: result.result.revision,
    audioLifecycle: 'mute/intro/click/content rejection/reset passed', network: { requests, serverRequests: received, uploads: 0, unexpected: 0, sockets: 0 },
    limitation: 'Synthetic API/storage/audio proof; placeholder font/texture/model do not prove rendering or original fidelity' }, null, 2));
} finally { await browser.close(); await new Promise(resolve => server.close(resolve)); }
