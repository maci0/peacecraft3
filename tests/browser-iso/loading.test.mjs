import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const source = await readFile(new URL('../../web/loading.js', import.meta.url), 'utf8');
const { createLoadingProgress, monitorWasmDownload } = await import(`data:text/javascript;base64,${Buffer.from(source).toString('base64')}`);

function fixture() {
  const progress = { dataset: {}, hidden: true, max: 1,
    set value(value) { this.currentValue = value; },
    removeAttribute(name) { if (name === 'value') delete this.currentValue; },
  };
  const status = { dataset: {}, textContent: '' };
  const root = { getElementById: id => ({ progress, status })[id] ?? null };
  return { progress, status, ui: createLoadingProgress({ progressId: 'progress', statusId: 'status', root }) };
}

test('only reliable bounded byte totals produce determinate percentage', () => {
  const { progress, status, ui } = fixture();
  ui.update({ phase: 'Downloading demo', loadedBytes: 25, totalBytes: 100, totalReliable: true });
  assert.equal(progress.currentValue, 25);
  assert.equal(progress.max, 100);
  assert.equal(progress.hidden, false);
  assert.match(status.textContent, /25%/);
  for (const work of [
    { loadedBytes: 25, totalBytes: 100 },
    { loadedBytes: 25, totalBytes: 0, totalReliable: true },
    { loadedBytes: 101, totalBytes: 100, totalReliable: true },
    { loadedBytes: -1, totalBytes: 100, totalReliable: true },
    { loadedBytes: 1.5, totalBytes: 100, totalReliable: true },
    { loadedBytes: 1, totalBytes: Infinity, totalReliable: true },
    { loadedBytes: 1, totalBytes: Number.MAX_SAFE_INTEGER + 1, totalReliable: true },
  ]) {
    ui.update({ phase: 'Inspecting locally', ...work });
    assert.equal(progress.currentValue, undefined);
    assert.doesNotMatch(status.textContent, /%/);
  }
});

test('initialization, completion, failure and reset clear prior phase state', () => {
  const { progress, status, ui } = fixture();
  ui.update({ phase: 'Initializing demo' });
  assert.equal(progress.currentValue, undefined);
  assert.equal(status.textContent, 'Initializing demo');
  ui.complete('Demo ready');
  assert.equal(progress.currentValue, 1);
  assert.equal(progress.max, 1);
  assert.equal(status.dataset.state, 'complete');
  ui.update({ phase: 'Extracting authored dependencies' });
  assert.equal(progress.currentValue, undefined);
  ui.error('Unsupported image; current assets retained');
  assert.equal(progress.hidden, true);
  assert.equal(status.dataset.state, 'error');
  assert.match(status.textContent, /current assets retained/);
  ui.reset('Select another ISO');
  assert.equal(progress.hidden, true);
  assert.equal(status.dataset.state, 'idle');
  assert.equal(status.textContent, 'Select another ISO');
});

test('non-demo callers with absent DOM or missing progress controls are harmless', () => {
  for (const root of [undefined, { getElementById: () => null }]) {
    const ui = createLoadingProgress({ progressId: 'none', statusId: 'none', root });
    ui.update({ phase: 'Reading', loadedBytes: 0, totalBytes: 1, totalReliable: true });
    ui.complete(); ui.error(); ui.reset();
  }
});
test('download monitor preserves bytes and reports measured chunks before initialization', async () => {
  const updates = [], ui = { update: work => updates.push(work) };
  const response = new Response(new ReadableStream({ start(controller) {
    controller.enqueue(new Uint8Array([0, 1])); controller.enqueue(new Uint8Array([2, 3, 4])); controller.close();
  } }), { headers: { 'content-length': '5', 'content-type': 'application/wasm' } });
  const measured = monitorWasmDownload(response, ui);
  assert.equal(measured.headers.get('content-type'), 'application/wasm');
  assert.deepEqual(new Uint8Array(await measured.arrayBuffer()), new Uint8Array([0, 1, 2, 3, 4]));
  assert.deepEqual(updates.map(w => w.loadedBytes), [0, 2, 5, undefined]);
  assert.equal(updates[1].totalReliable, true);
  assert.equal(updates.at(-1).phase, 'Initializing demo');
});
test('encoded or missing content lengths stay indeterminate and download failures propagate', async () => {
  for (const headers of [{ 'content-length': '2', 'content-encoding': 'gzip' }, {}]) {
    const { ui, progress } = fixture();
    const measured = monitorWasmDownload(new Response(new Uint8Array([1, 2]), { headers }), ui);
    assert.equal(progress.currentValue, undefined);
    await measured.arrayBuffer();
  }
  assert.throws(() => monitorWasmDownload(new Response(null, { status: 404 }), { update() {} }), /HTTP 404/);
  const broken = new Response(new ReadableStream({ pull(controller) { controller.error(new Error('read failed')); } }));
  await assert.rejects(monitorWasmDownload(broken, { update() {} }).arrayBuffer(), /read failed/);
});
