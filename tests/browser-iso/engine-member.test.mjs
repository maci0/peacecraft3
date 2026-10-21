import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { crc32 } from '../../web/iso/freearc.js';
import { extractArcMembers } from '../../web/iso/arc-extract.js';
import { CLIENT_IMAGE_BYTES, directEngineFile, planEngineMembers, selectNamedEngine } from '../../web/iso/engine-member.js';
import { stageStoredInstallerFiles } from '../../web/iso/installer-import.js';
import { importIsoCandidates } from '../../web/iso/import.js';
import { discoverIso } from '../../web/iso.js';
import { isoFixture } from './fixtures.mjs';

const SOLID = 2 * 1024 * 1024 * 1024;
const SELECTED = 5 * 256 * 1024 * 1024;

function solidArchive(parts) {
  const all = new Uint8Array(parts.reduce((sum, part) => sum + part.bytes.length, 0));
  let at = 0;
  const files = parts.map(part => {
    const file = {
      path: part.path,
      size: part.bytes.length,
      crc: crc32(part.bytes),
      directory: Boolean(part.directory),
      block: 0,
    };
    all.set(part.bytes, at);
    at += part.bytes.length;
    return file;
  });
  return {
    all,
    archive: {
      files,
      blocks: [{ method: 'storing', offset: 0, storedSize: all.length, originalSize: all.length }],
    },
  };
}

function budgets(extra = {}) {
  return { maxSelectedBytes: SELECTED, decodedBlockBytes: SOLID, ...extra };
}

async function copyStream({ read, onWrite }) {
  const bytes = [];
  for (;;) {
    const chunk = await read(16);
    if (!chunk.length) break;
    bytes.push(...chunk);
  }
  await onWrite(new Uint8Array(bytes));
}

function memorySinks() {
  return {
    async createSink() {
      const bytes = [];
      return {
        async write(chunk) { bytes.push(...chunk); },
        async close() { return new Blob([new Uint8Array(bytes)]); },
        async abort() {},
      };
    },
  };
}

function fileEntry(path) {
  const bytes = new Uint8Array([1, 2, 3, 4]);
  return {
    path,
    file: { arrayBuffer: async () => bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) },
  };
}

function orderApi(calls) {
  return {
    begin_archive_import() { calls.push(['begin']); },
    cancel_archive_import() { calls.push(['cancel']); },
    stage_archive(name) { calls.push(['stage_archive', name]); return 'staged'; },
    stage_client_image(source, bytes) { calls.push(['stage_client_image', source, bytes.byteLength]); },
    commit_archive_import() { calls.push(['commit']); return ['edition unknown']; },
    stored_assets() { return ['asset']; },
    asset_revision() { return 3; },
  };
}

test('a unique game.dll is the named engine and hides the executable candidate', () => {
  const files = [
    { path: 'Dir\\Game.DLL', size: 12, directory: false },
    { path: 'Warcraft III.exe', size: 274432, directory: false },
    { path: 'game.dll', size: 0, directory: true },
  ];
  const picked = selectNamedEngine(files);
  assert.equal(picked.status, 'selected');
  assert.equal(picked.role, 'named-engine');
  assert.equal(picked.member.path, 'Dir\\Game.DLL');
  assert.equal(selectNamedEngine([
    { path: 'a/game.dll', size: 4, directory: false },
    { path: 'b/game.dll', size: 4, directory: false },
    { path: 'Warcraft III.exe', size: 8, directory: false },
  ]).status, 'ambiguous-named-engine');
  const over = selectNamedEngine([
    { path: 'game.dll', size: CLIENT_IMAGE_BYTES + 1, directory: false },
    { path: 'Warcraft III.exe', size: 8, directory: false },
  ]);
  assert.equal(over.status, 'over-client-image-budget');
  assert.equal(over.member, null);
});

test('with no game.dll a unique Warcraft III.exe is only a named candidate', () => {
  const picked = selectNamedEngine([
    { path: 'Frozen Throne.exe', size: 274432, directory: false },
    { path: 'Warcraft III.exe', size: 100, directory: false },
  ]);
  assert.equal(picked.status, 'selected');
  assert.equal(picked.role, 'named-executable-candidate');
  assert.equal(picked.member.size, 100);
  assert.equal(selectNamedEngine([
    { path: 'Warcraft III.exe', size: 1, directory: false },
    { path: 'other/Warcraft III.exe', size: 1, directory: false },
  ]).status, 'ambiguous-executable-candidate');
  assert.equal(directEngineFile([{ path: '/SETUP.EXE', size: 40 }]).status, 'absent');
  const direct = directEngineFile([
    { path: '/SETUP.EXE', size: 40, offset: 0 },
    { path: '/game.dll', size: 4, offset: 9 },
  ]);
  assert.equal(direct.status, 'selected');
  assert.equal(direct.file.path, '/game.dll');
  assert.equal(direct.file.offset, 9);
});

test('the engine member is appended only inside the existing selected and solid budgets', () => {
  const { archive } = solidArchive([
    { path: 'skip.txt', bytes: Uint8Array.of(7, 7) },
    { path: 'war3.mpq', bytes: Uint8Array.of(77, 80, 81, 26) },
    { path: 'game.dll', bytes: Uint8Array.of(0x4d, 0x5a, 1) },
  ]);
  const mpq = archive.files[1];
  const planned = planEngineMembers(archive, [mpq], budgets());
  assert.equal(planned.status, 'selected');
  assert.equal(planned.role, 'named-engine');
  assert.deepEqual(planned.members.map(file => file.path), ['war3.mpq', 'game.dll']);
  const selected = planEngineMembers(archive, [mpq], budgets({ maxSelectedBytes: mpq.size }));
  assert.equal(selected.status, 'selected-budget');
  assert.deepEqual(selected.members, [mpq]);
  const solid = planEngineMembers(archive, [mpq], budgets({ decodedBlockBytes: archive.blocks[0].originalSize - 1 }));
  assert.equal(solid.status, 'solid-budget');
  assert.equal(solid.image, null);
});

test('a synthetic solid stream keeps the named engine beside the MPQ and leaves the other executable', async () => {
  const mpqBytes = Uint8Array.of(77, 80, 81, 26, 1, 2);
  const dllBytes = Uint8Array.of(0x4d, 0x5a, 9, 9);
  const exeBytes = Uint8Array.of(5, 6, 7);
  const { all, archive } = solidArchive([
    { path: 'skip.txt', bytes: Uint8Array.of(1) },
    { path: 'war3.mpq', bytes: mpqBytes },
    { path: 'base\\game.dll', bytes: dllBytes },
    { path: 'Warcraft III.exe', bytes: exeBytes },
  ]);
  const plan = planEngineMembers(archive, [archive.files[1]], budgets());
  assert.equal(plan.image.path, 'base\\game.dll');
  const results = await extractArcMembers(new Blob([all]), archive, plan.members, new Uint8Array([1]),
    { decodeStream: copyStream }, { createSink: memorySinks().createSink, readBytes: 16 });
  assert.deepEqual(results.map(entry => entry.member.path), ['war3.mpq', 'base\\game.dll']);
  assert.deepEqual(new Uint8Array(await results[0].file.arrayBuffer()), mpqBytes);
  assert.deepEqual(new Uint8Array(await results[1].file.arrayBuffer()), dllBytes);

  const exeOnly = solidArchive([
    { path: 'war3.mpq', bytes: mpqBytes },
    { path: 'Warcraft III.exe', bytes: exeBytes },
  ]);
  const exePlan = planEngineMembers(exeOnly.archive, [exeOnly.archive.files[0]], budgets());
  assert.equal(exePlan.role, 'named-executable-candidate');
  const exeResults = await extractArcMembers(new Blob([exeOnly.all]), exeOnly.archive, exePlan.members, new Uint8Array([1]),
    { decodeStream: copyStream }, { createSink: memorySinks().createSink, readBytes: 16 });
  assert.deepEqual(new Uint8Array(await exeResults[1].file.arrayBuffer()), exeBytes);
});

test('staging records image order and cancellation without a second asset store', async () => {
  const image = { source: 'Fixture:/ELAMIGOS.BIN::game.dll', bytes: Uint8Array.of(0x4d, 0x5a) };
  const calls = [];
  await stageStoredInstallerFiles([fileEntry('war3.mpq')], 'Fixture', orderApi(calls), { clientImage: image });
  assert.deepEqual(calls.map(call => call[0]), ['begin', 'stage_archive', 'stage_client_image', 'commit']);
  assert.equal(calls[2][1], image.source);
  assert.equal(calls[2][2], 2);

  const cancelled = [];
  const controller = new AbortController();
  await assert.rejects(() => stageStoredInstallerFiles([fileEntry('war3.mpq')], 'Fixture', orderApi(cancelled), {
    signal: controller.signal,
    clientImage: { source: image.source, file: { size: 2, arrayBuffer: async () => Uint8Array.of(9, 9).buffer } },
    onProgress(work) { if (work.phase === 'hydrate') controller.abort(); },
  }), error => error.name === 'AbortError');
  assert.deepEqual(cancelled.map(call => call[0]), ['begin', 'stage_archive', 'stage_client_image', 'cancel']);

  const failed = [];
  const api = orderApi(failed);
  api.stage_client_image = () => { failed.push(['stage_client_image']); throw new Error('budget'); };
  await assert.rejects(() => stageStoredInstallerFiles([fileEntry('war3.mpq')], 'Fixture', api, { clientImage: image }),
    error => error.message === 'budget');
  assert.deepEqual(failed.map(call => call[0]), ['begin', 'stage_archive', 'stage_client_image', 'cancel']);

  const plain = [];
  await stageStoredInstallerFiles([fileEntry('war3.mpq')], 'Fixture', orderApi(plain), {});
  assert.deepEqual(plain.map(call => call[0]), ['begin', 'stage_archive', 'commit']);
});

test('a direct ISO slices a unique game.dll and does not slice an installer executable', async () => {
  const blob = new File([Uint8Array.of(1, 2, 3, 4, 0x4d, 0x5a, 9, 8)], 'loose.iso');
  const mpq = { path: '/A.MPQ', offset: 0, size: 4, archiveSize: 4 };
  function discovery(files) {
    return { files, candidates: [mpq], provenance: { sourceName: 'loose.iso' } };
  }
  const setupOnly = [];
  await importIsoCandidates(blob, discovery([{ path: '/SETUP.EXE', offset: 0, size: 4 }]), [mpq], orderApi(setupOnly));
  assert.deepEqual(setupOnly.map(call => call[0]), ['begin', 'stage_archive', 'commit']);

  const both = [];
  const result = await importIsoCandidates(blob, discovery([
    { path: '/SETUP.EXE', offset: 0, size: 4 },
    { path: '/game.dll', offset: 4, size: 4 },
  ]), [mpq], orderApi(both));
  assert.deepEqual(both.map(call => call[0]), ['begin', 'stage_archive', 'stage_client_image', 'commit']);
  assert.equal(both[2][1], 'loose.iso:/game.dll@4');
  assert.equal(result.clientImageStatus, 'selected');
  assert.equal(result.clientImageRole, 'named-engine');
  assert.equal(result.clientImageSource, 'loose.iso:/game.dll@4');
});

test('two sources near the global selected cap reserve the other source before the image', () => {
  const cap = 1000;
  const open = { maxSelectedBytes: cap, decodedBlockBytes: SOLID };
  function source(mpqSize, imageSize) {
    const mpq = { path: 'war3.mpq', size: mpqSize, directory: false, block: 0 };
    const dll = { path: 'game.dll', size: imageSize, directory: false, block: 1 };
    return {
      mpq,
      archive: {
        files: [mpq, dll],
        blocks: [{ originalSize: mpqSize }, { originalSize: imageSize }],
      },
    };
  }
  const left = source(600, 50);
  const right = source(400, 50);
  assert.equal(planEngineMembers(left.archive, [left.mpq], open).status, 'selected');
  const leftPlan = planEngineMembers(left.archive, [left.mpq], { ...open, reservedBytes: right.mpq.size });
  const rightPlan = planEngineMembers(right.archive, [right.mpq], { ...open, reservedBytes: left.mpq.size });
  assert.equal(leftPlan.status, 'selected-budget');
  assert.equal(leftPlan.image, null);
  assert.deepEqual(leftPlan.members.map(file => file.path), ['war3.mpq']);
  assert.equal(rightPlan.status, 'selected-budget');
  assert.equal(rightPlan.image, null);
  const fit = source(560, 40);
  assert.equal(planEngineMembers(fit.archive, [fit.mpq], { ...open, reservedBytes: 400 }).status, 'selected');
  const over = source(560, 41);
  assert.equal(planEngineMembers(over.archive, [over.mpq], { ...open, reservedBytes: 400 }).status, 'selected-budget');
  assert.throws(() => planEngineMembers(left.archive, [left.mpq], { ...open, reservedBytes: -1 }), TypeError);
});

test('client image length is checked before the read and cancellation does not commit', async () => {
  let reads = 0;
  const calls = [];
  await assert.rejects(() => stageStoredInstallerFiles([fileEntry('war3.mpq')], 'Fixture', orderApi(calls), {
    clientImage: {
      source: 'archive::game.dll',
      file: { size: CLIENT_IMAGE_BYTES + 1, arrayBuffer() { reads += 1; throw new Error('read'); } },
    },
  }), error => error.message === 'Installer client image is missing its source identity or exceeds its budget');
  assert.equal(reads, 0);
  assert.deepEqual(calls.map(call => call[0]), ['begin', 'stage_archive', 'cancel']);

  const unsized = [];
  await assert.rejects(() => stageStoredInstallerFiles([fileEntry('war3.mpq')], 'Fixture', orderApi(unsized), {
    clientImage: {
      source: 'archive::game.dll',
      file: { arrayBuffer() { reads += 1; throw new Error('read'); } },
    },
  }), error => error.message === 'Installer client image is missing its source identity or exceeds its budget');
  assert.equal(reads, 0);
  assert.deepEqual(unsized.map(call => call[0]), ['begin', 'stage_archive', 'cancel']);

  const blank = [];
  await assert.rejects(() => stageStoredInstallerFiles([fileEntry('war3.mpq')], 'Fixture', orderApi(blank), {
    clientImage: { source: '  ', bytes: Uint8Array.of(1) },
  }), error => error.message === 'Installer client image is missing its source identity or exceeds its budget');
  assert.deepEqual(blank.map(call => call[0]), ['begin', 'stage_archive', 'cancel']);

  const mismatch = [];
  await assert.rejects(() => stageStoredInstallerFiles([fileEntry('war3.mpq')], 'Fixture', orderApi(mismatch), {
    clientImage: {
      source: 'archive::game.dll',
      file: { size: 4, arrayBuffer() { reads += 1; return Promise.resolve(Uint8Array.of(1, 2).buffer); } },
    },
  }), error => error.message === 'Installer client image length does not match its declared size');
  assert.equal(reads, 1);
  assert.deepEqual(mismatch.map(call => call[0]), ['begin', 'stage_archive', 'cancel']);

  const controller = new AbortController();
  const aborted = [];
  await assert.rejects(() => stageStoredInstallerFiles([fileEntry('war3.mpq')], 'Fixture', orderApi(aborted), {
    signal: controller.signal,
    clientImage: {
      source: 'archive::game.dll',
      file: {
        size: 2,
        async arrayBuffer() { controller.abort(); return Uint8Array.of(8, 9).buffer; },
      },
    },
  }), error => error.name === 'AbortError');
  assert.deepEqual(aborted.map(call => call[0]), ['begin', 'stage_archive', 'cancel']);
});

test('the worker reserves other selected archives before the image', () => {
  const worker = readFileSync(new URL('../../web/iso/arc-worker.js', import.meta.url), 'utf8');
  assert.match(worker, /planEngineMembers/);
  assert.match(worker, /entry\.member === plan\.image/);
  assert.match(worker, /ARC_MEMBER_FORMAT/);
  assert.match(worker, /reservedBytes: mpqTotal - chosenBytes/);
  assert.match(worker, /maxSelectedBytes: MAX_SELECTED_BYTES/);
  assert.equal(worker.includes('14858960'), false);
  assert.equal(worker.includes('14837224'), false);
});

test('primary ISO version suffix selects original record without changing its provenance',async()=>{
 const bytes=Uint8Array.of(13,9,5);
 const discovery=await discoverIso(new Blob([isoFixture([{name:'Warcraft III.exe;1',bytes}])]));
 const selected=directEngineFile(discovery.files);
 assert.equal(selected.status,'selected');
 assert.equal(selected.file,discovery.files[0]);
 assert.equal(selected.file.path,'/Warcraft III.exe;1');
 assert.equal(selectNamedEngine([{path:'Warcraft III.exe;1',size:3}]).status,'absent');
});
test('ISO engine versions remain ambiguous instead of choosing a version',()=>{
 assert.equal(directEngineFile([{path:'/Game.dll;1',size:3},{path:'/Game.dll;2',size:3},{path:'/Warcraft III.exe;1',size:3}]).status,'ambiguous-named-engine');
 assert.equal(directEngineFile([{path:'/Warcraft III.exe;1',size:3},{path:'/Warcraft III.exe;2',size:3}]).status,'ambiguous-executable-candidate');
});
