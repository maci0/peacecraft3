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

### Open passive-source interaction

The current recreation stores already-boosted pools alongside held item
rawcodes, charges and flags, but no identity for the passive contribution.
Restoration resolves those item definitions against the receiving map and
publishes the saved pools unchanged. A receiving map can validly override the
same item's `Ilif` amount: an old pool containing one amount can then be
repriced on drop using a different amount. Client/install/edition checks do
not discriminate per-map ability overrides.

This is a code-path consistency gap, not a measured native cache policy.
The correction must establish how the original restores current pools and
current-map item effects before choosing whether to rederive pools or retain
more saved state. A blanket refusal of changed maps would not establish the
required cross-map behavior. A controlled comparison needs both a same-map
restore and a second map with a typed override of the held item's amount,
using the same persisted native cache. No original W3V bytes are part of the
recreation's private profile format.

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

## Headless profile admission (2026-10-21)

Headless startup takes an explicit optional PC3 profile byte slice. `None` means
a fresh profile; supplied bytes are validated against the existing profile format
and budgets before startup. Each session owns an independent in-memory store.
`SaveGameCache` and reload operate on that store, and `Startup::campaign_profile`
exports its current saved bytes for the caller to retain or pass to a later map.
It never discovers or overwrites the machine's campaign profile.

The admission digest uses a new domain version and includes absence or the
SHA-256 of the initial profile bytes. Saving later cannot change the initial
session identity. Thus peers with different initial profiles cannot share an
identity merely because their executable, map and seed match. The application
continues to supply its existing native/browser local storage explicitly to the
shared campaign host. This changes profile ownership, not W3V compatibility or
the modeled unit payload's fidelity limits.

Private checks: 1,157 playback package tests pass (45 ignored gates), 202 network
feature-suite tests pass (7 ignored gates), strict all-target playback/network
clippy and both library WASM checks pass. The explicit Human01 profile gate passes
and the app all-target check with `test-fixtures` is clean. The gate also passes
with `XDG_DATA_HOME` pointing at a regular-file sentinel: the old local-storage
path would fail, while session storage succeeds and leaves the sentinel hash
unchanged. The gate checks equal and unequal saved profile identities, stored
values, unchanged initial identity after saving and isolation between sessions.
These are recreation storage/identity checks, not original-client cache parity.

## Explicit mission availability and next-map requests (2026-10-22)

The source-declared `SetMissionAvailable` now persists the executed campaign and
mission indices and boolean. Unknown entries stay unknown: this does not supply
an invented fresh-profile unlock table. The selected common declaration is
validated against the timer declaration's source; malformed or duplicate
contracts refuse admission. Negative indices and excessive record/byte counts
refuse as recreation input policies, not measured native exceptional behavior.

The existing native file/browser local-storage transport carries a bounded,
recreation-owned `PCXPROFILE1` envelope only once mission progress exists. It has
two separately framed sections: the existing game-cache bytes and `PCXPROGRESS1`
explicit mission assignments. Legacy cache-only bytes remain readable and are
written unchanged when no progress section exists. This is not an original W3V
or original player-profile importer. Older builds cannot read the new envelope.

Both channels share one transaction lock per attached profile pair. An update
loads the physical profile, replaces its section, validates the complete bounded
envelope, and uses the physical storage's atomic save. The new in-memory mission
value publishes only after that save succeeds. Mission writes preserve the last
saved cache section; they do not flush unsaved working hero/cache changes.
`SaveGameCache` preserves the progress section. This does not establish
cross-process or independently attached host transaction isolation.

Headless startup accepts and exports the same envelope through its existing
explicit session-owned profile bytes. It never accesses the machine profile.
Synthetic tests cover independent writes, saved values, legacy compatibility,
malformed/truncated/duplicate records, failed-save preservation, read limits and
concurrent section updates through one attached pair.

`SetNextLevel` retains the source map path on the current host, verbatim, and
returns to the script. Empty clears that request; embedded NUL and excessive
bytes refuse as application policies. This request is session-local: it does not
load another map or end the game. UI availability filtering, fresh-profile
unlock defaults and a consumer that performs the actual chapter transition
remain open. Neither setter proves completed campaign progression.

An ordinary-player private controller on unchanged Human01 recruits the authored
footmen, defeats the Warlord group and reaches the authored next-level preparation.
Before this change it refused at requested tick 91768 on `SetMissionAvailable`.
It now advances to requested tick 91832, where the script completes the unassigned
`udg_QuestOrcs` and the current quest host rejects the null argument. The script
actually creates `udg_QuestVillage`; no quest alias or original-map edit was made.
This is headless recreation evidence, not native or rendered campaign acceptance.
