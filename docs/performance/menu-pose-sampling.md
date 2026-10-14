# Borrowed menu pose sampling

`pcx_mdl::sample_bone_track` samples a borrowed track through the existing
component sampler. The menu's global-clock path uses it instead of constructing
a single-track `Bone` and cloning the original bone and all its owned keys.
Interpolation arithmetic, quaternion components, clock selection and missing
or zero-duration behavior are unchanged. Local-only bones still select the
first track of each kind; the mixed/global path still applies tracks in order,
so the last repeated kind wins. This preserves reconstruction behavior, not
verified native animation semantics.

The isolated `pose_sampling` integration test checks all four interpolation
kinds, translation/rotation/scaling, empty and single-key tracks, identities and
duplicate selection. A counting allocator requires zero allocations or
reallocations during repeated borrowed sampling. The app regression compares
exact pose bits with the previous cloning path, including clock fallbacks and
duplicates. Neither gate uses wall-clock timing.

```sh
cargo test --locked -p pcx-mdl --test pose_sampling
cargo test --locked -p peacecraft3 borrowed_tracks_preserve
```

## Original fixtures

A source-leaf measurement extracts the current app function and compares it
with the previous body at `70446c4`. Parsing and fixture loading happen outside
the counted window. Eight frames span each authored looping Stand clip, with
elapsed times spanning the longest stored GLBS duration. Three internal
repetitions and three process runs agree on counts and exact pose-bit digests.
The results below are observations of the Rust 1.99.0 optimized harness build,
not a compiler-independent allocation or cross-platform floating-point claim.

| Legacy archive member | Calls | Previous allocations / requested bytes | Borrowed allocations / requested bytes | Pose-bit digest, both |
| --- | ---: | ---: | ---: | --- |
| `War3.mpq :: UI\Glues\MainMenu\MainMenu3d\MainMenu3d.mdx` | 2,992 | 12,064 / 26,780,072 | 0 / 0 | `5011408c5f48f551` |
| `War3x.mpq :: UI\Glues\MainMenu\MainMenu3D_Exp\MainMenu3D_Exp.mdx` | 1,304 | 11,504 / 2,281,664 | 0 / 0 | `dd027edcdd7bc1b1` |

Both paths report zero reallocations. The member SHA-256 identities are
`98351d0e6f79023832d94959c91fef540f6c59692118544770fbcd4422669e64`
and `15586ffd9dc25b82a4f8ce2e679581f6e14cbcdc0916c7422d89308f010f6293`.
The RoC served copy has a different digest and is not this measurement input.

Local harness source, pinned library/compiler identities and three logs are in
`.scratch/menu-pose-root/`. Original bytes are not committed. The measurement
includes BONE tracks only; helper conversion, skeleton composition, mesh
skinning, frame allocations, FPS and GPU/native rendering are outside it.
