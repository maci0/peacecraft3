# Build provenance and isolated targets

## Reproduced cross-worktree freshness failure

On installed Cargo 1.100.0 (`3acc79c0c`, 2026-11-05), a small synthetic
two-workspace test reproduced stale path-dependency reuse. Both workspaces had
the same package names and versions, but their library functions returned
different strings. The second workspace's source modification times preceded the
first build; its application source was touched afterward to force relinking.
With one shared target directory, the second application printed its new marker
and the first library's value. Cargo reported that dependency as `fresh: true`
under the second workspace's package ID.

The private reproducer and structured result are retained at
`/tmp/pc3-cache-provenance-repro/`. This observation is specific to the recorded
toolchain, source timestamps and cache reuse. It establishes why serialized
access and a current `package_id` are insufficient build-source evidence.

The chapter executable audit independently found an older heading diagnostic
instead of the reviewed source's replacement. Those executions retain their
input-accounting observations, but attribution to the reviewed turning core is
withdrawn until a verified rebuild. Newly added root-test symbols prove only
those symbols are present, not every linked dependency's source.

## Verification procedure

- Keep one target directory per checkout/worktree. Never lend its workspace
  artifacts to another checkout, even after all processes finish.
- Reusing third-party caches requires removing all local/path packages and their
  reverse dependents from the copied target first. Preserve the original cache
  and acceptance artifacts. The current reset derived 24 path packages and 16
  additional reverse dependents from Cargo metadata; do not copy that count as a
  permanent package list.
- Freeze source and configuration before building. Record Cargo/rustc versions,
  target, features, Rust flags and the actual command. Retain Cargo JSON and
  private verbose diagnostics for the linked artifacts.
- The first verified build must compile active local/path units and affected
  reverse dependents from the intended manifests, rather than report them fresh
  from an unverified copied cache. Hash the emitted libraries, metadata and final
  executable and retain their dependency relationships.
- Subsequent incremental builds may reuse that same checkout's verified
  artifacts only while their source/configuration and artifact hashes still
  match. A source or executable hash without the dependency relationship is not
  a complete closure receipt. Literal/path checks are useful diagnostics, not
  substitutes for that receipt.
- Run exact named tests only after display/audio isolation and linked-closure
  verification. Retain failed or invalidated receipts separately; do not rewrite
  them into acceptance. Timing is diagnostic, never a deterministic-work gate.

The prepared per-checkout targets and cache-reset logs are recorded privately
under `/tmp/pc3-app-provenance-reset/`. No original game input was changed or
published by the reproducer or reset.
