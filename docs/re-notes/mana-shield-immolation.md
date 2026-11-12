# Mana Shield and Immolation: toggle cleanup

Native-code evidence: Legacy executable 1.29.2.9231, SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`.
This is static inspection, not an original-client runtime observation.

## Observed class dispatch

Mana Shield's class-name getter is `0xc79c10`; its name slot is
`0xfaffdc`, giving vtable `0xfaff84`. Immolation's getter is `0xb6d600`;
its name slot is `0xf27844`, giving vtable `0xf277ec`.
Both vtables select ON handler `0xb99a80` at slot `+0x340` and OFF handler
`0xb99b50` at `+0x344`. Their OFF update hooks at `+0x34c` are different:
Mana Shield `0xc79f10`, Immolation `0xb6da30`.

Mana Shield's OFF hook checks ability flag `0x80000`. If clear, it resolves
the retained reference at `+0x128/+0x12c`, passes the resolved object through
`0xc7a7e0`, and, when nonnull, calls owner routine `0x6d8530` with that object.
It resets both reference words to `0xffffffff`. Immolation's hook has the
same structure at `0xb6da30`, with flag `0x8000`, reference `+0xe8/+0xec`,
and conversion routine `0xb6ea90`.

The adjacent callbacks (`0xc79fd0`, `0xb6daf0`) compare the incoming object's
reference words at `+0xc/+0x10` with those retained references. On equality
they set their respective flag and invoke virtual `+0x344` (the shared OFF
handler). This establishes reference-matched automatic deactivation; it does
not establish why the referenced object expires.

## Inference and remaining refusal

The retained objects appear to own the active effects. Calling `0x6d8530`
appears to detach them; its full semantics have not been traced here.
The different cleanup hooks mean accepting ON through the ordinary autocast
timer path would not reproduce these effects.

No behavior change is admitted by this note. ON creation, mana drain timing,
Mana Shield damage interception and mana-per-hit-point ratio, and Immolation
periodic damage and target filtering remain undecoded. Existing precise ON
refusals remain in place. Parser tests and static inspection do not prove
runtime behavior or rendered fidelity.

## Mana Shield ON wrapper also differs

Mana Shield slot `+0x338` points to `0xb28d00`, whereas Immolation uses
`0xb2a300`. At `0xb28d00`, dispatch first consults virtual `+0x31c` and,
conditionally, `0xb4b2c0` and the low bits selected by mask `0x0e` from virtual
`+0x314`. The alternate branch compares virtual `+0x350` against the ON order
from `+0x310` before testing those bits. The meanings of those virtual results
and the later dispatch are unresolved. Thus a shared ON-handler address alone
is insufficient evidence for admitting Mana Shield's ON order.
