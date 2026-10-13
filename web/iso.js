// ISO 9660 primary namespace only. All input reads stay in the supplied Blob.
const SECTOR = 2048;
const ascii = bytes => String.fromCharCode(...bytes);
export class IsoError extends Error {
  constructor(code, message, offset) {
    super(`${code}: ${message}${offset === undefined ? '' : ` at image byte ${offset}`}`);
    this.name = 'IsoError'; this.code = code; this.offset = offset;
  }
}
const fail = (code, message, offset) => { throw new IsoError(code, message, offset); };
function range(offset, size, end) {
  if (!Number.isSafeInteger(offset) || !Number.isSafeInteger(size) || offset < 0 || size < 0 || offset > end - size)
    fail('BOUNDS', `range ${offset}+${size} exceeds ${end}`, offset);
}
function both(bytes, offset, width) {
  const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const le = width === 4 ? v.getUint32(offset, true) : v.getUint16(offset, true);
  const be = width === 4 ? v.getUint32(offset + width) : v.getUint16(offset + width);
  if (le !== be) fail('ENDIAN_MISMATCH', 'duplicated ISO number disagrees', offset);
  return le;
}
/** Discover files without copying the image or running installers. No edition inference. */
export async function discoverIso(blob, { signal, onProgress, maxEntries = 100000 } = {}) {
  if (!blob || typeof blob.slice !== 'function' || !Number.isSafeInteger(blob.size)) throw new TypeError('Expected a File or Blob');
  if (!Number.isSafeInteger(maxEntries) || maxEntries < 1) throw new TypeError('maxEntries must be a positive integer');
  let reads = 0, bytesRead = 0;
  async function read(offset, size) {
    signal?.throwIfAborted(); range(offset, size, blob.size);
    const bytes = new Uint8Array(await blob.slice(offset, offset + size).arrayBuffer());
    signal?.throwIfAborted();
    if (bytes.length !== size) fail('TRUNCATED', 'short Blob read', offset);
    reads++; bytesRead += size; onProgress?.({ reads, bytesRead, offset }); return bytes;
  }
  let pvd, terminated = false;
  for (let sector = 16; sector < 80; sector++) {
    const bytes = await read(sector * SECTOR, SECTOR);
    const id = ascii(bytes.subarray(1, 6));
    if (['BEA01', 'NSR02', 'NSR03', 'TEA01'].includes(id)) fail('UNSUPPORTED_UDF', 'UDF requires a verified local decoder', sector * SECTOR);
    if (id !== 'CD001' || bytes[6] !== 1) fail('NOT_ISO9660', 'invalid volume descriptor signature/version', sector * SECTOR);
    if (bytes[0] === 2) fail('UNSUPPORTED_SUPPLEMENTARY', 'Joliet/supplementary namespaces are not decoded; refusing namespace fallback', sector * SECTOR);
    if (bytes[0] === 1) {
      if (pvd) fail('DUPLICATE_PVD', 'multiple primary descriptors', sector * SECTOR);
      pvd = bytes;
    } else if (bytes[0] === 255) { terminated = true; break; }
    else if (bytes[0] !== 0) fail('UNSUPPORTED_DESCRIPTOR', `descriptor type ${bytes[0]}`, sector * SECTOR);
  }
  if (!pvd || !terminated) fail('DESCRIPTORS', 'primary descriptor and terminator required');
  const blockSize = both(pvd, 128, 2), blocks = both(pvd, 80, 4);
  if (blockSize !== SECTOR) fail('UNSUPPORTED_BLOCK_SIZE', `logical block size ${blockSize}`);
  const volumeSize = blocks * blockSize; range(0, volumeSize, blob.size);
  if (both(pvd, 120, 2) !== 1 || both(pvd, 124, 2) !== 1) fail('UNSUPPORTED_VOLUME_SET', 'multi-volume image');
  function record(bytes, at, absolute) {
    const length = bytes[at];
    if (length < 34 || at + length > bytes.length) fail('DIRECTORY_RECORD', 'invalid record length', absolute);
    const r = bytes.subarray(at, at + length), nameLength = r[32];
    if (!nameLength || 33 + nameLength + (nameLength % 2 === 0 ? 1 : 0) > length) fail('DIRECTORY_RECORD', 'invalid identifier length', absolute);
    if (r[1] || r[26] || r[27]) fail('UNSUPPORTED_EXTENT', 'extended attributes/interleaving unsupported', absolute);
    if (r[25] & 128) fail('UNSUPPORTED_MULTI_EXTENT', 'multi-extent file unsupported', absolute);
    if (r[25] & ~3) fail('UNSUPPORTED_RECORD_FLAGS', `directory flags ${r[25]}`, absolute);
    if (both(r, 28, 2) !== 1) fail('UNSUPPORTED_VOLUME_SET', 'foreign-volume extent', absolute);
    const extent = both(r, 2, 4), size = both(r, 10, 4), offset = extent * blockSize;
    range(offset, size, volumeSize);
    const nameBytes = r.subarray(33, 33 + nameLength);
    if ([...nameBytes].some(n => n > 127)) fail('UNSUPPORTED_IDENTIFIER', 'non-ASCII primary identifier', absolute);
    const name = ascii(nameBytes);
    if (name.includes('/') || name.includes('\\')) fail('DIRECTORY_RECORD', 'identifier contains path separator', absolute);
    return { length, extent, offset, size, name, directory: Boolean(r[25] & 2) };
  }
  const root = record(pvd, 156, 16 * SECTOR + 156);
  if (!root.directory || root.name !== '\0') fail('ROOT_RECORD', 'invalid root directory');
  const files = [], candidates = [], diagnostics = [], visited = new Set();
  const queue = [{ ...root, path: '' }]; let entries = 0;
  for (let index = 0; index < queue.length; index++) {
    const dir = queue[index];
    if (!dir.size || dir.size % SECTOR) fail('DIRECTORY_EXTENT', 'directory size must contain complete sectors', dir.offset);
    if (visited.has(dir.extent)) fail('DIRECTORY_CYCLE', 'repeated directory extent', dir.offset);
    visited.add(dir.extent);
    for (let pos = 0; pos < dir.size; pos += SECTOR) {
      const bytes = await read(dir.offset + pos, SECTOR);
      for (let at = 0; at < bytes.length;) {
        if (bytes[at] === 0) {
          if (bytes.subarray(at).some(n => n !== 0)) fail('DIRECTORY_PADDING', 'nonzero bytes after padding', dir.offset + pos + at);
          break;
        }
        const r = record(bytes, at, dir.offset + pos + at); at += r.length;
        if (++entries > maxEntries) fail('ENTRY_LIMIT', `more than ${maxEntries} records`);
        if (r.name === '\0' || r.name === '\x01') continue;
        const path = `${dir.path}/${r.name}`;
        if (r.directory) { queue.push({ ...r, path }); continue; }
        const file = { path, offset: r.offset, size: r.size, extent: r.extent };
        files.push(file);
        const head = await read(r.offset, Math.min(r.size, 64));
        const signature = ascii(head.subarray(0, 4));
        if (signature === 'MPQ\x1a') {
          if (head.length < 32) fail('MPQ_HEADER', `${path}: truncated MPQ header`, r.offset);
          const v = new DataView(head.buffer, head.byteOffset, head.byteLength);
          const headerSize = v.getUint32(4, true), archiveSize = v.getUint32(8, true), version = v.getUint16(12, true);
          if (version !== 0) { diagnostics.push({ ...file, code: 'UNSUPPORTED_MPQ_VERSION', message: `MPQ header version ${version} requires archive decoder validation` }); continue; }
          if (headerSize !== 32 || archiveSize < headerSize) fail('MPQ_HEADER', `${path}: invalid header/archive size`, r.offset);
          range(0, archiveSize, r.size);
          range(v.getUint32(16, true), v.getUint32(24, true) * 16, archiveSize);
          range(v.getUint32(20, true), v.getUint32(28, true) * 16, archiveSize);
          candidates.push({ ...file, format: 'mpq', archiveSize, headerVersion: version });
        } else if (signature === 'MPQ\x1b') diagnostics.push({ ...file, code: 'UNSUPPORTED_MPQ_USERDATA', message: 'MPQ user-data wrapper requires archive decoder validation' });
        else if (head[0] === 77 && head[1] === 90) {
          if (head.length < 64) fail('PE_HEADER', `${path}: truncated DOS header`, r.offset);
          const peOffset = new DataView(head.buffer).getUint32(60, true);
          range(peOffset, 24, r.size);
          const pe = await read(r.offset + peOffset, 24);
          if (ascii(pe.subarray(0, 4)) !== 'PE\0\0') diagnostics.push({ ...file, code: 'UNSUPPORTED_DOS_EXECUTABLE', message: 'MZ executable without PE header; no executable is run' });
          else diagnostics.push({ ...file, code: 'UNSUPPORTED_PE_PAYLOAD', message: 'PE executable detected; installer type/payload unverified and no local installer decoder connected' });
        } else if (signature === 'MSCF') diagnostics.push({ ...file, code: 'UNSUPPORTED_CAB', message: 'CAB payload requires a verified local decoder' });
        else if (ascii(head.subarray(0, 6)) === '7z\xbc\xaf\x27\x1c') diagnostics.push({ ...file, code: 'UNSUPPORTED_7Z', message: '7z payload requires a verified local decoder' });
        else diagnostics.push({ ...file, code: 'UNRECOGNIZED_CONTENT', message: 'No supported payload signature at file start; embedded/repacked payloads are not scanned' });
      }
    }
  }
  return { provenance: { sourceName: typeof blob.name === 'string' ? blob.name : null, sourceSize: blob.size, filesystem: 'ISO9660-primary', volumeIdentifier: ascii(pvd.subarray(40, 72)).trimEnd(), volumeSize, edition: null, build: null }, files, candidates, diagnostics, work: { reads, bytesRead, entries } };
}
/** A local bounded view for the archive decoder; discovery never registers assets. */
export function sliceIsoFile(blob, file) {
  range(file.offset, file.size, blob.size);
  return blob.slice(file.offset, file.offset + file.size);
}
