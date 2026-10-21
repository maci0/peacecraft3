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

The unit-cache extension below adds modeled unit progress and inventory transfer.
Cache synchronization and original W3V import remain unsupported. Full native
hero serialization, including XP which the current hero runtime does not model,
is not claimed.

## Initial primitive-cache verification (2026-10-19)

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

## Unit-cache extension (2026-10-21)

`StoreUnit`, `RestoreUnit`, `HaveStoredUnit` and `FlushStoredUnit` now share the
same profile, keys and arena. Unit values occupy their own typed namespace.
Restore does not consume the saved entry. Missing cache/unit entries return
null so the shipped map can run its own fallback hero creation; a malformed,
foreign or unsupported saved value refuses instead of becoming an absent one.

Source target: Legacy 1.29.2.9231, executable SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
The primary registrations bind StoreUnit to `0x4ac460`, RestoreUnit to `0x4a5650`,
HaveStoredUnit to `0x498a40`, and FlushStoredUnit to `0x4905c0`. The broad stub
registrar also names them; runtime registrar selection is not newly established.
Static RestoreUnit lookup reaches `0x4cb0b0` then `0x518c70`; missing lookup
returns zero before construction. The construction path at `0x518ca0` creates
the unit and restores items by their slot index. The stored item value copied
to `+0x18c` is the charge field, independently read by GetItemCharges at
`0x492d50`. StoreUnit's path through `0x519b50` copies those charges and delegates
hero data to `0x6d8b20`. These are bounded static reads, not a runtime comparison
or a complete decode of the native unit payload.

The private snapshot carries the rawcode, selected client-image digest,
install/edition, modeled hero level/skill points/ranks, permanent strength,
life/mana pools and maxima, and item rawcodes/slots/charge counts/explicit flags.
It retains runtime state rather than compiling asset values into the program.
Restoration resolves unit/item definitions again from the selected current map
catalogs, checks the client and install/edition, and checks learned ranks against
current learning rules. Source changes that make the snapshot unsupported refuse.

Every unit/item definition, slot, record/charge policy and arena reservation is
checked before publishing the unit. Prepared items attach directly to their
saved slots. This avoids a second pickup or automatic powerup grant. The saved
pools already include passive bonuses, and removing a restored passive item
uses the ordinary repricing path. Pending orders, pause, animation phase,
spell/item cooldowns and queued events are not copied into a new chapter.
Regeneration starts on the receiving host's clock.

Pool preservation, explicit item-flag preservation, omission of transient state
and the no-pickup-event path are recreation choices still requiring an original
client comparison. The native cached inventory loop observed here covers six
slots; the recreation currently uses the admitted inventory policy rather than
copying that measured limit into code. The native hero payload and its remaining
mask are not fully decoded. XP and other unimplemented hero state, native W3V
import, exact cross-chapter native equivalence and full campaign acceptance stay
open. This extension does not claim to fill those gaps.

Profiles with units use private PC3CACHE version 2; primitive-only writes retain
version 1. Both versions read, and old primitive profiles preserve their values.
Unit payloads have their own PC3UNIT version, checked counts, finite pools,
unique ordered ranks/slots and bounded allocations. Unit bytes count against
the existing storage policy, with no new resource knob. An older build cannot
read a version-2 profile; no original W3V file is modified.

Synthetic tests cover persistence into a new host, owner/position arguments,
clock rebasing, permanent strength, sparse slots, charge overrides, passive
removal, typed flushes, v1 compatibility, malformed payloads and refusals without
partial creation for slot, source/client, item/charge policy and injected held
powerup failures. The final private-target checks pass: 1,151 pcx-play package
tests (44 ignored), strict all-target clippy, the WASM library check and the
peacecraft3 all-target check with test-fixtures. Required campaign-drop tests
used explicitly supplied local fixtures.

The selected unchanged Human03 startup, with a known-absent profile in an
isolated XDG_DATA_HOME, now follows its missing-cache fallback
and reaches SetHeroLevel at `war3map.j:2316`, which refuses on missing hero-event
delivery. It is still not accepted, and no original-client run was performed.
