// Run after starting session_server and `python3 -m http.server 8931 --directory web`.
// Pass an installed playwright-core module as argv[2]; no test dependency enters the game.
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
const { chromium } = await import(pathToFileURL(process.argv[2]).href);
const base = process.argv[3] ?? 'http://127.0.0.1:8931';
const socketUrl = process.argv[4] ?? 'ws://127.0.0.1:8932';
const browser = await chromium.launch({ executablePath: process.env.CHROMIUM ?? '/usr/bin/chromium', headless: true, args: ['--no-sandbox'] });
try {
  const pages = await Promise.all([browser.newPage(), browser.newPage()]);
  for (const page of pages) {
    await page.goto(`${base}/network.js`);
    await page.evaluate(async ({ base, socketUrl }) => {
      const { PeacecraftSession } = await import(`${base}/network.js`);
      window.session = new PeacecraftSession(socketUrl);
      window.worlds = new Map(); window.rejected = []; window.errors = [];
      session.addEventListener('world', ({ detail }) => {
        worlds.set(detail.tick.toString(), JSON.stringify(detail, (_, v) => typeof v === 'bigint' ? v.toString() : v));
        if (worlds.size > 256) worlds.delete(worlds.keys().next().value);
      });
      session.addEventListener('rejected', ({ detail }) => rejected.push(detail));
      session.addEventListener('error', ({ detail }) => errors.push(detail));
      session.connect();
    }, { base, socketUrl });
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
  console.log(JSON.stringify({ players, commandTick: tick, identicalWorldTick: checkedTick, spoofRejected: true, reconnectWorldTick: compareTick, resyncEqual: true }));
} finally { await browser.close(); }
