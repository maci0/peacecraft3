import { discoverIso, sliceIsoFile } from '../iso.js';
import { directEngineFile } from './engine-member.js';

export function defaultIsoCandidates(candidates) {
  const eligible = candidates.filter(candidate => candidate.archiveSize <= 512 * 1024 * 1024);
  if (eligible.length !== candidates.length || eligible.reduce((sum, candidate) => sum + candidate.archiveSize, 0) > 5 * 256 * 1024 * 1024) return [];
  const names = eligible.map(candidate => candidate.path.split(/[\\/:]/).at(-1).replace(/;\d+$/, '').toLowerCase());
  if (names.length === 1) return eligible;
  const family = new Set(['war3.mpq', 'war3local.mpq', 'war3x.mpq', 'war3xlocal.mpq', 'deprecated.mpq']);
  return names.filter(name => name === 'war3.mpq').length === 1 && new Set(names).size === names.length && names.every(name => family.has(name)) ? eligible : [];
}

// Resource budgets, not measured game data. Each archive is copied separately.
export async function importIsoCandidates(file, discovery, selected, api, {
  signal, onProgress, maxArchiveBytes = 512 * 1024 * 1024,
  maxTotalBytes = 5 * 256 * 1024 * 1024,
} = {}) {
  if (!selected.length) throw new Error('NO_SELECTION: select at least one content-validated MPQ');
  let total = 0;
  for (const candidate of selected) {
    if (!discovery.candidates.includes(candidate)) throw new Error('INVALID_SELECTION: candidate is not from this discovery');
    if (candidate.archiveSize > maxArchiveBytes) throw new Error(`${candidate.path}: archive exceeds ${maxArchiveBytes} byte memory budget`);
    total += candidate.archiveSize;
  }
  if (total > maxTotalBytes) throw new Error(`selected archives exceed ${maxTotalBytes} byte memory budget`);
  signal?.throwIfAborted(); api.begin_archive_import();
  try {
    let imported = 0;
    for (const candidate of selected) {
      signal?.throwIfAborted();
      const bytes = new Uint8Array(await sliceIsoFile(file, { offset: candidate.offset, size: candidate.archiveSize }).arrayBuffer());
      signal?.throwIfAborted();
      const summary = api.stage_archive(`${discovery.provenance.sourceName ?? 'local Blob'}:${candidate.path}@${candidate.offset}`, bytes);
      imported += bytes.length;
      onProgress?.({ phase: 'archives', imported, total, path: candidate.path, summary });
      // Yield for cancel and relative UI progress before the next archive.
      await new Promise(resolve => setTimeout(resolve, 0));
    }
    const engine = directEngineFile(discovery.files);
    let clientImageSource = null;
    if (engine.file) {
      signal?.throwIfAborted();
      const image = new Uint8Array(await sliceIsoFile(file, engine.file).arrayBuffer());
      if (image.byteLength !== engine.file.size) throw new Error('ISO client image length does not match its directory record');
      clientImageSource = `${discovery.provenance.sourceName ?? 'local Blob'}:${engine.file.path}@${engine.file.offset}`;
      api.stage_client_image(clientImageSource, image);
    }
    signal?.throwIfAborted();
    onProgress?.({ phase: 'hydrate', imported, total });
    await new Promise(resolve => setTimeout(resolve, 0));
    signal?.throwIfAborted();
    const diagnostics = api.commit_archive_import();
    return { provenance: discovery.provenance, archives: selected.map(({ path, offset, archiveSize }) => ({ path, offset, size: archiveSize })),
      canonicalPaths: api.stored_assets(), revision: api.asset_revision(), diagnostics,
      clientImageStatus: engine.status, clientImageSource, clientImageRole: engine.role };
  } catch (error) { api.cancel_archive_import(); throw error; }
}
export { discoverIso };
