# Technology requirements raw authored catalog

`pcx-objects/src/requirements.rs` exposes `RequirementsCatalog`, built directly from
`DefaultTables`. It walks every family and rawcode already resolved by the shipped
editor data, selects only metadata fields whose category is `tech`, and records
the joined SLK/profile result without evaluating the technology tree.

Missing objects, missing cells, profile conflicts, malformed values and profile
index overflow remain `Refusal` outcomes. No requirement count defaults to `1`,
and `DependencyOr`/`all`/`any` behavior is not interpreted.

Original-asset coverage is represented by `crates/pcx-objects/tests/requirements.rs`
(ignored, install-gated).
