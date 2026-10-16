// Independent local decoder worker. It never runs installer code or sends files.
import { loadFreeArcCodec } from '../pkg/freearc/freearc-codec.mjs';
import { inspectInstaller } from './installer.js';
import { ArcError, checkedLimits, openFreeArc, redactMethod } from './freearc.js';
import { extractArcMembers, createTemporaryArcStorage } from './arc-extract.js';
import { planEngineMembers } from './engine-member.js';

let codec, current, currentId, state, storage;
// Independent per-attempt decoded-intermediate work policy, not a measured asset
// size, REP final-output estimate or working-memory allowance.
const MAX_INTERMEDIATE_BYTES = 2 * 1024 * 1024 * 1024;
const MAX_SELECTED_BYTES = 1024 * 1024 * 1024;
let lastProgress = 0;
const progress = work => {
  const now = performance.now();
  if (work.phase !== 'decode' || now - lastProgress >= 100) { postMessage({ type: 'progress', work }); lastProgress = now; }
};
async function loadCodec() {
  if (!codec) {
    progress({ phase: 'Loading local installer decoder' });
    const response = await fetch(new URL('../pkg/freearc/freearc-codec.wasm', import.meta.url));
    if (!response.ok) throw new Error(`Decoder download: HTTP ${response.status}`);
    codec = await loadFreeArcCodec(await response.arrayBuffer());
  }
  return {
    decodeLzma: (bytes, options) => codec.decodeLzma(bytes, options),
    decodeStream: options => codec.decodeStream({ ...options, maxMemoryBytes: 256 * 1024 * 1024 }),
    async aesCtrTransform(bytes, { key, iv }, { blockOffset = 0, signal } = {}) {
      signal?.throwIfAborted();
      const cipher = codec.aesCtr(key, iv, BigInt(blockOffset));
      try { return cipher.transform(bytes); }
      finally { cipher.close(); }
    },
  };
}
function relativeSource(path) {
  const normalized = path.replaceAll('\\', '/');
  if (!normalized || normalized.startsWith('/') || normalized.includes(':') || normalized.split('/').some(p => !p || p === '.' || p === '..'))
    throw new Error('Unsupported installer source path');
  return normalized;
}
async function inspect({ iso, discovery }, signal) {
  const activeCodec = await loadCodec(); signal.throwIfAborted();
  const executables = discovery.diagnostics.filter(d => d.code === 'UNSUPPORTED_PE_PAYLOAD');
  if (executables.length !== 1) throw new Error('Exactly one checked installer executable is required; no installer priority guessed');
  progress({ phase: 'Checking local installer metadata' });
  const installer = await inspectInstaller(iso, executables[0], { decodeLzma: activeCodec.decodeLzma, signal });
  const bindings = installer.bindings.filter(binding => binding.source.kind === 'literal');
  if (bindings.length !== 1) throw new Error('Exactly one authored base archive binding is required');
  const sources = [], candidates = [];
  for (const binding of bindings) {
    const path = `/${relativeSource(binding.source.path)}`;
    const entries = discovery.files.filter(entry => entry.path === path);
    if (entries.length !== 1) throw new Error('Installer source member is missing or ambiguous in the selected namespace');
    const entry = entries[0], blob = iso.slice(entry.offset, entry.offset + entry.size);
    progress({ phase: 'Reading encrypted archive directory locally' });
    const archive = await openFreeArc(blob, binding.password, activeCodec, { signal });
    const sourceIndex = sources.length;
    sources.push({ entry, blob, archive, password: binding.password });
    for (let memberIndex = 0; memberIndex < archive.files.length; memberIndex++) {
      const member = archive.files[memberIndex];
      if (member.directory || !member.path.toLowerCase().endsWith('.mpq')) continue;
      candidates.push({ id: `${sourceIndex}:${memberIndex}`, path: `${entry.path}::${member.path}`, archiveSize: member.size,
        validation: 'directory-only', source: 'stored base files before language patches' });
    }
  }
  state = { sources, candidates, activeCodec };
  return { candidates, provenance: discovery.provenance, diagnostic: { installer: installer.diagnostic,
    variant: 'stored base files before language patches', languagePatchesApplied: false,
    sources: sources.map(source => ({ path: source.entry.path, offset: source.entry.offset, size: source.entry.size,
      entries: source.archive.files.length, blocks: source.archive.blocks.map(block => ({ ...block, method: redactMethod(block.method) })) })) } };
}
async function extract({ ids }, signal) {
  if (!state || !Array.isArray(ids) || !ids.length || new Set(ids).size !== ids.length) throw new Error('Invalid installer member selection');
  const wanted = ids.map(id => {
    const candidate = state.candidates.find(candidate => candidate.id === id);
    if (!candidate) throw new Error('Installer candidate was not discovered in this transaction');
    return candidate;
  });
  const mpqTotal = wanted.reduce((sum, c) => sum + c.archiveSize, 0);
  if (mpqTotal > MAX_SELECTED_BYTES) throw new Error('Selected archive bytes exceed the local import budget');
  await storage?.remove(); storage = await createTemporaryArcStorage();
  const files = [];
  let clientImage = null;
  let clientImageStatus = 'absent';
  const solidBudget = checkedLimits().decodedBlockBytes;
  for (let index = 0; index < state.sources.length; index++) {
    const source = state.sources[index], chosen = wanted.filter(c => Number(c.id.split(':')[0]) === index);
    if (!chosen.length) continue;
    const selected = chosen.map(candidate => source.archive.files[Number(candidate.id.split(':')[1])]);
    const chosenBytes = chosen.reduce((sum, candidate) => sum + candidate.archiveSize, 0);
    const plan = planEngineMembers(source.archive, selected, {
      maxSelectedBytes: MAX_SELECTED_BYTES, decodedBlockBytes: solidBudget,
      reservedBytes: mpqTotal - chosenBytes,
    });
    let takeImage = Boolean(plan.image);
    if (takeImage && (clientImage || clientImageStatus === 'ambiguous-across-sources')) {
      clientImage = null;
      clientImageStatus = 'ambiguous-across-sources';
      takeImage = false;
    }
    const extracted = await extractArcMembers(source.blob, source.archive, takeImage ? plan.members : selected, source.password, state.activeCodec,
      { signal, createSink: storage.createSink, onProgress: progress, maxIntermediateBytes: MAX_INTERMEDIATE_BYTES, maxSelectedBytes: MAX_SELECTED_BYTES });
    for (const entry of extracted) {
      if (takeImage && entry.member === plan.image) {
        clientImage = { source: `${source.entry.path}::${entry.member.path}`, role: plan.role, file: entry.file };
        clientImageStatus = 'selected';
        continue;
      }
      const candidate = chosen.find(c => source.archive.files[Number(c.id.split(':')[1])] === entry.member);
      const header = new Uint8Array(await entry.file.slice(0, 32).arrayBuffer());
      if (header.length < 32 || header[0] !== 77 || header[1] !== 80 || header[2] !== 81 || header[3] !== 26)
        throw new ArcError('ARC_MEMBER_FORMAT', `decoded MPQ-named file has no supported MPQ header: ${entry.member.path}`);
      files.push({ id: candidate.id, path: candidate.path, file: entry.file });
    }
    if (!clientImage && clientImageStatus !== 'ambiguous-across-sources') clientImageStatus = plan.status;
  }
  return { files, clientImage, clientImageStatus, variant: 'stored base files before language patches', languagePatchesApplied: false };
}
onmessage = async event => {
  const { id, command, args } = event.data ?? {};
  if (command === 'cancel') {
    if (current && currentId === id) current.abort();
    return;
  }
  if (current) { postMessage({ id, error: { message: 'Installer decoder is busy' } }); return; }
  currentId = id;
  current = new AbortController();
  try {
    let result;
    if (command === 'inspect') result = await inspect(args, current.signal);
    else if (command === 'extract') result = await extract(args, current.signal);
    else if (command === 'cleanup') { await storage?.remove(); storage = undefined; result = true; }
    else throw new Error('Unknown installer decoder command');
    postMessage({ id, result });
  } catch (error) {
    await storage?.remove().catch(() => {}); storage = undefined;
    postMessage({ id, error: { name: error.name, code: error.code, message: error.message } });
  } finally { current = undefined; currentId = undefined; }
};
