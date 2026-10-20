# Script UI localization queries

GetLocalizedString and GetLocalizedHotkey are admitted together over explicitly
supplied GlobalStrings StringList and UI/MiscUI profile assets. The Bank retains
each asset's label, edition/version/source identity and SHA256, plus the reviewed
client rules identity. Normal preparation resolves the assets through the selected
asset resolver; no retained scratch extraction supplies production labels.

The StringList reader handles a UTF-8 BOM, whitespace, line/block comments,
quoted strings, escaped quotation marks and escaped backslashes. Warcraft color
and line markup remains authored text. Unknown escapes/extra grammar, malformed
or duplicate identifiers and bounded input/entry/string budget violations refuse
with the input label. Hotkeys uses the source-selected section name with ASCII
case-insensitive section/key lookup. Quoted ASCII character literals and signed
decimal/hex integer forms are retained as integer values. Duplicate selected
sections or case-equivalent keys refuse. Other profile sections are not hotkey
records. Non-ASCII hotkey comparison is not qualified.

## Selected native evidence

Reviewed client SHA256:
`3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
VERSIONINFO 1.29.2.9231. GetLocalizedString is registered `(S)S` at
0x493970; GetLocalizedHotkey is registered `(S)I` at 0x493940.

The string wrapper attempts native map TRIGSTR lookup through 0x708dc0 and
0x708d70, then queries UI StringList dictionaries through 0x895a00. That helper
tries its primary dictionary before its fallback dictionary. Successful nonempty
text is copied into the wrapper's bounded output buffer. Missing/empty dictionary
text takes the original-input path through 0x474ce0. The StringList lookup at
0x84bfb0 compares keys through 0x433450/0xa77520 with an exact byte comparison;
a differently cased string key consequently uses the original-input fallback.

The hotkey wrapper reads a profile section name indirectly through the operand
at 0x493957 and calls 0x4304c0. That profile lookup uses the case-insensitive
comparison at 0x96aec0/0x433490. The source section is HotKeys, while the supplied
UI profile spells it Hotkeys. Its game-over records explicitly associate the
hotkey identifiers with GlobalStrings identifiers. The wrapper supplies its
missing-key integer from the push instruction at 0x493954. Empty/null input
returns the independently checked XOR-zero branch at 0x493967. Rules reads the
section pointer/string, fallback operand and output-buffer bound at runtime;
those data values are not copied into constants.

The Bank reproduces missing-key original-input string fallback only after source
bank admission. Missing hotkeys return the source-derived integer default.
Found text follows the source byte bound. A truncation that would split UTF-8
refuses because the current VM represents strings as valid UTF-8. Missing banks
or declarations remain unsupported. Native declaration contracts must match the
checked host source. Selected dictionary precedence beyond the explicitly
supplied StringList/profile bank (such as separately loaded custom FDF tables)
is not recreated or inferred by this API.

## Map WTS and verification

The existing localized VM wrappers remain unchanged: literal TRIGSTR references
resolve against the selected map WTS and preserve its text. The original native
also resolves dynamically constructed TRIGSTR tokens. The UI Bank has no map WTS
context, so unresolved TRIGSTR-prefixed input refuses explicitly rather than
falling through as an unknown UI key. General native TRIGSTR numeric parsing and
subsequent UI lookup are not claimed by the literal-WTS test.

Synthetic tests cover escaped strings/comments/BOM, text markup, exact string
case, case-insensitive profile lookup, character/integer hotkeys, duplicates,
budgets, operand mutations/truncations, output bounds, source receipts,
wrong/missing declarations and real owned VM calls through the existing WTS
wrapper. The ignored local-input gate requires PCX_LOCALIZATION_CLIENT,
PCX_LOCALIZATION_STRINGS and PCX_LOCALIZATION_HOTKEYS and checks all four actual
defeat-dialog keys against selected assets. These are parser/native-code/VM
evidence; original running-game and rendered-dialog comparisons remain separate.
No original asset bytes are committed.
