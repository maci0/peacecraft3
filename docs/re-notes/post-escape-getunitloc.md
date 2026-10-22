# Post-Escape `GetUnitLoc` wrong-family failure

The actual menu app, after Escape's intro is skipped and control returns, fails
`GetUnitLoc` with a wrong-family handle at about 10s of game time. This note
records the production-VM reproduction and the call chain that produces it.
It is diagnostics, not a fix: `unit_removal_host` / `Entry::RemovedUnit` shape
is owned separately by the removal-seam work and is not changed here.

## Static call chain (unchanged Human01)

- `Trig_Intro_Cancel_Actions` removes `udg_Uther` (Human01.j line 2101) when the
  intro is skipped (`Trig_Intro_Cancel_Func036001`).
- During the intro, `Trig_Intro_Cinematic_Actions` orders
  `udg_StartingFootman01/02` to the centers of `gg_rct_Footman_01_End` and
  `gg_rct_Footman_02_End` (lines 2387-2388).
- `Trig___Footman01_End_Movement` / `Trig___Footman02_End_Movement` are EnterRect
  triggers (lines 2798-2844). Their actions call
  `SetUnitFacingToFaceUnitTimed(udg_StartingFootman01/02, udg_Uther, 1.00)`
  (lines 2811 / 2836).
- `SetUnitFacingToFaceUnitTimed` (Blizzard.j line 4742) starts with
  `local location unitLoc = GetUnitLoc(target)` (line 4743). The target is the
  removed `udg_Uther`, so `GetUnitLoc` sees a tombstone handle.

## Production-VM reproduction

A scratch-only diagnostic (`.scratch/campaign-route/diagnose-getunitloc.rs`)
mirrors the flow deterministically:

1. Start the selected Human01, step 64 ticks, fire
   `EVENT_PLAYER_END_CINEMATIC` (same as the existing escape gate).
2. Step 2048 ticks so `Trig_Intro_Cancel` runs and control is restored.
3. Probe `udg_Uther` with `GetUnitX`; it refuses with
   `handle identifies another family` — the tombstone confirmation.
4. Issue the authored intro move orders (Human01.j lines 2387-2388) to
   `udg_StartingFootman01/02` for `gg_rct_Footman_01/02_End`.
5. The next step faults with:

```
native `GetUnitLoc` is not implemented; it is declared as `GetUnitLoc takes unit
whichUnit returns location` ... at Scripts\Blizzard.j:4743 ...: GetUnitLoc ...
handle identifies another family
```

The failure reproduces with seeds 0, 1 and 3 at tick 2155 (clock
33.671875). The only EnterRect action for those rects and units calls
`SetUnitFacingToFaceUnitTimed(..., udg_Uther, ...)`, so the failing
`GetUnitLoc(target)` argument is the removed `udg_Uther` tombstone.

The host does not expose a JASS call stack; the evidence therefore combines (a)
the reproducing VM run, (b) the tombstone probe, and (c) the unique static call
site for the exercised rect/unit pair.

## Status

This is a real post-Escape gameplay stop, not a static inference. It does not
determine whether `GetUnitLoc` should accept a removed handle or whether the
authored callback should be suppressed; that decision belongs to the removal-seam
owner and the menu/UI wiring, and is out of scope for this diagnostic.
