// Static Inno 5.5.0 / Pascal Script bindings. No installer code is executed.
// Format references: innoextract 1.9 loader/offsets.cpp, stream/block.cpp;
// Pascal Script TPSExec.LoadData, ReadVariable and RunScript (build 23).
export class InstallerError extends Error {
  constructor(code, message, source, offset) {
    super(`${code}: ${message}${offset === undefined ? '' : ` at member byte ${offset}`}`);
    this.name = 'InstallerError'; this.code = code; this.source = source; this.offset = offset;
  }
}
const DEFAULTS = Object.freeze({ maxExecutableBytes: 32 * 1024 * 1024, maxHeaderBytes: 8 * 1024 * 1024,
  maxDictionaryBytes: 64 * 1024 * 1024, maxScriptBytes: 2 * 1024 * 1024, maxStringBytes: 1024 * 1024,
  maxTypes: 4096, maxProcedures: 8192, maxGlobals: 8192, maxInstructions: 1000000, maxResources: 4096,
  maxBindings: 64, maxAnalysisWork: 16000000 });
function policy(values) {
  const limits = { ...DEFAULTS, ...values };
  for (const value of Object.values(limits)) if (!Number.isSafeInteger(value) || value < 1) throw new TypeError('Installer limits must be positive safe integers');
  return limits;
}
function problem(code, message, source, offset) { throw new InstallerError(code, message, source, offset); }
function bounded(value, ceiling, source, field) {
  if (!Number.isSafeInteger(value) || value < 0 || value > ceiling) problem('BUDGET', `${field} exceeds its explicit limit`, source);
}
function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
  }
  return (crc ^ 0xffffffff) >>> 0;
}
const latin = bytes => Array.from(bytes, byte => String.fromCharCode(byte)).join('');
class Reader {
  constructor(bytes, source, start = 0, end = bytes.length) { this.bytes = bytes; this.source = source; this.pos = start; this.end = end; }
  take(n) {
    if (!Number.isSafeInteger(n) || n < 0 || this.pos > this.end - n) problem('MALFORMED', 'record exceeds its container', this.source, this.pos);
    const result = this.bytes.subarray(this.pos, this.pos + n); this.pos += n; return result;
  }
  u8() { return this.take(1)[0]; }
  u16() { const b = this.take(2); return b[0] | b[1] << 8; }
  u32() { const b = this.take(4); return new DataView(b.buffer, b.byteOffset, 4).getUint32(0, true); }
  string(limits, wide = false) {
    const n = this.u32(); bounded(n * (wide ? 2 : 1), limits.maxStringBytes, this.source, 'string bytes');
    const raw = this.take(n * (wide ? 2 : 1));
    if (!wide) return { kind: 'literal', text: latin(raw), bytes: raw, encoding: 'ansi' };
    let text = '';
    for (let i = 0; i < raw.length; i += 2) text += String.fromCharCode(raw[i] | raw[i + 1] << 8);
    return { kind: 'literal', text, bytes: raw, encoding: 'utf16le' };
  }
}

function setupOffset(bytes, source, limits) {
  const read = offset => new Reader(bytes, source, offset);
  if (latin(read(0).take(2)) !== 'MZ') problem('UNSUPPORTED_PE', 'DOS signature required', source);
  const pe = read(0x3c).u32(), r = read(pe);
  if (latin(r.take(4)) !== 'PE\0\0') problem('MALFORMED_PE', 'PE signature required', source, pe);
  r.u16(); const count = r.u16(); bounded(count, limits.maxResources, source, 'PE sections');
  r.take(12); const optionalSize = r.u16(); r.u16(); const optional = r.pos;
  if (optionalSize < 120 || read(optional).u16() !== 0x10b) problem('UNSUPPORTED_PE', 'PE32 optional header required', source);
  r.take(optionalSize);
  const sections = [];
  for (let i = 0; i < count; i++) {
    r.take(8); r.u32(); const va = r.u32(), size = r.u32(), raw = r.u32(); r.take(16);
    read(raw).take(size); sections.push({ va, size, raw });
  }
  function map(va, size) {
    const found = sections.filter(s => va >= s.va && va - s.va <= s.size - size);
    if (found.length !== 1) problem('MALFORMED_PE', 'resource RVA is absent or ambiguous', source);
    return found[0].raw + va - found[0].va;
  }
  const directory = read(optional + 112), resourceVA = directory.u32(), resourceSize = directory.u32();
  const root = map(resourceVA, resourceSize);
  let resources = 0;
  function entries(relative) {
    if (relative > resourceSize - 16) problem('MALFORMED_PE', 'resource directory exceeds resource section', source);
    const table = read(root + relative); table.take(12); const n = table.u16() + table.u16();
    resources += n; bounded(resources, limits.maxResources, source, 'resource entries');
    if (relative + 16 + n * 8 > resourceSize) problem('MALFORMED_PE', 'resource entries exceed section', source);
    return Array.from({ length: n }, () => ({ id: table.u32(), target: table.u32() }));
  }
  function selected(table, id) {
    const hits = entries(table).filter(e => e.id === id);
    if (hits.length !== 1 || !(hits[0].target & 0x80000000)) problem('UNSUPPORTED_LOADER', 'unique installer resource directory required', source);
    return hits[0].target & 0x7fffffff;
  }
  const names = selected(0, 10), languages = selected(names, 11111);
  const leaves = entries(languages);
  if (leaves.length !== 1 || leaves[0].target & 0x80000000) problem('AMBIGUOUS_LOADER', 'unique installer resource language required', source);
  const entry = leaves[0].target;
  if (entry > resourceSize - 16) problem('MALFORMED_PE', 'resource data entry exceeds section', source);
  const data = read(root + entry), pointer = data.u32(), size = data.u32();
  if (size !== 44) problem('UNSUPPORTED_LOADER', 'unsupported offset table size', source);
  const offset = map(pointer, size), table = read(offset), framed = table.take(size);
  const signature = [114, 68, 108, 80, 116, 83, 205, 230, 215, 123, 11, 42];
  if (signature.some((b, i) => framed[i] !== b)) problem('UNSUPPORTED_LOADER', 'unsupported Inno loader framing', source, offset);
  if (crc32(framed.subarray(0, 40)) !== new Reader(framed, source, 40).u32()) problem('CRC', 'loader table checksum mismatch', source, offset);
  const fields = new Reader(framed, source, 12);
  if (fields.u32() !== 1) problem('UNSUPPORTED_LOADER', 'unsupported loader revision', source, offset);
  fields.u32(); fields.u32(); fields.u32(); fields.u32();
  const header = fields.u32(); fields.u32();
  read(header).take(64 + 9);
  return header;
}

async function decodeHeader(bytes, offset, source, decodeLzma, limits, signal) {
  const r = new Reader(bytes, source, offset), version = latin(r.take(64)).split('\0')[0];
  if (version !== 'Inno Setup Setup Data (5.5.0) (u)') problem('UNSUPPORTED_INNO', 'only checked Inno 5.5.0 unicode framing is supported', source, offset);
  const expected = r.u32(), header = r.take(5), h = new Reader(header, source);
  if (crc32(header) !== expected) problem('CRC', 'setup block header checksum mismatch', source, r.pos - 9);
  const size = h.u32(), compressed = h.u8();
  if (compressed > 1) problem('MALFORMED', 'invalid compression flag', source);
  bounded(size, limits.maxExecutableBytes, source, 'stored setup block');
  const stored = r.take(size), chunks = new Reader(stored, source), spans = []; let length = 0;
  while (chunks.pos < chunks.end) {
    signal?.throwIfAborted(); const checksum = chunks.u32(), span = chunks.take(Math.min(4096, chunks.end - chunks.pos));
    if (!span.length || crc32(span) !== checksum) problem('CRC', 'setup chunk checksum mismatch', source, offset + 73 + chunks.pos);
    spans.push(span); length += span.length;
  }
  bounded(length, limits.maxExecutableBytes, source, 'packed header');
  if (!compressed) bounded(length, limits.maxHeaderBytes, source, 'decoded header');
  const payload = new Uint8Array(length); let at = 0;
  for (const span of spans) { payload.set(span, at); at += span.length; }
  if (!compressed) { bounded(length, limits.maxHeaderBytes, source, 'decoded header'); return payload; }
  const props = new Reader(payload, source), properties = props.u8(), dictSize = props.u32();
  if (properties >= 225) problem('MALFORMED_LZMA', 'invalid LZMA properties', source);
  bounded(dictSize, limits.maxDictionaryBytes, source, 'LZMA dictionary');
  if (!dictSize) problem('MALFORMED_LZMA', 'zero LZMA dictionary', source);
  if (typeof decodeLzma !== 'function') problem('UNSUPPORTED_CODEC', 'bounded raw LZMA decoder required', source);
  // This framing declares no output length. The callback must stop at the
  // supplied ceiling during decoding, not allocate unrestricted output first.
  let decoded;
  try { decoded = await decodeLzma(payload.subarray(5), { properties, dictSize, maxOutputBytes: limits.maxHeaderBytes, signal }); }
  catch (error) { if (signal?.aborted) signal.throwIfAborted(); problem('DECODE', 'bounded LZMA decoder refused setup header', source); }
  signal?.throwIfAborted();
  if (!(decoded instanceof Uint8Array)) problem('DECODE', 'decoder must return bytes', source);
  bounded(decoded.length, limits.maxHeaderBytes, source, 'decoded header');
  return decoded;
}

function parseScript(bytes, source, limits) {
  const r = new Reader(bytes, source);
  if (r.u32() !== 0x53504649) problem('MALFORMED_IFPS', 'IFPS signature required', source);
  const build = r.u32(); if (build !== 23) problem('UNSUPPORTED_IFPS', 'checked Pascal Script build 23 required', source);
  const nt = r.u32(), np = r.u32(), ng = r.u32(), main = r.u32(); r.u32();
  bounded(nt, limits.maxTypes, source, 'types'); bounded(np, limits.maxProcedures, source, 'procedures'); bounded(ng, limits.maxGlobals, source, 'globals');
  if (main !== 0xffffffff && main >= np) problem('MALFORMED_IFPS', 'main procedure index out of range', source);
  const types = [], simple = new Set([1,2,3,4,5,6,7,8,9,10,13,14,16,17,18,19,20,24,28,29]);
  for (let i = 0; i < nt; i++) {
    let type = r.u8(); const exported = (type & 128) && type !== 130 && type !== 131;
    if (exported) type -= 128;
    if (simple.has(type)) {}
    else if (type === 25 || type === 21) r.string(limits);
    else if (type === 26) r.take(16);
    else if (type === 23) { if (r.u32() > 256) problem('UNSUPPORTED_IFPS', 'set type too large', source); }
    else if (type === 22) r.take(12);
    else if (type === 12) { if (r.u32() >= nt) problem('MALFORMED_IFPS', 'array type out of range', source); }
    else if (type === 11) { const n = r.u32(); bounded(n, limits.maxTypes, source, 'record fields'); for (let f = 0; f < n; f++) if (r.u32() >= nt) problem('MALFORMED_IFPS', 'record field type out of range', source); }
    else problem('UNSUPPORTED_IFPS', 'unsupported type record', source, r.pos - 1);
    if (exported) r.string(limits);
    if (r.u32()) problem('UNSUPPORTED_IFPS', 'type attributes unsupported', source);
    types.push(type);
  }
  const procedures = [];
  for (let i = 0; i < np; i++) {
    const flags = r.u8(); if (flags & ~7) problem('UNSUPPORTED_IFPS', 'unsupported procedure flags', source);
    let proc;
    if (flags & 1) { const name = latin(r.take(r.u8())); const declaration = flags & 2 ? r.string(limits).text : ''; proc = { external: true, name, declaration }; }
    else {
      const offset = r.u32(), size = r.u32(); new Reader(bytes, source, offset).take(size);
      if (!size) problem('MALFORMED_IFPS', 'empty internal procedure', source);
      proc = { external: false, offset, size };
      if (flags & 2) { proc.name = r.string(limits).text; proc.declaration = r.string(limits).text; }
    }
    if (flags & 4 && r.u32()) problem('UNSUPPORTED_IFPS', 'procedure attributes unsupported', source);
    procedures.push(proc);
  }
  for (let i = 0; i < ng; i++) { if (r.u32() >= nt) problem('MALFORMED_IFPS', 'global type out of range', source); const flags = r.u8(); if (flags & ~1) problem('UNSUPPORTED_IFPS', 'global flags unsupported', source); if (flags & 1) r.string(limits); }
  const tableEnd = r.pos, intervals = procedures.filter(p => !p.external).sort((a,b) => a.offset-b.offset);
  let end = tableEnd;
  for (const p of intervals) { if (p.offset < end) problem('MALFORMED_IFPS', 'procedure overlaps metadata or another procedure', source); end = p.offset + p.size; }
  if (end !== bytes.length) problem('MALFORMED_IFPS', 'unframed trailing script bytes', source);
  function operand(cr) {
    const kind = cr.u8(), param = cr.u32();
    if (kind === 0) return { kind: 'var', address: param };
    if (kind === 2 || kind === 3) return { kind: 'field', address: param, index: cr.u32(), dynamic: kind === 3 };
    if (kind !== 1 || param >= types.length) problem('MALFORMED_IFPS', 'invalid operand/type', source, cr.pos);
    const t = types[param];
    if ([10,14,19,28].includes(t)) return cr.string(limits, t === 19 || t === 28);
    const sizes = {1:1,2:1,18:1,3:2,4:2,20:2,5:4,6:4,21:4,7:4,8:8,9:10,17:8,29:8,24:8};
    const size = sizes[t]; if (!size) problem('UNSUPPORTED_IFPS', 'unsupported constant type', source);
    const raw = cr.take(size), v = new DataView(raw.buffer, raw.byteOffset, size);
    const value = t === 6 ? v.getInt32(0,true) : size === 1 ? raw[0] : size === 2 ? v.getUint16(0,true) : size === 4 ? v.getUint32(0,true) : undefined;
    return { kind: 'number', value };
  }
  let instructionCount = 0;
  for (const proc of procedures) {
    if (proc.external) continue;
    const cr = new Reader(bytes, source, proc.offset, proc.offset + proc.size), instructions = [];
    while (cr.pos < cr.end) {
      bounded(++instructionCount, limits.maxInstructions, source, 'instructions');
      const at = cr.pos-proc.offset, op = cr.u8(), ins = { at, op, args: [] };
      const args = n => { for (let i = 0; i < n; i++) ins.args.push(operand(cr)); };
      if ([0,14,22].includes(op)) args(2);
      else if (op === 1) { ins.calc = cr.u8(); args(2); }
      else if ([2,3,13,15,16,21,23,24].includes(op)) args(1);
      else if ([4,9,255].includes(op)) {}
      else if (op === 5) { ins.target = cr.u32(); if (ins.target >= np) problem('MALFORMED_IFPS', 'call target out of range', source); }
      else if ([6,18,25,26].includes(op)) ins.jump = cr.u32() | 0;
      else if ([7,8].includes(op)) { ins.jump = cr.u32() | 0; args(1); }
      else if (op === 10) { ins.type = cr.u32(); ins.base = cr.u32(); }
      else if (op === 11) ins.type = cr.u32();
      else if (op === 12) { ins.compare = cr.u8(); args(3); }
      else if (op === 17) { args(1); ins.not = cr.u8(); }
      else if (op === 19) { ins.offsets = [cr.u32(),cr.u32(),cr.u32(),cr.u32()]; }
      else if (op === 20) ins.position = cr.u8();
      else problem('UNSUPPORTED_IFPS', 'unsupported opcode', source, proc.offset+at);
      if (ins.type !== undefined && ins.type >= nt) problem('MALFORMED_IFPS', 'instruction type out of range', source);
      ins.end = cr.pos-proc.offset; if (ins.jump !== undefined) ins.destination = ins.end+ins.jump;
      instructions.push(ins);
    }
    const starts = new Set(instructions.map(i => i.at)); starts.add(proc.size);
    for (const ins of instructions) {
      if (ins.destination !== undefined && !starts.has(ins.destination)) problem('MALFORMED_IFPS', 'branch does not target an instruction boundary', source);
      if (ins.offsets) {
        ins.exceptionTargets = ins.offsets.filter(n => n !== 0xffffffff).map(n => ins.end + n);
        if (ins.exceptionTargets.some(at => !starts.has(at))) problem('MALFORMED_IFPS', 'exception branch does not target an instruction boundary', source);
      }
    }
    proc.instructions = instructions;
  }
  return { build, procedures, types, globals: ng };
}

function importedName(proc) {
  if (!proc.external) return null;
  if (proc.name) return proc.name.toUpperCase();
  const parts = proc.declaration.split('\0');
  if (parts[0]?.startsWith('dll:files:') && parts.length >= 3) return parts[1].toUpperCase();
  return null;
}
const unknown = () => ({ kind: 'unknown' });
function extractBindings(script, source, limits) {
  const imports = script.procedures.map(importedName), anchors = imports.map((name,i) => name === 'ISARCEXTRACT' ? i : -1).filter(i => i >= 0);
  const declarations = { EXPANDCONSTANT: [1,0], ACTIVELANGUAGE: [1], FINDFIRST: [1,0,1], FINDNEXT: [1,1], POS: [1,0,0] };
  for (const [name, expected] of Object.entries(declarations)) {
    const matches = script.procedures.filter((_,i) => imports[i] === name);
    if (matches.length > 1) problem('AMBIGUOUS_IMPORT', 'duplicate supported script import', source);
    if (matches.length && (matches[0].name.toUpperCase() !== name || latin(Uint8Array.from(expected)) !== matches[0].declaration))
      problem('UNSUPPORTED_IMPORT', 'supported import declaration does not match its checked signature', source);
  }
  if (anchors.length !== 1) problem('AMBIGUOUS_IMPORT', 'exactly one ISArcExtract import required', source);
  const target = script.procedures[anchors[0]], declaration = target.declaration.split('\0');
  const firstZero = target.declaration.indexOf('\0'), secondZero = target.declaration.indexOf('\0',firstZero+1);
  const parameters = target.declaration.slice(secondZero+1);
  // DLL framing: calling convention, delay-load, alternate search path,
  // result-present flag, then one by-reference flag per argument.
  if (!declaration[0].startsWith('dll:files:') || declaration[1]?.toUpperCase() !== 'ISARCEXTRACT' || parameters.length !== 14
    || parameters.charCodeAt(0) !== 3 || parameters.charCodeAt(3) !== 1 || [...parameters.slice(4)].some(c => c.charCodeAt(0) !== 0))
    problem('UNSUPPORTED_IMPORT', 'ISArcExtract ten-argument DLL declaration required', source);
  const bindings = [];
  let analysisWork = 0;
  for (let pi = 0; pi < script.procedures.length; pi++) {
    const proc = script.procedures[pi]; if (proc.external) continue;
    const instructionMap = new Map(proc.instructions.map(i => [i.at,i]));
    function dominates(definition, call) {
      // Reachability with a definition removed detects every alternate entry,
      // including loops. It avoids assuming linear disassembly is execution.
      const pending = [0], visited = new Set();
      while (pending.length) {
        const at = pending.pop();
        bounded(++analysisWork,limits.maxAnalysisWork,source,'static analysis work');
        if (at === definition || visited.has(at)) continue;
        if (at === call) return false;
        visited.add(at); const ins = instructionMap.get(at); if (!ins || ins.op === 9) continue;
        if (ins.exceptionTargets) pending.push(...ins.exceptionTargets);
        if (ins.destination !== undefined) pending.push(ins.destination);
        if (![6,25].includes(ins.op)) pending.push(ins.end);
      }
      return true;
    }
    // Internal calls begin with the return-address slot at the stack base.
    const stack = [unknown()], globals = new Map(), guards = []; let flag = unknown(), lastUnknownCall = -1;
    function read(op) {
      if (!op) return unknown();
      if (op.kind === 'literal' || op.kind === 'number') return op;
      if (op.kind === 'field') return { kind: 'field', record: globals.get(op.address), index: op.index, dynamic: op.dynamic };
      if (op.kind !== 'var') return unknown();
      return op.address < 0x40000000 ? globals.get(op.address) ?? unknown() : stack[op.address-0x60000000] ?? unknown();
    }
    function write(op, value) {
      if (op?.kind === 'field') { globals.delete(op.address); return; }
      if (op?.kind !== 'var') return;
      if (op.address < 0x40000000) globals.set(op.address, value);
      else { const index = op.address-0x60000000; if (index >= 0 && index < stack.length) stack[index] = value; }
    }
    function result(value) { const ref = stack.at(-1); if (ref?.kind === 'ref') write(ref.operand, value); }
    for (const ins of proc.instructions) {
      bounded(++analysisWork,limits.maxAnalysisWork,source,'static analysis work');
      const [a,b,c] = ins.args;
      if (ins.op === 11) stack.push(unknown());
      else if (ins.op === 2) stack.push(read(a));
      else if (ins.op === 3) stack.push({ kind: 'ref', operand: a });
      else if (ins.op === 4) stack.pop();
      else if (ins.op === 0) write(a, b.kind === 'literal' ? { ...b, definition: ins.at } : read(b));
      else if (ins.op === 14) write(a, { kind: 'ref', operand: b });
      else if (ins.op === 1) write(a, { kind: ins.calc === 0 ? 'concat' : ins.calc === 7 ? 'and' : 'unknown', left: read(a), right: read(b) });
      else if (ins.op === 12) write(a, { kind: 'compare', compare: ins.compare, left: read(b), right: read(c) });
      else if (ins.op === 17) flag = { kind: 'flag', expression: read(a), not: ins.not };
      else if (ins.op === 18) guards.push({ at: ins.at, destination: ins.destination, flag });
      else if ([13,15,16,21,22,23,24].includes(ins.op)) write(a, unknown());
      else if (ins.op === 5) {
        const name = imports[ins.target];
        if (name === 'EXPANDCONSTANT') result({ kind: 'expand', argument: stack.at(-2) });
        else if (name === 'ACTIVELANGUAGE') result({ kind: 'active-language' });
        else if (name === 'FINDFIRST') {
          const ptr = stack.at(-3), mask = stack.at(-2);
          if (ptr?.kind === 'ref' && ptr.operand?.kind === 'var') {
            const record = { kind: 'find-record', address: ptr.operand.address, mask, call: ins.at };
            globals.set(ptr.operand.address, record);
          }
          result(unknown());
        } else if (name === 'POS') result({ kind: 'pos', needle: stack.at(-2), haystack: stack.at(-3) });
        else if (ins.target === anchors[0]) {
          bounded(bindings.length+1,limits.maxBindings,source,'extraction bindings');
          const args = stack.slice(-11,-1).reverse(), password = args[6], path = args[2];
          if (stack.at(-1)?.kind !== 'ref' || args.length !== 10 || password?.kind !== 'literal' || password.encoding !== 'ansi')
            problem('UNSUPPORTED_PASSWORD', 'archive password must be a directly bound ANSI literal', source, proc.offset+ins.at);
          if (password.definition === undefined || !dominates(password.definition,ins.at))
            problem('UNSUPPORTED_CONTROL_FLOW', 'password assignment does not dominate extraction call', source, proc.offset+ins.at);
          if (password.definition < lastUnknownCall) problem('UNSUPPORTED_PASSWORD', 'unknown call intervenes in password dataflow', source, proc.offset+ins.at);
          let selection;
          if (path?.kind === 'expand' && path.argument?.kind === 'literal' && /^\{src\}[\\/][^{}*?]+$/i.test(path.argument.text)) {
            if (!dominates(path.argument.definition,ins.at)) problem('UNSUPPORTED_CONTROL_FLOW', 'source assignment does not dominate call', source);
            selection = { kind: 'literal', path: path.argument.text.replace(/^\{src\}[\\/]/i, '') };
          } else if (path?.kind === 'concat' && path.left?.kind === 'expand' && path.left.argument?.kind === 'literal' && /^\{src\}[\\/]$/i.test(path.left.argument.text)
            && path.right?.kind === 'field' && path.right.index === 0 && !path.right.dynamic && path.right.record?.kind === 'find-record') {
            const record = path.right.record, mask = record.mask;
            if (!dominates(record.call,ins.at) || !dominates(path.left.argument.definition,ins.at)) problem('UNSUPPORTED_CONTROL_FLOW', 'source search does not dominate extraction call', source);
            const activeGuards = guards.filter(g => g.at < ins.at && g.destination > ins.at && g.flag.not === 1 && dominates(g.at,ins.at)).map(g => g.flag.expression);
            const directoryGuard = activeGuards.find(g => g?.kind === 'compare' && g.compare === 5 && g.right?.value === 0 && g.left?.kind === 'and'
              && g.left.right?.value === 16 && g.left.left?.kind === 'field' && g.left.left.index === 1 && g.left.left.record === record);
            const languageGuard = activeGuards.find(g => g?.kind === 'compare' && g.compare === 2 && g.right?.value === 0 && g.left?.kind === 'pos'
              && g.left.needle?.kind === 'active-language' && g.left.haystack?.kind === 'field' && g.left.haystack.index === 0 && g.left.haystack.record === record);
            if (!directoryGuard || !languageGuard || mask?.kind !== 'concat' || mask.left?.kind !== 'expand' || mask.left.argument?.kind !== 'literal' || !/^\{src\}[\\/]?[^{}]*$/i.test(mask.left.argument.text) || mask.right?.kind !== 'literal')
              problem('UNSUPPORTED_SELECTION', 'language source lacks anchored FindFirst, directory exclusion or ActiveLanguage guard', source, proc.offset+ins.at);
            const pattern = mask.left.argument.text.replace(/^\{src\}[\\/]?/i,'') + mask.right.text;
            selection = { kind: 'language', pattern: pattern.replace(/^[\\/]/,''), match: 'case-sensitive-substring', excludeDirectories: true,
              evidence: { findFirstOffset: record.call, guardedCallOffset: ins.at } };
          } else problem('UNSUPPORTED_SOURCE', 'archive source is not a supported positively bound source expression', source, proc.offset+ins.at);
          const binding = { source: selection, evidence: { procedure: pi, callOffset: ins.at, import: ins.target, argumentOrder: 'reverse-stack', passwordArgument: 7 }, passwordLength: password.bytes.length };
          Object.defineProperty(binding, 'password', { value: password.bytes.slice(), enumerable: false });
          bindings.push(binding); result(unknown());
        } else {
          lastUnknownCall = ins.at;
          for (const value of stack) if (value?.kind === 'ref' && value.operand?.address < 0x40000000) globals.delete(value.operand.address);
          result(unknown());
        }
      }
    }
  }
  if (!bindings.length) problem('UNSUPPORTED_CALLS', 'no supported ISArcExtract calls found', source);
  if (bindings.filter(b => b.source.kind === 'literal').length !== 1 || bindings.filter(b => b.source.kind === 'language').length > 1)
    problem('AMBIGUOUS_CALLS', 'unique literal and at most one language binding required', source);
  return bindings;
}

/** Parse an already-decoded setup header; useful for bounded synthetic fixtures. */
export function inspectInstallerHeader(header, { source = 'local installer', limits: overrides } = {}) {
  const limits = policy(overrides); bounded(header.length, limits.maxHeaderBytes, source, 'decoded header');
  const framed = new Reader(header, source); const candidates = [];
  // Traverse consecutive length-framed fields from the actual header start.
  // Stop when fixed setup fields begin; never scan arbitrary literal occurrences.
  while (framed.pos <= framed.end - 4) {
    const at = framed.pos, size = framed.u32();
    if (size > framed.end-framed.pos) break;
    if (size >= 4 && latin(header.subarray(framed.pos,framed.pos+4)) === 'IFPS') {
      bounded(size, limits.maxScriptBytes, source, 'compiled script');
      candidates.push({ at: framed.pos, bytes: framed.take(size) });
    } else framed.take(size);
    if (framed.pos <= at) problem('MALFORMED', 'nonadvancing header field', source);
  }
  if (candidates.length !== 1) problem('AMBIGUOUS_SCRIPT', 'exactly one length-framed IFPS container required', source);
  const script = parseScript(candidates[0].bytes, source, limits), bindings = extractBindings(script, source, limits);
  const diagnostic = { format: 'Inno 5.5.0 unicode / IFPS build 23', scriptOffset: candidates[0].at, scriptBytes: candidates[0].bytes.length,
    bindings: bindings.map(({ source: path, evidence, passwordLength }) => ({ source: path, evidence, password: '[redacted]', passwordLength })) };
  return { bindings, diagnostic };
}

export async function inspectInstaller(blob, executableEntry, { decodeLzma, signal, limits: overrides } = {}) {
  const limits = policy(overrides), source = executableEntry?.path ?? 'local installer';
  const offset = executableEntry?.offset, size = executableEntry?.size;
  if (!blob || typeof blob.slice !== 'function' || !Number.isSafeInteger(offset) || offset < 0 || !Number.isSafeInteger(size) || size < 0 || offset > blob.size-size)
    problem('SOURCE', 'executable entry exceeds supplied Blob', source);
  bounded(size, limits.maxExecutableBytes, source, 'executable bytes'); signal?.throwIfAborted();
  const bytes = new Uint8Array(await blob.slice(offset,offset+size).arrayBuffer()); signal?.throwIfAborted();
  if (bytes.length !== size) problem('SOURCE', 'short executable Blob read', source);
  const headerOffset = setupOffset(bytes, source, limits);
  const header = await decodeHeader(bytes, headerOffset, source, decodeLzma, limits, signal);
  return inspectInstallerHeader(header, { source, limits });
}
