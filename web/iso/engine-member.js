// One named member from a directory that is already open.
// The name selects bytes to decode. It does not identify a release.
export const CLIENT_IMAGE_BYTES = 32 * 1024 * 1024;

export function memberBaseName(path) {
  const normalized = String(path).replaceAll('\\', '/');
  const cut = normalized.lastIndexOf('/');
  return (cut < 0 ? normalized : normalized.slice(cut + 1)).toLowerCase();
}

function namedFiles(files, baseName) {
  return files.filter(file => !file.directory && memberBaseName(file.path) === baseName);
}

function withinImageBudget(member, maxImageBytes) {
  return Number.isSafeInteger(member.size) && member.size >= 1 && member.size <= maxImageBytes;
}

// Unique game.dll wins. A game.dll that is ambiguous or over the client-image
// budget does not fall through to Warcraft III.exe. With no game.dll, a unique
// Warcraft III.exe is only a named candidate.
export function selectNamedEngine(files, { maxImageBytes = CLIENT_IMAGE_BYTES } = {}) {
  const engines = namedFiles(files, 'game.dll');
  if (engines.length > 1) return { member: null, role: null, status: 'ambiguous-named-engine' };
  if (engines.length === 1) {
    if (!withinImageBudget(engines[0], maxImageBytes))
      return { member: null, role: 'named-engine', status: 'over-client-image-budget' };
    return { member: engines[0], role: 'named-engine', status: 'selected' };
  }
  const executables = namedFiles(files, 'warcraft iii.exe');
  if (executables.length > 1) return { member: null, role: null, status: 'ambiguous-executable-candidate' };
  if (executables.length === 1) {
    if (!withinImageBudget(executables[0], maxImageBytes))
      return { member: null, role: 'named-executable-candidate', status: 'over-client-image-budget' };
    return { member: executables[0], role: 'named-executable-candidate', status: 'selected' };
  }
  return { member: null, role: null, status: 'absent' };
}

// mpqMembers stay the archive transaction. The engine member is appended only
// when it fits the caller's existing selected-output and solid-output budgets.
export function planEngineMembers(archive, mpqMembers, {
  maxSelectedBytes,
  decodedBlockBytes,
  maxImageBytes = CLIENT_IMAGE_BYTES,
  reservedBytes = 0,
} = {}) {
  if (!Number.isSafeInteger(maxSelectedBytes) || maxSelectedBytes < 1)
    throw new TypeError('selected-output budget is required');
  if (!Number.isSafeInteger(decodedBlockBytes) || decodedBlockBytes < 1)
    throw new TypeError('solid output budget is required');
  if (!Number.isSafeInteger(reservedBytes) || reservedBytes < 0)
    throw new TypeError('reserved selected bytes are required');
  const picked = selectNamedEngine(archive.files, { maxImageBytes });
  const members = mpqMembers.slice();
  if (!picked.member) return { members, image: null, role: picked.role, status: picked.status };
  const block = archive.blocks?.[picked.member.block];
  if (!block || !Number.isSafeInteger(block.originalSize) || block.originalSize > decodedBlockBytes)
    return { members, image: null, role: picked.role, status: 'solid-budget' };
  const selectedBytes = reservedBytes + mpqMembers.reduce((sum, file) => sum + file.size, 0);
  if (!Number.isSafeInteger(selectedBytes) || selectedBytes + picked.member.size > maxSelectedBytes)
    return { members, image: null, role: picked.role, status: 'selected-budget' };
  members.push(picked.member);
  return { members, image: picked.member, role: picked.role, status: 'selected' };
}

// Direct ISO records only. A unique named file is returned for the existing
// slice helper. Any other payload, including an installer executable, is absent.
export function directEngineFile(files) {
  const records = (files ?? []).map(file => ({
    // ISO file versions belong to the directory record, not the basename.
    path: String(file.path).replace(/;[0-9]+$/, ''),
    size: file.size,
    directory: Boolean(file.directory),
    source: file,
  }));
  const picked = selectNamedEngine(records);
  if (!picked.member) return { file: null, role: picked.role, status: picked.status };
  return { file: picked.member.source, role: picked.role, status: 'selected' };
}

export function checkedMemberPath(value, iso = false) {
  const path = String(value).replaceAll('\\', '/');
  const normalized = iso ? path.replace(/^\//, '').replace(/;[0-9]+$/, '') : path;
  if (!normalized || normalized.includes(':') || /[\x00-\x1f\x7f]/.test(normalized)
      || normalized.split('/').some(p => !p || p === '.' || p === '..'))
    throw new Error('Invalid client source member path');
  return normalized.toLowerCase();
}
// Only the selected engine's own parent is eligible; no directory/search priority.
export function demandedModule(files, enginePath, request, { iso = false } = {}) {
  if (request?.status !== 'available') return null;
  const owner = checkedMemberPath(request.module_name);
  if (owner.includes('/')) throw new Error('Imported module owner is not a leaf');
  const main = checkedMemberPath(enginePath, iso);
  const parent = main.includes('/') ? main.slice(0, main.lastIndexOf('/') + 1) : '';
  const candidates = files.filter(f => !f.directory && checkedMemberPath(f.path, iso) === parent + owner);
  if (candidates.length > 1) throw new Error('Imported client module is ambiguous');
  const candidate = candidates[0] ?? null;
  if (candidate && !withinImageBudget(candidate, CLIENT_IMAGE_BYTES)) throw new Error('Imported module exceeds client budget');
  return candidate;
}
export async function readClientFile(file, signal) {
  signal?.throwIfAborted();
  if (!withinImageBudget(file, CLIENT_IMAGE_BYTES)) throw new Error('Client source exceeds image budget');
  const bytes = new Uint8Array(await file.arrayBuffer());
  signal?.throwIfAborted();
  if (bytes.byteLength !== file.size) throw new Error('Client source length changed');
  return bytes;
}
export function beginClientImport(api) {
  if (api.begin_archive_import_bound) return api.begin_archive_import_bound();
  api.begin_archive_import();
  return { stage_archive: api.stage_archive, stage_client_image: api.stage_client_image,
    commit: api.commit_archive_import, cancel: api.cancel_archive_import };
}
export function stageClientSources(tx, source, bytes, metadata, module = null) {
  if (tx.stage_client_sources) tx.stage_client_sources(source, bytes, JSON.stringify(metadata), module);
  else {
    if (module) throw new Error('Auxiliary sources require checked import support');
    tx.stage_client_image(source, bytes);
  }
}
