# Game-event subscriptions

`TriggerRegisterGameEvent(trigger, gameevent)` now retains a distinct event
identity in the existing session arena. Admission checks the loaded native ABI,
declaration sources, `gameevent` ancestry and the selected `ConvertGameEvent`
domain. No event numbers are copied into the host. Invalid families, destroyed
triggers and exhausted identity capacity refuse before allocating a subscription.

`ObjectHost::fire_game_event(source_global)` queues matching subscriptions for the
existing script-session delivery path. It reserves the complete batch before
publication and bounds pending events by the session arena policy. Duplicate
subscriptions stay distinct; destroyed triggers are skipped, and disabled
triggers are filtered by the normal delivery path. Actions observe the same
`GetTriggeringTrigger`/`GetTriggerEventId` context as other delivered events.

The selected `Blizzard.j` helpers `TriggerRegisterShowSkillEventBJ` and
`TriggerRegisterBuildSubmenuEventBJ` call this native with source-authored globals.
The unchanged prologue reaches the first helper during trigger initialization.
This is shipped-script evidence for those registration callers, not evidence
that a UI menu has been opened.

Synthetic regressions cover distinct subscriptions, matching, enable/destruction,
wrong signatures/families, arena and pending-event bounds, and execution of an
authored action in the existing VM. Registration never manufactures a game event.
Engine/UI producers for the full game-event domain remain separate acceptance
work; the current command card does not yet implement the hero-skill/build
submenu interactions that produce those two tutorial events.
