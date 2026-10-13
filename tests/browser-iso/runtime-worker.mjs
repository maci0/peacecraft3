import init, * as api from '/web/pkg/peacecraft3.js';
import { discoverIso, importIsoCandidates } from '/web/iso/import.js';
import { clickDescriptor } from '/web/iso/audio.js';
import { isoFixture, mpqFixture, menuMembers } from './fixtures.mjs';
let checks = 0;
const check = (condition, message) => { if (!condition) throw Error(message); checks++; };
try {
  await init();
  const musicPath = api.menu_music_path();
  const members = menuMembers(musicPath);
  const file = new File([isoFixture([{ name: 'NOT-NAMED.MPQ;1', bytes: mpqFixture(members) }])], 'synthetic-local.iso');
  const discovery = await discoverIso(file), before = api.asset_revision();
  api.begin_archive_import(); api.stage_archive('staging-proof', mpqFixture(members));
  check(api.asset_revision() === before, 'staging changed revision');
  check(api.stored_assets().length === 0, 'staging exposed partial assets'); api.cancel_archive_import();
  const result = await importIsoCandidates(file, discovery, discovery.candidates, api);
  check(result.revision === before + 1, 'commit must increment revision exactly once');
  const namedPath = 'Synthetic\\NamedOnly.txt';
  const named = api.archive_member(namedPath);
  check(new TextDecoder().decode(named) === 'named extraction without a listfile', 'actual Rust named extraction failed');
  check(api.stored_asset('menu3d/mainmenu3d.mdx')?.length > 0, 'model alias hydration missing');
  check(api.stored_asset('menu3d/texture.blp')?.length === 4, 'authored model texture hydration missing');
  check(api.stored_asset('menu/standardtemplates.fdf')?.length > 0, 'FDF alias hydration missing');
  const fontFiles = api.menu_font_files();
  check(fontFiles.length === 1, 'selected synthetic style must derive exactly one font');
  const fontAlias = api.menu_font_alias(fontFiles[0]);
  check(fontAlias?.startsWith('menu/fonts/'), 'derived font alias missing');
  check(api.stored_asset(fontAlias)?.length > 0, 'derived font hydration missing');
  check(api.stored_asset('menu/war3skins.txt')?.length > 0, 'skin alias hydration missing');
  const click = clickDescriptor(api.archive_member('UI\\SoundInfo\\UISounds.slk'), 'UI\\SoundInfo\\UISounds.slk');
  check(click.settings.volume === 37 / 127, 'audio volume was not derived');
  check(api.archive_member(click.path).length === 60, 'actual Rust click extraction failed');
  check(api.archive_member(musicPath).length === 60, 'music contract extraction failed');
  const priorPaths = api.stored_assets(), priorRevision = api.asset_revision();
  const conflict = { ...members, [fontFiles[0]]: new TextEncoder().encode('unequal synthetic font') };
  const conflictingFile = new File([isoFixture([{ name: 'A.BIN;1', bytes: mpqFixture(members) }, { name: 'B.BIN;1', bytes: mpqFixture(conflict) }])], 'synthetic-conflict.iso');
  const conflicts = await discoverIso(conflictingFile); let error;
  try { await importIsoCandidates(conflictingFile, conflicts, conflicts.candidates, api); } catch (e) { error = String(e); }
  check(error?.includes('AMBIGUOUS_MEMBER'), 'unequal archive bytes did not fail closed');
  check(api.asset_revision() === priorRevision, 'failed commit changed revision');
  check(JSON.stringify(api.stored_assets()) === JSON.stringify(priorPaths), 'failed commit exposed partial assets');
  const skinPath = Object.keys(members).find(path => path.toLowerCase().endsWith('war3skins.txt'));
  const skin = new TextDecoder().decode(members[skinPath]);
  const binding = skin.split(/\r?\n/).find(line => line.includes('='));
  const duplicateSkin = { ...members, [skinPath]: new TextEncoder().encode(`${skin}\n[OtherSyntheticSection]\n${binding}\n`) };
  api.begin_archive_import(); api.stage_archive('duplicate-skin', mpqFixture(duplicateSkin));
  let duplicateError; try { api.commit_archive_import(); } catch (e) { duplicateError = String(e); api.cancel_archive_import(); }
  check(duplicateError?.includes('SelectedSyntheticFace'), 'duplicate face across sections did not fail with face context');
  check(api.asset_revision() === priorRevision, 'duplicate skin rejection changed revision');
  check(JSON.stringify(api.stored_assets()) === JSON.stringify(priorPaths), 'duplicate skin rejection changed committed paths');
  const text = new TextEncoder(), decode = new TextDecoder();
  const framePath = Object.keys(members).find(path => path.toLowerCase().endsWith('mainmenu.fdf'));
  const secondFontPath = `OtherFonts\\${fontFiles[0].split(/[\\/]/).pop()}`;
  const extraFrame = '\nFrame "BUTTON" "ExitButton" {\n ButtonText "SecondSyntheticText",\n Frame "TEXT" "SecondSyntheticText" INHERITS "SyntheticTextStyle" {\n FrameFont "SecondSyntheticFace", 0.017, "",\n Text "SYNTHETIC_LABEL",\n }\n}\n';
  const aliasMembers = { ...members,
    [skinPath]: text.encode(`${skin}\nSecondSyntheticFace=${secondFontPath}\n`),
    [framePath]: text.encode(decode.decode(members[framePath]) + extraFrame),
    [secondFontPath]: text.encode('different bytes for the same served basename'),
  };
  api.begin_archive_import(); api.stage_archive('alias-conflict', mpqFixture(aliasMembers));
  let aliasError; try { api.commit_archive_import(); } catch (e) { aliasError = String(e); api.cancel_archive_import(); }
  check(aliasError?.includes('AMBIGUOUS_ASSET_ALIAS'), 'different fonts sharing a served alias were not refused');
  check(api.asset_revision() === priorRevision, 'alias rejection changed revision');
  check(JSON.stringify(api.stored_assets()) === JSON.stringify(priorPaths), 'alias rejection exposed partial assets');
  aliasMembers[secondFontPath] = members[fontFiles[0]];
  api.begin_archive_import(); api.stage_archive('equal-alias', mpqFixture(aliasMembers)); api.commit_archive_import();
  check(api.asset_revision() === priorRevision + 1, 'equal aliases should commit once');
  check(api.menu_font_files().length === 2, 'equal aliases lost one canonical font binding');
  const beforeEqualArchives = api.asset_revision();
  api.begin_archive_import(); api.stage_archive('equal-1', mpqFixture(members)); api.stage_archive('equal-2', mpqFixture(members));
  api.commit_archive_import(); check(api.asset_revision() === beforeEqualArchives + 1, 'equal duplicates should commit once');
  postMessage({ ok: true, result, checks, fonts: { files: fontFiles, aliases: fontFiles.map(path => api.menu_font_alias(path)) }, selftest: api.selftest(), audio: { click, musicPath, music: [...api.archive_member(musicPath)], clickBytes: [...api.archive_member(click.path)] } });
} catch (error) { postMessage({ ok: false, error: String(error), stack: error?.stack }); }
