# Returning ordinary campaign guards to their posts

The unchanged Human02 opening-minute gate currently reaches `ReturnGuardPosts`
at 8.312500 seconds. The host still refuses this native. The stopped experiments
below qualify a real return action for an idle ordinary guard; they do not justify
an empty success or returning every same-owner unit.

## Selected-code evidence

The selected Legacy executable is SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
Registration `0xab6434` binds `ReturnGuardPosts\0`, `()V\0`, handler `0xab6ca0`
and registrar `0x8ccc80`. The handler resolves the owned AI context and forwards
both reference paths to `0xac1b20`.

That consumer walks the same context `+0x1c8/+0x1cc` post list used by fill.
It resolves each assigned unit through post `+0x38/+0x3c`, requires unit `+0x3a0`
to equal zero, then calls `0xad3b50` with the post and unit. An earlier inferred
`0xad3b60` label was an interior address, not the action entry; retained branch
arithmetic and REA's returned function body correct that label.

The action reads the post's `+0x28/+0x30` pair, performs a position-related check,
then checks a unit component at `+0x3f8` and its `+0x3c` value. On its admitted
branch it builds a point-order packet using source operand `0xd000f`, dispatches
it through `0x690590`, then calls the existing activity-refresh helper `0x699d90`.
The observed order below agrees with that operand. The gameplay meaning of the
`+0x3a0` and component gates is not established here. The position check includes
numeric wrappers and a runtime object; it does not justify copying an observed
arrival distance into a constant.

REA references retained locally:

- Registration: `ev_c34144e58c8e1bbad3ef4c40fffae78262a814202edc1b77386327195d64102f`.
- Handler: `ev_4673ee6e6f5c9f7139416d420ffad2df6e0267f9a3e1a66c03ec750b8a986a68`.
- Post consumer: `ev_bb021574f9dd289a9a912218ebdc278e0f04053006e3d37ccae73dd087b38fc3`.
- Action: `ev_6c815064baf3cd54503a7e5fcfb1357a21ac14eaf9d900a71f7da1fbf715ce5a`.
- Position-related helper: `ev_cc9973f8b18e0c89a17dd145f581e0e2dae53e5bbe08cff0b7e58e5d06daf877`.

The named local inspector checks retained REA bytes against PE section mapping,
hashes each window, and derives the registration, forwarding, action-call and
order relationships. Initial analysis at the handler/consumer entries reported
missing procedure boundaries; those refusals are retained. Action analysis
returned the containing function with assembly and pseudocode. The owned REA
session is closed.

## Matched original-client observation

Three stopped RoC runs use the same authored Human02 grunt, barracks and fortress
creation statements. The diagnostic AI calls `InitAI`, `CreateCaptains` and
`SetCampaignAI`. A resource acknowledgment must arrive before the map displaces
the grunt by a diagnostic +256 X translation. That translation is an experiment
condition, not game data. At sample 10 the map requests one AI-side return call;
the AI acknowledges completion and continues a bounded sequence counter.

The no-return arm omits only that call. The released arm retains the call but
uses `RemoveGuardPosition` before displacement. Map/AI normalization checks the
allowed call and diagnostic identity differences. Each arm samples the same
canonical actor's type, owner, life, order and coordinates, alongside edition,
clock and acknowledgment/query progress.

| Arm | Observed result |
| --- | --- |
| Return enabled | Order changes from zero to 851983, the guard moves toward its original post, then reports zero order again. |
| No-return control | Remains at the accepted displaced position with order zero through the terminal sample. |
| Guard released, return enabled | Also remains displaced with order zero while the AI call and sequence progress complete. |

In the return arm the accepted displaced position is `(-857, 1790)`, versus the
authored original `(-1113, 1790)`. The order is observed at 6.188 seconds; the
terminal settled position is `(-1099.554, 1789.905)`. These coordinates and sampled
times describe this run only. They do not establish a universal arrival extent,
exact polling cadence, or native route algorithm. The actor stays alive with
unchanged owner and life in all arms.

Stopped result SHA-256 identities:

- Return: `75bb24e0d34613a8192a185c8ae357ad479ba5b6bca044d307af56e43545fc59`.
- No return: `ac183acb10c3102bfa951a2319e4974d0d7423c8c95fc3559e9b4180e5cfb02b`.
- Released: `dc0b0fa34695674c928a5064d7d6166d891f85ffdda7c9804cf02ef2a523f02f`.

Maps, scripts, source/build/launch receipts, captures, raw markers and results
remain local and ignored under `.scratch/return-guard-posts/`. All map and AI
sources parse and resolve; packed members round-trip byte-for-byte. The collector
requires the exact bounded marker inventory, stopped source identity, startup and
completion acknowledgment, continuing AI query progress, unchanged actor identity
and life, finite numeric fields and terminal equality. Three synthetic tests,
including malformed-marker and sequence mutants, pass. The first synthetic
positive used an incorrect pre-start sequence value; that fixture was corrected.
The map loading-screen prompt was captured and advanced explicitly in each run.
No screenshot matching or full campaign acceptance is claimed.

## Implementation direction and open behavior

Reuse the canonical per-player posts retained by fill, the source-bound attack
order and the existing prepared activity/placement path. Released, dead and
foreign-owned post references must not become a new all-owner return list. No
native linked-list or packet-layout clone is needed to reproduce the behavior.

Before widening the consumer, qualify the interaction with an existing move or
hold order. The native eligibility gates, combat/ability/effect states, blocked
routes, repeated calls and arrival/polling parity remain open. Source-derived
order identity and admission need a connected reader/host test; parser success
alone will not prove the return movement. Human02 and the full game goal remain
incomplete while the runtime consumer is absent.
