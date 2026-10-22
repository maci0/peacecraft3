import test from 'node:test';
import assert from 'node:assert/strict';
import { defaultIsoCandidates, discoverIso, importIsoCandidates } from '../../web/iso/import.js';
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
const completeEditionCandidates = [
  ['/elamigos-1.bin::war3.mpq', 447331787],
  ['/elamigos-1.bin::War3Patch.mpq', 125201961],
  ['/elamigos-1.bin::War3x.mpq', 402990907],
  ['/elamigos-1.bin::War3xlocal.mpq', 46073074],
  ['/elamigos-1.bin::Movies\\HumanEd.mpq', 20393390],
  ['/elamigos-1.bin::Movies\\HumanOp.mpq', 26366936],
  ['/elamigos-1.bin::Movies\\IntroX.mpq', 19385684],
  ['/elamigos-1.bin::Movies\\NightElfEd.mpq', 19138382],
  ['/elamigos-1.bin::Movies\\OrcEd.mpq', 28432146],
  ['/elamigos-1.bin::Movies\\OutroX.mpq', 23743732],
  ['/elamigos-1.bin::Movies\\TutorialIn.mpq', 16447210],
  ['/elamigos-1.bin::Movies\\TutorialOp.mpq', 21283740],
  ['/elamigos-1.bin::Movies\\UndeadEd.mpq', 15052312],
].map(([path, archiveSize]) => ({ path, archiveSize }));

test('unique base archive is selected independently of unrelated installer members', () => {
  const base = { path: '/DATA.ARC::war3.mpq', archiveSize: 40 };
  const expansion = { path: '/DATA.ARC::War3x.mpq', archiveSize: 30 };
  assert.deepEqual(defaultIsoCandidates([expansion, base]), [base]);
  const local = { path: '/War3Local.mpq', archiveSize: 300 * 1024 * 1024 };
  const expansionLocal = { path: '/War3xLocal.mpq', archiveSize: 300 * 1024 * 1024 };
  assert.deepEqual(defaultIsoCandidates([base, expansion, local, expansionLocal]), [base]);
  assert.deepEqual(defaultIsoCandidates(completeEditionCandidates), [completeEditionCandidates[0]]);
  assert.equal(defaultIsoCandidates(completeEditionCandidates)[0], completeEditionCandidates[0]);
  assert.deepEqual(defaultIsoCandidates([{ path: '/WAR3.MPQ;1', archiveSize: 40 }]).map(c => c.path), ['/WAR3.MPQ;1']);
  assert.deepEqual(defaultIsoCandidates([base, { ...expansion, archiveSize: 1024 * 1024 * 1024 }]), [base]);
});

test('missing, duplicate or oversized base refuses an automatic selection', () => {
  const base = { path: '/DATA.ARC::war3.mpq', archiveSize: 40 };
  const expansion = { path: '/DATA.ARC::War3x.mpq', archiveSize: 30 };
  const expansionLocal = { path: '/DATA.ARC::War3xlocal.mpq', archiveSize: 20 };
  const patch = { path: '/DATA.ARC::War3Patch.mpq', archiveSize: 10 };
  assert.deepEqual(defaultIsoCandidates([base, { ...base }]), []);
  assert.deepEqual(defaultIsoCandidates([expansion, expansionLocal, patch]), []);
  assert.deepEqual(defaultIsoCandidates([base, { ...base, path: '/WAR3.MPQ;1' }]), []);
  assert.deepEqual(defaultIsoCandidates([{ ...base, archiveSize: 513 * 1024 * 1024 }]), []);
  assert.deepEqual(defaultIsoCandidates([]), []);
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
