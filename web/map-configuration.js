import { createLoadingProgress } from './loading.js';

const installed = new WeakMap();
const U32_MAX = 0xffff_ffff;

function explicitInteger(text, label) {
  if (!/^(0|[1-9][0-9]*)$/.test(text)) throw new Error(`${label}: enter a whole decimal number without signs or spaces.`);
  const value = Number(text);
  if (!Number.isSafeInteger(value) || value > U32_MAX) throw new Error(`${label}: value must fit the browser's unsigned 32-bit range.`);
  return value;
}

// One local producer for the existing Rust inbox; policies live in the supplied JSON.
export function installMapConfiguration({ api, root = document, requestFrame = requestAnimationFrame, cancelFrame = cancelAnimationFrame } = {}) {
  const form = root.getElementById('map-configuration');
  if (!form) return undefined;
  if (installed.has(form)) return installed.get(form);
  const fileInput = root.getElementById('map-config-file');
  const byteInput = root.getElementById('map-config-limit');
  const submitButton = root.getElementById('map-config-submit');
  const ordinalInput = root.getElementById('map-config-ordinal');
  const inspectButton = root.getElementById('map-config-inspect');
  const revisionOutput = root.getElementById('map-config-revision');
  const archiveOutput = root.getElementById('map-config-archive');
  const loading = createLoadingProgress({ root, progressId: 'map-config-progress', statusId: 'map-config-status' });
  let importBusy = false, reading = false, pendingId, frame, stopped = false, retained;
  function message(error) { return error instanceof Error ? error.message : String(error); }
  function updateControls() {
    const busy = stopped || importBusy || reading || pendingId !== undefined;
    fileInput.disabled = byteInput.disabled = ordinalInput.disabled = busy;
    inspectButton.disabled = busy || ordinalInput.value === '';
    submitButton.disabled = busy || !fileInput.files?.[0] || byteInput.value === '';
  }
  function retain(handle) { retained?.free(); retained = handle; }
  function refreshArchives() {
    if (stopped) return;
    try { revisionOutput.textContent = `Committed archive revision: ${api.asset_revision()}. No archive order or version is inferred.`; }
    catch (error) { revisionOutput.textContent = `Archive revision unavailable: ${message(error)}`; }
  }
  function inspect() {
    if (stopped || importBusy || reading || pendingId !== undefined) return;
    let handle;
    try {
      const ordinal = explicitInteger(ordinalInput.value, 'Archive ordinal');
      const revision = api.asset_revision();
      refreshArchives();
      handle = api.inspect_map_archive(ordinal, revision);
      archiveOutput.textContent = handle.status() === 'selected'
        ? `Archive ${handle.index()} at revision ${handle.committed_revision()}: ${handle.archive_name()} (${handle.archive_bytes()} bytes). Version and member policy must be supplied in your JSON.`
        : `${handle.kind()}: ${handle.diagnostic()}`;
    } catch (error) { archiveOutput.textContent = `Archive inspection failed: ${message(error)}`; }
    finally { handle?.free(); }
  }
  function schedule() { if (!stopped && frame === undefined && pendingId !== undefined) frame = requestFrame(poll); }
  function poll() {
    frame = undefined;
    if (stopped || pendingId === undefined) return;
    let handle;
    try {
      handle = api.poll_map_configuration_result(pendingId);
      const status = handle.status();
      if (status === 'pending') { schedule(); return; }
      if (status === 'completed' || status === 'failed') {
        const id = pendingId;
        pendingId = undefined; // Completed poll acknowledges exactly this request; free is separate.
        const kind = handle.kind(), diagnostic = handle.diagnostic();
        retain(handle); handle = undefined;
        if (status === 'completed' && kind === 'map-admitted') loading.complete(`Request ${id}: map data parsed. This demo does not run gameplay.`);
        else loading.error(`Request ${id} — ${kind}: ${diagnostic}`);
      } else throw new Error(`Unexpected configuration status: ${status}`);
    } catch (error) {
      // Retain the id: a failed JS call does not prove Rust completed or acknowledged it.
      loading.error(`Request ${pendingId}: polling failed; awaiting a successful poll. ${message(error)}`);
      schedule();
    } finally { handle?.free(); updateControls(); }
  }
  async function submit() {
    if (stopped || importBusy || reading || pendingId !== undefined) return;
    const file = fileInput.files?.[0];
    let limit;
    try {
      limit = explicitInteger(byteInput.value, 'Metadata byte limit');
      if (!file) throw new Error('Choose a local metadata JSON file.');
      if (!Number.isSafeInteger(file.size) || file.size < 0 || file.size > limit) throw new Error(`Metadata file has ${file.size} bytes; your limit is ${limit}. Choose a smaller file or change the explicit limit.`);
    } catch (error) { loading.error(message(error)); return; }
    reading = true; updateControls();
    loading.update({ phase: 'Reading local configuration metadata' });
    let handle;
    try {
      const bytes = await file.arrayBuffer();
      if (stopped) return;
      if (importBusy) throw new Error('Archive import began while reading. Wait for it to finish, then submit your metadata again.');
      if (bytes.byteLength !== file.size || bytes.byteLength > limit) throw new Error('Metadata read length differs from the selected file or exceeds your limit.');
      const text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
      // Rust checks the separate UTF-8/serde limit after wasm-bindgen String ingress.
      handle = api.submit_map_configuration(text, limit);
      const status = handle.status();
      if (status === 'submitted') {
        pendingId = handle.id();
        if (!Number.isInteger(pendingId) || pendingId < 1 || pendingId > U32_MAX) throw new Error('Submission returned an invalid request identifier.');
        retained?.free(); retained = undefined;
        loading.update({ phase: `Request ${pendingId}: waiting for map admission` });
        schedule();
      } else if (status === 'failed') {
        const kind = handle.kind(), diagnostic = handle.diagnostic();
        retain(handle); handle = undefined;
        loading.error(`${kind}: ${diagnostic}`);
      } else throw new Error(`Unexpected submission status: ${status}`);
    } catch (error) { loading.error(`Configuration request failed: ${message(error)}`); }
    finally { handle?.free(); reading = false; updateControls(); }
  }
  function onSubmit(event) { event.preventDefault(); void submit(); }
  function onInput() { updateControls(); }
  form.addEventListener('submit', onSubmit);
  inspectButton.addEventListener('click', inspect);
  for (const element of [fileInput, byteInput, ordinalInput]) element.addEventListener('input', onInput);
  const controller = {
    setImportBusy(busy) { importBusy = busy; refreshArchives(); updateControls(); },
    refreshArchives,
    dispose() {
      stopped = true;
      if (frame !== undefined) cancelFrame(frame);
      frame = undefined; retained?.free(); retained = undefined;
      form.removeEventListener('submit', onSubmit); inspectButton.removeEventListener('click', inspect);
      for (const element of [fileInput, byteInput, ordinalInput]) element.removeEventListener('input', onInput);
      updateControls();
    },
  };
  installed.set(form, controller);
  refreshArchives(); updateControls();
  return controller;
}
