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
native handlers or schedule timers. Parsed-data publication and executable-session
publication are separate caller operations. The launcher does not automatically
attach a Host when map data is loaded. There is still no production gameplay
Host for imported player/unit/handle/event state, and unsupported natives remain
errors. Execution inherits the VM's existing stack and instruction-budget limits;
this resource adds no total memory or fuel bound.
