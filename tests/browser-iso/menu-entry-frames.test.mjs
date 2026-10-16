import assert from 'node:assert/strict';
import test from 'node:test';
import { block, menuEntryFrames } from './app-import-runtime.mjs';

test('fixture Rust array contracts accept owned arrays and borrowed slices', () => {
  const source = 'const ITEMS_OTHER: &[&str] = &["wrong"];\nconst ITEMS: &[&str] = &["first", "second"];\nconst LAYERS: [&str; 1] = ["layer"];';
  assert.equal(block(source, 'ITEMS'), '"first", "second"');
  assert.equal(block(source, 'LAYERS'), '"layer"');
  assert.throws(() => block(source, 'MISSING'), /Missing Rust contract MISSING/);
});

test('fixture entries follow labelled source controls and ownership, not a Rust button list', () => {
  const source = `Frame "GLUETEXTBUTTON" "Style" {
ButtonText "InheritedText",
}
Frame "FRAME" "Outside" {
Frame "BUTTON" "HiddenButton" INHERITS "Style" {
}
}
Frame "FRAME" "SelectedRoot" {
Frame "BACKDROP" "AuthoredPlate" {
Frame "BUTTON" "DifferentButton" INHERITS "Style" {
SetAllPoints,
}
Frame "GLUEBUTTON" "IconOnly" {
}
}
Frame "GLUETEXTBUTTON" "SecondButton" {
ButtonText "SecondCaption",
}
}
`;
  assert.deepEqual(menuEntryFrames(source, 'SelectedRoot'), [
    { id: 'different', button: 'DifferentButton', plate: 'DifferentButton', textFrame: 'InheritedText' },
    { id: 'second', button: 'SecondButton', plate: 'SecondButton', textFrame: 'SecondCaption' },
  ]);
  assert.deepEqual(menuEntryFrames(source, 'Outside').map(entry => entry.button), ['HiddenButton']);
  assert.throws(() => menuEntryFrames(source, 'Absent'), /Missing authored control root Absent/);
});

test('derived fixture IDs preserve existing click IDs and malformed inheritance fails', () => {
  const source = `Frame "FRAME" "SelectedRoot" {
Frame "BUTTON" "SinglePlayerButton" {
ButtonText "One",
}
Frame "BUTTON" "BattleNetButton" {
ButtonText "Two",
}
Frame "BUTTON" "ExitButton" {
ButtonText "Three",
}
}
`;
  assert.deepEqual(menuEntryFrames(source, 'SelectedRoot').map(entry => entry.id), ['single_player', 'battle_net', 'exit']);
  const malformed = `Frame "FRAME" "SelectedRoot" {
Frame "BUTTON" "LoopButton" INHERITS "LoopStyle" {
}
}
Frame "BUTTON" "LoopStyle" INHERITS "LoopButton" {
}
`;
  assert.throws(() => menuEntryFrames(malformed, 'SelectedRoot'), /Frame inheritance cycle/);
});
