// Identity gate for the browser session client, run against a session_server it did not configure.
// Pass an installed playwright-core module as argv[2]. The static server and session host from the
// browser session check must already be running. PCX_SESSION_IDENTITY is the counterpart identity,
// from `session_server identity`, never from the host under test.
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';

const { chromium } = await import(pathToFileURL(process.argv[2]).href);
const base = process.argv[3] ?? 'http://127.0.0.1:8931';
const socketUrl = process.argv[4] ?? 'ws://127.0.0.1:8932';
const identity = process.env.PCX_SESSION_IDENTITY ?? (() => { throw new Error('set PCX_SESSION_IDENTITY to the counterpart identity'); })();
// Same shape, one different digit: a page that expects this value must be refused.
const wrong = (identity[0] === '0' ? '1' : '0') + identity.slice(1);
const UNIT_SCALE = 65536;

const browser = await chromium.launch({ executablePath: process.env.CHROMIUM ?? '/usr/bin/chromium', headless: true, args: ['--no-sandbox'] });
try {
  const [matching, mismatched] = await Promise.all([browser.newPage(), browser.newPage()]);
  const pageErrors = [];
  for (const page of [matching, mismatched]) {
    page.on('pageerror', error => pageErrors.push(error.message));
    await page.goto(`${base}/network.js`);
    // Count binary frames a page sends. A refused page must send none.
    await page.evaluate(() => {
      window.binarySends = 0;
      const send = WebSocket.prototype.send;
      WebSocket.prototype.send = function (data) {
        if (typeof data !== 'string') window.binarySends += 1;
        return send.call(this, data);
      };
    });
  }

  await mismatched.evaluate(async ({ base, socketUrl, wrong }) => {
    const { PeacecraftSession } = await import(`${base}/network.js`);
    window.session = new PeacecraftSession(socketUrl, wrong);
    window.events = [];
    session.addEventListener('rejected', ({ detail }) => events.push({ type: 'rejected', detail }));
    session.addEventListener('joined', () => events.push({ type: 'joined' }));
    session.addEventListener('world', () => events.push({ type: 'world' }));
    session.connect();
  }, { base, socketUrl, wrong });
  await mismatched.waitForFunction(() => events.some(event => event.type === 'rejected'));

  assert.deepEqual(
    await mismatched.evaluate(() => events.find(event => event.type === 'rejected').detail),
    { reason: 'identity', expected: wrong, host: identity },
  );
  assert.equal(await mismatched.evaluate(() => session.player), null);
  assert.deepEqual(
    await mismatched.evaluate(() => events.filter(event => event.type !== 'rejected').map(event => event.type)),
    [],
    'a refused page must not join or show a world',
  );
  assert.equal(
    await mismatched.evaluate(() => {
      try { session.commands([{ kind: 'move', x: 0, y: 0 }]); return 'accepted'; } catch (error) { return error.message; }
    }),
    'Session is not ready',
  );
  assert.equal(
    await mismatched.evaluate(() => {
      try { session.resync(); return 'accepted'; } catch (error) { return error.message; }
    }),
    'Session identity is not verified',
  );
  assert.equal(await mismatched.evaluate(() => binarySends), 0, 'a refused page sent a binary datagram');

  // The same-source counterpart joins, and its order moves its unit on the shared host.
  await matching.evaluate(async ({ base, socketUrl, identity }) => {
    const { PeacecraftSession } = await import(`${base}/network.js`);
    window.session = new PeacecraftSession(socketUrl, identity);
    window.worlds = [];
    session.addEventListener('world', ({ detail }) => worlds.push(detail));
    session.connect();
  }, { base, socketUrl, identity });
  await matching.waitForFunction(() => session.player !== null && session.world !== null);
  const before = await matching.evaluate(() => session.world.units.map(({ id, owner, x, y }) => ({ id, owner, x, y })));
  const mine = await matching.evaluate(() => session.player);
  const start = before.find(unit => unit.owner === mine);
  await matching.evaluate(({ x, y, scale }) => session.commands([{ kind: 'move', x: x + 10 * scale, y }]), { x: start.x, y: start.y, scale: UNIT_SCALE });
  await matching.waitForFunction(({ id, x }) => {
    const unit = session.world.units.find(candidate => candidate.id === id);
    return unit.x > x;
  }, { id: start.id, x: start.x });
  const after = await matching.evaluate(() => session.world.units.map(({ id, owner, x, y }) => ({ id, owner, x, y })));
  for (const unit of before) {
    if (unit.id === start.id) continue;
    assert.deepEqual(after.find(candidate => candidate.id === unit.id), unit, `unit ${unit.id} moved without an order`);
  }
  assert.equal(pageErrors.length, 0, pageErrors.join('; '));
  console.log(JSON.stringify({
    refusedReason: 'identity',
    refusedSends: 0,
    matchingPlayer: mine,
    movedUnit: start.id,
    pageErrors: pageErrors.length,
  }));
} finally {
  await browser.close();
}
