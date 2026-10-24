import test from 'node:test';
import assert from 'node:assert/strict';
import { createInstallerDecoder, stageStoredInstallerFiles } from '../../web/iso/installer-import.js';

function fileEntry(path) {
  const bytes = new Uint8Array([1]);
  return { path, file: { arrayBuffer: async () => bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) } };
}

function revisionApi(start) {
  let pending = null, revision = start, visible = ['previous'];
  const api = {
    beginCount: 0,
    commitCount: 0,
    begin_archive_import() { api.beginCount += 1; pending = []; },
    cancel_archive_import() { pending = null; },
    stage_archive(name, bytes) {
      if (!pending) throw new Error('no active archive import');
      pending.push({ name, bytes });
      return 'staged';
    },
    commit_archive_import() {
      if (!pending) throw new Error('no active archive import');
      visible = pending.map(entry => entry.name);
      pending = null;
      api.commitCount += 1;
      revision += 1;
      return ['edition unknown'];
    },
    stored_assets() { return visible.slice(); },
    asset_revision() { return revision; },
  };
  return api;
}

test('retired installer extract cannot change the active import revision', async () => {
  const posted = [];
  let worker;
  class FakeWorker {
    constructor() { worker = this; }
    postMessage(message) { posted.push(message); }
    terminate() {}
  }
  const decoder = createInstallerDecoder({ WorkerClass: FakeWorker });
  const sourceName = 'FixtureSource';
  const api = revisionApi(11);
  const retired = new AbortController();
  const retiredExtract = decoder.extract(['retired-member'], retired.signal);
  const retiredId = posted.at(-1).id;
  const retiredSettled = retiredExtract.then(value => ({ fulfilled: value }), error => ({ rejected: error }));
  retired.abort();
  const stale = { files: [fileEntry('retired.mpq')], variant: 'stored base files before language patches', languagePatchesApplied: false };
  worker.onmessage({ data: { id: retiredId, result: stale } });
  const retiredOutcome = await retiredSettled;
  assert.equal(retiredOutcome.rejected?.name, 'AbortError');
  assert.equal(posted.find(message => message.command === 'cancel')?.id, retiredId);
  await assert.rejects(
    () => stageStoredInstallerFiles(stale.files, sourceName, api, { signal: retired.signal }),
    error => error.name === 'AbortError',
  );
  assert.equal(api.beginCount, 0);
  assert.equal(api.commitCount, 0);
  assert.equal(api.asset_revision(), 11);

  const live = new AbortController();
  const liveExtract = decoder.extract(['live-member'], live.signal);
  const liveId = posted.at(-1).id;
  assert.notEqual(liveId, retiredId);
  worker.onmessage({ data: { id: retiredId, result: stale } });
  const liveResult = { files: [fileEntry('live.mpq')], variant: 'stored base files before language patches', languagePatchesApplied: false };
  worker.onmessage({ data: { id: liveId, result: liveResult } });
  const extracted = await liveExtract;
  const staged = await stageStoredInstallerFiles(extracted.files, sourceName, api, { signal: live.signal });
  assert.equal(api.beginCount, 1);
  assert.equal(api.commitCount, 1);
  assert.equal(staged.revision, 12);
  assert.equal(api.asset_revision(), 12);
  assert.deepEqual(api.stored_assets(), ['FixtureSource (stored, unpatched):live.mpq']);

  worker.onmessage({ data: { id: retiredId, result: stale } });
  assert.equal(api.beginCount, 1);
  assert.equal(api.commitCount, 1);
  assert.equal(api.asset_revision(), 12);
  decoder.close();
});

test('cancellation during a file read rolls back every staged prefix', async () => {
  const api = revisionApi(11);
  const controller = new AbortController();
  const reason = new Error('caller cancelled this import');
  const files = [fileEntry('first.mpq'), {
    path: 'second.mpq',
    file: { async arrayBuffer() { controller.abort(reason); return new ArrayBuffer(1); } },
  }];
  await assert.rejects(stageStoredInstallerFiles(files, 'FixtureSource', api, {
    signal: controller.signal,
  }), error => error === reason);
  assert.equal(api.beginCount, 1);
  assert.equal(api.commitCount, 0);
  assert.equal(api.asset_revision(), 11);
  assert.deepEqual(api.stored_assets(), ['previous']);
  assert.throws(() => api.stage_archive('late.mpq', new Uint8Array()), /no active archive import/);
});

test('the actual decoder worker ignores cancellation for a retired request id', async t => {
  const original = Object.fromEntries(['fetch', 'onmessage', 'postMessage', 'AbortController']
    .map(key => [key, Object.getOwnPropertyDescriptor(globalThis, key)]));
  t.after(() => {
    for (const [key, descriptor] of Object.entries(original)) {
      if (descriptor) Object.defineProperty(globalThis, key, descriptor);
      else delete globalThis[key];
    }
  });
  const Controller = globalThis.AbortController;
  let active, failDownload;
  const posted = [];
  globalThis.AbortController = class extends Controller { constructor() { super(); active = this; } };
  globalThis.fetch = () => new Promise((_, reject) => { failDownload = reject; });
  globalThis.onmessage = undefined;
  globalThis.postMessage = message => posted.push(message);
  await import('../../web/iso/arc-worker.js');
  const pending = globalThis.onmessage({ data: { id: 2, command: 'inspect', args: {} } });
  assert.equal(typeof failDownload, 'function');
  await globalThis.onmessage({ data: { id: 1, command: 'cancel' } });
  assert.equal(active.signal.aborted, false);
  await globalThis.onmessage({ data: { id: 2, command: 'cancel' } });
  assert.equal(active.signal.aborted, true);
  failDownload(active.signal.reason);
  await pending;
  assert.equal(posted.at(-1).id, 2);
  assert.equal(posted.at(-1).error.name, 'AbortError');
});
