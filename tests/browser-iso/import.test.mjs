import test from 'node:test';
import assert from 'node:assert/strict';
import { defaultIsoCandidate, discoverIso, importIsoCandidates } from '../../web/iso/import.js';
import { soundRows, clickDescriptor } from '../../web/iso/audio.js';
import { isoFixture, mpqFixture, menuMembers, clickMetadata } from './fixtures.mjs';
function adapter(failAt) {
  let pending, visible = ['previous'], revision = 8;
  return {
    begin_archive_import() { pending = []; },
    cancel_archive_import() { pending = null; },
    stage_archive(name, bytes) { if (failAt === 'stage') throw Error('CODEC_OR_TABLE_ERROR'); pending.push({ name, bytes }); return 'staged'; },
    commit_archive_import() { if (failAt === 'commit') throw Error('AMBIGUOUS_MEMBER'); visible = ['new']; pending = null; revision++; return ['edition unknown']; },
    stored_assets() { return visible; }, asset_revision() { return revision; },
    pending() { return pending; },
  };
}
async function source() {
  const file = new File([isoFixture([{ name: 'PAYLOAD.BIN;1', bytes: mpqFixture(menuMembers()) }])], 'authored.iso');
  return { file, discovery: await discoverIso(file) };
}
test('unique base archive is the default without guessing priority among conflicting archives', () => {
  const base = { path: '/DATA.ARC::war3.mpq', archiveSize: 40 };
  const expansion = { path: '/DATA.ARC::War3x.mpq', archiveSize: 30 };
  assert.equal(defaultIsoCandidate([expansion, base]), base);
  assert.equal(defaultIsoCandidate([base, { ...base }]), null);
  assert.equal(defaultIsoCandidate([expansion, { path: '/DATA.ARC::War3Patch.mpq', archiveSize: 20 }]), null);
  assert.equal(defaultIsoCandidate([{ path: '/WAR3.MPQ;1', archiveSize: 40 }]).path, '/WAR3.MPQ;1');
  assert.equal(defaultIsoCandidate([{ ...base, archiveSize: 513 * 1024 * 1024 }]), null);
});
test('browser importer stages bounded archive content and commits once', async () => {
  const { file, discovery } = await source(), api = adapter();
  const result = await importIsoCandidates(file, discovery, discovery.candidates, api);
  assert.equal(result.revision, 9); assert.equal(result.provenance.edition, null);
  assert.equal(result.archives[0].path, '/PAYLOAD.BIN;1'); assert.deepEqual(result.canonicalPaths, ['new']);
  assert.equal(api.pending(), null);
});
test('stage/commit failures and cancellation retain previous store and revision', async () => {
  const { file, discovery } = await source();
  for (const phase of ['stage', 'commit']) {
    const api = adapter(phase);
    await assert.rejects(importIsoCandidates(file, discovery, discovery.candidates, api));
    assert.equal(api.asset_revision(), 8); assert.deepEqual(api.stored_assets(), ['previous']); assert.equal(api.pending(), null);
  }
  const api = adapter(), controller = new AbortController();
  await assert.rejects(importIsoCandidates(file, discovery, discovery.candidates, api, { signal: controller.signal, onProgress() { controller.abort(); } }), e => e.name === 'AbortError');
  assert.equal(api.asset_revision(), 8); assert.deepEqual(api.stored_assets(), ['previous']);
});
test('selection and memory budgets reject before opening a transaction', async () => {
  const { file, discovery } = await source(), api = adapter();
  await assert.rejects(importIsoCandidates(file, discovery, [], api), /NO_SELECTION/);
  await assert.rejects(importIsoCandidates(file, discovery, [{ ...discovery.candidates[0] }], api), /INVALID_SELECTION/);
  await assert.rejects(importIsoCandidates(file, discovery, discovery.candidates, api, { maxArchiveBytes: 1 }), /memory budget/);
  assert.equal(api.asset_revision(), 8);
});
test('audio mapping/settings derive from authored cells, not static click values', () => {
  const descriptor = clickDescriptor(clickMetadata(37), 'synthetic.slk');
  assert.equal(descriptor.path, 'Synthetic\\Audio\\click.wav'); assert.equal(descriptor.settings.volume, 37 / 127);
  assert.equal(descriptor.settings.noDuplicates, true);
  const text = 'ID;PTEST\nC;X1;Y1;K"SoundName"\nC;X2;K"Notes"\nC;X1;Y2;K"Other"\nC;X2;K"contains;semicolon and ""quotes"""\nE\n';
  assert.equal(soundRows(new TextEncoder().encode(text))[0].Notes, 'contains;semicolon and "quotes"');
  assert.throws(() => clickDescriptor(clickMetadata(999), 'synthetic.slk'), /Volume/);
  assert.throws(() => soundRows(new TextEncoder().encode('ID;PTEST\nC;X1;Y1;K"unfinished')), /unterminated/);
});
