# Executed unit guard participation

The selected legacy client is PE VERSIONINFO 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
`unit_guard_rules::Rules` requires that image identity, bounds its input, and
retains the source identity and digest. Runtime instruction operands supply the
release stop order and enrollment masks; no extracted game values are checked in.

## Native-code evidence

Bounded local disassembly follows `RemoveGuardPosition` from registration wrapper
`0x4a50f0`, through `0x483940` and `0xab6a50`, to AI manager removal at
`0xac1500` and a second AI-list removal at `0xacc910`. The manager unconditionally
sends the order constructed at `0x690690` before clearing its matching guard
association. Repeated removal therefore still stops a subsequently accepted
order when the unit has an admitted manager/component. Admission cross-checks
that decoded order against the selected source-bound activity stop order.

Creation (`0x687f68`) and owner transfer (`0x69d795`) call enrollment
`0xab4550`. Its first mask addresses the same field as the dead getter
`0x667050`; the reader checks their displacement and bit-shift relationship.
The second exclusion reads the field exposed by `0x66e7d0`. Its catalog meaning
has not been established, so the recreation does not invent raw native flags
from object metadata. `SetUnitCreepGuard` uses a separate field and is not
implemented by disabling ordinary guard participation.

These are shipped instruction observations from the qualified local image,
not a recording of native execution. Private disassembly inputs/output stay
under `.scratch/unit-guard-host/` and are not committed.

## RecycleGuardPosition

Registration push at `0x4a101e` (name `0xe77690`), handler `0x4a4e30`. A null or
removed unit ends the call (`0x483940`). Otherwise `0xab5f80` runs three steps on the
unit object, in this order:

1. `0xab6a50`, the removal `RemoveGuardPosition` uses (AI manager release at
   `0xac1500`, which stops the unit, and AI-list removal at `0xacc910`).
2. `0x68e1a0` with argument 0. It walks the list at unit `+0x3e8` and clears bit `0x2`
   of each entry's flag word (`+0x20`) when the entry's counter at `+0x3c` is not
   positive, then calls the entry's virtual at `+0xc8`. Registration calls it with
   argument 1 and the removal event handler (`0xac46fd`) with 0. The meaning of that
   flag and of the list entries is not decoded.
3. `0xab4550`, the registration creation (`0x687f68`) and owner transfer
   (`0x69d795`) call. It skips a unit with `+0x164` bit `0x100` or with bit `0x10` in
   the unit's virtual `+0x114` result, then registers the unit with its owner's AI
   state (`0xabb260`, newest registration first, see `ai-construction.md`). It sets
   `+0x164` bit `0x4` and calls `0x68e1a0` with 1 only when that bit was clear.
   When the owner has no AI state it notifies each of the 24 player slots instead
   (`0x6bffb0`); that branch is not modeled.

Recreation (`unit_guard_host.rs`): the unit takes the `RemoveGuardPosition` release
(stop order, patrol, inventory order and combat target cleared), then the creation
and transfer enrollment path re-enrolls it: an eligible computer unit gets a fresh
guard record at its current position (`released` false, no return in progress,
chase timer reset), and every unit becomes the newest town registration of its
owner. A human-owned or ineligible unit keeps no guard record. Open: the ability list
flag of step 2 (`0x68e1a0`) has no recreation state, and step 3 sets the `+0x164`
bit `0x4` that is not modeled, so a unit recycled after registration keeps the flag
cleared by step 2 in the original.

## Observable recreation

The bounded host tracks a computer-owned, live, armed mobile unit's accepted
home position, participation, chase timer and current return. Computer ownership
comes from the admitted symbolic `MAP_CONTROL_COMPUTER` enumerand and executed
`GetPlayerController` state. Human ownership is checked before guard-specific
movement classification. Current supported movement classes are the authored
`foot` and `horse` records; unsupported computer guard classes refuse instead
of guessing special native enrollment behavior.

`CombatRules` parses `GuardDistance` and `GuardReturnTime` from the selected
Misc source. Its comments describe returning after exceeding the distance and
chasing for the interval without being attacked. The host uses those rules,
resets the chase timer on accepted attacks, and sends a real movement path home
when the timer expires. Existing movement/collision geometry supplies arrival
extent; there is no copied home radius. A stopped or obstructed return settles
at its accepted position to avoid issuing the same failed path every tick.
An explicit point order completes before an idle home return; an explicit new
order cancels the current return. Return ownership suppresses acquisition only
for that unit while it is returning.

Pursuing managed creeps also use the authored `MaxGuardDistance`: once strictly
beyond that maximum, the home return bypasses the chase timeout, including its
incoming-attack resets. This uses the host's existing creep-owner definition,
the admitted neutral-aggressive player identity, rather than applying the maximum
to every computer-controlled unit. The selected startup host binds that identity
before scripts execute; low-level guard advancement refuses if it is missing.
Released guards and explicit point movement retain their existing behavior.

`RemoveGuardPosition` sends an actual stop through the movement host, cancels
pending inventory interaction and combat target, then releases guard
participation. It does not disable combat acquisition globally or reject later
authored orders. Repeated calls stop later activity again. Ownership transfer
re-enrolls at the accepted current position; removal discards the bounded record.
Player handles, units and the script session remain intact.

Managed membership, timeout scheduling and obstructed-return settlement are
recreation choices. Special native AI type exclusions, AI rebuilding/replacement,
other creep classifications and the complete native manager policy remain
fidelity work. The maximum-distance rule follows the shipped Misc comments;
an original-client boundary comparison remains unqualified. Parser tests establish
decoding and rejection; host movement tests establish the implemented recreation
behavior, not native parity.

## Verification

The maximum-distance regression fails before the rule is connected and passes
afterward. Synthetic cases cover an incoming-attack reset at the maximum,
strictly beyond it, release, explicit movement and non-creep computer ownership.
The combined private suite passes 1,507 core tests and 609 application tests,
strict native/WASM Clippy and formatting. The selected Continue startup still
publishes 152 Human01 units and starts Human02; this is not the opening-minute
gate or native creep-boundary parity. Receipts remain local under
`.scratch/campaign-targeting-root-review/guard-*`.

Synthetic instruction fixtures vary the mask bits and stop identity, and reject
malformed/truncated windows. Host tests execute creation, pursuit, timed return,
release/repeated release, owner transfer and removal through the script host.
The explicit local-original reader gate is
`unit_guard_rules::tests::local_original_guard_enrollment_and_stop_order_are_qualified`
and requires `PEACECRAFT3_GUARD_IMAGE`. Actual campaign continuation must be
verified separately through the parent replay; parser/host tests do not prove it.

On 2026-10-20, `cargo test -p pcx-play --lib guard` passed seven tests with the
explicit original gate ignored. Running that gate separately with the selected
local client passed one test. The parent campaign replay and aggregate target
checks remain separate verification; these focused results do not assert their
outcome.
