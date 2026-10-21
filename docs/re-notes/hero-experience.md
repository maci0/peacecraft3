# Hero experience: source and runtime checkpoint

Experience awards are not implemented. `HeroProgress` retains level and skill
points, not XP or XP suspension; `combat_host::kill_unit` queues death callbacks
without an experience award. The declared `GetHeroXP`, `SetHeroXP`, `AddHeroXP`,
`SuspendHeroXP` and `IsSuspendedXP` paths are not admitted. Do not compensate by
raising combat damage, hero level, or resource pools in a campaign test.

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

## Required continuation

Implement source-derived thresholds and XP state/natives, preserving level-up
events and the map's suspension cap. Connect eligible deaths to awards only
after deriving the grant, range, allied sharing, creep and rounding rules.
Persist XP/suspension through the existing campaign cache with an explicit
compatibility policy. Re-run the same preserved itinerary and continue through
chapter victory and the next-map transition. Neither is achieved here.
