# Hero skill state during initialization

The campaign factory can now admit hero counters and ability ranks on the same
executed unit record used by creation, ownership and world queries. It takes the
selected ability tables, optional map W3A overlay, progression profiles and
source-qualified counter rules. No additional unit or ability handles are
allocated. Without that admission the existing unsupported-native boundary stays.

`SelectHeroSkill` consumes one unspent point and raises the admitted alias's rank
when its authored level requirement and maximum permit it. `GetUnitAbilityLevel`,
`GetHeroLevel` and `GetHeroSkillPoints` read that state. Null, absent-skill,
insufficient-point, level-requirement and maximum-rank cases are covered. Custom
ability aliases retain their own identity and stock base, including supported
scalar learning-field overrides. Unsupported indexed/unknown requirement edits
refuse before learning. The existing `SetHeroLevel` path also updates admitted
hero level and available points, caps them using supplied rules, and does not
lower a level. Its XP/stat/effect consumers remain incomplete.

## Inputs and evidence

Unit metadata selects `UnitAbilities/abilList`, `heroAbilList` and the nonhero
`UnitBalance/level` fallback. AbilityData supplies `levels`, `reqLevel` and
`levelSkip`. A zero per-ability skip uses the selected profile's
`Misc/HeroAbilityLevelSkip`; `Misc/MaxHeroLevel` supplies the cap. The profile
reader reuses the existing object-profile lexer and preserves explicit stock/map
ordering. The private Human01 run reads stock `Units\MiscGame.txt`; its map has
no `war3mapMisc.txt` or W3A overlay. This does not qualify every mode's profile
selection policy.

The Legacy executable SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`
supplies the initial counter, point-pool clamp, skill-slot bound and first rank.
The level getter at `0x691920` reads the hero component's initialized counter and
increments it; its nonhero branch reads the class level instead. Those are
different values and are not substituted for one another. The initialization
windows around `0xb3d7e5`, `0xb3f135` and `0xb3f1b8` qualify the counter/slot rules.
The ability lookup at `0x6d6570` has an implicit hero-component branch as well as
ordinary alias lookup. Its alias is decoded from the supplied comparison and
joined to the same component used by the level query. Other implicit components
without admitted state refuse instead of reporting an invented zero.

These are bounded static observations and a source-backed recreation state path,
not a new original-game runtime or proof of complete ability internals. Matching
hero-skill/level listeners use the queued delivery described below. Unqualified
event identities or missing event runtime refuse before mutation. Event values
come from the loaded declarations. The
actual source names are `EVENT_PLAYER_HERO_SKILL` and `EVENT_PLAYER_HERO_LEVEL`;
the earlier candidate's extra `UNIT` token incorrectly treated unrelated
player-unit listeners as unqualified.

## Unchanged Human01 execution

The script now completes its `SelectHeroSkill` call on the existing `Hart` unit.
An owning-host query returns `AHhb` rank one, hero level one and zero remaining
skill points. The run then completes two more ownership transfers before
refusing `SetUnitAnimation("stand defend")` at map span `110016..110069`.

All 155 creation requests and accepted positions exactly match the retained
original words. Prior group, protection and XP-factor results are unchanged;
the earlier ownership result is preserved alongside the two additional calls.
The campaign cache is absent in a fresh profile before and after execution.
The run uses explicit private factory inputs and recreation seed one. It does
not establish original seed initialization, normal application bootstrap or a
playable campaign.

Validation passes 577 playback-library tests, one progression-profile test, six
application trigger tests, seventeen hero cases executed in WASI, strict native
and browser-WASM checks, both full application builds and formatting. Coverage
includes source identity, mutated source windows, custom aliases, counter limits,
missing declarations, implicit components and listener refusal. Earlier failed
source selection, provenance, compilation and event-name attempts are retained
under `.scratch/campaign-hero-skill-root-20261019/`, beside the successful fourth
Human01 driver and `verification-final-1.json`.

Spell effects, general availability/dependency evaluation, XP/stat gains,
visual level-up effects and complete ability mutation remain
open. Initial ability levels here do not by themselves implement combat effects.
Normal Host preparation, live actors and controls, first-campaign completion and
original/native/WASM deterministic/rendered parity remain unfinished. No original
assets were committed or uploaded, and no public site deployment occurred.

Holy Light, the first source-used spell effect, is in [holy light ability](holy-light-ability.md).

## Hero events (2026-10-21)

`SetHeroLevel` and successful `SelectHeroSkill` now queue matching unit and
current-owner player registrations through the existing trigger session.
Admission validates the four response declarations against the selected unit
declaration source: `GetLevelingUnit`, `GetLearningUnit`, `GetLearnedSkill` and
`GetLearnedSkillLevel`. Event ordinals are evaluated from the selected `common.j`
initializers; synthetic tests deliberately use different ordinals.

The host resolves filters and reserves delivery capacity before changing the
hero. Callbacks see the updated counters and vitals. Skill events capture their
alias and learned rank, so later learning cannot overwrite an earlier queued
event's response. Filters use the hero as `GetFilterUnit`; player registrations
match its owner when the change occurs. Disabled/destroyed triggers are skipped,
and delivery clears its response context. A refused learn or unchanged/lower
level emits nothing. A bulk level increase emits one final-state notification.

For the reviewed executable above, primary `SetHeroLevel` handler `0x4a88b0`
reaches updater `0xb3e0c0` through `0x690910`. The counter-increment loop ends
before the changed-state guard at `0xb3e4b4` and the notification call to
`0x689d30` at `0xb3e4d5`. This supports one notification after a bulk increase;
it is static evidence, not a runtime capture. `SelectHeroSkill`'s primary handler
is `0x4a6750`, reaching `0x696fd0` and `0xb3f320`. The loaded script declarations
establish the event and response names. Which registration family is live and
native synchronous callback ordering remain unqualified.

Delivery uses the existing script-boundary queue. It does not claim native
synchronous nesting or original-client callback parity. Synthetic VM tests cover
both registration families, current-owner routing, filters, captured ranks,
disabled delivery, source-contract refusal and response cleanup. Visual level-up
effects, XP-driven leveling and full campaign acceptance remain separate work.

Private validation on this change: `pcx-play` package 1,155 passed, 0 failed,
44 ignored local-data gates; strict all-target clippy, WASM library check and
`peacecraft3 --all-targets --features test-fixtures` check pass. The package run
supplies the existing campaign-drop fixtures explicitly. Unchanged Human03 with
seed one, ROC mode and a known-absent isolated profile passes its hero setup and
next refuses `UnitAddSleep` at `Blizzard.j:3524`. That startup gate exits 101;
it does not establish campaign playability or original-client parity.
