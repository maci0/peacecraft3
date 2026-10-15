# Opening and menu transition boundary

The requested full-screen split plays both after the opening movie and between
menu screens, according to the supplied observation. It remains unimplemented.
The existing panel Birth/Stand animation does not establish that full-screen
effect, and a once-per-launch substitute would miss the second occurrence.

The retained MainMenu, OptionsMenu and SinglePlayerMenu FDF owners have
SetAllPoints relationships but no qualified split geometry, motion or timing
binding. The bounded retained graph search found slider properties, not a gate
animation. Those results and source identities remain in
`.scratch/menu-opening-gate-both/`; they are not proof that the engine lacks a
transition.

## One original caller inspected

The exact Legacy executable is SHA-256
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
with previously measured VERSIONINFO 1.29.2.9231. This does not assign a release
to adjacent archives. The retained glue-layer dossier
`ev_f47689b3e4b855d205277e54d8b9c615d2b52270a280438bd43525f39038f869`
identifies an unconditional call at `0x544a52` to the known layer initializer
`0x55f370`.

The newly inspected caller is exactly `0x544980..0x544b49` (half-open), 457 bytes,
SHA-256 `cc5750ccf54e4697c13a02c5d0c7ca04aaaef44d85b975a5f45de342e929c109`.
Capstone decodes 104 instructions from its entry through its final RET. An
independent bounded objdump check agrees on every instruction boundary. The
complete image identity and initialized PE-section mapping were checked first.
No target execution, callee body, executable-wide scan or provider session was
used.

Observed in this body:

- It preserves incoming ECX as ESI, writes two table pointers and initializes
  object fields, including zeroing the three layer slots before calling the
  common layer initializer. It returns ESI. This is construction-shaped flow;
  that label is an interpretation of these instructions, not a recovered name.
- The layer initializer receives a Boolean derived from a global byte. This
  establishes an argument dependency, not a transition duration or screen role.
- Code addresses `0x551fd0`, `0x582530` and `0x591890` are passed to other
  functions. Their consumers and callback semantics remain unread. None is
  labelled a gate driver from address proximity alone.
- The only direct printable ASCII reference identified in this body is
  `MasterFont`. Its later consumer receives a floating-point argument, whose
  semantic role was not established and is not reused as animation timing.

There is no directly identified upper/lower geometry, image-copy/split operation
or animation selector in this caller. Its delegated effects remain unknown, so
this bounded negative does not rule out transition work inside a callee.
Startup and navigation reachability of this caller are still unproved.

## Reconstruction boundary

The current application already has separate movie-completion and screen-change
entry points. They can share transition ownership once the effect's source and
relationships are established; internal architecture need not match the native
engine. Known panel sequence data may independently improve panel navigation,
but must not be presented as proof of the requested full-screen gate. No gate
art, motion, duration or sound has been invented.

The next concrete static boundary is the consumer of a passed code pointer,
starting with the `0x551fd0` argument at the call to `0x895ac0`. Its role and body
extent must be established before following it. This note does not authorize a
broad constructor/callback sweep or claim that this pointer drives transitions.

Local receipts, both disassembly checks and their hashes are in
`.scratch/menu-opening-native-caller-root/`. No original payload is committed or
published. Parser checks, these static observations and the current menu browser
captures do not prove the missing gate's rendered appearance or sound.
