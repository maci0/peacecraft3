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
