// Dedicated decoder instance per selected ISO; data crosses only local worker messages.
import { CLIENT_IMAGE_BYTES } from './engine-member.js';

function declaredClientImageLength(clientImage) {
  if (clientImage.bytes instanceof Uint8Array) return clientImage.bytes.byteLength;
  const size = clientImage.file?.size;
  return Number.isSafeInteger(size) ? size : Number.NaN;
}

export function createInstallerDecoder({ onProgress, WorkerClass = globalThis.Worker } = {}) {
  const worker = new WorkerClass(new URL('./arc-worker.js', import.meta.url), { type: 'module' });
  const requests = new Map(); let sequence = 0, closed = false;
  function rejectAll(error) { for (const request of requests.values()) { request.removeAbort?.(); request.reject(error); } requests.clear(); }
  worker.onmessage = event => {
    const { id, result, error, type, work } = event.data;
    if (type === 'progress') { onProgress?.(work); return; }
    const request = requests.get(id); if (!request) return;
    requests.delete(id); request.removeAbort?.();
    if (error) { const e = new Error(error.message); e.name = error.name ?? 'Error'; e.code = error.code; request.reject(e); }
    else request.resolve(result);
  };
  worker.onerror = event => rejectAll(new Error(event.message || 'Local installer decoder worker failed'));
  function call(command, args, signal) {
    if (closed) return Promise.reject(new Error('Local installer decoder is closed'));
    signal?.throwIfAborted(); const id = ++sequence;
    return new Promise((resolve, reject) => {
      const abort = () => {
        const request = requests.get(id);
        if (!request) return;
        requests.delete(id);
        request.removeAbort();
        try { worker.postMessage({ id, command: 'cancel' }); }
        finally { request.reject(signal.reason ?? new DOMException('The operation was aborted.', 'AbortError')); }
      };
      signal?.addEventListener('abort', abort, { once: true });
      requests.set(id, { resolve, reject, removeAbort: () => signal?.removeEventListener('abort', abort) });
      if (signal?.aborted) { abort(); return; }
      worker.postMessage({ id, command, args });
    });
  }
  return {
    inspect: (iso, discovery, signal) => call('inspect', { iso, discovery }, signal),
    extract: (ids, signal) => call('extract', { ids }, signal),
    cleanup: () => call('cleanup'),
    close() { closed = true; worker.terminate(); rejectAll(new Error('Local installer decoder closed')); },
  };
}

// Caller-side transaction for files already extracted by this decoder.
// Abort is checked before begin so a retired request cannot open a revision.
export async function stageStoredInstallerFiles(files, sourceName, api, { signal, onProgress, clientImage = null } = {}) {
  signal?.throwIfAborted();
  api.begin_archive_import();
  try {
    for (const entry of files) {
      signal?.throwIfAborted();
      onProgress?.({ phase: 'archive', path: entry.path });
      const bytes = new Uint8Array(await entry.file.arrayBuffer());
      signal?.throwIfAborted();
      api.stage_archive(`${sourceName}:${entry.path} (stored, unpatched)`, bytes);
      await new Promise(resolve => setTimeout(resolve, 0));
    }
    if (clientImage) {
      const source = clientImage.source;
      const declared = declaredClientImageLength(clientImage);
      if (typeof source !== 'string' || !source.trim() || !Number.isSafeInteger(declared) || declared < 1 || declared > CLIENT_IMAGE_BYTES)
        throw new Error('Installer client image is missing its source identity or exceeds its budget');
      signal?.throwIfAborted();
      const bytes = clientImage.bytes instanceof Uint8Array
        ? clientImage.bytes
        : new Uint8Array(await clientImage.file.arrayBuffer());
      signal?.throwIfAborted();
      if (bytes.byteLength !== declared)
        throw new Error('Installer client image length does not match its declared size');
      api.stage_client_image(source, bytes);
    }
    signal?.throwIfAborted();
    onProgress?.({ phase: 'hydrate' });
    await new Promise(resolve => setTimeout(resolve, 0));
    signal?.throwIfAborted();
    const diagnostics = api.commit_archive_import();
    return { diagnostics, canonicalPaths: api.stored_assets(), revision: api.asset_revision() };
  } catch (error) {
    api.cancel_archive_import();
    throw error;
  }
}
