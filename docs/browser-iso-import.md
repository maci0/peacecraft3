# Local ISO discovery

`web/iso.js` exports `discoverIso(blob, options)` and `sliceIsoFile(blob, file)`.
The discovery module reads the selected browser `File` with
`Blob.slice().arrayBuffer()`. It does not allocate a whole-image buffer,
make network calls or run executables. This example shows direct MPQ discovery.
Installer members require the separate local decoder described in the
[Pages guide](pages-demo.md).

```js
import { discoverIso, sliceIsoFile } from './iso.js';
const controller = new AbortController();
const result = await discoverIso(file, {
  signal: controller.signal,
  onProgress: ({ bytesRead, reads }) => showProgress(bytesRead, reads),
});
for (const candidate of result.candidates) {
  const archiveBlob = sliceIsoFile(file, {
    offset: candidate.offset,
    size: candidate.archiveSize,
  });
  // Pass archiveBlob to the local decoder for member validation.
}
```

Progress measures bytes actually inspected, including repeated reads, rather than
percentage of image size. Cancellation is checked before/after every asynchronous
read. A pending Blob read cannot itself be interrupted; each read is at most 2048
bytes. Metadata storage is bounded by `maxEntries` (default 100000 directory
records). Volume descriptor scanning has a 64-sector budget. Limits are resource
policies, not inferred game data.

The result contains `provenance`, `files`, `candidates`, `diagnostics`, and `work`.
Provenance records the filename label, source size, and selected filesystem,
descriptor offset, Joliet level, volume identifier and volume size. Edition and
build remain null. Paths preserve the selected namespace’s identifiers and
version suffixes.

File offsets and extent sizes come from the directory records. Direct MPQ
candidates also carry the header version and declared `archiveSize`. Import
reads that archive size, which can be smaller than the full file extent. Use
the returned offsets with the same input Blob.

ISO9660 primary and Joliet traversal checks duplicated endian numbers, volume/image bounds,
record lengths, sector padding, extent bounds, directory cycles, and entry limits.
It follows directory records rather than path-table guesses. Files are inspected
by bytes at their declared extent, irrespective of filename. MPQ v0 headers must
have a valid header/archive size and bounded hash/block tables. This is structural
candidate discovery for files directly in the ISO. Installer-directory
choices are provisional until extraction and an MPQ header check. The app may
preselect a unique `war3.mpq` within budget. Confirm that selection to import
and publish the validated archives. The parser does not
establish edition, content identity or archive priority.

Fatal image errors throw `IsoError`. Unsupported file contents appear in
the result’s per-file diagnostics.

- A single recognized Joliet descriptor selects the Joliet namespace. Unknown
  or multiple supplementary descriptors refuse without a primary fallback.
  Detected UDF recognition descriptors reject with `UNSUPPORTED_UDF`.
  The scanner does not implement a complete UDF detector or decoder.
- Multi-volume, multi-extent, interleaved and extended-attribute extents reject.
  Rock Ridge system-use metadata is not decoded. Joliet identifiers are read
  from their own UCS-2 records.
- MPQ user-data wrappers and later header versions are reported as requiring a
  verified archive decoder. Embedded signatures are not searched for.
- DOS/PE, CAB, 7z and FreeArc signatures produce specific unsupported
  diagnostics. When discovery finds no direct MPQ candidate, the app can use
  `UNSUPPORTED_FREEARC` to start inspecting a supported installer profile. A PE
  header establishes executable content, not Inno/NSIS identity or installer
  success. Unknown/repacked files remain `UNRECOGNIZED_CONTENT`. No executable
  runs and there is no remote extraction fallback.
- `IsoError` carries a `code` and an offset when available. That offset can
  be relative to the descriptor or record being checked. Per-file diagnostics
  retain the authored path, ISO extent offset and size.

## Import and rollback

Hydration validates canonical names and renderer aliases before publication.
Two unequal files mapping to the same alias reject with `AMBIGUOUS_ASSET_ALIAS`;
the previous visible store and revision survive. Identical content may share
an alias. Checked memory accounting charges only newly stored names, and failed
inserts do not partially populate the staging map.

## Synthetic verification

Run with Node's built-in test runner (verified on Node 26.11.1):

```sh
node --test tests/browser-iso/iso.test.mjs
```

The tests generate ISO bytes in memory without using original assets. They check content-based recognition despite
misleading filenames, local slices, provenance, a 2048-byte maximum read, network
API traps recording zero calls, malformed bounds/endian numbers/records/padding,
MPQ table bounds, unsupported namespaces/extents, PE blockers, directory recursion
and cycles, entry budget, and cancellation. This Node suite covers the
parser and API with synthetic inputs. Browser and archive-member evidence
comes from separate checks.

## Earlier browser integration checks

The generated WASM API passed 26 synthetic archive and transaction assertions
in a Chromium worker. See the [test guide](../tests/browser-iso/README.md)
for current commands and fixture requirements. Chromium audio checks cover mute, intro pause/resume, click,
content rejection and Blob URL replacement/reset. Network capture found zero
uploads, unexpected requests or sockets. A separate rendered-app file-input check
recognized a content-valid MPQ under a misleading filename without replacing
active assets during discovery. These checks do not establish original installer
ISO acceptance or rendered fidelity from the synthetic fixture art.

### 14 October 2026 control-layer capture

The 2026-10-14 local Chromium run used generated WASM SHA-256
`303dd8666f58251aa6640e9519d391e0965a56843dbf276158160c70d43acdaf`
and the retained 115-member fixture with explicit archive-variant provenance.
The corrected label assertion followed each FDF-derived text-frame name and the
renderer diagnostic's quoted text; it still requires fresh logs after commit.
The earlier failure against the obsolete diagnostic format is preserved.

The rerun passed real file-input import, revision increment, six fresh authored
labels/faces, scene rebuilding, six F8 visibility checks on that build, imported click audio
before and after resize, and conflicting-member rollback. All six labels were
independently readable in the capture; automatic full-scene OCR was inconclusive.
The 113 captured requests included two local Blob GETs during import, no HTTP
requests during import, no uploads, no sockets and no page errors.

Proof and 19 captures remain local under
`.scratch/post-control-art-browser/run-2-contract-fix/`. This is functional
acceptance of a locally repacked fixture, not retail installer ISO acceptance or
full native fidelity. Unsupported border mappings and absent inset defaults are
reported and omitted. The run did not qualify panel assets, sky projection,
lower framing, material equivalence or original gameplay. The
[Pages guide](pages-demo.md#earlier-verification-runs) records separate
stored-base installer evidence.
