// Actual production worker, codec and bounded local temporary storage; no installer executes.
import assert from 'node:assert/strict';
import { openAsBlob } from 'node:fs';
import { readFile, mkdir, open, rm, mkdtemp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
import { discoverIso } from '../../web/iso.js';
import { defaultIsoCandidates } from '../../web/iso/import.js';
import { createInstallerDecoder, stageStoredInstallerFiles } from '../../web/iso/installer-import.js';
const [modulePath, wasmPath, isoPath, outputPath] = process.argv.slice(2);
assert(modulePath && wasmPath && isoPath && outputPath, 'Pass matching generated JS/WASM, explicit local ISO and receipt output');
const api = await import(pathToFileURL(resolve(modulePath)).href);
api.initSync({ module: await readFile(wasmPath) });
const temp = await mkdtemp(join(tmpdir(), 'pcx-installer-module-'));
function child(parent, name) { assert(/^[a-zA-Z0-9_-]+$/.test(name)); return join(parent, name); }
function directory(path) { return {
  async getDirectoryHandle(name) { const p = child(path, name); await mkdir(p, { recursive: true }); return directory(p); },
  async getFileHandle(name) { const p = child(path, name); return {
    async createWritable() { const file = await open(p, 'w'); let closed = false; return {
      async write(bytes) { assert(!closed); await file.write(bytes); },
      async close() { if (!closed) { closed = true; await file.close(); } },
      async abort() { if (!closed) { closed = true; await file.close(); } await rm(p, { force: true }); },
    }; }, async getFile() { return openAsBlob(p); },
  }; }, async removeEntry(name) { await rm(child(path, name), { recursive: true, force: true }); },
}; }
Object.defineProperty(globalThis.navigator, 'storage', { configurable: true, value: { getDirectory: async () => directory(temp) } });
const codecUrl = new URL('../../web/pkg/freearc/freearc-codec.wasm', import.meta.url);
globalThis.fetch = async url => { assert.equal(String(url), String(codecUrl), 'worker attempted a nonlocal codec request'); return new Response(await readFile(codecUrl)); };
let bridge;
globalThis.postMessage = data => queueMicrotask(() => bridge.onmessage?.({ data }));
globalThis.onmessage = undefined;
await import('../../web/iso/arc-worker.js');
const workerHandler = globalThis.onmessage;
class WorkerBridge {
  constructor() { assert(!bridge); bridge = this; }
  postMessage(data) { queueMicrotask(() => workerHandler({ data })); }
  terminate() { bridge = null; }
}
let latestPhase;
const decoder = createInstallerDecoder({ WorkerClass: WorkerBridge, onProgress(work) {
  if (work.phase !== latestPhase) { latestPhase = work.phase; console.error(JSON.stringify({ phase: work.phase })); }
} });
const receipt = { kind: 'supplied-stored-installer', source: isoPath };
try {
  const iso = await openAsBlob(isoPath), discovery = await discoverIso(iso);
  receipt.iso_bytes = iso.size;
  const inspected = await decoder.inspect(iso, discovery);
  const selected = defaultIsoCandidates(inspected.candidates); assert(selected.length, 'no unambiguous eligible base archive');
  const extracted = await decoder.extract(selected.map(c => c.id));
  assert(extracted.files.length); receipt.selected = extracted.files.map(f => ({ path: f.path, bytes: f.file.size }));
  let clientModule = null;
  if (extracted.clientImage) {
    const main = new Uint8Array(await extracted.clientImage.file.arrayBuffer());
    receipt.main_sha256 = createHash('sha256').update(main).digest('hex');
    receipt.request = JSON.parse(api.client_module_request(main));
    if (receipt.request.status === 'available') {
      const auxiliary = await decoder.extractClientModule(extracted.extractionId, receipt.request);
      clientModule = auxiliary.module;
      assert(clientModule, 'qualified stored main has no demanded same-source module');
      const bytes = new Uint8Array(await clientModule.file.arrayBuffer());
      receipt.module_sha256 = createHash('sha256').update(bytes).digest('hex');
    }
    extracted.clientImage.bytes = main;
  }
  receipt.result = await stageStoredInstallerFiles(extracted.files, 'local supplied stored installer', api, { clientImage: extracted.clientImage, clientModule });
  receipt.committed_module = JSON.parse(api.committed_client_module_identity());
  assert.equal(api.committed_client_image_identity(), extracted.clientImage?.source);
  if (clientModule) {
    assert.equal(receipt.committed_module.module_sha256, receipt.module_sha256);
    await assert.rejects(decoder.extractClientModule(extracted.extractionId, receipt.request), /repeated/);
  }
  console.log(JSON.stringify({ status: 'pass', receipt: outputPath, auxiliary: receipt.committed_module.status }));
} finally {
  await decoder.cleanup().catch(() => {}); decoder.close();
  await rm(temp, { recursive: true, force: true });
  const { writeFile } = await import('node:fs/promises'); await writeFile(outputPath, JSON.stringify(receipt, null, 2));
}
