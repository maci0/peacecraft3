// Real generated WASM transactions and production importers. All bytes remain local.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { resolve, basename } from 'node:path';
import { pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
import { importLocalArchives } from '../../web/local-archives.js';
import { discoverIso, importIsoCandidates } from '../../web/iso/import.js';
import { stageStoredInstallerFiles } from '../../web/iso/installer-import.js';
import { menuMembers, mpqFixture, isoFixture } from './fixtures.mjs';
const [modulePath, wasmPath, mainPath, backendPath] = process.argv.slice(2);
assert(modulePath && wasmPath && mainPath && backendPath, 'Pass generated JS/WASM and explicit local main/backend paths');
const api = await import(pathToFileURL(resolve(modulePath)).href);
api.initSync({ module: await readFile(wasmPath) });
const main = await readFile(mainPath), backend = await readFile(backendPath);
const sha = b => createHash('sha256').update(b).digest('hex');
const request = JSON.parse(api.client_module_request(main));
assert.equal(request.status, 'available'); assert.equal(request.main_sha256, sha(main));
assert.equal(request.module_name.toLowerCase(), basename(backendPath).toLowerCase());
const archive = mpqFixture(menuMembers(api.menu_music_path()));
const mainFile = new File([main], basename(mainPath)), moduleFile = new File([backend], basename(backendPath));
const payload = new File([archive], 'payload.mpq');
const status = () => JSON.parse(api.committed_client_module_identity());
const check = source => {
  const s = status(); assert.equal(s.status, 'available'); assert.equal(s.module_sha256, sha(backend));
  assert.equal(s.main_sha256, sha(main)); assert.equal(s.policy_status, 'available'); assert.equal(api.committed_client_image_identity(), source); return s;
};
await importLocalArchives([payload, mainFile], api, { clientModuleFile: moduleFile });
const local = check(mainFile.name);
// Stale tickets cannot stage, commit or cancel a newer import, even with identical bytes.
const retired = api.begin_archive_import_bound(); const live = api.begin_archive_import_bound();
retired.cancel(); assert.throws(() => retired.stage_archive('old', archive), /retired/);
live.stage_archive('live', archive);
live.stage_client_sources(mainFile.name, main, JSON.stringify({ kind: 'local', container: 'explicit loose co-selection', main_member: mainFile.name, module_member: moduleFile.name, module_source: moduleFile.name }), backend);
live.commit(); assert.equal(status().generation, local.generation);
const revision = api.asset_revision(), before = status();
const bad = api.begin_archive_import_bound(); bad.stage_archive('bad', mpqFixture({}));
bad.stage_client_sources(mainFile.name, main, JSON.stringify({ kind: 'local', container: 'explicit loose co-selection', main_member: mainFile.name, module_member: moduleFile.name, module_source: moduleFile.name }), backend);
assert.throws(() => bad.commit()); bad.cancel(); assert.equal(api.asset_revision(), revision); assert.deepEqual(status(), before);
const wrong = api.begin_archive_import_bound(); wrong.stage_archive('wrong', archive);
assert.throws(() => wrong.stage_client_sources(mainFile.name, main, JSON.stringify({ kind: 'iso', container: 'fixture', main_member: 'game/main.exe', module_member: `other/${moduleFile.name}`, module_source: 'elsewhere' }), backend), /same-parent/);
wrong.stage_client_sources(mainFile.name, main, JSON.stringify({ kind: 'local', container: 'explicit loose co-selection', main_member: mainFile.name, module_member: moduleFile.name, module_source: moduleFile.name }), backend);
wrong.commit(); assert.equal(status().generation, before.generation);
// A local ISO assembled from supplied members proves this pipeline, not retail ISO acceptance.
const iso = new File([isoFixture([{ name: 'PAYLOAD.BIN;1', bytes: archive }, { name: `${mainFile.name};1`, bytes: main }, { name: `${moduleFile.name};1`, bytes: backend }])], 'local-client-fixture.iso');
const discovery = await discoverIso(iso);
await importIsoCandidates(iso, discovery, discovery.candidates, api);
const direct = status(); assert.equal(direct.status, 'available'); assert.equal(direct.module_sha256, sha(backend)); assert.equal(direct.generation, local.generation);
// Stored staging uses the actual WASM carrier; real worker extraction is a separate gate.
await stageStoredInstallerFiles([{ path: 'stored::payload.mpq', file: payload }], 'fixture-container', api, {
  clientImage: { source: `base.arc::game/${mainFile.name}`, member: `game/${mainFile.name}`, file: mainFile },
  clientModule: { source: `base.arc::game/${moduleFile.name}`, member: `game/${moduleFile.name}`, mainSource: `base.arc::game/${mainFile.name}`, file: moduleFile },
});
check(`base.arc::game/${mainFile.name}`); assert.equal(status().generation, local.generation);
const badBackend = Uint8Array.from(backend); badBackend[badBackend.length - 1] ^= 1;
await importLocalArchives([payload, mainFile], api, { clientModuleFile: new File([badBackend], moduleFile.name) });
assert.equal(status().policy_status, 'unavailable'); assert.equal(status().module_sha256, sha(badBackend));
await importLocalArchives([payload, mainFile], api); assert.equal(status().status, 'absent');
await importLocalArchives([payload, mainFile], api, { clientModuleFile: moduleFile });
api.add_archive('legacy', archive); assert.equal(status().status, 'unattested');
api.drop_assets(); assert.equal(status().status, 'unattested');
console.log(JSON.stringify({ main_sha256: sha(main), module_sha256: sha(backend), generation: local.generation, pipelines: ['local', 'direct-local-built-iso', 'stored-installer-staging'], result: 'pass' }));
