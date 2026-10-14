// Progress reflects caller-reported work. Unknown totals remain indeterminate.
export function monitorWasmDownload(response, ui) {
  if (!response.ok) throw new Error(`Demo download: HTTP ${response.status}`);
  const length = response.headers.get('content-length');
  const totalBytes = length && /^\d+$/.test(length) ? Number(length) : undefined;
  const encoding = response.headers.get('content-encoding');
  const totalReliable = !encoding || encoding === 'identity';
  ui.update({ phase: 'Downloading demo', loadedBytes: 0, totalBytes, totalReliable });
  if (!response.body) return response;
  const reader = response.body.getReader(); let loadedBytes = 0;
  const body = new ReadableStream({
    async pull(controller) {
      try {
        const { done, value } = await reader.read();
        if (done) { ui.update({ phase: 'Initializing demo' }); controller.close(); return; }
        loadedBytes += value.byteLength;
        ui.update({ phase: 'Downloading demo', loadedBytes, totalBytes, totalReliable });
        controller.enqueue(value);
      } catch (error) { controller.error(error); }
    },
    cancel(reason) { return reader.cancel(reason); },
  });
  return new Response(body, { status: response.status, statusText: response.statusText, headers: response.headers });
}

export function createLoadingProgress({ progressId, statusId, root = globalThis.document } = {}) {
  const progress = root?.getElementById(progressId);
  const status = root?.getElementById(statusId);
  function describe(message, state) {
    if (status) {
      status.textContent = message;
      status.dataset.state = state;
    }
    if (progress) progress.dataset.state = state;
  }
  function indeterminate() {
    if (progress) progress.removeAttribute('value');
  }
  return {
    update({ phase, loadedBytes, totalBytes, totalReliable = false } = {}) {
      if (progress) progress.hidden = false;
      const validLoaded = Number.isSafeInteger(loadedBytes) && loadedBytes >= 0;
      const knownTotal = totalReliable && validLoaded && Number.isSafeInteger(totalBytes) && totalBytes > 0 && loadedBytes <= totalBytes;
      let message = phase ?? 'Loading';
      if (knownTotal) {
        if (progress) { progress.max = totalBytes; progress.value = loadedBytes; }
        message += `: ${Math.floor(loadedBytes / totalBytes * 100)}% (${loadedBytes.toLocaleString()} / ${totalBytes.toLocaleString()} bytes)`;
      } else {
        indeterminate();
        if (validLoaded) message += `: ${loadedBytes.toLocaleString()} bytes processed`;
      }
      describe(message, 'loading');
    },
    complete(message = 'Complete') {
      if (progress) { progress.hidden = false; progress.max = 1; progress.value = 1; }
      describe(message, 'complete');
    },
    error(message = 'Loading failed') {
      indeterminate();
      if (progress) progress.hidden = true;
      describe(message, 'error');
    },
    reset(message = 'Waiting for local files') {
      indeterminate();
      if (progress) progress.hidden = true;
      describe(message, 'idle');
    },
  };
}
