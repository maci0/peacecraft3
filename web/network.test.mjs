import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { decodeSnapshot } from './network.js';

const golden = JSON.parse(await readFile(new URL('./snapshot-selection-golden.json', import.meta.url), 'utf8'));
const bytesOf = hex => {
  const copy = new Uint8Array(Buffer.from(hex, 'hex'));
  return copy.buffer;
};
const shape = ['id', 'owner', 'x', 'y', 'moveTarget', 'selected'];

function assertWorld(actual, expected) {
  assert.equal(typeof actual.tick, 'bigint');
  assert.equal(actual.tick, BigInt(expected.tick));
  assert.equal(actual.nextUnitId, expected.nextUnitId);
  assert.deepEqual(Object.keys(actual).sort(), ['nextUnitId', 'tick', 'units']);
  assert.equal(actual.units.length, expected.units.length);
  for (let i = 0; i < actual.units.length; i++) {
    assert.deepEqual(Object.keys(actual.units[i]), shape);
    assert.deepEqual(actual.units[i], expected.units[i]);
  }
}

test('rust encoder packets keep tick, identity, order, target and selection', () => {
  for (const entry of golden.accepted) {
    const world = decodeSnapshot(bytesOf(entry.bytes));
    assertWorld(world, entry);
  }
  const v0 = golden.accepted.find(entry => entry.name === 'v0_default_selected');
  const narrowed = golden.accepted.find(entry => entry.name === 'v1_narrowed');
  const raw = hex => Buffer.from(hex, 'hex');
  assert.equal(raw(v0.bytes).includes(Buffer.from('SEL')), false);
  assert.equal(raw(narrowed.bytes).includes(Buffer.from('SEL')), true);
  assert.equal(narrowed.units[1].selected, false);
  assert.ok(BigInt(golden.accepted.find(entry => entry.name === 'v1_reordered').tick) > BigInt(Number.MAX_SAFE_INTEGER));
});

test('unknown version, truncation, bad flags and trailing bytes are refused whole', () => {
  for (const entry of golden.refused) {
    assert.equal(typeof entry.rust, 'string');
    assert.ok(entry.rust.length > 0, entry.name);
    assert.throws(() => decodeSnapshot(bytesOf(entry.bytes)), error => error.message === entry.error, entry.name);
  }
});

// Drops a SEL suffix and decodes the remainder as version 0. That accepts the
// packet while marking every unit selected.
function ignoreSelectionTrailer(hex) {
  const raw = Buffer.from(hex, 'hex');
  const tag = raw.indexOf('SEL');
  return decodeSnapshot(bytesOf((tag < 0 ? raw : raw.subarray(0, tag)).toString('hex')));
}

test('ignoring the selection trailer loses the narrowed flag', () => {
  const narrowed = golden.accepted.find(entry => entry.name === 'v1_narrowed');
  const faithful = decodeSnapshot(bytesOf(narrowed.bytes));
  const mutant = ignoreSelectionTrailer(narrowed.bytes);
  assert.equal(faithful.units[1].selected, false);
  assert.equal(mutant.units[1].selected, true);
  assert.deepEqual(mutant.units.map(unit => unit.moveTarget), faithful.units.map(unit => unit.moveTarget));
  assert.notDeepEqual(mutant.units.map(unit => unit.selected), faithful.units.map(unit => unit.selected));
});
