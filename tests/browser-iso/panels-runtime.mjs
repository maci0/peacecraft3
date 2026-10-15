// Synthetic source graph through the production WASM transaction API. No draw claim.
import { menuMembers, mpqFixture, wavFixture } from './fixtures.mjs';
const text = s => new TextEncoder().encode(s);
const decode = b => new TextDecoder().decode(b);
const words = (...values) => {
  const b = new Uint8Array(values.length * 4), v = new DataView(b.buffer);
  values.forEach((n, i) => v.setUint32(i * 4, n, true)); return b;
};
const concat = (...parts) => {
  const b = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0; for (const p of parts) { b.set(p, at); at += p.length; } return b;
};
const chunk = (tag, b) => concat(text(tag), words(b.length), b);
const record = b => concat(words(b.length + 4), b);
const block = (tag, count, b) => concat(text(tag), words(count), b);
const table = rows => text('ID;PWXL;N;E\r\n' + rows.flatMap((r, y) => r.map((s, x) =>
  `C;X${x + 1};Y${y + 1};K"${s}"`)).join('\r\n') + '\r\nE\r\n');
const scriptPath = 'Scripts\\Blizzard.j', lookupPath = 'UI\\SoundInfo\\AnimLookups.slk';
const soundsPath = 'UI\\SoundInfo\\AnimSounds.slk', panelPath = 'Synthetic\\Panel.mdx';
const optionsSoundPath = 'Synthetic\\Audio\\options.wav';
const navSoundPath = 'Synthetic\\Audio\\navigation.wav';
const otherSoundPath = 'Synthetic\\Audio\\other.wav';
const soundPath = 'Synthetic\\Audio\\panel.wav', imagePath = 'Synthetic\\Panel.tga';

function model({ navigation = true, options = true, empty = false, extraStartup = false, reverseEvents = false } = {}) {
  const names = ['MainMenu Birth', 'MainMenu Stand',
    ...(navigation || options ? ['MainMenu Death'] : []),
    ...(navigation ? ['SinglePlayer Birth', 'SinglePlayer Stand', 'SinglePlayer Death'] : []),
    ...(options ? ['Options Birth', 'Options Stand', 'Options Death'] : [])];
  const sequences = names.map((name, i) => {
    const b = new Uint8Array(132), v = new DataView(b.buffer); b.set(text(name));
    v.setUint32(80, i * 100, true); v.setUint32(84, i * 100 + 73, true);
    v.setUint32(92, name.endsWith('Stand') ? 0 : 1, true); return b;
  });
  const info = new Uint8Array(372); info.set(text('Synthetic panel'));
  const texture = new Uint8Array(268); texture.set(text(imagePath), 4);
  const layer = words(28, 0, 1, 0, 0xffffffff, 0, 0x3f800000);
  const material = record(concat(words(0, 0), text('LAYS'), words(1), layer));
  const geometry = record(concat(
    block('VRTX', 3, new Uint8Array(36)), block('NRMS', 3, new Uint8Array(36)),
    block('PTYP', 1, words(4)), block('PCNT', 1, words(3)),
    block('PVTX', 3, new Uint8Array([0, 0, 1, 0, 2, 0])),
    block('MATS', 0, new Uint8Array(44)), block('UVAS', 1, new Uint8Array()),
    block('UVBS', 3, new Uint8Array(24)),
  ));
  const event = new Uint8Array(96), ev = new DataView(event.buffer);
  ev.setUint32(0, 96, true); event.set(text('SNDXTEST'), 4); ev.setInt32(88, -1, true);
  const eventAt = (code, frame) => {
    const node = event.slice(); node.set(text(`SNDX${code}`), 4);
    return concat(node, text('KEVT'), words(1, 0xffffffff, frame));
  };
  const events = [eventAt('TEST', 5)];
  if (navigation) events.push(eventAt('NEXT', names.indexOf('SinglePlayer Birth') * 100 + 5));
  if (options) events.push(eventAt('OPTS', names.indexOf('Options Birth') * 100 + 5));
  if (extraStartup) events.push(eventAt('FAIL', 6));
  if (reverseEvents) events.reverse();
  return concat(text('MDLX'), chunk('VERS', words(800)), chunk('MODL', info),
    chunk('SEQS', concat(...sequences)), chunk('MTLS', material), chunk('TEXS', texture),
    chunk('GEOS', empty ? new Uint8Array() : geometry), chunk('EVTS', concat(...events)));

}

function members(musicPath, options) {
  const b = menuMembers(musicPath), skinPath = 'UI\\war3skins.txt';
  b[skinPath] = text(decode(b[skinPath]) +
    `GlueSpriteLayerTopLeft_V0=${panelPath}\nGlueSpriteLayerTopRight_V0=${panelPath}\n` +
    'GlueSpriteLayerTopLeft_V1=Synthetic\\AbsentExpansion.mdx\nGlueSpriteLayerTopRight_V1=Synthetic\\AbsentExpansion.mdx\n');
  b[panelPath] = model(options);
  b[scriptPath] = text('function SetSoundVolumeBJ takes sound h, real p returns nothing\ncall SetSoundVolume(h, PercentToInt(p, 37))\nendfunction\n' +
    'function PercentToInt takes real p, integer m returns integer\nlocal integer r=R2I(p*I2R(m)*0.025)\nif (r<0) then\nset r=0\nendif\nif (r>m) then\nset r=m\nendif\nreturn r\nendfunction\n');
  b[lookupPath] = table([['AnimSoundEvent', 'SoundLabel'], ['TEST', 'SyntheticPanel'], ['NEXT', 'SyntheticNavigation'], ['OPTS', 'SyntheticOptions']]);
  b[soundsPath] = table([['SoundName', 'DirectoryBase', 'FileNames', 'Volume'],
    ['SyntheticPanel', 'Synthetic\\Audio\\', 'panel.wav', '19'],
    ['SyntheticNavigation', 'Synthetic\\Audio\\', 'navigation.wav', '23'],
    ['SyntheticOptions', 'Synthetic\\Audio\\', 'options.wav', '29']]);
  b[soundPath] = wavFixture(); b[navSoundPath] = wavFixture(); b[optionsSoundPath] = wavFixture();
  const image = new Uint8Array(21), v = new DataView(image.buffer);
  image[2] = 2; v.setUint16(12, 1, true); v.setUint16(14, 1, true); image[16] = 24;
  image.set([31, 91, 173], 18); b[imagePath] = image;
  return b;
}

export function runPanelChecks(api) {
  let checks = 0;
  const check = (ok, message) => { if (!ok) throw Error(message); checks++; };
  const commit = (label, data, profiles) => {
    api.begin_archive_import(); api.stage_archive(label, mpqFixture(data, profiles));
    try { return api.commit_archive_import(); }
    catch (e) { api.cancel_archive_import(); throw e; }
  };
  const snapshot = () => JSON.stringify({ revision: api.asset_revision(),
    editions: api.local_menu_editions(), assets: api.stored_assets().map(p => [p, [...api.stored_asset(p)]]) });
  const good = members(api.menu_music_path());
  const notes = commit('synthetic-panel-graph', good);
  check(!notes.some(n => n.includes('PANEL_NAVIGATION_DISABLED') || n.includes('PANEL_OPTIONS_DISABLED')), 'complete phases disabled a navigation owner');
  check(JSON.stringify(api.local_menu_editions()) === '["roc"]', 'panels enabled absent expansion');
  for (const p of [scriptPath, lookupPath, soundsPath, soundPath, navSoundPath, optionsSoundPath, panelPath, imagePath]) {
    check(JSON.stringify([...api.stored_asset(p)]) === JSON.stringify([...good[p]]), `dependency not hydrated: ${p}`);
  }
  const before = snapshot();
  const failures = [];
  for (const p of [scriptPath, lookupPath, soundsPath, soundPath, panelPath, imagePath]) {
    const missing = { ...good }; delete missing[p]; failures.push([`missing ${p}`, missing, p]);
  }
  failures.push(['malformed PCM', { ...good, [soundPath]: text('not a waveform') }, soundPath],
    ['unsupported script', { ...good, [scriptPath]: text('function Other takes nothing returns nothing\nendfunction\n') }, scriptPath],
    ['duplicate label', { ...good, [soundsPath]: table([['SoundName', 'DirectoryBase', 'FileNames', 'Volume'],
      ['SyntheticPanel', 'Synthetic\\Audio\\', 'panel.wav', '19'], ['SyntheticPanel', '', 'other.wav', '20']]) }, soundsPath],
    ['duplicate header', { ...good, [lookupPath]: table([['AnimSoundEvent', 'SoundLabel', 'SoundLabel'], ['TEST', 'SyntheticPanel', 'other']]) }, lookupPath],
    ['malformed image', { ...good, [imagePath]: text('not a texture') }, imagePath],
    ['empty panel', { ...good, [panelPath]: model({ empty: true }) }, panelPath]);
  for (const [label, candidate, context] of failures) {
    let error; try { commit(label, candidate); } catch (e) { error = String(e); }
    check(error?.includes(context), `${label} did not refuse with source context: ${error}`);
    check(snapshot() === before, `${label} changed committed assets/revision/editions`);
  }
  for (const [label, candidate] of [
    ['missing navigation waveform', (() => { const b = { ...good }; delete b[navSoundPath]; return b; })()],
    ['malformed navigation waveform', { ...good, [navSoundPath]: text('not PCM') }],
    ['missing navigation lookup', { ...good, [lookupPath]: table([['AnimSoundEvent', 'SoundLabel'], ['TEST', 'SyntheticPanel']]) }],
  ]) {
    const notes = commit(label, candidate);
    check(notes.some(n => n.includes('PANEL_NAVIGATION_DISABLED')), `${label} did not disable optional navigation`);
    check(api.stored_asset(soundPath)?.length === good[soundPath].length, `${label} lost startup audio`);
    check(api.stored_asset(navSoundPath) === undefined, `${label} published partial navigation audio`);
  }
  const profiles = { [soundPath]: { fileSize: 60, flags: 0x81000200 } };
  for (const mask of [0x01, 0x41, 0x81]) {
    const revision = api.asset_revision();
    const unavailable = commit(`unsupported-panel-sound-codec-${mask}`,
      { ...good, [soundPath]: new Uint8Array([mask, 99]) }, profiles);
    check(unavailable.some(n => n.includes('PANEL_AUDIO_UNAVAILABLE') && n.includes(soundPath.replaceAll('\\', '/')) && n.includes('compression')),
      `unsupported sound codec not diagnosed: ${unavailable}`);
    check(JSON.stringify([...api.stored_asset(panelPath)]) === JSON.stringify([...good[panelPath]]), 'codec capability discarded admitted panel geometry');
    check(JSON.stringify([...api.stored_asset(navSoundPath)]) === JSON.stringify([...good[navSoundPath]]), 'codec capability discarded another playable sound');
    check(!unavailable.some(n => n.includes('PANEL_NAVIGATION_DISABLED')), 'codec capability disabled admitted navigation');
    check(api.stored_asset(soundPath) === undefined, 'unsupported sound bytes published');
    check(JSON.stringify(api.local_menu_editions()) === '["roc"]', 'codec failure lost static RoC menu');
    check(api.asset_revision() === revision + 1, 'codec fallback did not commit exactly once');
  }
  const retained = snapshot();
  for (const reverse of [false, true]) {
    const candidate = { ...good, [soundPath]: new Uint8Array([0x81, 99]) };
    const sources = [['unsupported source', candidate, profiles], ['other matching source', { [soundPath]: good[soundPath] }, {}]];
    if (reverse) sources.reverse();
    api.begin_archive_import();
    for (const [name, data, profile] of sources) api.stage_archive(name, mpqFixture(data, profile));
    let error;
    try { api.commit_archive_import(); } catch (e) { error = String(e); api.cancel_archive_import(); }
    check(error?.includes(soundPath.replaceAll('\\', '/')), `unverified multi-source sound was muted: ${error}`);
    check(snapshot() === retained, 'unverified multi-source sound changed committed store');
  }
  let oversized;
  try {
    commit('oversized-unsupported-panel-sound', { ...good, [soundPath]: new Uint8Array([0x81, 99]) },
      { [soundPath]: { ...profiles[soundPath], fileSize: 64 * 1024 * 1024 + 1 } });
  } catch (e) { oversized = String(e); }
  check(oversized?.includes(soundPath.replaceAll('\\', '/')) && /limit|budget/.test(oversized), `oversized sound did not refuse before codec fallback: ${oversized}`);
  check(snapshot() === retained, 'oversized sound changed committed store');
  for (const mask of [0x04, 0x05]) {
    let error;
    try { commit(`reserved-panel-codec-${mask}`, { ...good, [soundPath]: new Uint8Array([mask, 99]) }, profiles); }
    catch (e) { error = String(e); }
    check(error?.includes('compression'), `reserved compression mask was accepted: ${mask}`);
    check(snapshot() === retained, 'reserved mask changed committed store');
  }
  let combinedRollbackCases = 0;
  for (const reverseEvents of [false, true]) {
    for (const missing of [false, true]) {
      const candidate = { ...good, [panelPath]: model({ extraStartup: true, reverseEvents }),
        [soundPath]: new Uint8Array([0x81, 99]),
        [lookupPath]: table([['AnimSoundEvent', 'SoundLabel'], ['TEST', 'SyntheticPanel'],
          ['NEXT', 'SyntheticNavigation'], ['FAIL', 'OtherStartup']]),
        [soundsPath]: table([['SoundName', 'DirectoryBase', 'FileNames', 'Volume'],
          ['SyntheticPanel', 'Synthetic\\Audio\\', 'panel.wav', '19'],
          ['SyntheticNavigation', 'Synthetic\\Audio\\', 'navigation.wav', '23'],
          ['OtherStartup', 'Synthetic\\Audio\\', 'other.wav', '17']]),
      };
      if (!missing) candidate[otherSoundPath] = text('malformed PCM');
      let error;
      try { commit(`mixed-codec-failure-${reverseEvents}-${missing}`, candidate, profiles); }
      catch (e) { error = String(e); }
      check(error?.includes(otherSoundPath), `unsupported codec concealed another required failure: ${error}`);
      check(snapshot() === retained, 'mixed codec failure changed committed store');
      combinedRollbackCases++;
    }
  }
  for (const reverseEvents of [false, true]) {
    const candidate = { ...good, [panelPath]: model({ extraStartup: true, reverseEvents }),
      [soundPath]: new Uint8Array([0x81, 99]), [otherSoundPath]: wavFixture(),
      [lookupPath]: table([['AnimSoundEvent', 'SoundLabel'], ['TEST', 'SyntheticPanel'],
        ['NEXT', 'SyntheticNavigation'], ['FAIL', 'OtherStartup']]),
      [soundsPath]: table([['SoundName', 'DirectoryBase', 'FileNames', 'Volume'],
        ['SyntheticPanel', 'Synthetic\\Audio\\', 'panel.wav', '19'],
        ['SyntheticNavigation', 'Synthetic\\Audio\\', 'navigation.wav', '23'],
        ['OtherStartup', 'Synthetic\\Audio\\', 'other.wav', '17']]),
    };
    const notes = commit(`mixed-playable-codec-${reverseEvents}`, candidate, profiles);
    check(notes.some(n => n.includes('PANEL_AUDIO_UNAVAILABLE')), 'mixed playable/unsupported source lost its capability diagnostic');
    for (const path of [panelPath, otherSoundPath, navSoundPath]) {
      check(JSON.stringify([...api.stored_asset(path)]) === JSON.stringify([...candidate[path]]), `mixed codec source discarded playable/geometry dependency: ${path}`);
    }
    check(api.stored_asset(soundPath) === undefined, 'mixed codec source fabricated a waveform');
  }
  const withoutOptions = commit('single-only-panels', members(api.menu_music_path(), { options: false }));
  check(withoutOptions.some(n => n.includes('PANEL_OPTIONS_DISABLED')), 'missing Options phases not diagnosed');
  check(!withoutOptions.some(n => n.includes('PANEL_NAVIGATION_DISABLED')), 'missing Options phases disabled Single Player');
  check(api.stored_asset(navSoundPath)?.length === good[navSoundPath].length, 'missing Options discarded Single Player audio');
  const withoutSingle = commit('options-only-panels', members(api.menu_music_path(), { navigation: false }));
  check(withoutSingle.some(n => n.includes('PANEL_NAVIGATION_DISABLED')), 'missing Single Player phases not diagnosed');
  check(!withoutSingle.some(n => n.includes('PANEL_OPTIONS_DISABLED')), 'missing Single Player disabled Options');
  check(api.stored_asset(optionsSoundPath)?.length === good[optionsSoundPath].length, 'independent Options audio not hydrated');
  const malformedOptions = { ...good, [optionsSoundPath]: text('invalid optional waveform') };
  const malformedNotes = commit('malformed-options-audio', malformedOptions);
  check(malformedNotes.some(n => n.includes('PANEL_OPTIONS_DISABLED')), 'malformed Options audio not diagnosed');
  check(!malformedNotes.some(n => n.includes('PANEL_NAVIGATION_DISABLED')), 'malformed Options disabled Single Player');
  check(api.stored_asset(optionsSoundPath) === undefined, 'malformed optional waveform was staged');
  const startup = commit('startup-only-panels', members(api.menu_music_path(), { navigation: false, options: false }));
  check(startup.some(n => n.includes('PANEL_NAVIGATION_DISABLED')), 'missing optional phases not diagnosed');
  check(api.stored_asset(soundPath)?.length === good[soundPath].length, 'startup dependency lost');
  return { checks, rollbackCases: failures.length, combinedRollbackCases, scope: 'synthetic production WASM hydration; no draw/audio/native proof' };
}
