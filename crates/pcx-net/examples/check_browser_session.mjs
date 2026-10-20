// Run after starting session_server and `python3 -m http.server 8931 --directory web`.
// Pass an installed playwright-core module as argv[2]; no test dependency enters the game.
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
const { chromium } = await import(pathToFileURL(process.argv[2]).href);
const base = process.argv[3] ?? 'http://127.0.0.1:8931';
const socketUrl = process.argv[4] ?? 'ws://127.0.0.1:8932';
// The expected identity comes from an independent counterpart run, never from the host under test:
// `session_server identity`. The page refuses to show state or send without it.
const identity = process.env.PCX_SESSION_IDENTITY ?? (() => { throw new Error('set PCX_SESSION_IDENTITY to the counterpart identity'); })();
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM ?? '/usr/bin/chromium', headless: true, args: ['--no-sandbox'] });
try {
  const pages = await Promise.all([browser.newPage(), browser.newPage()]);
  const requests = [], sockets = [], pageErrors = [];
  for (const page of pages) {
    page.on('request', request => requests.push({ url: request.url(), method: request.method(), body: request.postData() }));
    page.on('websocket', socket => sockets.push(socket.url()));
    page.on('pageerror', error => pageErrors.push(error.message));
    await page.goto(`${base}/network.js`);
    await page.evaluate(async ({ base, socketUrl, identity }) => {
      const { PeacecraftSession } = await import(`${base}/network.js`);
      window.session = new PeacecraftSession(socketUrl, identity);
      window.worlds = new Map(); window.rejected = []; window.errors = [];
      session.addEventListener('world', ({ detail }) => {
        worlds.set(detail.tick.toString(), JSON.stringify(detail, (_, v) => typeof v === 'bigint' ? v.toString() : v));
        if (worlds.size > 256) worlds.delete(worlds.keys().next().value);
      });
      session.addEventListener('rejected', ({ detail }) => rejected.push(detail));
      session.addEventListener('error', ({ detail }) => errors.push(detail));
      session.connect();
    }, { base, socketUrl, identity });
    await page.waitForFunction(() => session.player !== null && session.world !== null);
  }
  const [a, b] = pages;
  const players = await Promise.all(pages.map(page => page.evaluate(() => session.player)));
  assert.equal(new Set(players).size, 2);
  const initial = await a.evaluate(() => session.world.units);
  const ticks = await Promise.all(pages.map(page => page.evaluate(() => session.world.tick.toString())));
  const tick = (ticks.map(BigInt).reduce((a, b) => a > b ? a : b) + 12n).toString();
  for (const [index, page] of pages.entries()) {
    const start = initial.find(u => u.owner === players[index]).x;
    await page.evaluate(({ tick, x }) => session.commands([{ kind: 'move', x, y: 0 }], tick), { tick, x: start + (index === 0 ? 10 : -10) * 65536 });
  }
  const checkedTick = (BigInt(tick) + 4n).toString();
  for (const page of pages) await page.waitForFunction(tick => worlds.has(tick), checkedTick);
  const states = await Promise.all(pages.map(page => page.evaluate(tick => worlds.get(tick), checkedTick)));
  assert.equal(states[0], states[1], 'both browsers must receive identical authority');
  const world = JSON.parse(states[0]);
  assert.ok(world.units.find(u => u.owner === players[0]).x > initial.find(u => u.owner === players[0]).x);
  assert.ok(world.units.find(u => u.owner === players[1]).x < initial.find(u => u.owner === players[1]).x);

  // Force a selection trailer, then return to the legacy all-selected payload.
  const deselectTick = (BigInt(await a.evaluate(() => session.world.tick.toString())) + 12n).toString();
  await a.evaluate(tick => session.commands([
    { kind: 'stop' },
    { kind: 'select', x: 100 * 65536, y: 100 * 65536 },
    { kind: 'move', x: 20 * 65536, y: 0 },
  ], tick), deselectTick);
  const deselectedTick = (BigInt(deselectTick) + 2n).toString();
  for (const page of pages) await page.waitForFunction(tick => worlds.has(tick), deselectedTick);
  const deselected = await Promise.all(pages.map(page => page.evaluate(tick => worlds.get(tick), deselectedTick)));
  assert.equal(deselected[0], deselected[1], 'selection trailer must reach both browsers');
  const held = JSON.parse(deselected[0]).units.find(unit => unit.owner === players[0]);
  assert.equal(held.selected, false);
  assert.equal(held.moveTarget, null, 'move after an empty selection must not move the unit');
  const heldTick = (BigInt(deselectedTick) + 3n).toString();
  for (const page of pages) await page.waitForFunction(tick => worlds.has(tick), heldTick);
  for (const page of pages) {
    const unit = JSON.parse(await page.evaluate(tick => worlds.get(tick), heldTick)).units.find(unit => unit.owner === players[0]);
    assert.equal(unit.x, held.x);
    assert.equal(unit.selected, false);
  }
  const reselectTick = (BigInt(await a.evaluate(() => session.world.tick.toString())) + 12n).toString();
  await a.evaluate(({ tick, x, y }) => session.commands([
    { kind: 'select', x, y },
    { kind: 'move', x: x + 65536, y },
  ], tick), { tick: reselectTick, x: held.x, y: held.y });
  const reselectedTick = (BigInt(reselectTick) + 2n).toString();
  for (const page of pages) await page.waitForFunction(tick => worlds.has(tick), reselectedTick);
  const reselected = await Promise.all(pages.map(page => page.evaluate(tick => worlds.get(tick), reselectedTick)));
  assert.equal(reselected[0], reselected[1]);
  const moving = JSON.parse(reselected[0]).units.find(unit => unit.owner === players[0]);
  assert.equal(moving.selected, true);
  assert.ok(moving.x > held.x, 'reselected unit must accept a later move');

  // Forge a valid codec packet for the other player's slot; the server must reject it.
  await a.evaluate(player => {
    const bytes = new Uint8Array(11); const view = new DataView(bytes.buffer);
    bytes[0] = 1; view.setBigUint64(1, session.world.tick + 4n, true); bytes[9] = player; bytes[10] = 3;
    session.socket.send(bytes);
  }, players[1]);
  await a.waitForFunction(() => rejected.some(e => e.includes('assigned')));
  await a.evaluate(() => session.disconnect());
  await a.waitForFunction(() => session.player === null);
  await b.waitForFunction(tick => session.world.tick > BigInt(tick) + 8n, checkedTick);
  const stopped = await b.evaluate(player => session.world.units.find(u => u.owner === player).x, players[0]);
  const disconnectedTick = await b.evaluate(() => session.world.tick.toString());
  await b.waitForFunction(tick => session.world.tick > BigInt(tick) + 5n, disconnectedTick);
  assert.equal(await b.evaluate(player => session.world.units.find(u => u.owner === player).x, players[0]), stopped, 'disconnected player must stop moving');
  await a.evaluate(() => { worlds.clear(); session.connect(); });
  await a.waitForFunction(() => session.player !== null && session.world !== null);
  assert.equal(await a.evaluate(() => session.player), players[0], 'reconnect must preserve the player slot');
  assert.ok(BigInt(await a.evaluate(() => session.world.tick.toString())) >= BigInt(disconnectedTick));
  await a.evaluate(() => session.resync());
  const compareTick = (BigInt(await a.evaluate(() => session.world.tick.toString())) + 5n).toString();
  for (const page of pages) await page.waitForFunction(tick => worlds.has(tick), compareTick);
  const resynced = await Promise.all(pages.map(page => page.evaluate(tick => worlds.get(tick), compareTick)));
  assert.equal(resynced[0], resynced[1], 'reconnected browser must share current authority');
  for (const page of pages) assert.deepEqual(await page.evaluate(() => errors), []);
  assert.deepEqual(pageErrors, []);
  assert.ok(requests.length > 0);
  for (const request of requests) {
    const url = new URL(request.url);
    assert.equal(url.origin, new URL(base).origin);
    assert.ok(['/network.js', '/favicon.ico'].includes(url.pathname), `unexpected HTTP input ${url.pathname}`);
    assert.equal(request.method, 'GET');
    assert.equal(request.body, null);
  }
  assert.ok(sockets.length >= 3, 'both connections and reconnect must be observed');
  assert.ok(sockets.every(url => new URL(url).href === new URL(socketUrl).href));
  console.log(JSON.stringify({ players, commandTick: tick, identicalWorldTick: checkedTick, spoofRejected: true, reconnectWorldTick: compareTick, resyncEqual: true, selectionTrailerEqual: true, emptySelectionHeld: true, reselectedMoved: true, httpGets: requests.length, unexpectedHttpRequests: 0, webSocketConnections: sockets.length, pageErrors: pageErrors.length }));
} finally { await browser.close(); }
