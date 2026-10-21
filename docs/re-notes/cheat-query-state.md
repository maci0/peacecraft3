# New-session cheat queries

The selected legacy client (VERSIONINFO 1.29.2.9231, reviewed image SHA256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`)
registers `IsNoDefeatCheat` as `()B` at 0x4994b0 and `IsNoVictoryCheat`
at 0x4994e0. Both obtain slot state through 0x45df90, follow its context
pointer and first state entry, and test a byte mask in the flags word.
Missing state returns false. The two mask operands remain input data.

Initialization at 0x605ed0 allocates the state object, writes its vtable,
initializes its flags with the immediate operand of the instruction at
0x605f02, then stores the object into the same first context entry at
0x605f1b. `cheat_rules::Rules` checks the reviewed image identity and reads
these bounded initialization/query windows. It checks query shape, matching
state/flags offsets and disjoint nonzero masks. Neither initial flags nor
query masks are copied into runtime constants. Synthetic windows vary flags,
masks and offsets and exercise truncation/malformed shapes.

The host admits the optional boolean getters only after their contracts and
declaration source match the checked host's native source. Missing declarations
remain unsupported. State belongs to a newly assembled session. The recreation
currently has no admitted cheat mutation; `Cheat` stays an explicit unsupported
native. Its selected native wrapper at 0x48a740 routes text through
0x95bf90 to console dispatch at 0x95b740; silently accepting that mutation
would invalidate this bounded query support. Saved/native cheat-state import is
outside this slice.

Human01's authored Arthas death trigger sets game-over, delivers its failure
message, waits, and invokes `CustomDefeatBJ`. Blizzard's `AllowVictoryDefeat`
then queries `IsNoDefeatCheat`. The selected new-session source flags allow
this normal defeat; this support does not suppress or undo the hero's death.
VM tests exercise both query results, normal defeat, unsupported mutation,
wrong declarations/source, missing admission and argument errors.

This is shipped-script/native-code and recreated VM evidence, not a comparison
against a running original game. No original bytes are committed. A separate
combat question remains: the opening cinematic helper itself grants neither
protection nor pause. Combat now consumes explicit invulnerability and unit pause
(see [unit-pause.md](unit-pause.md)); those calls alone do not explain an observed
early Arthas death. Native team initialization is still unqualified.
