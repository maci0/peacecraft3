# Parallel ownership

Several lanes work in this repository at once. These are the rules that keep
their edits from colliding. They describe what the code and the repository
already do; they are not new policy.

## The tracker is append-only

`TRACKER.md` is a shared list, so nobody rewrites another's entry.
`pcx_play::fuzz_report` is the reference implementation of that rule: it
turns a compared pair of traces into one TRACKER-ready snippet and returns
it. It never edits `TRACKER.md` itself, because the caller appends the
returned snippet as one entry. A lane that finds something to record adds an
entry; it does not rewrite the entries around it.

## One owner per file at a time

A file is owned by the lane that is editing it. `AGENTS.md` asks the lanes to
coordinate file ownership before editing shared files, and the practical
consequence is that a lane which finds an uncommitted change in a file it needs
does not edit over it. It waits for the owning lane to commit, or asks the
owning lane to make the change.

The gates the lanes share make this necessary rather than merely tidy.
`cargo fmt --check`, `cargo clippy --workspace --all-targets` and
`cargo test --workspace` all read the whole working tree, so a half-written
file in one lane's tree breaks the next lane's run — and an untracked file
under `crates/` breaks them too, because the workspace builds every target.

## Stale claims

A claim is held while a lane works on a file and is released when the lane
commits it. A change left uncommitted in the working tree by a lane that has
stopped is a stale claim. The next lane that needs that file may take it over,
and it must then either commit the stale change or revert it deliberately: it
must not be silently discarded, because the change may hold work that is not
recoverable from the transcript.

A stale claim is also a stale *assertion*. A claim in `TRACKER.md` or in a
re-note persists until the lane that made it releases it; the text does not
expire on its own, so a reader has to check the claim against the code before
relying on it. Notes that name a test or a command keep that name accurate:
`docs/re-notes/campaign-human05.md` and `docs/re-notes/fuzz-tierc.md` both
name something a reader can run.
