# Original script execution boundary

This observation exercises local shipped source bytes through the existing JASS VM.
It is not execution inside the original engine and does not prove campaign/native parity.

## Sources and identity

The installed Legacy executable reports `1.29.2.9231`; that identity does not establish
that adjacent archives belong to that release. The probe selected these members directly,
without making a native archive-priority claim:

| Local member | Decoded bytes | FNV-1a 64 |
| --- | ---: | --- |
| Legacy `War3.mpq:Scripts\common.j` | 178677 | `7b07e9ec6175f6cb` |
| Legacy `War3x.mpq:Scripts\Blizzard.j` | 452936 | `019155681675eed7` |
| Legacy `Maps/(4)LostTemple.w3m:war3map.j` | read at runtime | `440ed4eac8387ef2` |

Checksums above describe this observation, not embedded input constants. Individual
archive release identity remains unverified. The source members were extracted with
the existing `mpqxtract`; the map's embedded MPQ start was found from its magic bytes.
The temporary files were deleted when the probe exited. No original contents are
included in this note or checked-in fixtures.

## Observed execution

`MapScripts::compile` resolved 979 functions and 1260 native declarations from the
three supplied source files. These counts are observations, not acceptance thresholds.
`playrun-script` then attempted global initialization followed by `config`/`main` through
`NullHost`. Initialization stopped before the map entrypoints:

- source: the supplied `common.j`, line 176, column 67;
- call: `GetPlayerNeutralPassive`;
- declaration: `takes nothing returns integer`, read from that same source;
- status: unsupported Host native, returned as a source/span-bearing error.

There was no missing-native zero, fabricated handle or completed quest. A retry after
failed initialization is now refused with the saved error; previously the VM's `ready`
flag allowed entrypoints to run against partially initialized globals.

## Native evidence boundary

The REA skill was read before binary analysis. A focused string search for native
registration names (`ConvertRace`, `CreateTimer`, `TimerStart`, `CreateTrigger`,
`TriggerExecute`) was attempted against the exact Legacy `Warcraft III.exe`, with its
existing digest-addressed snapshot. The provider timed out after its declared 330000 ms
limit and returned `provider_timeout`. It supplied no usable native-code evidence or
Evidence ID. No timer, trigger, conversion or neutral-player implementation was inferred
from that failure.

A smaller follow-up query for `GetPlayerNeutralPassive` using REA 6.4.0 was refused
with `evidence_integrity_mismatch`: the retained snapshot had `profile_mismatch`.
No evidence from that snapshot was treated as a successful native query.

A fresh REA 6.4.0 snapshot of the exact Legacy executable succeeded for the smaller
search. Evidence `ev_0d20d0485b960faa0a19fa0cde970dfce38c61b7f05a3a60061b095af11b3555`
records its SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
Ghidra 12.1.3, and the defined string `GetPlayerNeutralPassive` at `0xe78300`.
This establishes a registration-name search seed, not the native's return value or
side effects. Native xrefs/implementation investigation continues separately.

The older extracted `Game.dll` reporting `1.26.0.6401` is not an identity oracle for the
Legacy archives. Native behavior, original event ordering, stock campaigns, DotA,
Athena and Footman Frenzy remain open. The next native investigation should start from
the observed neutral-player getter and establish the target's registration and return
path; any runtime comparison must be recorded separately from this VM-source probe.

## Host handle transport

The source-aware VM now transports opaque non-null `HandleId` tokens provided by
the Host. Tokens can be retained in globals/arrays, passed to natives, compared
with each other and compared with `null`. Numeric conversion, allocation, recycling
and native `GetHandleId` mappings are not implemented or inferred.

Shipped-byte comparison examples occur in the selected expansion `Blizzard.j`:
`IssueHauntOrderAtLocBJ` has local handle declarations at lines 3432–3433 and a
null comparison at line 3442; `ReplaceUnitBJ` has local handle declarations at
lines 3735–3736 and a null comparison at line 3743. These observations establish
that non-null native values must flow through the VM and compare with null;
they do not establish native handle allocation or reuse. Synthetic tests exercise
that Host transport contract without minting original game objects.

## Authored nullable strings

The selected expansion `Blizzard.j` contains explicit string/null expressions in
`UnitId2StringBJ` (line 3005), `OrderId2StringBJ` (3039 and 3045),
`GetStoredStringBJ` (7249), `LoadStringBJ` (7280), and `PickMeleeAI` (8662 and 8664).
This is shipped-source expression evidence. It is not an observation of the native
string pool, string-handle numeric IDs, or original runtime execution.

The VM accepts present string values compared with null in either operand order,
and retains ordinary null/null equality. Synthetic regressions cover both `==` and
`!=`, both operand orders, present/empty strings, null/null, and numeric mismatches
against null and strings. This change implements the source-expression path without
inferring any native behavior from a provisional or mistaken binary callee.
