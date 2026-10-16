// Local archive picker. A file whose name ends in .exe or .dll is one optional
// client file. That file name is the source identity passed to stage_client_image.
// CLIENT_IMAGE_BYTES is the memory budget. Size and the file name do not decide
// whether the file supports source display choices.

export const CLIENT_IMAGE_BYTES = 32 * 1024 * 1024;

const CLIENT_INPUT_MESSAGE = {
  ambiguous: 'Choose one client file with the archives.',
  empty: 'The client file is empty.',
  budget: `That client file is larger than ${CLIENT_IMAGE_BYTES / (1024 * 1024)} MiB.`,
};

export function localClientInputError(reason) {
  const error = new Error(CLIENT_INPUT_MESSAGE[reason] ?? 'The client file could not be used.');
  error.localClientInput = true;
  error.reason = reason;
  return error;
}

function isClientImageFile(file) {
  const name = typeof file?.name === 'string' ? file.name : '';
  const dot = name.lastIndexOf('.');
  if (dot < 0) return false;
  const suffix = name.slice(dot + 1).toLowerCase();
  return suffix === 'exe' || suffix === 'dll';
}

// A declared length is accepted only as a safe integer from 1 through the budget.
// Zero stays empty. Any other declared length is rejected.
function clientSizeRejection(file) {
  const size = file.size;
  if (Number.isSafeInteger(size) && size >= 1 && size <= CLIENT_IMAGE_BYTES) return null;
  return size === 0 ? 'empty' : 'budget';
}

// Split one picker selection. Client identity is the chosen suffix, not file size,
// MIME type, or another file in the batch. More than one client file is ambiguous.
export function classifyLocalArchiveSelection(files) {
  const archives = [];
  const clients = [];
  for (const file of files ?? []) {
    if (isClientImageFile(file)) clients.push(file);
    else archives.push(file);
  }
  if (clients.length > 1) return { ok: false, reason: 'ambiguous' };
  const client = clients[0] ?? null;
  if (client) {
    const reason = clientSizeRejection(client);
    if (reason) return { ok: false, reason };
  }
  return { ok: true, archives, client };
}

async function readClientImage(file, signal) {
  signal?.throwIfAborted();
  const reason = clientSizeRejection(file);
  if (reason) throw localClientInputError(reason);
  const source = file.name;
  const bytes = new Uint8Array(await file.arrayBuffer());
  signal?.throwIfAborted();
  if (bytes.byteLength === 0) throw localClientInputError('empty');
  if (bytes.byteLength > CLIENT_IMAGE_BYTES) throw localClientInputError('budget');
  if (bytes.byteLength !== file.size) throw localClientInputError('budget');
  return { source, bytes };
}

// Archive-only callers pass archiveOnly and keep their existing import.
// A client file is read and rejected before begin_archive_import. Its bytes are
// passed only to stage_client_image, after the archives and before commit.
export async function importLocalArchives(files, api, options = {}) {
  const classified = classifyLocalArchiveSelection(files);
  if (!classified.ok) throw localClientInputError(classified.reason);
  if (!classified.archives.length && !classified.client) {
    return { usedTransaction: false, summaries: [], diagnostics: [], clientStaged: false };
  }
  let signal = options.signal ?? null;
  if (classified.client && options.prepareTransaction) {
    const prepared = options.prepareTransaction() ?? {};
    if (prepared.signal) signal = prepared.signal;
  }
  const image = classified.client ? await readClientImage(classified.client, signal) : null;
  if (!image && options.archiveOnly) {
    await options.archiveOnly(classified.archives);
    return { usedTransaction: false, summaries: [], diagnostics: [], clientStaged: false };
  }
  signal?.throwIfAborted();
  api.begin_archive_import();
  try {
    const summaries = [];
    let loadedBytes = 0;
    const totalBytes = classified.archives.reduce((sum, file) => sum + file.size, 0);
    for (const file of classified.archives) {
      const bytes = new Uint8Array(await file.arrayBuffer());
      signal?.throwIfAborted();
      summaries.push(api.stage_archive(file.name, bytes));
      loadedBytes += bytes.length;
      options.onArchiveProgress?.({ loadedBytes, totalBytes });
      await new Promise(resolve => setTimeout(resolve, 0));
      signal?.throwIfAborted();
    }
    signal?.throwIfAborted();
    if (image) api.stage_client_image(image.source, image.bytes);
    if (options.beforeCommit) await options.beforeCommit();
    signal?.throwIfAborted();
    const diagnostics = api.commit_archive_import();
    return { usedTransaction: true, summaries, diagnostics, clientStaged: image !== null };
  } catch (error) {
    api.cancel_archive_import();
    throw error;
  }
}
