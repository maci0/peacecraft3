# Campaign game caches

The startup application attaches a bounded cache profile to the actual
`MapStartupHost` when the selected program declares `InitGameCache`. Its handles
use the existing object arena. Every initialization issues a new identity;
repeated filenames share data. Present primitive-cache declarations must match
their loaded signatures and the object Host's common source. Omitted natives
stay unsupported. No second script VM or object arena is introduced.

Supported operations are initialization, save/reload, whole-cache and mission
flushes, and integer/real/boolean/string store, get, presence and typed flush.
Types occupy separate namespaces at the same mission/key. A stored zero or false
remains present; missing values use the API's zero/false/null result. Real values
retain their exact bits. ASCII filename, mission and entry comparisons ignore
case; slash and backslash remain distinct. Non-ASCII keys and embedded NUL are
unsupported rather than assigned an invented locale rule.

## Source evidence and reconstruction choices

Static reads of the selected Legacy executable retain both its functional and
generic fallback registrations. The functional initialization calls the shared
filename manager, then creates a new game-cache object. Integer, real and string
stores resolve separate typed tables beneath the mission. Filename, mission and
entry lookup reach the same comparison wrapper; its normal ASCII path folds
uppercase letters before comparison. Save validates the handle and then calls
the collection manager without forwarding a filename. These reads support
independent handles, shared filename data, separate types and collection-wide
save. They are static code evidence, not a newly observed original runtime.

The recreation uses an injectable profile store and its own versioned binary
format. Ordered keys, bounded lengths and explicit real bits make serialization
portable and deterministic. It does not read or overwrite original `.w3v` files.
Unknown versions, malformed records, duplicate keys and excessive sizes refuse;
they are not treated as a missing profile. A known-absent profile starts empty.
Save failure returns false and retains the previous persisted bytes; dirty
in-memory data remains available. Failed reload retains the current profile.
The Host exposes the storage error for diagnostics. Successful reload replaces
profile data while existing handles continue to identify their filenames.

Flushing a cache removes its shared data and invalidates the supplied handle;
another handle for that filename can recreate data. Strict malformed/dead-handle
refusals and memory/work ceilings are recreation policies. Original native cache
count limits, mode-dependent filename restrictions, locale behavior and all
invalid-argument cases still need qualification. No full native-cache equivalence
is claimed.

## Application storage

Native storage is `peacecraft3/campaign-cache.bin` below `XDG_DATA_HOME`, falling
back to `~/.local/share`. Windows uses `APPDATA`; macOS uses
`~/Library/Application Support`. Writes use an exclusively created temporary
file, a completed write/sync and an atomic replacement. Script filenames are
logical keys and never become filesystem paths. The browser stores the same
bytes locally under `peacecraft3.campaign-cache.v1`; denied access and quota
errors remain failures. The loader and code-only packaging manifest include
the browser adapter.

The application's ceilings are 1,024 caches, 16,384 typed entries, 4 MiB retained
text, 8 MiB encoded storage and 1 MiB of filenames across issued handles. These
are application resource policies, not measurements of original limits. Profile
bytes are an explicit simulation-test input: deterministic comparisons must
fix the initial profile as well as commands and RNG state. Storage callbacks can
use an isolated in-memory snapshot instead of a machine's existing user data.

`StoreUnit`, `RestoreUnit`, unit-cache queries, cache synchronization and original
W3V import remain unsupported. In particular, this does not yet carry Arthas's
hero/inventory state into Human02. It does not silently save an incomplete hero.

## Verification

All 496 native Host cases, 21 application library cases and 132 map-integration
cases pass. Seven cache cases execute under actual WASI; four application
storage-binding checks execute as actual Cargo WebAssembly in Node with a local
storage test double. A separate JavaScript test covers absence, byte limits,
quota/read errors and failed-write preservation. Strict native/WASM Clippy,
both application builds and formatting pass. These checks do not establish real
browser persistence, original file compatibility or native gameplay fidelity.

With an isolated fresh PC3 profile, unchanged Human01 scripts pass cache
initialization on the production factory and the application's storage adapter.
Blizzard.j retains the returned arena handle. Globals/config still complete and
155 unit-creation requests remain. The next refusal is `EnableUserUI(false)`.
The run retains the earlier explicit recreation seed, version observation and
compatibility catalog; the original game was not launched. Normal remaining
Host inputs, live units/controls, cinematics, progression and full campaign
playability remain open. Private command receipts and static reads are in
`.scratch/campaign-cache-root-20261019/`.
