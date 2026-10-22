# Campaign targeting observations

Two matched diagnostic maps ran on the reviewed Legacy 1.29.2.9231 executable,
SHA-256 `3f2ed0120d80578bf07e4423296dade1adfb959d59a2d20a7584224559570eed`,
with `-classic`; every marker reported RoC. The programs called
`SetTargetHeroes(true)` or `SetTargetHeroes(false)`, then `SetUnitsFlee(false)`.
They did not run `CampaignAI`, `InitAI` or `CreateCaptains`. These observations
therefore concern that limited setup, not an executing campaign strategy.

Each map created one player-zero `ogru` attacker and two hero/two ordinary
player-three targets. Geometry and creation order matched between maps. The
attacker was paused before targets existed; the attacked-event observer was
registered on the target owner before starting the program. The script paused
all targets, started the program synchronously, then unpaused the attacker.
There were no forced attack orders. The event callback filtered the exact
attacker handle and recorded target user data; periodic samples separately
recorded the nearer targets' life and pause state.

| Measurement | Targeting enabled | Targeting disabled |
| --- | ---: | ---: |
| First observed attack time | 0.188 | 0.188 |
| First target | nearer hero, user data 10 | nearer hero, user data 10 |
| Attack events through 20 seconds | 13 | 13 |
| Sampled nearer-hero life at 20 seconds | 453 | 453 |
| Sampled nearer-ordinary life at 20 seconds | 680 | 680 |

Both runs produced 200 periodic samples and all 219 expected phase, load,
sample and attack files. A player-zero bank write from zero to the diagnostic
watermark 17 preceded the map's after-start marker; the other player's bank
remained zero. The program's load marker reported the requested flag. Its
`I2S(GetAiPlayer())` field was empty in both runs; the collector retains that
text without interpreting it as a number. The independent bank witness remains
separate from this unresolved string-conversion observation.

The measured attacks did not differ between flags. This does not establish
that the native is inert, that heroes are never avoided in campaign combat,
or that any internal unit list was empty. The attached-unit state and the
consumer of the policy were not observed. The production native remains
unsupported until its world behavior has a faithful consumer.

The true/false map hashes are respectively
`5f7984b851e42db9952e670eabcdbce1990111957e496dd269d6ae3c76f2b7b3`
and `cbd0602955d2edaa913497c844decb35422386f725cff7625e7a1b099fe31d9c`.
Script parse/resolve and every packed-member roundtrip passed. The recorder
checks source hashes, actual Preload wrappers, filenames, phases, load bank,
edition, sample/event sequence and output bounds; its two tests exercise both
labels, empty player text, no-attack output and eight independent refusals.
The scripts disable the observer and pause the attacker at completion, with
explicit sample/event caps. Raw files, hashes and all source/build/launch
receipts remain local in ignored `.scratch/campaign-targeting-run/`.
Rendered presentation and native Windows remain unqualified.
