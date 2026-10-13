import test from 'node:test';
import assert from 'node:assert/strict';
import { uniqueSkinFont } from './app-import-runtime.mjs';
import { menuMembers } from './fixtures.mjs';
test('synthetic selected style names a face bound to the supplied font bytes', () => {
  const m = menuMembers(), decode = bytes => new TextDecoder().decode(bytes);
  const template = decode(m['UI\\FrameDef\\Glue\\StandardTemplates.fdf']);
  const face = template.match(/FrameFont "([^"]+)"/)[1];
  const binding = uniqueSkinFont(decode(m['UI\\war3skins.txt']), face);
  assert.ok(m[binding.file]?.length > 0);
  assert.match(decode(m['UI\\FrameDef\\Glue\\MainMenu.fdf']), /SinglePlayerButton[\s\S]*ButtonText "SyntheticSelectedText"[\s\S]*INHERITS "SyntheticTextStyle"/);
});
test('skin binding refuses absent, empty and repeated faces without section priority', () => {
  assert.throws(() => uniqueSkinFont('[A]\nOther=X', 'Face'), /exactly one/);
  assert.throws(() => uniqueSkinFont('[A]\nFace=', 'Face'), /Empty/);
  for (const source of ['[A]\nFace=X\nFace=X', '[A]\nFace=X\n[B]\nFace=X', '[A]\nFace=X\n[A]\nFace=Y']) {
    assert.throws(() => uniqueSkinFont(source, 'Face'), /exactly one/);
  }
  assert.throws(() => uniqueSkinFont('Face=X', 'Face'), /Malformed/);
});
