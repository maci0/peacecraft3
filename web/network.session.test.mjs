// Deterministic socket schedules for PeacecraftSession. A fake WebSocket delivers each event
// on demand, so refusal, queued and late deliveries happen in a fixed order.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { PeacecraftSession } from './network.js';

const golden = JSON.parse(await readFile(new URL('./snapshot-selection-golden.json', import.meta.url), 'utf8'));
const snapshotBytes = () => new Uint8Array(Buffer.from(golden.accepted[0].bytes, 'hex')).buffer;

const IDENTITY = '00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff';
const OTHER = 'ffeeddccbbaa99887766554433221100ffeeddccbbaa99887766554433221100';
const TOKEN = '0123456789abcdef0123456789abcdef';

class FakeSocket extends EventTarget {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;
  static made = [];
  constructor(url) {
    super();
    this.url = url;
    this.readyState = FakeSocket.CONNECTING;
    this.bufferedAmount = 0;
    this.binaryType = 'blob';
    this.sent = [];
    this.closed = null;
    FakeSocket.made.push(this);
  }
  send(data) {
    if (this.readyState !== FakeSocket.OPEN) return;
    this.sent.push(data);
  }
  close(code, reason) {
    this.closed = { code, reason };
    this.readyState = FakeSocket.CLOSED;
  }
  // Deliver one event at a time; each caller decides the order.
  open() {
    this.readyState = FakeSocket.OPEN;
    this.dispatchEvent(new Event('open'));
  }
  message(data) {
    this.dispatchEvent(Object.assign(new Event('message'), { data }));
  }
}

const binarySends = socket => socket.sent.filter(data => typeof data !== 'string').length;

function harness(identity = IDENTITY) {
  FakeSocket.made = [];
  globalThis.WebSocket = FakeSocket;
  const session = new PeacecraftSession('ws://fake.test', identity, TOKEN);
  const events = [];
  for (const type of ['joined', 'world', 'rejected', 'error']) {
    session.addEventListener(type, ({ detail }) => events.push({ type, detail }));
  }
  return { session, events, socket: () => FakeSocket.made.at(-1) };
}

const hello = (player, session = IDENTITY) => JSON.stringify({ player, tickMs: 25, session });

test('a mismatched greeting is refused, closed with 4001 and sends nothing', () => {
  const { session, events, socket } = harness();
  session.connect();
  socket().open();
  assert.equal(socket().sent[0], TOKEN, 'the token is the only first message');
  socket().message(hello(0, OTHER));
  assert.deepEqual(events, [{ type: 'rejected', detail: { reason: 'identity', expected: IDENTITY, host: OTHER } }]);
  assert.deepEqual(socket().closed, { code: 4001, reason: 'Session identity mismatch' });
  assert.equal(session.player, null);
  assert.throws(() => session.commands([{ kind: 'stop' }]), /Session is not ready/);
  assert.throws(() => session.resync(), /Session identity is not verified/);
  assert.equal(binarySends(socket()), 0);
});

test('a greeting without an identity is refused like a mismatched one', () => {
  const { session, events, socket } = harness();
  session.connect();
  socket().open();
  socket().message(JSON.stringify({ player: 0, tickMs: 25 }));
  assert.equal(events[0].detail.host, null);
  assert.equal(events[0].detail.reason, 'identity');
  assert.equal(binarySends(socket()), 0);
});

test('a snapshot queued before the greeting is not shown and sends nothing', () => {
  const { session, events, socket } = harness();
  session.connect();
  socket().open();
  socket().message(snapshotBytes());
  assert.equal(session.world, null);
  assert.deepEqual(events, []);
  socket().message(hello(0));
  socket().message(snapshotBytes());
  assert.deepEqual(events.map(event => event.type), ['joined', 'world']);
  assert.equal(binarySends(socket()), 0, 'verification alone sends no datagram');
});

test('a verified page sends exactly the commands it issues', () => {
  const { session, socket } = harness();
  session.connect();
  socket().open();
  socket().message(hello(0));
  socket().message(snapshotBytes());
  session.commands([{ kind: 'stop' }]);
  assert.equal(binarySends(socket()), 1);
});

test('a late greeting from a replaced socket cannot verify the new one', () => {
  const { session, events, socket } = harness();
  session.connect();
  const first = socket();
  first.open();
  first.message(hello(0, OTHER));
  // A reconnect after refusal replaces the socket; the old socket's late greeting must be ignored.
  session.connect();
  const second = socket();
  first.message(hello(0));
  assert.equal(session.player, null);
  assert.equal(events.filter(event => event.type === 'joined').length, 0);
  assert.throws(() => session.commands([{ kind: 'stop' }]), /Session is not ready/);
  second.open();
  second.message(hello(1, OTHER));
  assert.equal(events.at(-1).type, 'rejected');
  assert.equal(binarySends(first) + binarySends(second), 0);
});
