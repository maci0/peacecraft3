// Locally authored synthetic format records only; no original asset bytes.
const text = s => new TextEncoder().encode(s);
const table = new Uint32Array(1280); let seed = 0x100001;
for (let byte = 0; byte < 256; byte++) for (let part = 0; part < 5; part++) {
  seed = (seed * 125 + 3) % 0x2aaaab; const high = (seed & 65535) << 16;
  seed = (seed * 125 + 3) % 0x2aaaab; table[part * 256 + byte] = high | (seed & 65535);
}
function hash(name, part) {
  let a = 0x7fed7fed, b = 0xeeeeeeee;
  for (const c of text(name.replaceAll('/', '\\').toUpperCase())) {
    a = (table[part * 256 + c] ^ ((a + b) >>> 0)) >>> 0;
    b = (c + a + b + (b << 5) + 3) >>> 0;
  }
  return a;
}
function encrypt(bytes, key) {
  const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let a = key, b = 0xeeeeeeee;
  for (let i = 0; i < bytes.length; i += 4) {
    b = (b + table[1024 + (a & 255)]) >>> 0;
    const plain = v.getUint32(i, true); v.setUint32(i, plain ^ ((a + b) >>> 0), true);
    a = (((~a << 21) + 0x11111111) | (a >>> 11)) >>> 0;
    b = (plain + b + (b << 5) + 3) >>> 0;
  }
}
export function mpqFixture(members) {
  const entries = Object.entries(members), slots = 2 ** Math.ceil(Math.log2(Math.max(2, entries.length * 2)));
  const hashes = new Uint8Array(slots * 16); hashes.fill(255);
  const hv = new DataView(hashes.buffer), blocks = new Uint8Array(entries.length * 16), bv = new DataView(blocks.buffer);
  let pos = 40;
  entries.forEach(([name, data], index) => {
    let slot = hash(name, 0) % slots;
    while (hv.getUint32(slot * 16 + 12, true) !== 0xffffffff) slot = (slot + 1) % slots;
    hv.setUint32(slot * 16, hash(name, 1), true); hv.setUint32(slot * 16 + 4, hash(name, 2), true);
    hv.setUint32(slot * 16 + 8, 0, true); hv.setUint32(slot * 16 + 12, index, true);
    [pos, data.length, data.length, 0x81000000].forEach((n, i) => bv.setUint32(index * 16 + i * 4, n, true));
    pos += data.length;
  });
  const hashPos = pos, blockPos = pos + hashes.length, size = blockPos + blocks.length;
  const result = new Uint8Array(size), view = new DataView(result.buffer);
  result.set([77, 80, 81, 26]); view.setUint32(4, 32, true); view.setUint32(8, size, true); result[14] = 3;
  [hashPos, blockPos, slots, entries.length].forEach((n, i) => view.setUint32(16 + i * 4, n, true));
  pos = 40; for (const [, data] of entries) { result.set(data, pos); pos += data.length; }
  encrypt(hashes, hash('(hash table)', 3)); encrypt(blocks, hash('(block table)', 3));
  result.set(hashes, hashPos); result.set(blocks, blockPos); return result;
}
export function wavFixture() {
  const b = new Uint8Array(60), v = new DataView(b.buffer);
  b.set(text('RIFF')); v.setUint32(4, b.length - 8, true); b.set(text('WAVEfmt '), 8);
  v.setUint32(16, 16, true); v.setUint16(20, 1, true); v.setUint16(22, 1, true);
  v.setUint32(24, 8000, true); v.setUint32(28, 16000, true); v.setUint16(32, 2, true); v.setUint16(34, 16, true);
  b.set(text('data'), 36); v.setUint32(40, 16, true); return b;
}
export function clickMetadata(volume = 37) {
  const headers = ['SoundName', 'DirectoryBase', 'FileNames', 'Volume', 'Pitch', 'Flags'];
  const row = ['GlueScreenClick', 'Synthetic\\Audio', 'click.wav', volume, 1, 'NODUPLICATES'];
  return text('ID;PSYNTHETIC\r\n' + [headers, row].flatMap((values, y) => values.map((value, x) =>
    `C;X${x + 1};Y${y + 1};K${typeof value === 'number' ? value : `"${value}"`}`)).join('\r\n') + '\r\nE\r\n');
}
export function menuMembers(musicPath) {
  const texturePath = 'Synthetic\\Texture.blp';
  const model = new Uint8Array(292), v = new DataView(model.buffer);
  model.set(text('MDLXVERS')); v.setUint32(8, 4, true); v.setUint32(12, 800, true);
  model.set(text('TEXS'), 16); v.setUint32(20, 268, true); model.set(text(texturePath), 28);
  const fontPath = 'Synthetic\\Fonts\\SelectedFace.ttf';
  const style = 'Frame "TEXT" "SyntheticTextStyle" {\n FrameFont "SelectedSyntheticFace", 0.017, "",\n FontJustificationH JUSTIFYCENTER,\n FontJustificationV JUSTIFYMIDDLE,\n FontFlags "FIXEDSIZE",\n FontColor 1 0.5 0.25 1,\n FontHighlightColor 1 1 1 1,\n FontDisabledColor 0.5 0.5 0.5 1,\n FontShadowColor 0 0 0 0.8,\n FontShadowOffset 0.002 -0.003,\n}\n';
  const members = {
    'UI\\FrameDef\\GlobalStrings.fdf': text('StringList {\n SYNTHETIC_LABEL "Synthetic label",\n}\n'),
    'UI\\FrameDef\\Glue\\StandardTemplates.fdf': text(style + 'Frame "BACKDROP" "SyntheticBackdrop" {\n BackdropBackground "Synthetic\\Texture.blp",\n}\n'),
    'UI\\FrameDef\\Glue\\MainMenu.fdf': text('Frame "FRAME" "MainMenuFrame" {\n Frame "BUTTON" "SinglePlayerButton" {\n ButtonText "SyntheticSelectedText",\n Frame "TEXT" "SyntheticSelectedText" INHERITS "SyntheticTextStyle" {\n Text "SYNTHETIC_LABEL",\n }\n }\n}\n'),
    'UI\\war3skins.txt': text(`[SyntheticSection]\nSelectedSyntheticFace=${fontPath}\n`),
    [fontPath]: text('synthetic font placeholder: API storage proof only'),
    'UI\\Glues\\MainMenu\\MainMenu3d\\MainMenu3d.mdx': model,
    [texturePath]: new Uint8Array([1, 9, 3, 7]),
    'UI\\SoundInfo\\UISounds.slk': clickMetadata(),
    'Synthetic\\Audio\\click.wav': wavFixture(),
    'Synthetic\\NamedOnly.txt': text('named extraction without a listfile'),
  };
  if (musicPath) members[musicPath] = wavFixture(); // Content, not suffix, decides audio.
  return members;
}
export function isoFixture(files) {
  const S = 2048;
  function both(bytes, at, value, width = 4) {
    const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    if (width === 4) { v.setUint32(at, value, true); v.setUint32(at + 4, value); }
    else { v.setUint16(at, value, true); v.setUint16(at + 2, value); }
  }
  function record(name, extent, size, directory = false) {
    const r = new Uint8Array(33 + name.length + (name.length % 2 === 0 ? 1 : 0));
    r[0] = r.length; both(r, 2, extent); both(r, 10, size); r[25] = directory ? 2 : 0;
    both(r, 28, 1, 2); r[32] = name.length; r.set(text(name), 33); return r;
  }
  let sector = 24;
  const records = files.map(file => { const r = { ...file, extent: sector }; sector += Math.max(1, Math.ceil(file.bytes.length / S)); return r; });
  const b = new Uint8Array(sector * S), pvd = b.subarray(16 * S, 17 * S);
  pvd[0] = 1; pvd.set(text('CD001'), 1); pvd[6] = 1; pvd.fill(32, 40, 72); pvd.set(text('SYNTHETIC-LOCAL'), 40);
  both(pvd, 80, sector); both(pvd, 120, 1, 2); both(pvd, 124, 1, 2); both(pvd, 128, S, 2); pvd.set(record('\0', 20, S, true), 156);
  const end = b.subarray(17 * S, 18 * S); end[0] = 255; end.set(text('CD001'), 1); end[6] = 1;
  let pos = 20 * S;
  for (const r of [record('\0', 20, S, true), record('\x01', 20, S, true), ...records.map(file => record(file.name, file.extent, file.bytes.length))]) { b.set(r, pos); pos += r.length; }
  records.forEach(file => b.set(file.bytes, file.extent * S)); return b;
}
