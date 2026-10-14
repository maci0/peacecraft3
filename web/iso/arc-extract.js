import { ArcError, boundedRange, checkedLimits, crc32, deriveCipher, readArcBytes } from './freearc.js';

// The codec emits one solid stream; boundaries and checksums come from its directory.
export async function extractArcMembers(blob, archive, selected, password, codec, {
  signal, onProgress, createSink, limits: limitOverrides, readBytes = 1024 * 1024,
  maxSelectedBytes = 1024 * 1024 * 1024,
} = {}) {
  const limits = checkedLimits(limitOverrides);
  if (!createSink || !Number.isSafeInteger(readBytes) || readBytes < 16 || readBytes % 16 || readBytes > limits.controlBytes)
    throw new TypeError('A bounded sink and aligned read size are required');
  if (!Number.isSafeInteger(maxSelectedBytes) || maxSelectedBytes < 1) throw new TypeError('Invalid selected-output budget');
  if (!selected.length) throw new ArcError('ARC_SELECTION', 'select at least one archive member');
  let selectedBytes = 0; const wanted = new Set();
  for (const file of selected) {
    if (!archive.files.includes(file) || file.directory || wanted.has(file)) throw new ArcError('ARC_SELECTION', 'invalid or repeated member selection');
    wanted.add(file); selectedBytes += file.size;
    if (!Number.isSafeInteger(selectedBytes) || selectedBytes > maxSelectedBytes) throw new ArcError('ARC_LIMIT', 'selected output exceeds configured budget');
  }
  const results = new Map(), sinks = new Map();
  try {
    for (const file of selected) { signal?.throwIfAborted(); sinks.set(file, await createSink(file)); }
    const blockIndexes = [...new Set(selected.map(file => file.block))];
    const total = blockIndexes.reduce((sum, index) => sum + archive.blocks[index].originalSize, 0);
    let decodedTotal = 0;
    for (const index of blockIndexes) {
      signal?.throwIfAborted();
      const block = archive.blocks[index];
      boundedRange(block.offset, block.storedSize, blob.size);
      if (block.originalSize > limits.decodedBlockBytes) throw new ArcError('ARC_LIMIT', 'solid output exceeds configured budget');
      const methods = block.method.split('+'); let cipher;
      if (methods.at(-1)?.startsWith('aes-')) cipher = await deriveCipher(methods.pop(), password, { signal, limits: limitOverrides });
      let sourceAt = 0, consumed = 0, written = 0, fileIndex = 0, withinFile = 0, fileCrc = 0;
      let buffered = new Uint8Array();
      const files = archive.files.filter(file => file.block === index);
      async function closeCompletedFiles() {
        while (fileIndex < files.length && withinFile === files[fileIndex].size) {
          const file = files[fileIndex];
          if (fileCrc !== file.crc) throw new ArcError('ARC_CRC', `decoded member checksum mismatch: ${file.path}`);
          const sink = sinks.get(file);
          if (sink) results.set(file, await sink.close());
          fileIndex++; withinFile = 0; fileCrc = 0;
        }
      }
      await closeCompletedFiles();
      try {
        await codec.decodeStream({ method: methods.join('+'), expectedSize: block.originalSize,
          maxOutputBytes: limits.decodedBlockBytes, maxDictionaryBytes: limits.dictionaryBytes, signal,
          async read(requested) {
            signal?.throwIfAborted();
            if (!Number.isSafeInteger(requested) || requested < 1) throw new ArcError('ARC_CODEC', 'invalid codec read request');
            if (!buffered.length && sourceAt === block.storedSize) return new Uint8Array();
            // Source chunks stay CTR-aligned; odd codec reads take bounded slices.
            if (!buffered.length) {
              const size = Math.min(readBytes, block.storedSize - sourceAt);
              buffered = await readArcBytes(blob, block.offset + sourceAt, size, { signal, limit: readBytes });
              if (cipher) buffered = await codec.aesCtrTransform(buffered, cipher, { blockOffset: sourceAt / 16, signal });
              sourceAt += size;
            }
            const count = Math.min(requested, buffered.length);
            const bytes = buffered.subarray(0, count); buffered = buffered.subarray(count); consumed += count;
            onProgress?.({ phase: 'decode', sourceBytes: consumed, sourceTotal: block.storedSize, decodedBytes: decodedTotal + written, decodedTotal: total });
            return bytes;
          },
          async onWrite(chunk) {
            signal?.throwIfAborted();
            // This is a view into the codec's budgeted memory, not a control
            // block allocation. REP can yield an entire dictionary window.
            if (!(chunk instanceof Uint8Array) || written > block.originalSize - chunk.length)
              throw new ArcError('ARC_SIZE', 'codec output exceeds declared output limit');
            let at = 0;
            while (at < chunk.length) {
              await closeCompletedFiles();
              if (fileIndex >= files.length) throw new ArcError('ARC_SIZE', 'solid stream exceeds file boundaries');
              const file = files[fileIndex], size = Math.min(chunk.length - at, file.size - withinFile);
              const bytes = chunk.subarray(at, at + size);
              fileCrc = crc32(bytes, fileCrc);
              const sink = sinks.get(file); if (sink) await sink.write(bytes);
              at += size; withinFile += size; written += size;
            }
            await closeCompletedFiles();
            onProgress?.({ phase: 'decode', sourceBytes: consumed, sourceTotal: block.storedSize, decodedBytes: decodedTotal + written, decodedTotal: total });
          },
        });
        if (consumed !== block.storedSize || buffered.length || written !== block.originalSize || fileIndex !== files.length)
          throw new ArcError('ARC_SIZE', 'codec stopped before the complete checked solid stream');
      } finally { cipher?.key.fill(0); cipher?.iv.fill(0); }
      decodedTotal += written;
    }
    return selected.map(file => ({ member: file, file: results.get(file) }));
  } catch (error) {
    await Promise.allSettled([...sinks.values()].map(sink => sink.abort()));
    throw error;
  }
}

export async function createTemporaryArcStorage({ storage = globalThis.navigator?.storage } = {}) {
  if (!storage?.getDirectory) throw new ArcError('ARC_STORAGE', 'this browser lacks local temporary file storage for bounded installer extraction');
  const root = await storage.getDirectory();
  const name = `peacecraft3-import-${crypto.randomUUID()}`;
  const directory = await root.getDirectoryHandle(name, { create: true }); let sequence = 0;
  return {
    async createSink() {
      const handle = await directory.getFileHandle(String(sequence++), { create: true });
      const stream = await handle.createWritable(); let closed = false;
      return {
        async write(bytes) { if (closed) throw new ArcError('ARC_STORAGE', 'temporary sink already closed'); await stream.write(bytes); },
        async close() { if (!closed) { await stream.close(); closed = true; } return handle.getFile(); },
        async abort() { if (!closed) { closed = true; await stream.abort(); } },
      };
    },
    async remove() { await root.removeEntry(name, { recursive: true }); },
  };
}
