// Dedicated decoder instance per selected ISO; data crosses only local worker messages.
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
      const abort = () => worker.postMessage({ command: 'cancel' });
      signal?.addEventListener('abort', abort, { once: true });
      requests.set(id, { resolve, reject, removeAbort: () => signal?.removeEventListener('abort', abort) });
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
