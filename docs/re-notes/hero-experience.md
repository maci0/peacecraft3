# Hero experience: source and runtime checkpoint

Hero XP counters, suspension and script natives are implemented through the
selected-source admission. Combat deaths with a credited hostile killer now
award XP using the loaded grant policies, and then queue the ordinary death
callbacks. Do not compensate for campaign failures by raising combat damage,
hero level, or resource pools in a test.

## Automatic awards

`hero_experience_awards::Awards` reads the normal/hero grant lists and A/B/C
extensions, unit-level ceiling, creep factors, range, global experience,
building policy, maximum-level drain and summoned-kill factor from ordered
stock/map profiles. Duplicate, missing, malformed or over-budget fields refuse;
no grant values are embedded in production. XP rounding reads the selected
image operand described below.

The host selects living, shown heroes belonging to the killer or receiving its
directed shared-XP alliance. Nearby participants take precedence. When there
are none, the global policy admits the killer owner's heroes. A suspended hero
participates in the split but receives no XP; maximum-level heroes drain only
when the loaded policy enables that. A dead source can still credit its owner
for a later projectile impact, while dead recipient heroes are excluded.
Friendly kills and deaths without a credited killer give no award.

Grant, summoned factor, split, creep factor and the recipient player's assigned
XP handicap are applied using the already admitted numeric policy. The result
is rounded with the image operand and added through the same XP/level/event
path as the script natives. Eligibility/order outside the six measured cases,
allied sharing, suspended/max-level draining, buildings, summoned victims,
boundary geometry and exact event interleaving are recreation choices with
native qualification still open. Automatic level-up effect art is not rendered.

No default handicap is invented: a nonzero award for a player whose factor has
not been assigned refuses with that context. The existing setter/getter model
does not yet qualify the final native initial factor. Older caches without XP
also refuse an award requiring that missing counter. Preflight computes award
counters before death housekeeping, but earlier damage is not rolled back on
failure; the owning session stops, as for other failed world projections.

Synthetic tests exercise the grant recurrence/overrides/refusals, exactly-once
lethal delivery, sharing, creep/handicap arithmetic, global fallback, dead and
suspended recipients, late impact credit, maximum-level drain and unknown-factor
refusal. The explicit local `local_original_kill_experience_matches_recorded_probe`
gate reads the retained original-client receipt, checks its template/profile
SHA identities, and matches all six independent XP-delta observations below.
It is a diagnostic comparison, not normal player input or unchanged-map acceptance.

Validation on this increment: 1,132 `pcx-play` library tests pass with 33 ignored;
the award-parser synthetic test passes. Strict all-target Clippy on play/objects,
both WASM library checks and native `peacecraft3 --all-targets` check pass. The
three Human01 opening/XP-cap/Escape gates and the six-case marker comparison
pass after the award hook is connected. No elapsed-time or FPS threshold is a gate.

## Implemented state and compatibility

`hero_skills::Rules::with_experience` derives required-XP thresholds from ordered
stock/map profiles and initial XP/suspension from the selected executable's
constructor operands. Its instruction shapes and client SHA are validated before
admission. Synthetic operands and profiles test derivation, overrides, bounds and
malformed input. Production new-game preparation uses this path.

`GetHeroXP`, `SetHeroXP`, `AddHeroXP`, `SuspendHeroXP` and `IsSuspendedXP` share
the existing hero-level, skill-point and vitality state. Positive setters and
additions preserve the measured suspension behavior. A lower setter is ignored;
negative additions and integer overflow refuse explicitly. Direct level raises
bring XP up to the corresponding source threshold; that synchronization is a
recreation choice, not yet a native `SetHeroLevel` observation.

The selected Human01 gate `selected_campaign_hero_xp_runs_the_authored_level_cap`
raises XP to the source-derived next-level threshold, drains the ordinary event
queue, and observes the map's real level trigger suspending XP. Further addition
leaves XP unchanged. This is a diagnostic stimulus, not a player-earned award or
chapter acceptance. Native same-stack callback ordering remains unqualified;
this gate proves the host's queued delivery boundary.

Recreation-owned `PC3UNIT` version 3 stores XP and suspension. Versions 1 and 2
still decode and restore their existing progress, with XP explicitly absent.
No XP is invented for those old saves: XP-native operations on such a restored
hero refuse. New records with XP inconsistent with their saved level refuse
before replacing ability state. This does not claim original W3V compatibility.

Private-target validation: `pcx-play --lib` passes 1,129 tests with 32 ignored;
the complete `pcx-objects` package passes with both supplied install paths set.
Its first broad run without those paths failed the existing aggregate original-
asset census (zero inputs); that is not counted as a pass. Strict all-target
Clippy on both crates, both WASM library checks, and native
`peacecraft3 --all-targets` check pass. The three selected Human01 gates pass:
the XP cap, authored Escape/control return, and two equal no-input 60-second
opening runs with Arthas alive at his initial life. Logs remain local under
`.scratch/campaign-route/xp-*`. These checks do not prove rendered XP UI,
original save compatibility or chapter completion.

## Unchanged-map route (2026-10-22)

After the startup-alliance correction, a production headless Human01 session
receives the same local end-cinematic event as the application's Escape path.
The source intro-cancel trigger restores control and interface visibility, sets
its skipped/arrival globals, and preserves the active main quest. The explicit
ignored test `selected_campaign_escape_returns_control_through_the_authored_trigger`
passes against the selected install. It does not call the trigger action directly
or force those globals or presentation flags.

An unfinished ordinary-player itinerary goes through the map's named bandit
rectangles, attacks the camp, picks up the authored ledger, and returns it to the
source NPC. It observes the actual quest completion and permanent-strength
reward. No units are teleported, no quest/global flags are written, and no unit
statistics are replaced. It then loses the party/hero to hostile combat before
chapter victory. Using learned Holy Light improves survival but does not finish
the itinerary. Those failures are not proof of incorrect combat balance.

The draft and all failed run logs remain in local `.scratch/campaign-route/`,
including `headless-startup-victory-draft.rs.txt` and its SHA receipt. They are
not a committed passing victory test. This preserves the chapter-completion
requirement rather than declaring the side quest to be the end of the game.

With automatic awards connected, a repeat ordinary-input itinerary earns level
2 and XP 232 during the bandit fight. The authored level event suspends XP;
the ledger still completes and grants its permanent-strength reward. Two
strategies then lose Arthas at the later orc fight. A regroup/heal strategy has
already lost its footmen before that fight; its surviving-party check is not
evidence that the original party recovered. Those failures still do not prove
incorrect combat balance.

A further ordinary group focus-fire strategy stops earlier with a concrete
host error: at game time 84.015625 (tick 5377), movement work is exhausted for
unit 825, chasing `[3632.0, -3992.7148]`, waypoint 0 of 13, with its motion epoch
restarted at that same time and zero distance credited. This is a recreation
movement-budget failure, not a native observation. It needs a bounded chase
regression and a progress fix rather than increasing the work allowance.
The latest source draft and all failed route logs are retained in local
`.scratch/campaign-route/route-xp-*`; no failing route is published as a passing
chapter gate. Victory and next-map transition remain unachieved.

## Shipped data

The selected `War3.mpq` member `Units\\MiscGame.txt` contains the required-XP
table, hero/normal-unit grant tables, their A/B/C extension coefficients, creep
factors, summoned-kill factor, range and global/building/max-level policies.
Its formula comment defines `f(x) = A*f(x-1) + B*x + C`. Every value must be
read from the selected stock/map profiles; the observations below are not
production constants. Full table indexing, sharing, fractional rounding and
kill eligibility still need qualification.

## Original-client diagnostics

Both local maps preserve the original Human01 `config` and `main` statements,
adding only a timer entry and diagnostic functions. All other original archive
members round-trip unchanged. Their receipts set `acceptance_evidence: false`.
The supplied executable is SHA-256 `3f2ed012…570eed`, VERSIONINFO 1.29.2.9231;
the template is `f5b15561…40ee9`. These are private Wine/software-rendered
references, not native Windows or unchanged-map acceptance.

Each marker is its own bounded preload file and records the stage, game time,
level, XP and suspension. Parse/resolve, member round-trip and staged-byte
checks pass. The observed chapter card was dismissed with one harness Space.

- Run `37f79ba56bca` starts at level 1, XP 0, suspension false. Setting XP to
  the first source-table entry (200) gives level 2 and suspension true. All later
  setters/additions leave XP at 200. This is interference from the authored
  level event, not a measured higher-level threshold.
- Human01's `Trig_Disable_Arthas` condition checks level at least 2. Its action
  disables itself and calls `SuspendHeroXPBJ(false, hero)`, whose supplied BJ
  body inverts the flag for `SuspendHeroXP`. This is a real campaign XP cap.
- Run `9de74051dd37` explicitly clears suspension before each stimulus, records
  that precondition, and retains all other template initialization. The source-
  derived candidate totals 200, 400 and 500 produce levels 2, 2 and 3. Thus the
  first required-XP entry is the threshold for level 2; the recurrence uses
  target level 3 for the next threshold, not table index 2.
- In that second run, setting XP back to 200 leaves XP 500 / level 3. Adding one
  gives XP 501. Suspending and adding one leaves XP 501; restoring suspension
  false changes only that flag. No negative/overflow or maximum-level behavior
  was measured.

Raw markers, hashes, scripts, maps, captures and launch/action receipts remain
local. Both owned reference containers are stopped. No original bytes are
committed or uploaded.

### Kill, range and sharing diagnostic

Run `7c82649e3788` preserves the same template initialization and adds one timer
with six synchronous `UnitDamageTarget` stimuli. Each created victim is `hfoo`;
the original reports its unit level as 2. Every record confirms an accepted
damage call and a dead victim. An added second `Hart` hero shares the first
hero's owner. The diagnostic explicitly clears the first hero's XP suspension
before each stimulus. Distances below are actual queried positions, not requested
placement coordinates; terrain relocated the distant victims.

| Victim owner / case | Distance from Arthas | Arthas XP delta | Second hero delta |
|---|---:|---:|---:|
| Player 0, near | 300.001 | 32 | absent |
| Player 0, far | 2893.524 | 32 | absent |
| Neutral aggressive, near | 300.001 | 26 | absent |
| Neutral aggressive, far | 2893.524 | 26 | absent |
| Player 0, two heroes near | 300.001 | 16 | 16 |
| Player 0, only second hero near | 2450.786 | 0 | 32 |

Human01 sets its hero player's XP handicap to 80 percent in its original script.
The stock level-2 normal grant recurrence gives 40; its handicap gives 32. The
creep factor at hero level 1 yields a fractional candidate of 25.6, observed as
26. That single fractional observation alone does not establish rounding for
other values. The code link below provides additional evidence. The far-only and second-near observations show
that a simple global award to every owned hero is incorrect. They do not settle
allied sharing, exact range boundaries, dead/suspended/max-level participants,
buildings, summoned victims or alternate `GlobalExperience` settings.

All six bounded marker files and source/staging/action receipts are retained in
local `.scratch/campaign-route/xp-kills-v1/`. Parse/resolve, archive round-trip
and staged-byte comparison passed. The observed chapter card was dismissed by
one Space key. The owned reference container is stopped.

## Native-code links

The primary registrations are in the selected image at `0x49D7BC` onward:
GetHeroXP handler `0x492BE0`, SetHeroXP `0x4A8B30`, AddHeroXP `0x489450`,
SuspendHeroXP `0x4AC980`, IsSuspendedXP `0x499830`. The corresponding stub
registrations also exist; changing runtime query results qualify this diagnostic
channel, not every registration family.

The getter tail-calls `0x693780`, reading the hero component's XP field.
SetHeroXP subtracts current XP and takes only a positive difference
(`0x4A8B8F`–`0x4A8B96`). Add and Set share `0x690910`, which dispatches to
`0xB3E0C0`. That body skips delivery when its suspension field is nonzero
(`0xB3E103`). `0x69F4A0` writes the same field for SuspendHeroXP. The hero
constructor initializes XP at `0xB3D829`. These links agree with the runtime
observations; they do not yet qualify kill sharing or arithmetic edge cases.

At `0xB3E352` and `0xB3E370`, XP updates call `0x9A1D70` before the integer
conversion at `0x9A07B0`. The helper adds the float at `0x10DB0A8` through
`0x9A01C0` (the existing source-qualified addition helper), then calls
`0x9A1520`. The operand bytes are `00 00 00 3F`, and the latter helper clears
fraction bits for nonnegative inputs. This is code evidence for floor after
adding the image's half-unit operand, consistent with the observed 25.6 to 26.
Any runtime implementation must read that operand from the selected image;
the measured value is not a production constant. Award multipliers and
level-dependent splitting are now connected as described above; their complete
native ordering remains unqualified.

## Required continuation

Qualify the remaining eligibility, arithmetic/event timing, initial player XP
handicap and legacy-cache migration policy. Fix the movement-work failure exposed
by ordinary group target orders.
Re-run the same preserved itinerary and continue through
chapter victory and the next-map transition. Neither is achieved here.
