import test from 'node:test';
import assert from 'node:assert/strict';
import {
  CLIENT_IMAGE_BYTES,
  classifyLocalArchiveSelection,
  importLocalArchives,
  localClientInputError,
} from '../../web/local-archives.js';

// The fake records call arguments. Its visible names change only inside
// commit_archive_import. It does not publish a client image and it does not
// model Rust hydrate or a published profile.

function recordingApi(start = 4) {
  const calls = [];
  let pending = null;
  let revision = start;
  let visible = ['previous'];
  const api = {
    calls,
    begin_archive_import() {
      calls.push({ op: 'begin_archive_import' });
      pending = [];
    },
    cancel_archive_import() {
      calls.push({ op: 'cancel_archive_import' });
      pending = null;
    },
    stage_archive(name, bytes) {
      calls.push({ op: 'stage_archive', name, bytes });
      if (!pending) throw new Error('no active archive import');
      pending.push({ name });
      return `staged ${name}`;
    },
    stage_client_image(source, bytes) {
      calls.push({ op: 'stage_client_image', source, bytes });
      if (!pending) throw new Error('no active archive import');
    },
    commit_archive_import() {
      calls.push({ op: 'commit_archive_import' });
      if (!pending) throw new Error('no active archive import');
      visible = pending.map(entry => entry.name);
      pending = null;
      revision += 1;
      return ['edition unknown'];
    },
    add_archive() { throw new Error('add_archive is outside this helper'); },
    add_asset() { throw new Error('add_asset is outside this helper'); },
    stored_assets() { return visible.slice(); },
    asset_revision() { return revision; },
  };
  return api;
}

function picked(name, bytes, size = bytes.length) {
  const body = Uint8Array.from(bytes);
  const file = {
    name,
    size,
    reads: 0,
    async arrayBuffer() {
      file.reads += 1;
      return body.buffer.slice(body.byteOffset, body.byteOffset + body.byteLength);
    },
  };
  return file;
}

function ops(api) {
  return api.calls.map(call => call.op);
}

function imageCalls(api) {
  return api.calls.filter(call => call.op === 'stage_client_image');
}

test('suffix selects one client file and leaves every other file an archive', () => {
  const mpq = picked('delayed.mpq', [1]);
  mpq.type = 'application/x-msdownload';
  const dll = picked('Notes.DLL', [2]);
  dll.type = 'application/octet-stream';
  const result = classifyLocalArchiveSelection([
    mpq,
    picked('Root.MPQ', [3]),
    dll,
    picked('bare', [4]),
    picked('pack.exe.mpq', [5]),
    picked('pack.mpq.exe', [6]),
  ]);
  assert.equal(result.ok, false);
  assert.equal(result.reason, 'ambiguous');

  const one = classifyLocalArchiveSelection([
    mpq,
    picked('Root.MPQ', [3]),
    dll,
    picked('bare', [4]),
    picked('pack.exe.mpq', [5]),
  ]);
  assert.equal(one.ok, true);
  assert.equal(one.client, dll);
  assert.deepEqual(one.archives.map(file => file.name), ['delayed.mpq', 'Root.MPQ', 'bare', 'pack.exe.mpq']);
  assert.equal(classifyLocalArchiveSelection([picked('X.ExE', [1])]).client.name, 'X.ExE');
});

test('client input messages stay on the file choice', () => {
  assert.equal(localClientInputError('ambiguous').message, 'Choose one client file with the archives.');
  assert.equal(localClientInputError('empty').message, 'The client file is empty.');
  assert.equal(localClientInputError('budget').message, 'That client file is larger than 32 MiB.');
  assert.equal(localClientInputError('ambiguous').localClientInput, true);
});

test('an empty selection does not open a transaction', async () => {
  const api = recordingApi();
  const result = await importLocalArchives([], api);
  assert.equal(result.usedTransaction, false);
  assert.equal(result.clientStaged, false);
  assert.deepEqual(ops(api), []);
  assert.deepEqual(api.stored_assets(), ['previous']);
});

test('archive-only import can keep the caller path and never opens a transaction', async () => {
  const api = recordingApi();
  const added = [];
  const first = picked('alpha.mpq', [1, 2]);
  const second = picked('beta.mpq', [3]);
  const result = await importLocalArchives([first, second], api, {
    prepareTransaction() { throw new Error('archive-only must not prepare a transaction'); },
    async archiveOnly(archives) {
      for (const file of archives) {
        added.push({ name: file.name, bytes: new Uint8Array(await file.arrayBuffer()) });
      }
    },
  });
  assert.equal(result.usedTransaction, false);
  assert.equal(result.clientStaged, false);
  assert.deepEqual(ops(api), []);
  assert.deepEqual(added.map(entry => [entry.name, [...entry.bytes]]), [['alpha.mpq', [1, 2]], ['beta.mpq', [3]]]);
  assert.deepEqual(api.stored_assets(), ['previous']);
  assert.equal(api.asset_revision(), 4);
});

test('archive-only transaction stages archives and does not stage a client image', async () => {
  const api = recordingApi();
  const result = await importLocalArchives([
    picked('alpha.mpq', [1]),
    picked('beta.mpq', [2, 3]),
  ], api);
  assert.equal(result.clientStaged, false);
  assert.deepEqual(ops(api), [
    'begin_archive_import',
    'stage_archive',
    'stage_archive',
    'commit_archive_import',
  ]);
  assert.deepEqual(api.calls.filter(call => call.op === 'stage_archive').map(call => call.name), ['alpha.mpq', 'beta.mpq']);
  assert.deepEqual(api.stored_assets(), ['alpha.mpq', 'beta.mpq']);
  assert.equal(api.asset_revision(), 5);
});

test('one client file is staged after its archives and its bytes stay on that call', async () => {
  const api = recordingApi();
  const progress = [];
  const storage = [];
  const fetches = [];
  const originalStorage = globalThis.localStorage;
  const originalFetch = globalThis.fetch;
  globalThis.localStorage = { setItem: (...args) => storage.push(args), getItem() { return null; } };
  globalThis.fetch = (...args) => {
    fetches.push(args);
    return Promise.reject(new Error('fetch'));
  };
  const archive = picked('alpha.mpq', [9, 9]);
  const client = picked('My File.DLL', [4, 5, 6]);
  try {
    const result = await importLocalArchives([client, archive], api, {
      onArchiveProgress(update) { progress.push(update); },
    });
    assert.equal(result.clientStaged, true);
    assert.deepEqual(ops(api), [
      'begin_archive_import',
      'stage_archive',
      'stage_client_image',
      'commit_archive_import',
    ]);
    assert.equal(api.calls[1].name, 'alpha.mpq');
    assert.deepEqual([...api.calls[1].bytes], [9, 9]);
    assert.equal(api.calls[2].source, 'My File.DLL');
    assert.deepEqual([...api.calls[2].bytes], [4, 5, 6]);
    assert.notEqual(api.calls[1].bytes, api.calls[2].bytes);
    assert.deepEqual(progress, [{ loadedBytes: 2, totalBytes: 2 }]);
    assert.deepEqual(api.stored_assets(), ['alpha.mpq']);
    assert.equal(api.stored_assets().includes('My File.DLL'), false);
    assert.deepEqual(storage, []);
    assert.deepEqual(fetches, []);
  } finally {
    if (originalStorage === undefined) delete globalThis.localStorage;
    else globalThis.localStorage = originalStorage;
    if (originalFetch === undefined) delete globalThis.fetch;
    else globalThis.fetch = originalFetch;
  }
});

test('a client file uses the transaction even when an archive-only callback is present', async () => {
  const api = recordingApi();
  let archiveOnly = 0;
  await importLocalArchives([picked('solo.exe', [8])], api, {
    archiveOnly() { archiveOnly += 1; },
  });
  assert.equal(archiveOnly, 0);
  assert.deepEqual(ops(api), ['begin_archive_import', 'stage_client_image', 'commit_archive_import']);
  assert.equal(imageCalls(api)[0].source, 'solo.exe');
  assert.deepEqual(api.stored_assets(), []);
});

test('a small client file and a file at the byte budget are both staged', async () => {
  const api = recordingApi();
  await importLocalArchives([picked('notes.dll', [1, 2, 3, 4])], api);
  assert.equal(imageCalls(api)[0].source, 'notes.dll');
  assert.equal(imageCalls(api)[0].bytes.length, 4);

  const capped = recordingApi();
  const exact = picked('cap.exe', [7], CLIENT_IMAGE_BYTES);
  exact.arrayBuffer = async () => new ArrayBuffer(CLIENT_IMAGE_BYTES);
  await importLocalArchives([exact], capped);
  assert.equal(imageCalls(capped)[0].bytes.length, CLIENT_IMAGE_BYTES);
  assert.equal(imageCalls(capped)[0].source, 'cap.exe');
});

test('ambiguous, empty, and over-budget client input does not open a transaction', async () => {
  const archive = picked('alpha.mpq', [1, 2, 3]);
  const cases = [
    [['one.exe', [1]], ['two.dll', [2]], 'ambiguous'],
    [['empty.dll', [], 0], 'empty'],
    [['big.exe', [1], CLIENT_IMAGE_BYTES + 1], 'budget'],
  ];
  for (const spec of cases) {
    const reason = spec.at(-1);
    const files = spec.slice(0, -1).map(entry => picked(...entry));
    files.push(archive);
    const api = recordingApi(11);
    const before = api.stored_assets();
    await assert.rejects(
      () => importLocalArchives(files, api),
      error => error.localClientInput === true && error.reason === reason,
    );
    assert.deepEqual(ops(api), []);
    assert.deepEqual(api.stored_assets(), before);
    assert.equal(api.asset_revision(), 11);
    assert.equal(archive.reads, 0);
    for (const file of files) assert.equal(file.reads, 0);
  }
  const lying = picked('short.dll', [1], 4);
  lying.arrayBuffer = async () => {
    lying.reads += 1;
    return new ArrayBuffer(CLIENT_IMAGE_BYTES + 1);
  };
  const api = recordingApi(11);
  await assert.rejects(
    () => importLocalArchives([lying, archive], api),
    error => error.reason === 'budget' && error.localClientInput === true,
  );
  assert.equal(lying.reads, 1);
  assert.equal(archive.reads, 0);
  assert.deepEqual(ops(api), []);
  assert.equal(api.asset_revision(), 11);
  assert.deepEqual(api.stored_assets(), ['previous']);

  const hollow = picked('hollow.dll', [1], 4);
  hollow.arrayBuffer = async () => {
    hollow.reads += 1;
    return new ArrayBuffer(0);
  };
  const hollowApi = recordingApi(11);
  await assert.rejects(
    () => importLocalArchives([hollow, archive], hollowApi),
    error => error.reason === 'empty' && error.localClientInput === true,
  );
  assert.equal(hollow.reads, 1);
  assert.equal(archive.reads, 0);
  assert.deepEqual(ops(hollowApi), []);

  const prepared = { count: 0 };
  const late = picked('late.dll', [1], 8);
  late.arrayBuffer = async () => {
    late.reads += 1;
    return new ArrayBuffer(CLIENT_IMAGE_BYTES + 1);
  };
  const lateApi = recordingApi(11);
  await assert.rejects(() => importLocalArchives([late, archive], lateApi, {
    prepareTransaction() {
      prepared.count += 1;
      return { signal: new AbortController().signal };
    },
  }), error => error.reason === 'budget');
  assert.equal(prepared.count, 1);
  assert.equal(late.reads, 1);
  assert.equal(archive.reads, 0);
  assert.deepEqual(ops(lateApi), []);
  assert.equal(lateApi.asset_revision(), 11);
});

test('negative and fractional declared lengths are rejected before begin', async () => {
  const archive = picked('alpha.mpq', [1]);
  for (const size of [-1, 1.5]) {
    let reads = 0;
    const client = picked('bad.dll', [1], size);
    client.arrayBuffer = async () => {
      reads += 1;
      return new ArrayBuffer(1);
    };
    const api = recordingApi(11);
    await assert.rejects(
      () => importLocalArchives([client, archive], api),
      error => error.localClientInput === true && error.reason === 'budget',
    );
    assert.equal(reads, 0);
    assert.equal(archive.reads, 0);
    assert.deepEqual(ops(api), []);
    assert.equal(api.asset_revision(), 11);
    assert.deepEqual(api.stored_assets(), ['previous']);
  }
});

test('a returned byte length that differs from the declared size is rejected before begin', async () => {
  const archive = picked('alpha.mpq', [9]);
  const client = picked('short.dll', [1, 2, 3, 4], 4);
  client.arrayBuffer = async () => {
    client.reads += 1;
    return new ArrayBuffer(2);
  };
  const api = recordingApi(11);
  await assert.rejects(
    () => importLocalArchives([client, archive], api),
    error => error.localClientInput === true && error.reason === 'budget',
  );
  assert.equal(client.reads, 1);
  assert.equal(archive.reads, 0);
  assert.deepEqual(ops(api), []);
  assert.equal(api.asset_revision(), 11);
  assert.deepEqual(api.stored_assets(), ['previous']);
});

test('abort before begin does not cancel, and abort after begin cancels without commit', async () => {
  const pre = new AbortController();
  pre.abort();
  const unread = picked('picked.dll', [7]);
  const preApi = recordingApi();
  await assert.rejects(
    () => importLocalArchives([unread, picked('alpha.mpq', [1])], preApi, { signal: pre.signal }),
    error => error.name === 'AbortError',
  );
  assert.equal(unread.reads, 0);
  assert.deepEqual(ops(preApi), []);
  assert.deepEqual(preApi.stored_assets(), ['previous']);

  const during = new AbortController();
  const client = {
    name: 'picked.dll',
    size: 1,
    reads: 0,
    async arrayBuffer() {
      this.reads += 1;
      during.abort();
      return Uint8Array.from([7]).buffer;
    },
  };
  const duringApi = recordingApi();
  await assert.rejects(
    () => importLocalArchives([client], duringApi, { signal: during.signal }),
    error => error.name === 'AbortError',
  );
  assert.equal(client.reads, 1);
  assert.deepEqual(ops(duringApi), []);
  assert.equal(duringApi.asset_revision(), 4);

  const mid = new AbortController();
  const midApi = recordingApi();
  const second = {
    name: 'beta.mpq',
    size: 1,
    async arrayBuffer() {
      mid.abort();
      return Uint8Array.from([2]).buffer;
    },
  };
  await assert.rejects(
    () => importLocalArchives([
      picked('alpha.mpq', [1]),
      second,
      picked('picked.dll', [9]),
    ], midApi, { signal: mid.signal }),
    error => error.name === 'AbortError',
  );
  assert.deepEqual(ops(midApi), [
    'begin_archive_import',
    'stage_archive',
    'cancel_archive_import',
  ]);
  assert.equal(imageCalls(midApi).length, 0);
  assert.deepEqual(midApi.stored_assets(), ['previous']);
  assert.equal(midApi.asset_revision(), 4);
});

test('a failed stage, image call, or commit cancels and leaves the visible names in place', async () => {
  for (const op of ['stage_archive', 'stage_client_image', 'commit_archive_import']) {
    const api = recordingApi(8);
    api[op] = () => {
      api.calls.push({ op });
      throw new Error(`${op} failed`);
    };
    await assert.rejects(
      () => importLocalArchives([picked('alpha.mpq', [1]), picked('picked.dll', [2])], api),
      error => error.message === `${op} failed`,
    );
    assert.equal(ops(api).at(-1), 'cancel_archive_import');
    assert.equal(ops(api).includes('commit_archive_import'), op === 'commit_archive_import');
    assert.deepEqual(api.stored_assets(), ['previous']);
    assert.equal(api.asset_revision(), 8);
    if (op === 'stage_archive') assert.equal(imageCalls(api).length, 0);
  }

  const api = recordingApi(8);
  await assert.rejects(() => importLocalArchives([
    picked('alpha.mpq', [1]),
    picked('picked.dll', [2, 3]),
  ], api, {
    beforeCommit() { throw new Error('validate failed'); },
  }), error => error.message === 'validate failed');
  assert.deepEqual(ops(api), [
    'begin_archive_import',
    'stage_archive',
    'stage_client_image',
    'cancel_archive_import',
  ]);
  assert.deepEqual([...imageCalls(api)[0].bytes], [2, 3]);
  assert.deepEqual(api.stored_assets(), ['previous']);
  assert.equal(api.asset_revision(), 8);
});

test('a cancelled replacement leaves the prior visible names, and a committed one records the new file name', async () => {
  const api = recordingApi(11);
  await importLocalArchives([picked('alpha.mpq', [1]), picked('first.dll', [4])], api);
  assert.deepEqual(api.stored_assets(), ['alpha.mpq']);
  assert.equal(api.asset_revision(), 12);
  assert.equal(imageCalls(api)[0].source, 'first.dll');

  const controller = new AbortController();
  const beta = {
    name: 'beta.mpq',
    size: 1,
    async arrayBuffer() {
      controller.abort();
      return Uint8Array.from([2]).buffer;
    },
  };
  await assert.rejects(
    () => importLocalArchives([beta, picked('second.exe', [5])], api, { signal: controller.signal }),
    error => error.name === 'AbortError',
  );
  assert.equal(api.calls.filter(call => call.op === 'commit_archive_import').length, 1);
  assert.equal(ops(api).at(-1), 'cancel_archive_import');
  assert.deepEqual(api.stored_assets(), ['alpha.mpq']);
  assert.equal(api.asset_revision(), 12);
  assert.deepEqual(imageCalls(api).map(call => call.source), ['first.dll']);

  await importLocalArchives([picked('gamma.mpq', [3]), picked('third.dll', [6, 7])], api);
  assert.deepEqual(api.stored_assets(), ['gamma.mpq']);
  assert.equal(api.asset_revision(), 13);
  assert.deepEqual(imageCalls(api).map(call => call.source), ['first.dll', 'third.dll']);
  assert.equal(api.stored_assets().includes('third.dll'), false);
  for (const call of imageCalls(api)) {
    assert.equal(api.stored_assets().some(name => name === call.source), false);
  }
});
