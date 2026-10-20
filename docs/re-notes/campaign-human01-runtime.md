# Human01 startup-to-runtime natives

The unchanged Human01 map publishes a session through the normal
`prepare_new_game`/application path and advances under `CampaignRuntime` for
1800 game seconds without a script refusal (no player input; the intro
cinematic plays and the mission then waits on the player). This note lists the
natives and runtime pieces added on that path, their sources, and what stays
unqualified.
Evidence: `.scratch/campaign-time-of-day-root-20261020/frontier-*` (production
build, then the actual application audit with an added fixed-tick runtime phase).

## Script threads (`pcx-script`, `thread_host.rs`)

`ScriptCall::Spawn` runs callbacks in order as one new thread. A park inside it
is split off the VM stack and stored under a host ticket; the caller continues
with `null`. `Vm::run_thread`/`Vm::resume_thread` start and continue top-level
threads. `TriggerExecute` always spawns; trigger actions delivered by timers and
unit ranges run as threads. `TriggerSleepAction` parks with a deadline on the
owned game clock. Each parked thread keeps its active trigger, event id and unit
range context. A sleep inside a native callback (`ForGroup` etc.) refuses.
The step budget restarts per entry. Ordering is a recreation policy: earlier
deadline first, a timer before a sleeper due at the same time. Real-time versus
game-time sleep semantics and the original's minimum wait are unmeasured.

## Time of day (`time_of_day_host.rs`)

`Units\MiscData.txt` (`DayLength`, `DayHours`, with `war3mapMisc.txt` overrides)
rates the clock. The initial hour and scale come from the reviewed client
initializers (`0x401300`, `0x401460`). `SetFloatGameState` (time of day),
`SuspendTimeOfDay`, `SetTimeOfDayScale`, `GetTimeOfDayScale` and live
`GetFloatGameState` are supported. The cycle starts unsuspended (recreation
choice). Time-of-day game-state event delivery is not implemented.

## Live camera (`camera_view_host.rs`)

Game-camera fields come from `UI\MiscData.txt` `[Camera]` at the application's
zoom index and resolve through the script's own `CAMERA_FIELD_*` constants.
`PanCameraToTimed`, `ResetToGameCamera`, `CameraSetSmoothingFactor`,
`SetCameraQuickPosition`, `GetCameraTargetPositionLoc` and timed
`CameraSetupApplyForceDuration` ramp linearly over the requested duration
(interpolation curve unqualified). `[CameraRates]` scroll rates are retained for
player scrolling. The application consumer still renders only setup snapshots.

## Software math (`script_math.rs`)

`SquareRoot` and `Atan2` port the client's own routines: integer square root with
a fixed-point correction, and a reduced rational arctangent built on the ported
add/multiply/reciprocal. The code windows are fingerprinted; coefficients, angles,
immediates and the loop bound are read from the image through the addresses and
immediates that code references. Wrappers return the zero constant when inputs are
within the shared equality threshold of zero (and for negative square roots).
Checked with a private driver (sqrt(2) = `3fb504f3`, sqrt(9) = 3.0000577,
atan2(-1,-1) = -2.3561945); no original-runtime comparison yet.

## Other natives

- `AddSpecialEffectTarget`/`DestroyEffect`: effect records on units; destroying
  null or destroyed effects is accepted (Human01 destroys one variable twice).
  Rendering at attachment points is not implemented.
- `SetSkyModel`, `SetTerrainFogEx`, `ResetTerrainFog`: retained arguments for the
  renderer; no sky or fog rendering yet.
- Destructables: invulnerability, kill, show/hide and X/Y/type getters. Death
  events and pathing changes on kill are not implemented.
- `SetUnitFacing`/`SetUnitFacingTimed`/`GetUnitFacing`: shorter-arc linear turn
  over the duration (turn-rate fidelity unqualified).
- `ClearMapMusic`, `PlayThematicMusic`, `EndThematicMusic`: retained state; no
  music playback consumer.
- `GetGameDifficulty`/`SetGameDifficulty`: explicit input, Normal until the
  campaign menu offers the choice.
- `RemoveUnit`: tombstone handle; occupancy, motion, groups, range candidates and
  attached effects drop the unit; actors retire.
- `VolumeGroupSetVolume`/`VolumeGroupReset`: per-group scales for the audio
  consumer (not yet applied).
- `GetUnitState` and `SetUnitState` vitals: `UnitBalance` `HP`, `manaN`,
  `mana0`; heroes add level-one `STR`/`INT` times `Units\MiscGame.txt`
  `StrHitPointBonus`/`IntManaBonus`. An SLK `-` mana cell is no mana pool.
  Regeneration, hero levels above one and death at zero life are not implemented.
- `AttachSoundToUnit`: see [unit attachment](script-sound-properties.md#unit-attachment).

## Event delivery (`event_delivery_host.rs`)

Engine changes queue fired triggers; the session delivers them at each script
boundary (after timers and unit ranges): registration filter, enabled check,
conditions, then actions as one thread. Responses (`GetTriggerUnit`,
`GetEnteringUnit`, `GetLeavingUnit`, `GetDyingUnit`, `GetKillingUnit`,
`GetAttacker`) are kept per thread. Deferred delivery is a recreation choice; the
original's synchronous nesting is unqualified. Region enter/leave events poll
membership of shown, live units each boundary; a point is inside when it lies in
any added rectangle (native cell cover unqualified). Membership seeds silently at
a registration's first poll.

## Combat (`combat_host.rs`)

Per unit: `UnitWeapons` weapon 1 (`weapsOn`, `rangeN1`, `acquire`, `cool1`,
`dmgpt1`, `dmgplus1`, `dice1`, `sides1`, `atkType1`, `weapTp1`), profile
`Missilespeed`, and `UnitBalance` `def`/`defType`. Rules: `Units\MiscGame.txt`
`DefenseArmor` and the `DamageBonus*` rows in the defense-type order its comment
documents. `IssueTargetOrder` "attack" sets the target; idle units acquire the
nearest hostile unit within acquire range. Units chase with the existing router,
swing at the damage point, wait the cooldown, and missiles land after
distance/speed. Dice draw from the shared script RNG. Death stops the unit, frees
its occupancy, plays `death`, and queues player-unit death, unit death and widget
death events; attacks queue `EVENT_UNIT_ATTACKED`. Recreation choices (all
unqualified): reach is range plus both collision radii, the usual armor
reduction curve, enemies unless a passive alliance exists, Neutral Passive never
acquired automatically, linear chase re-routing. Corpse decay, bounty,
experience, creep guarding and call-for-help are not implemented.

## Movement additions

Blocked or unreachable destinations route to the explored cell nearest the
target, as the original's units do; exhausted route work ends the search the same
way. `SetUnitPathing(false)` releases occupancy and moves straight without
collision; re-enabling uses the `ShowUnit` placement rule.

## More natives

- `GetSoundDuration` (explicit duration, else decoded
  length), `StopSound` requests and `TriggerWaitForSound` parks until the sound's
  remaining playback minus the offset.
- `SetCinematicScene`/`EndCinematicScene`, `PingMinimap`, `AddIndicator` and
  `UnitAddIndicator` retained for the transmission and minimap consumers.
- `ShowInterface` fades keep their start and duration; `EnableOcclusion` keeps its
  flag (no occlusion-fade pass is rendered).
- Local-player selection (`ClearSelection`, `SelectUnit`, `IsUnitSelected`,
  `GroupEnumUnitsSelected`, `SyncSelections`), shared with player input.
- Fog modifiers retained; no fog-of-war simulation yet.
- `IsUnitType` from definition and live state; status types read false.
- `PauseTimer`/`ResumeTimer`, `CreateUnitAtLoc`.

## Tests

Synthetic host tests cover threads, combat (damage cycle, missiles, lethal state,
`KillUnit`, event queue and responses), region enter/leave, removal, facing,
pathing, near routing, effects, selection, unit types, time of day, timer pause,
camera ramps and the presentation records; they also pass on `wasm32-wasip1`.
A synthetic-image test for the software math is still missing.

## Next

Application playability: the live camera and player scrolling, selection and
orders from mouse input, transmission/quest text, attachment-point effects and
death/decay presentation. Dialogue audio stays silent until the playback consumer
accepts fades, durations and parameter profiles.
