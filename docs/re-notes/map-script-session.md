# Persistent admitted script sessions

`MapScriptSession<H>` is an application resource retaining one owned VM, its
localized Host and the same shared parsed map context. A configuration caller
supplies the Host factory and explicit function IDs. Global initialization and
selected entries run in the candidate before it replaces the active session.
Failure preserves the previous session and returns source-labelled diagnostics.
The caller must isolate candidate Host effects; arbitrary external effects
cannot be rolled back by discarding a VM.

Callbacks use a ticket minted from the active session. Its private weak witness
is checked before function lookup, argument checking or Host execution. Replacing
or clearing a session invalidates its tickets, including replacement with the
same compiled program. Tickets retain no discarded context, VM or Host. Direct
function calls remain explicit calls into the current context; a bare function
ID does not encode its originating program and must not be rebound from an old
callback queue. Ticket checks associate the callee; caller-supplied argument
values must also belong to the active context.

The public integration tests exercise real Bevy resource updates and admitted
synthetic map containers. They check retained global state, localization,
independent Hosts, callback association, failed-candidate preservation, source
spans, initialization count and owner release. The test Hosts implement explicitly
synthetic behavior; their values and effects are not game-native evidence.

This resource does not choose entry names, create gameplay entities, implement
native handlers or choose an application clock. Parsed-data publication and executable-session
publication stay separate. After a successful `MapRequests` delivery,
`campaign_startup` resolves map-script `config` and `main` by name and calls
`replace_sequenced` with `map_startup_host::assemble`. The factory builds one
`ObjectHost` and six `SelectedMapNeutralHost` wrappers from caller-supplied
budgets, banks, camera input, catalog, neutral rules and optional research
observations. Post-config startup observations run on that candidate, after
`config` returns and before `main`. Factory failure and an unsupported native
leave the previous session in place. Absent host inputs do not publish a
session. `SelectedMapSources` is not written by startup. Native
`replace_selected`, resolver-backed message delivery and file drop publish the
exact receipt arc with the resolver that produced it. A binding made without
retaining that resolver, and every refused candidate, leave the previous pair
unchanged. Stock slot assignment admission is optional and caller-bounded.
The launcher still has no automatic producer for the qualified rules, banks, catalog and
runtime observations in `CampaignHostSupply`; importing assets alone does not
supply them.
Without explicit timer-runtime inputs, the qualified Human01 continuation still
refuses `TimerStart`; automatic timer scheduling,
world rendering and a completed chapter remain open. Execution inherits the VM's
existing stack and instruction-budget limits. This resource adds no total memory or fuel bound.

Startup checks the successful delivery's revision and admitted identity, its
edition/version and the exact published receipt arc before taking host inputs.
A binding revision change alone cannot restart the previous admission with new
inputs. File drops may relabel the source path without losing that receipt
association. The attempt witness is a weak context pointer: it prevents address
reuse from suppressing a later attempt while retaining no discarded map contents.
Missing inputs can be supplied later; a factory, post-config or execution refusal
consumes that candidate's inputs and preserves the previously published session.
Fresh caller inputs may retry that refused candidate without another map delivery.
The revision guard still runs before consuming them. Ordinary updates retain the
failure without retrying; supplying inputs alone does not restart a successful
session or an admission with an invalid entrypoint. Successful replacement retires
the previous session and its callback tickets.

The root Human01 verifier now constructs this production factory, resolves and
applies its captured post-config inputs through the same public helpers, and
runs the existing localized owned VM. Globals and `config` finish; `main` reaches
`TimerStart` in Blizzard.j at `447183..447286`. Existing identity/state checks
remain, including 904 identities, 144 rectangles and 155 units. Its object catalog
is explicitly the compatibility selection; this does not certify target-native
catalog selection or playable startup. The real-App integration cases use
synthetic assets and cover success, unsupported execution, a binding-only revision
change, file-drop relabeling with explicit stock admission, and post-config failure.
Local evidence is `.scratch/campaign-startup-host-root-20261017/`.

The VM's cooperative `invoke`/`resume` API is described in
[script-continuations.md](script-continuations.md). This session continues to use
synchronous `call`: a suspending candidate is refused and is not published. Its
current Host factory does not admit a suspending native. A future timer/event
driver must retain and resume the same VM; it must not restart the entrypoint or
replace suspension with a fabricated successful native result.


The positive-timeout [timer consumer](legacy-timer-start.md#bounded-recreation-timer-consumer)
opts the factory in through `timer_time` and delivers bounded expiries with
`advance_timers` on this published session. It mints the callback ticket at
expiry, after publication, and enters this same VM. It does not implement
suspending callbacks or choose a default application game clock. A matching
expiry-trigger subscription refuses before consuming the expiry.
