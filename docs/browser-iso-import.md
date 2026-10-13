# Local ISO discovery

`web/iso.js` exports `discoverIso(blob, options)` and `sliceIsoFile(blob, file)`.
It has no imports, dependencies, network APIs, application wiring, or executable
execution. Supply the selected browser `File` directly. Reads use only
`Blob.slice().arrayBuffer()`; no whole-image buffer is allocated.

```js
import { discoverIso, sliceIsoFile } from './iso.js';
const controller = new AbortController();
const result = await discoverIso(file, {
  signal: controller.signal,
  onProgress: ({ bytesRead, reads }) => showProgress(bytesRead, reads),
});
for (const candidate of result.candidates) {
  const archiveBlob = sliceIsoFile(file, candidate);
  // The application connects a local archive decoder and validates members before registration.
}
```

Progress measures bytes actually inspected, including repeated reads, rather than
percentage of image size. Cancellation is checked before/after every asynchronous
read. A pending Blob read cannot itself be interrupted; each read is at most 2048
bytes. Metadata storage is bounded by `maxEntries` (default 100000 directory
records). Volume descriptor scanning has a 64-sector budget. Limits are resource
policies, not inferred game data.

The result contains `provenance`, `files`, `candidates`, `diagnostics`, and `work`.
Provenance preserves the supplied filename as a label, parsed primary volume
identifier, source size, declared volume size, and explicitly selected filesystem.
Edition/build remain null: neither filenames nor ISO volume labels establish a
Warcraft release. Paths retain authored primary identifiers and version suffixes.
File offsets/sizes/extents come from directory records. Candidates additionally
carry MPQ header version and declared archive size. Use the same input Blob with
returned offsets; descriptors are not authenticated capabilities for another file.

Primary ISO9660 traversal checks duplicated endian numbers, volume/image bounds,
record lengths, sector padding, extent bounds, directory cycles, and entry limits.
It follows directory records rather than path-table guesses. Files are inspected
by bytes at their declared extent, irrespective of filename. MPQ v0 headers must
have a valid header/archive size and bounded hash/block tables. This is structural
candidate discovery: decryption, member validation, content identity, edition,
archive priority, mixed-install detection and Rust asset registration remain root
integration work. No candidate automatically becomes a selected asset set.

Unsupported states are explicit:

- Supplementary descriptors (including Joliet) reject the image rather than
  silently switching namespaces. UDF recognition descriptors encountered in the
  descriptor scan reject with `UNSUPPORTED_UDF`. This is not a complete UDF
  detector; hybrid images using an ISO primary namespace can be traversed only
  through that namespace. UDF contents are never inferred or decoded.
- Multi-volume, multi-extent, interleaved and extended-attribute extents reject.
  Rock Ridge system-use metadata is not decoded; only primary ISO paths are
  exposed. No Unicode or alternate namespace is guessed.
- MPQ user-data wrappers and later header versions are reported as requiring a
  verified archive decoder. Embedded signatures are not searched for.
- DOS/PE, CAB and 7z signatures produce specific unsupported diagnostics. A PE
  header establishes executable content, not Inno/NSIS identity or installer
  success. Unknown/repacked files remain `UNRECOGNIZED_CONTENT`. No executable
  runs and there is no remote extraction fallback.
- `IsoError` carries `code` and, where available, absolute image `offset`;
  diagnostics also include authored path, offset and size.

## Synthetic verification

Run with Node's built-in test runner (verified on Node 26.11.1):

```sh
node --test tests/browser-iso/iso.test.mjs
```

Six test groups generate their ISO bytes locally in memory; no original assets
or copied content are included. They check content-based recognition despite
misleading filenames, local slices, provenance, a 2048-byte maximum read, network
API traps recording zero calls, malformed bounds/endian numbers/records/padding,
MPQ table bounds, unsupported namespaces/extents, PE blockers, directory recursion
and cycles, entry budget, and cancellation. These tests are parser/API evidence;
no original ISO, in-browser network capture, archive-member gate or rendered
asset import has been claimed.

Original campaigns remain **BLOCKED** separately by the real game Host/native
execution gap (including exact `GetPlayerNeutralPassive` behavior). ISO discovery
and scripting source-expression tests do not establish campaign outcomes.

## Browser integration verification

The generated WASM API passed 15 synthetic archive/transaction assertions in a
Chromium worker. Chromium audio checks cover mute, intro pause/resume, click,
content rejection and Blob URL replacement/reset. Network capture found zero
uploads, unexpected requests or sockets. A separate rendered-app file-input check
recognized a content-valid MPQ under a misleading filename without replacing
active assets during discovery. These checks do not establish original installer
ISO acceptance or rendered fidelity from the synthetic fixture art.
