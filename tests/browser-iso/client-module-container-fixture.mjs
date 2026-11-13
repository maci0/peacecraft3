// Builds only a local test container from explicit supplied files; never upload its output.
import assert from 'node:assert/strict';
import { readFile, writeFile, stat } from 'node:fs/promises';
import { basename } from 'node:path';
import { script, setupHeader, executable } from './installer-fixtures.mjs';
import { isoFixture, mpqFixture, menuMembers } from './fixtures.mjs';
import { crc32 } from '../../web/iso/freearc.js';
const [mainPath, modulePath, output] = process.argv.slice(2);
assert(mainPath && modulePath && output, 'Pass explicit local main/module and local output ISO path');
for (const path of [mainPath, modulePath]) { const info = await stat(path); assert(info.size > 0 && info.size <= 32 * 1024 * 1024, 'client fixture budget'); }
const main = await readFile(mainPath), auxiliary = await readFile(modulePath);
const integer = n => {
  const value = BigInt(n);
  for (let width = 1; width <= 8; width++) if (value < 1n << BigInt(width * 7)) {
    let v = value << BigInt(width) | (1n << BigInt(width - 1)) - 1n;
    return Uint8Array.from({ length: width }, () => { const byte = Number(v & 255n); v >>= 8n; return byte; });
  }
  throw new Error('fixture integer exceeds admitted size');
};
const string = s => Buffer.concat([Buffer.from(s), Buffer.from([0])]);
const u32 = n => { const b = Buffer.alloc(4); b.writeUInt32LE(n >>> 0); return b; };
const join = (...parts) => Buffer.concat(parts.map(p => Buffer.from(p)));
const entries = [
  { name: 'war3.mpq', bytes: mpqFixture(menuMembers()) },
  { name: `game/${basename(mainPath)}`, bytes: main },
  { name: `game/${basename(modulePath)}`, bytes: auxiliary },
];
const payload = join(...entries.map(e => e.bytes));
// Literal REP and stored 4x4 subblocks exercise the admitted production stream chain.
const blockBytes = 32 * 1024, repParts = [u32(64 * 1024)];
for (let at = 0; at < payload.length; at += blockBytes) {
  const literal = payload.subarray(at, at + blockBytes);
  const body = join(u32(0), u32(literal.length), literal);
  repParts.push(u32(body.length), body);
}
repParts.push(u32(0));
const rep = join(...repParts), packedParts = [u32(0)];
for (let at = 0; at < rep.length; at += blockBytes) {
  const block = rep.subarray(at, at + blockBytes);
  packedParts.push(u32(0xffffffff), u32(block.length), block);
}
const packed = join(...packedParts);
const directory = join(integer(1), integer(entries.length), string('rep:64kb+4x4:tor:32kb'), integer(packed.length), integer(packed.length),
  integer(1), string(''), ...entries.map(e => string(e.name)), ...entries.map(() => integer(0)),
  ...entries.map(e => integer(e.bytes.length)), ...entries.map(() => u32(0)), ...entries.map(() => [0]), ...entries.map(e => u32(crc32(e.bytes))));
const footer = join(integer(1), integer(3), string('storing'), integer(directory.length), integer(directory.length), integer(directory.length), u32(crc32(directory)), [0], integer(0));
const descriptor = join([65, 114, 67, 1], integer(4), string('storing'), integer(footer.length), integer(footer.length), u32(crc32(footer)));
const arc = join([65, 114, 67, 1], packed, directory, footer, descriptor, u32(crc32(descriptor)));
const iso = isoFixture([{ name: 'SETUP.EXE;1', bytes: executable(setupHeader(script())) }, { name: 'fixture.arc', bytes: arc }]);
await writeFile(output, iso);
console.log(JSON.stringify({ status: 'created local synthetic stored container', bytes: iso.length }));
