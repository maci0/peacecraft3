# Campaign script pause

The selected Human02 Blademaster scene calls `PauseAllUnitsBJ(true)` after
hiding the selected player units. That BJ separately pauses computer-player
campaign scripts with `PauseCompAI` and units with `PauseUnit`. Its normal and
cancel paths each issue the matching false call. These are separate lifecycles;
pausing a campaign script does not itself cancel or pause unit orders.

## Selected native evidence

The reviewed Legacy executable is pinned by the existing campaign image digest.
The public registration joins the player resolver to its owned AI context. The
pause and resume paths increment and decrement the same signed context counter;
the context initializer supplies zero. A missing context returns without creating
one. The wake-event handler reads that counter: a nonzero counter reschedules the
same thread event, while zero enters the VM main/continuation execution path.
The retry operand joins a static integer-to-real initializer through the existing
numeric conversion helper. The production reader checks these relationships,
including the retry global, rather than inferring a duration from runtime samples.

The recreation retains this counter in the existing player VM context. Paused
wake tickets advance by the admitted retry interval without consuming their VM
continuation. Unpause changes only the counter; it neither wakes a thread early
nor shifts future sleep deadlines. Existing player/ticket ordering and the single
canonical world remain in use. Script failure and context retirement discard the
context's pause state with the context. Independently paused players remain paused.

## Original runtime observation

A local diagnostic retained the known-loadable full-map container and recorded an
AI heartbeat independently through a real player-state field. It paused twice at
logical 0.5, resumed once at 1.0 and again at 1.5. Sixteen retained records show
heartbeat 2 throughout both pauses and the first resume; the 1.75 sample is 3 and
the 2.5 terminal sample is 6. The isolated original client was stopped before
collection. The map, raw records, copied hashes and strict collector result remain
local under `.scratch/campaign-ai-pause-probe/arm-a/`.

This establishes nested script continuation pausing for that selected client. The
quarter-second sampling does not establish the exact resume instant or a general
unit-AI pause rule. Existing scheduler quantization differences are not resolved
by this observation.

## Connected cinematic frontier

The reachable scene audit covers its normal and cancel paths, authored mirror
caster setup, summon callback and animation callback. It additionally requires
real `AOmi` Mirror Image mechanics, `SetUnitAnimationWithRarity`, and conditional
`ReviveHeroLoc` when Uther is dead. The existing Water Elemental implementation
does not implement Mirror Image. These remain explicit capabilities; pause tests
or parser success do not establish a complete cinematic or mission victory.
