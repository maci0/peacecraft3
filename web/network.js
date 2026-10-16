// Binary transport for pcx-net's authoritative movement simulation.
const FX_ONE = 65536;
const LIMIT = 16000 * FX_ONE;

function datagram(kind, tick, tail = new Uint8Array()) {
  const bytes = new Uint8Array(9 + tail.length);
  bytes[0] = kind;
  new DataView(bytes.buffer).setBigUint64(1, BigInt(tick), true);
  bytes.set(tail, 9);
  return bytes;
}

export function decodeSnapshot(buffer) {
  const view = new DataView(buffer);
  if (view.byteLength < 17 || view.getUint8(0) !== 3) throw new Error('Invalid world snapshot');
  const tick = view.getBigUint64(1, true);
  const nextUnitId = view.getUint32(9, true);
  const count = view.getUint32(13, true);
  if (count > 4096) throw new Error('Too many units in snapshot');
  let at = 17;
  const units = [];
  for (let i = 0; i < count; i++) {
    if (at + 14 > view.byteLength) throw new Error('Truncated unit');
    const unit = { id: view.getUint32(at, true), owner: view.getUint8(at + 4), x: view.getInt32(at + 5, true), y: view.getInt32(at + 9, true), moveTarget: null };
    const target = view.getUint8(at + 13);
    at += 14;
    if (target) {
      if (at + 8 > view.byteLength) throw new Error('Truncated movement target');
      unit.moveTarget = [view.getInt32(at, true), view.getInt32(at + 4, true)];
      at += 8;
    }
    unit.selected = true;
    units.push(unit);
  }
  const flags = selectionFlags(view, at, count);
  if (flags) for (let i = 0; i < units.length; i++) units[i].selected = flags[i];
  return { tick, nextUnitId, units };
}

// Version 0 ends after the units and means every unit is selected. Version 1
// is the ASCII tag SEL, a version byte of 1, and one 0 or 1 flag per unit in
// the same order. Anything else is refused whole; flags are never dropped.
function selectionFlags(view, at, count) {
  if (at === view.byteLength) return null;
  const tag = at + 3 <= view.byteLength && view.getUint8(at) === 0x53 && view.getUint8(at + 1) === 0x45 && view.getUint8(at + 2) === 0x4c;
  if (!tag) throw new Error('Trailing snapshot bytes');
  if (at + 4 > view.byteLength) throw new Error('Truncated selection');
  const version = view.getUint8(at + 3);
  if (version !== 1) throw new Error('Unsupported selection version');
  const flagsAt = at + 4;
  if (flagsAt + count > view.byteLength) throw new Error('Truncated selection');
  if (flagsAt + count !== view.byteLength) throw new Error('Trailing snapshot bytes');
  const flags = [];
  for (let i = 0; i < count; i++) {
    const flag = view.getUint8(flagsAt + i);
    if (flag !== 0 && flag !== 1) throw new Error('Bad selection flag');
    flags.push(flag === 1);
  }
  return flags;
}

export class PeacecraftSession extends EventTarget {
  constructor(url, token = null) {
    super();
    this.url = url;
    this.token = token ?? Array.from(crypto.getRandomValues(new Uint8Array(16)), x => x.toString(16).padStart(2, '0')).join('');
    if (!/^[0-9a-f]{32}$/i.test(this.token)) throw new Error('Invalid reconnect token');
    this.socket = null;
    this.player = null;
    this.world = null;
    this.lastInputTick = -1n;
  }

  connect() {
    if (this.socket && this.socket.readyState < WebSocket.CLOSING) throw new Error('Already connected');
    this.player = null;
    this.world = null;
    this.lastInputTick = -1n;
    const socket = new WebSocket(this.url);
    this.socket = socket;
    socket.binaryType = 'arraybuffer';
    socket.addEventListener('open', () => socket.send(this.token));
    socket.addEventListener('message', ({ data }) => {
      if (socket !== this.socket) return;
      try {
        if (typeof data === 'string') {
          const hello = JSON.parse(data);
          if (hello.error) { this.dispatchEvent(new CustomEvent('rejected', { detail: hello.error })); return; }
          if (!Number.isInteger(hello.player) || hello.player < 0 || hello.player >= 16) throw new Error('Invalid player slot');
          this.player = hello.player;
          this.dispatchEvent(new CustomEvent('joined', { detail: hello }));
        } else {
          const world = decodeSnapshot(data);
          if (this.world && world.tick < this.world.tick) return;
          this.world = world;
          this.dispatchEvent(new CustomEvent('world', { detail: world }));
        }
      } catch (error) {
        this.dispatchEvent(new CustomEvent('error', { detail: error.message }));
        socket.close(1002, 'Invalid server packet');
      }
    });
    socket.addEventListener('close', () => { if (socket === this.socket) { this.player = null; this.dispatchEvent(new Event('disconnected')); } });
    socket.addEventListener('error', () => this.dispatchEvent(new CustomEvent('error', { detail: 'Session connection failed' })));
  }

  disconnect() { this.socket?.close(); }

  resync() { this.#send(datagram(4, this.world?.tick ?? 0n)); }

  // Coordinates are fixed-point simulation units, not canvas pixels.
  commands(commands, tick = null) {
    if (this.player === null || !this.world) throw new Error('Session is not ready');
    tick = tick === null ? ((this.world.tick + 4n) > this.lastInputTick ? this.world.tick + 4n : this.lastInputTick + 1n) : BigInt(tick);
    if (tick <= this.lastInputTick) throw new Error('Already submitted this tick; batch commands together');
    if (commands.length > 32) throw new Error('Too many commands');
    const bytes = [this.player];
    for (const command of commands) {
      const kind = { nop: 0, select: 1, move: 2, stop: 3 }[command.kind];
      if (kind === undefined) throw new Error('Unknown command');
      bytes.push(kind);
      if (kind === 1 || kind === 2) {
        const coordinates = new Uint8Array(8);
        const view = new DataView(coordinates.buffer);
        for (const [i, value] of [command.x, command.y].entries()) {
          if (!Number.isInteger(value) || Math.abs(value) > LIMIT) throw new Error('Invalid fixed-point coordinate');
          view.setInt32(i * 4, value, true);
        }
        bytes.push(...coordinates);
      }
    }
    this.#send(datagram(1, tick, new Uint8Array(bytes)));
    this.lastInputTick = tick;
    return tick;
  }

  #send(bytes) {
    if (this.socket?.readyState !== WebSocket.OPEN) throw new Error('Session is disconnected');
    if (this.socket.bufferedAmount > 65536) throw new Error('Session connection is too slow');
    this.socket.send(bytes);
  }
}
