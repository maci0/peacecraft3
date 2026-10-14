// Checked FreeArc 0.67 control records. Original data is read only from a Blob.
export class ArcError extends Error {
  constructor(code, message, offset) {
    super(`${code}: ${message}${offset === undefined ? '' : ` at archive byte ${offset}`}`);
    this.name = 'ArcError'; this.code = code; this.offset = offset;
  }
}
const fail = (code, message, offset) => { throw new ArcError(code, message, offset); };
export const DEFAULT_LIMITS = Object.freeze({ controlBytes: 8 * 1024 * 1024, dictionaryBytes: 128 * 1024 * 1024,
  entries: 100000, blocks: 10000, stringBytes: 4096, iterations: 1000000, decodedBlockBytes: 2 * 1024 * 1024 * 1024 });
export function checkedLimits(overrides = {}) {
  const limits = { ...DEFAULT_LIMITS };
  for (const [key, value] of Object.entries(overrides)) {
    if (!(key in limits) || !Number.isSafeInteger(value) || value < 1) throw new TypeError(`Invalid FreeArc limit ${key}`);
    limits[key] = value;
  }
  return limits;
}
export function boundedRange(offset, length, end) {
  if (![offset, length, end].every(Number.isSafeInteger) || offset < 0 || length < 0 || end < 0 || offset > end - length)
    fail('ARC_BOUNDS', 'range exceeds archive', offset);
}
const crcTable = Uint32Array.from({ length: 256 }, (_, n) => {
  let value = n;
  for (let bit = 0; bit < 8; bit++) value = (value >>> 1) ^ (0xedb88320 & -(value & 1));
  return value >>> 0;
});
export function crc32(bytes, previous = 0) {
  let crc = ~previous;
  for (const byte of bytes) crc = (crc >>> 8) ^ crcTable[(crc ^ byte) & 255];
  return ~crc >>> 0;
}
export class ArcCursor {
  constructor(bytes, limits = checkedLimits()) { this.bytes = bytes; this.at = 0; this.limits = limits; }
  take(size) { boundedRange(this.at, size, this.bytes.length); const bytes = this.bytes.subarray(this.at, this.at + size); this.at += size; return bytes; }
  u8() { return this.take(1)[0]; }
  u32() { const b = this.take(4); return new DataView(b.buffer, b.byteOffset, 4).getUint32(0, true); }
  integer() {
    const first = this.take(1)[0]; this.at--;
    let width = 9;
    for (let n = 1; n <= 8; n++) if ((first & ((1 << n) - 1)) === (1 << (n - 1)) - 1) { width = n; break; }
    const bytes = this.take(width); let value = 0n;
    for (let i = width - 1; i >= (width === 9 ? 1 : 0); i--) value = value * 256n + BigInt(bytes[i]);
    if (width !== 9) value >>= BigInt(width);
    if (value > BigInt(Number.MAX_SAFE_INTEGER)) fail('ARC_INTEGER', 'integer exceeds safe address range', this.at - width);
    return Number(value);
  }
  count(maximum, name) {
    const value = this.integer();
    if (value > maximum) fail('ARC_LIMIT', `${name} count exceeds configured limit`, this.at);
    return value;
  }
  string() {
    const end = this.bytes.indexOf(0, this.at);
    if (end < 0) fail('ARC_STRING', 'unterminated string', this.at);
    if (end - this.at > this.limits.stringBytes) fail('ARC_LIMIT', 'string exceeds configured limit', this.at);
    const bytes = this.take(end - this.at + 1).subarray(0, -1);
    if (bytes.some(b => b > 127 || b < 32)) fail('ARC_CHARACTER_SET', 'non-ASCII control-record string is unsupported', this.at - bytes.length - 1);
    return String.fromCharCode(...bytes);
  }
}
const signature = bytes => bytes.length >= 4 && bytes[0] === 65 && bytes[1] === 114 && bytes[2] === 67 && bytes[3] === 1;
export async function readArcBytes(blob, offset, length, { signal, limit = DEFAULT_LIMITS.controlBytes } = {}) {
  signal?.throwIfAborted(); boundedRange(offset, length, blob.size);
  if (length > limit) fail('ARC_LIMIT', 'read exceeds configured buffer limit', offset);
  const bytes = new Uint8Array(await blob.slice(offset, offset + length).arrayBuffer());
  signal?.throwIfAborted();
  if (bytes.length !== length) fail('ARC_TRUNCATED', 'short local Blob read', offset);
  return bytes;
}
export async function footerDescriptor(blob, options = {}) {
  const limits = checkedLimits(options.limits);
  const length = Math.min(blob.size, 4096);
  const tail = await readArcBytes(blob, blob.size - length, length, { signal: options.signal, limit: limits.controlBytes });
  let found = -1;
  for (let at = tail.length - 4; at >= 0; at--) if (signature(tail.subarray(at))) { found = at; break; }
  if (found < 0) fail('ARC_FOOTER', 'footer signature not found');
  const record = tail.subarray(found), offset = blob.size - length + found;
  if (record.length < 8) fail('ARC_FOOTER', 'truncated footer descriptor', offset);
  const body = record.subarray(0, -4), expected = new DataView(record.buffer, record.byteOffset + record.length - 4, 4).getUint32(0, true);
  if (crc32(body) !== expected) fail('ARC_CRC', 'footer descriptor checksum mismatch', offset);
  const cursor = new ArcCursor(body, limits); cursor.take(4);
  const type = cursor.integer(), method = cursor.string(), originalSize = cursor.integer(), storedSize = cursor.integer(), crc = cursor.u32();
  if (type !== 4 || !originalSize || !storedSize || cursor.at !== body.length) fail('ARC_FOOTER', 'unsupported footer descriptor shape', offset);
  boundedRange(offset - storedSize, storedSize, blob.size);
  if (originalSize > limits.controlBytes || storedSize > limits.controlBytes) fail('ARC_LIMIT', 'footer size exceeds configured control budget', offset);
  return { type, method, originalSize, storedSize, crc, offset: offset - storedSize, descriptorOffset: offset };
}
export function redactMethod(method) { return method.replace(/:([skci])[^:+]*/g, ':$1<redacted>'); }
function hex(value, name) {
  if (!value || value.length % 2 || !/^[0-9a-f]+$/i.test(value)) fail('ARC_CIPHER', `invalid ${name} encoding`);
  return Uint8Array.from(value.match(/../g), b => parseInt(b, 16));
}
function legacyHex(bytes) {
  const nibble = n => n > 9 ? n - 10 : n;
  return Uint8Array.from(bytes, b => nibble(b >> 4) * 16 + nibble(b & 15));
}
export function cipherParameters(method, limits = checkedLimits()) {
  const [name, ...parts] = method.split(':');
  if (name !== 'aes-256/ctr') fail('ARC_CIPHER', 'unsupported cipher profile');
  const params = new Map();
  for (const part of parts) {
    const key = part[0];
    if (!['n', 'r', 's', 'c', 'i', 'f'].includes(key) || params.has(key) || key === 'f' && part !== 'f') fail('ARC_CIPHER', 'unsupported or repeated cipher parameter');
    params.set(key, part.slice(1));
  }
  const iterations = Number(params.get('n'));
  if (!/^\d+$/.test(params.get('n') ?? '') || !Number.isSafeInteger(iterations) || iterations < 1 || iterations > limits.iterations) fail('ARC_LIMIT', 'key derivation iteration count exceeds supported budget');
  if (params.get('r') !== '0') fail('ARC_CIPHER', 'nondefault AES rounds unsupported');
  const salt = hex(params.get('s'), 'salt'), check = hex(params.get('c'), 'check'), iv = hex(params.get('i'), 'IV');
  if (salt.length > 64 || check.length < 1 || check.length > 32 || iv.length !== 16) fail('ARC_CIPHER', 'unsupported cipher parameter lengths');
  return { iterations, salt, check, iv, fixed: params.has('f') };
}
export async function deriveCipher(method, password, options = {}) {
  const limits = checkedLimits(options.limits); options.signal?.throwIfAborted();
  const params = cipherParameters(method, limits);
  if (!(password instanceof Uint8Array) || !password.length || password.length > 4096) fail('ARC_PASSWORD', 'missing or oversized installer password argument');
  const key = await crypto.subtle.importKey('raw', password, 'PBKDF2', false, ['deriveBits']);
  const derived = new Uint8Array(await crypto.subtle.deriveBits({ name: 'PBKDF2', hash: 'SHA-512', salt: params.salt, iterations: params.iterations }, key, (32 + params.check.length) * 8));
  options.signal?.throwIfAborted();
  let mismatch = 0;
  for (let i = 0; i < params.check.length; i++) mismatch |= params.check[i] ^ derived[32 + i];
  if (mismatch) fail('ARC_PASSWORD', 'installer argument does not match archive password check');
  const cipherKey = derived.slice(0, 32); derived.fill(0);
  return { key: params.fixed ? cipherKey : legacyHex(cipherKey), iv: params.fixed ? params.iv : legacyHex(params.iv) };
}
export function lzmaParameters(method, limits = checkedLimits()) {
  const [name, ...parts] = method.split(':');
  if (name !== 'lzma') fail('ARC_CODEC', 'control block needs an unsupported compression method');
  // Defaults are the registered FreeArc 0.67 LZMA profile, not game asset data.
  let dictSize = 64 * 1024 * 1024, lc = 3, lp = 0, pb = 2;
  const units = { b: 1, k: 1024, kb: 1024, m: 1024 ** 2, mb: 1024 ** 2, g: 1024 ** 3, gb: 1024 ** 3 };
  for (const part of parts) {
    let match = /^(\d+)(b|kb|k|mb|m|gb|g)$/.exec(part);
    if (match) { dictSize = Number(match[1]) * units[match[2]]; continue; }
    match = /^(lc|lp|pb)(\d+)$/.exec(part);
    if (match) { const n = Number(match[2]); if (match[1] === 'lc') lc = n; else if (match[1] === 'lp') lp = n; else pb = n; continue; }
    if (!['normal', 'fast', 'bt2', 'bt3', 'bt4', 'hc3', 'hc4'].includes(part) && !/^\d+$/.test(part)) fail('ARC_CODEC', 'unknown LZMA profile parameter');
  }
  if (!Number.isSafeInteger(dictSize) || dictSize < 1 || dictSize > limits.dictionaryBytes || lc > 8 || lp > 4 || pb > 4) fail('ARC_LIMIT', 'LZMA properties exceed supported limits');
  return { dictSize, properties: (pb * 5 + lp) * 9 + lc };
}
export async function decodeControl(blob, block, password, codec, options = {}) {
  const limits = checkedLimits(options.limits);
  if (block.originalSize > limits.controlBytes) fail('ARC_LIMIT', 'decoded control exceeds configured budget', block.offset);
  const methods = block.method.split('+');
  let bytes = await readArcBytes(blob, block.offset, block.storedSize, { signal: options.signal, limit: limits.controlBytes });
  if (methods.at(-1)?.startsWith('aes-')) {
    const cipher = await deriveCipher(methods.pop(), password, options);
    try { bytes = await codec.aesCtrTransform(bytes, cipher, { blockOffset: 0, signal: options.signal }); }
    finally { cipher.key.fill(0); cipher.iv.fill(0); }
  }
  if (methods.length !== 1) fail('ARC_CODEC', 'unsupported control block chain');
  if (methods[0] !== 'storing') bytes = await codec.decodeLzma(bytes, { ...lzmaParameters(methods[0], limits), expectedSize: block.originalSize, maxOutputBytes: limits.controlBytes, signal: options.signal });
  if (!(bytes instanceof Uint8Array) || bytes.length !== block.originalSize) fail('ARC_SIZE', 'decoded control length disagrees with descriptor', block.offset);
  if (crc32(bytes) !== block.crc) fail('ARC_CRC', 'decoded control checksum mismatch', block.offset);
  return bytes;
}
export function parseFooter(bytes, footerOffset, archiveSize, options = {}) {
  const limits = checkedLimits(options.limits), cursor = new ArcCursor(bytes, limits), blocks = [];
  const count = cursor.count(limits.blocks, 'control block');
  for (let i = 0; i < count; i++) {
    const type = cursor.integer(), method = cursor.string(), relative = cursor.integer(), originalSize = cursor.integer(), storedSize = cursor.integer(), crc = cursor.u32();
    const offset = footerOffset - relative; boundedRange(offset, storedSize, archiveSize);
    if (offset + storedSize > footerOffset || originalSize > limits.controlBytes || storedSize > limits.controlBytes) fail('ARC_LIMIT', 'control block range/size exceeds supported budget', offset);
    if (![1, 3, 5].includes(type)) fail('ARC_BLOCK_TYPE', 'unsupported control block type', offset);
    blocks.push({ type, method, offset, originalSize, storedSize, crc });
  }
  const locked = cursor.u8(); if (locked > 1) fail('ARC_FOOTER', 'invalid archive lock flag');
  const commentChars = cursor.count(limits.controlBytes / 4, 'comment'); cursor.take(commentChars * 4);
  // Optional historical fields stay opaque; they do not select assets or priority.
  return { blocks, locked: Boolean(locked), trailing: cursor.take(bytes.length - cursor.at) };
}
function safePath(directory, name) {
  const path = (directory ? `${directory}\\` : '') + name;
  if (!name || /^[\\/]/.test(path) || path.includes(':') || path.split(/[\\/]/).some(p => !p || p === '.' || p === '..')) fail('ARC_PATH', 'absolute, empty or traversing member path');
  return path;
}
export function parseDirectory(bytes, directoryOffset, archiveSize, options = {}) {
  const limits = checkedLimits(options.limits), c = new ArcCursor(bytes, limits);
  const blockCount = c.count(limits.blocks, 'data block'), fileCounts = []; let total = 0;
  for (let i = 0; i < blockCount; i++) {
    const count = c.count(limits.entries, 'file'); total += count;
    if (total > limits.entries) fail('ARC_LIMIT', 'aggregate file count exceeds limit'); fileCounts.push(count);
  }
  const methods = Array.from({ length: blockCount }, () => c.string());
  const offsets = Array.from({ length: blockCount }, () => directoryOffset - c.integer());
  const storedSizes = Array.from({ length: blockCount }, () => c.integer());
  const dirs = Array.from({ length: c.count(limits.entries, 'directory') }, () => c.string());
  const names = Array.from({ length: total }, () => c.string());
  const dirNumbers = Array.from({ length: total }, () => c.integer());
  const sizes = Array.from({ length: total }, () => c.integer());
  const times = Array.from({ length: total }, () => c.u32());
  const flags = Array.from({ length: total }, () => c.u8());
  const crcs = Array.from({ length: total }, () => c.u32());
  const blocks = [], files = [], paths = new Set(); let at = 0;
  for (let i = 0; i < blockCount; i++) {
    boundedRange(offsets[i], storedSizes[i], archiveSize);
    if (offsets[i] + storedSizes[i] > directoryOffset) fail('ARC_BOUNDS', 'data block overlaps directory', offsets[i]);
    if (storedSizes[i] && blocks.some(b => b.storedSize && offsets[i] < b.offset + b.storedSize && b.offset < offsets[i] + storedSizes[i])) fail('ARC_BOUNDS', 'data blocks overlap', offsets[i]);
    let originalSize = 0;
    for (let j = 0; j < fileCounts[i]; j++, at++) {
      if (dirNumbers[at] >= dirs.length || flags[at] > 1) fail('ARC_DIRECTORY', 'invalid directory index/flag');
      const path = safePath(dirs[dirNumbers[at]], names[at]), normalized = path.replaceAll('\\', '/').toLowerCase();
      if (paths.has(normalized)) fail('ARC_AMBIGUOUS_PATH', 'duplicate member path'); paths.add(normalized);
      const file = { path, size: sizes[at], crc: crcs[at], time: times[at], directory: Boolean(flags[at]), block: i, decodedOffset: originalSize };
      if (file.directory && file.size) fail('ARC_DIRECTORY', 'directory has nonzero payload size');
      originalSize += file.size;
      if (!Number.isSafeInteger(originalSize) || originalSize > limits.decodedBlockBytes) fail('ARC_LIMIT', 'decoded block exceeds configured limit');
      files.push(file);
    }
    blocks.push({ method: methods[i], offset: offsets[i], storedSize: storedSizes[i], originalSize, fileCount: fileCounts[i] });
  }
  return { blocks, files, trailing: c.take(bytes.length - c.at) };
}
export async function openFreeArc(blob, password, codec, options = {}) {
  const footer = await footerDescriptor(blob, options);
  const decoded = await decodeControl(blob, footer, password, codec, options);
  const control = parseFooter(decoded, footer.offset, blob.size, options);
  const directories = control.blocks.filter(b => b.type === 3);
  if (directories.length !== 1) fail('ARC_DIRECTORY', 'exactly one directory control block required');
  const block = directories[0];
  const directory = parseDirectory(await decodeControl(blob, block, password, codec, options), block.offset, blob.size, options);
  return { ...directory, control, footer, sourceSize: blob.size };
}
