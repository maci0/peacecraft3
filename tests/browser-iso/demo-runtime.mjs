// Actual generated-WASM/DOM gate against the asset-free Pages package.
import assert from 'node:assert/strict';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { isoFixture, mpqFixture, menuMembers } from './fixtures.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
const output = resolve(process.env.PCX_DEMO_OUTPUT ?? `${root}.scratch/pages-demo-root/browser`);
assert(output.startsWith(resolve(root, '.scratch') + '/'), 'captures must stay in private scratch');
const origin = process.env.PCX_DEMO_ORIGIN ?? 'http://127.0.0.1:8944/peacecraft3-demo/';
const { chromium } = await import(pathToFileURL(process.env.PCX_PLAYWRIGHT_MODULE ?? `${root}.scratch/browser-check/node_modules/playwright-core/index.mjs`));
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', args: ['--no-sandbox', '--enable-unsafe-swiftshader'] });
const requests = [], responses = [], errors = [], logs = [], sockets = [];
let proof, installerImport;
try {
  const page = await browser.newPage({ viewport: { width: 1200, height: 1000 } });
  page.setDefaultTimeout(120000);
  page.on('request', r => requests.push({ url: r.url(), method: r.method(), body: r.postDataBuffer() !== null }));
  page.on('response', r => responses.push({ url: r.url(), status: r.status() }));
  page.on('pageerror', e => errors.push(e.message));
  page.on('console', m => logs.push(m.text()));
  page.on('websocket', s => sockets.push(s.url()));
  await page.addInitScript(() => {
    window.loadingPhases = [];
    new MutationObserver(() => {
      const status = document.querySelector('#wasm-status');
      if (status && window.loadingPhases.at(-1) !== status.textContent) window.loadingPhases.push(status.textContent);
    }).observe(document, { subtree: true, childList: true, characterData: true });
  });
  await page.goto(origin, { waitUntil: 'domcontentloaded' });
  if (process.env.PCX_DEMO_CAPTURE_DOWNLOAD) {
    await page.waitForFunction(() => {
      const bar = document.querySelector('#wasm-progress');
      return bar?.hasAttribute('value') && bar.value > 0 && bar.value < bar.max;
    });
    assert(await page.locator('#iso').isDisabled(), 'file selection must wait for initialized handlers');
    assert(await page.locator('#archives').isDisabled());
    await page.screenshot({ path: `${output}/wasm-download-progress.png`, fullPage: true });
  }
  await page.waitForFunction(() => globalThis.peacecraft3?.local_menu_editions);
  assert.match(await page.locator('#wasm-status').textContent(), /Demo ready/);
  assert.equal(await page.locator('#wasm-progress').getAttribute('data-state'), 'complete');
  assert(!(await page.locator('#iso').isDisabled()), 'file selection must unlock once ready');
  assert((await page.evaluate(() => window.loadingPhases)).some(p => p.startsWith('Downloading demo')));
  assert.deepEqual(await page.evaluate(() => peacecraft3.stored_assets()), []);
  assert.deepEqual(await page.evaluate(() => peacecraft3.local_menu_editions()), []);
  assert(await page.locator('#demo-start').isDisabled());

  await page.screenshot({ path: `${output}/empty-desktop.png`, fullPage: true });
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: `${output}/empty-mobile.png`, fullPage: true });
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'mobile horizontal overflow');
  await page.setViewportSize({ width: 1200, height: 1000 });

  if (process.env.PCX_DEMO_INSTALLER_ISO) {
    await page.locator('#iso').setInputFiles(process.env.PCX_DEMO_INSTALLER_ISO);
    await page.waitForFunction(() => !document.querySelector('#iso').disabled);
    const discovery = await page.locator('#iso-provenance').textContent();
    assert(discovery.trim(), `Installer discovery did not produce provenance: ${await page.locator('#iso-progress').textContent()}`);
    const details = JSON.parse(discovery);
    assert.equal(details.provenance.filesystem, 'ISO9660-Joliet');
    assert.equal(details.diagnostics.filter(d => d.code === 'UNSUPPORTED_FREEARC').length, 6);
    assert.equal(details.installer.languagePatchesApplied, false);
    assert.match(details.installer.variant, /stored base files/);
    assert.equal(await page.locator('#iso-byte-progress').getAttribute('data-state'), 'complete');
    assert(await page.locator('#iso-candidates input').count() > 0);
    const selectedPaths = await page.locator('#iso-candidates input:checked').evaluateAll(inputs => inputs.map(input => input.parentElement.textContent.trim().split(' — ')[0]));
    assert.equal(selectedPaths.length, 1, 'the supplied installer should select only one archive');
    assert.match(selectedPaths[0], /(?:^|[\\/:])war3\.mpq$/i, 'the supplied installer should select its base archive');
    assert.deepEqual(await page.evaluate(() => peacecraft3.local_menu_editions()), []);
    await page.screenshot({ path: `${output}/original-installer-directory.png`, fullPage: true });
    if (process.env.PCX_DEMO_INSTALLER_MEMBER) {
      page.setDefaultTimeout(300000);
      const selection = await page.locator('#iso-candidates label').evaluateAll((labels, member) => labels.findIndex(label => label.textContent.trim().startsWith(member + ' —')), process.env.PCX_DEMO_INSTALLER_MEMBER);
      assert(selection >= 0, 'caller-selected installer member was not discovered');
      if (!(await page.locator('#iso-candidates input').nth(selection).isChecked()))
        for (const input of await page.locator('#iso-candidates input:checked').all()) await input.uncheck();
      const revision = await page.evaluate(() => peacecraft3.asset_revision());
      if (process.env.PCX_DEMO_INSTALLER_REFUSED_MEMBER) {
        const refused = await page.locator('#iso-candidates label').evaluateAll((labels, member) => labels.findIndex(label => label.textContent.trim().startsWith(member + ' —')), process.env.PCX_DEMO_INSTALLER_REFUSED_MEMBER);
        assert(refused >= 0);
        await page.locator('#iso-candidates input').nth(refused).check();
        await page.locator('#iso-import').click();
        await page.waitForFunction(() => !document.querySelector('#iso').disabled);
        assert.match(await page.locator('#iso-progress').textContent(), /ARC_MEMBER_FORMAT/);
        assert.equal(await page.evaluate(() => peacecraft3.asset_revision()), revision);
        await page.locator('#iso-candidates input').nth(refused).uncheck();
        await page.screenshot({ path: `${output}/installer-non-mpq-refusal.png`, fullPage: true });
      }
      await page.locator('#iso-candidates input').nth(selection).check();
      if (process.env.PCX_DEMO_INSTALLER_CANCEL) {
        await page.locator('#iso-import').click();
        await page.waitForFunction(() => document.querySelector('#iso-byte-progress').value > 0 && document.querySelector('#iso').disabled);
        await page.screenshot({ path: `${output}/installer-decode-progress.png`, fullPage: true });
        await page.locator('#iso-cancel').click();
        await page.waitForFunction(() => !document.querySelector('#iso').disabled);
        assert.match(await page.locator('#iso-progress').textContent(), /cancelled/i);
        assert.equal(await page.evaluate(() => peacecraft3.asset_revision()), revision);
      }
      await page.locator('#iso-import').click();
      await page.waitForFunction(() => !document.querySelector('#iso').disabled);
      assert.equal(await page.evaluate(() => peacecraft3.asset_revision()), revision + 1, await page.locator('#iso-progress').textContent());
      const imported = JSON.parse(await page.locator('#iso-provenance').textContent());
      assert.equal(imported.provenance.languagePatchesApplied, false);
      assert(imported.canonicalPaths.length > 0);
      const expectedEdition = process.env.PCX_DEMO_INSTALLER_EDITION;
      if (expectedEdition) assert.deepEqual(await page.evaluate(() => peacecraft3.local_menu_editions()), [expectedEdition]);
      await page.locator('#demo-start').click();
      await page.waitForFunction(() => peacecraft3.active_local_menu_edition() !== undefined && document.querySelector('#launcher-canvas').width > 0);
      await page.waitForFunction(() => document.querySelector('#launcher-canvas').width > 0);
      await page.waitForTimeout(1000);
      assert(logs.some(line => line.includes('Original menu background:')), 'authored menu scene did not load');
      await page.locator('#launcher-canvas').screenshot({ path: `${output}/installer-base-menu.png` });
      const temporaryFiles = await page.evaluate(async () => {
        const directory = await navigator.storage.getDirectory(), names = [];
        for await (const name of directory.keys()) if (name.startsWith('peacecraft3-import-')) names.push(name);
        return names;
      });
      assert.deepEqual(temporaryFiles, [], 'temporary decoded archive was retained');
      installerImport = { member: process.env.PCX_DEMO_INSTALLER_MEMBER, provenance: imported.provenance,
        canonicalPathCount: imported.canonicalPaths.length, revision: imported.revision,
        editions: await page.evaluate(() => peacecraft3.local_menu_editions()),
        temporaryFiles, cancellationChecked: Boolean(process.env.PCX_DEMO_INSTALLER_CANCEL),
        nonMpqRefusalChecked: Boolean(process.env.PCX_DEMO_INSTALLER_REFUSED_MEMBER) };
    }
  }

  async function commit(input, candidateCount = 1) {
    const before = await page.evaluate(() => peacecraft3.asset_revision());
    await page.locator('#iso').setInputFiles(input);
    await page.waitForFunction(count => document.querySelectorAll('#iso-candidates input').length === count && !document.querySelector('#iso').disabled, candidateCount);
    assert.equal(await page.locator('#iso-candidates input:checked').count(), 1, 'one relevant archive should be selected by default');
    await page.locator('#iso-import').click();
    await page.waitForFunction(() => !document.querySelector('#iso').disabled);
    assert.equal(await page.evaluate(() => peacecraft3.asset_revision()), before + 1,
      await page.locator('#iso-progress').textContent());
    assert.equal(await page.locator('#iso-byte-progress').getAttribute('data-state'), 'complete');
  }

  // Synthetic storage/edition policy proof only; this placeholder model/font
  // does not prove rendering or native edition identity.
  const base = menuMembers();
  base['UI\\FrameDef\\Glue\\MainMenu.fdf'] = new TextEncoder().encode('Frame "FRAME" "ControlLayer" {\n}\n');
  const conflicting = { 'UI\\FrameDef\\GlobalStrings.fdf': new TextEncoder().encode('StringList { SYNTHETIC_LABEL "Conflicting label", }') };
  await commit({ name: 'base.iso', mimeType: 'application/x-iso9660-image', buffer: Buffer.from(isoFixture([
    { name: 'WAR3.MPQ;1', bytes: mpqFixture(base) },
    { name: 'WAR3X.MPQ;1', bytes: mpqFixture(conflicting) },
  ])) }, 2);
  assert.deepEqual(await page.evaluate(() => peacecraft3.local_menu_editions()), ['roc']);
  assert.deepEqual(await page.locator('#demo-edition option').evaluateAll(options => options.map(o => o.value)), ['roc']);
  const rejected = await page.evaluate(() => {
    try { peacecraft3.open_local_menu('tft'); return false; }
    catch (error) { return String(error).includes('UNAVAILABLE_MENU_EDITION'); }
  });
  assert(rejected, 'base-only import enabled TFT');
  await page.locator('#demo-start').click();
  await page.waitForFunction(() => peacecraft3.active_local_menu_edition() === 'roc');
  await page.getByText('Already have extracted archives?', { exact: true }).click();
  await page.locator('#drop').click();
  await page.waitForFunction(() => peacecraft3.local_menu_editions().length === 0);
  assert(await page.locator('#demo-start').isDisabled());

  // Delay one real File read and dispatch a competing selection/clear event.
  // The shared import guard must keep ownership of the first transaction.
  const beforeDelayed = await page.evaluate(() => peacecraft3.asset_revision());
  await page.evaluate(() => {
    const original = File.prototype.arrayBuffer;
    File.prototype.arrayBuffer = function () {
      if (this.name !== 'delayed.mpq') return original.call(this);
      return new Promise(resolve => {
        window.releaseDemoRead = async () => {
          File.prototype.arrayBuffer = original;
          resolve(await original.call(this));
        };
      });
    };
  });
  await page.locator('#archives').setInputFiles({ name: 'delayed.mpq', mimeType: 'application/octet-stream', buffer: Buffer.from(mpqFixture(base)) });
  await page.waitForFunction(() => typeof window.releaseDemoRead === 'function');
  assert(await page.locator('#iso').isDisabled());
  assert(await page.locator('#drop').isDisabled());
  assert(await page.locator('#map-config-limit').isDisabled());
  assert(await page.locator('#map-config-file').isDisabled());
  assert.equal(await page.locator('#iso-byte-progress').getAttribute('value'), null, 'unmeasured local read should remain indeterminate');
  await page.screenshot({ path: `${output}/local-import-progress.png`, fullPage: true });
  await page.locator('#archives').setInputFiles({ name: 'competing.mpq', mimeType: 'application/octet-stream', buffer: Buffer.from(mpqFixture({ ...base, 'Synthetic\\NamedOnly.txt': new TextEncoder().encode('competing bytes') })) });
  await page.locator('#drop').dispatchEvent('click');
  assert.equal(await page.evaluate(() => peacecraft3.asset_revision()), beforeDelayed);
  await page.evaluate(() => window.releaseDemoRead());
  await page.waitForFunction(prior => peacecraft3.asset_revision() === prior + 1, beforeDelayed);
  await page.waitForFunction(() => !document.querySelector('#archives').disabled);
  assert(!(await page.locator('#map-config-limit').isDisabled()));
  assert(!(await page.locator('#map-config-file').isDisabled()));
  assert.equal(await page.evaluate(() => new TextDecoder().decode(peacecraft3.archive_member('Synthetic\\NamedOnly.txt'))), new TextDecoder().decode(base['Synthetic\\NamedOnly.txt']));

  if (process.env.PCX_DEMO_PREPARED) {
    const fixture = JSON.parse(await readFile(process.env.PCX_DEMO_PREPARED, 'utf8'));
    await commit(fixture.isoPath);
    assert.deepEqual(await page.evaluate(() => peacecraft3.local_menu_editions()), ['tft']);
    await page.locator('#demo-start').click();
    await page.waitForFunction(() => peacecraft3.active_local_menu_edition() === 'tft');
    await page.waitForFunction(() => document.querySelector('#launcher-canvas').width > 0);
    const expectedScene = `Original menu background: ${fixture.expectedScene.geosets} geosets, ${fixture.expectedScene.paths.length} textures, ${fixture.expectedScene.duration} ms loop; missing []`;
    const deadline = Date.now() + 120000;
    while (!logs.some(l => l.includes(expectedScene))) {
      assert(Date.now() < deadline, 'original scene did not rebuild');
      await page.waitForTimeout(250);
    }
    for (const label of fixture.labels) {
      const font = fixture.fonts.find(f => f.id === label.id);
      assert(logs.some(l => l.includes(`Menu label ${font.textFrame}: ${JSON.stringify(label.visible)} (hotkey `) && l.includes(`face ${JSON.stringify(font.face)}`)));
    }
    await page.locator('#launcher-canvas').screenshot({ path: `${output}/local-original-menu.png` });
  }
  const unexpected = requests.filter(r => r.method !== 'GET' || r.body || (!r.url.startsWith(origin) && !r.url.startsWith('blob:')) || /\/assets\//.test(r.url));
  assert.deepEqual(unexpected, [], 'hosted assets or upload/external request');
  assert.deepEqual(errors, []);
  assert.deepEqual(sockets, []);
  proof = { ok: true, requests, errors, sockets, baseOnlyRejectsTft: rejected, overlappingImportRefused: true,
    loadingPhases: await page.evaluate(() => window.loadingPhases),
    originalJolietInstallerChecked: Boolean(process.env.PCX_DEMO_INSTALLER_ISO),
    installerImport,
    originalMenuChecked: Boolean(process.env.PCX_DEMO_PREPARED),
    limitation: 'Synthetic base edition-policy proof and optional locally repacked original menu; not retail installer ISO or native/full-game fidelity' };
  await writeFile(`${output}/proof.json`, JSON.stringify(proof, null, 2));
} finally {
  await writeFile(`${output}/diagnostics.json`, JSON.stringify({ requests, responses, errors, logs, sockets }, null, 2));
  await browser.close();
}
console.log(JSON.stringify({ ok: proof.ok, baseOnlyRejectsTft: proof.baseOnlyRejectsTft, originalMenuChecked: proof.originalMenuChecked, requests: requests.length, errors: errors.length, sockets: sockets.length, output }));
