# Trigger-string materialization preflight

The `wts::Table` reader validates the complete borrowed record walk before
reserving metadata and copying the raw source. Materialization then uses fallible
reservations with region/element context. The extracted record helper preserves
the existing grammar, source offsets, comments, separators, record order, raw
bodies and duplicate-ID lookup policy.

Five late malformed synthetic inputs previously made four allocation requests
for 3,360 bytes each; fresh compiled instrumentation observes zero requests after
the correction. A valid 381-byte, 17-record input retains its views with two
requests. These are allocation counts for that compiled reader, not total process
memory or timing bounds.

Six actual map-script/custom-script consumer groups also execute as WASM,
covering localization, malformed/present/absent distinctions, used non-UTF8 text
and the existing finite caller WTS budget. An explicit local archive-set read of
`Maps\Campaign\NightElf06.w3m::war3map.wts` compares all 85 records against the
current baseline: raw bytes, IDs, bodies, comments and separators agree. Archive
release identity remains unestablished.

The legacy root `Wts` reader, total valid-input memory policy and original-game
encoding/localization behavior remain separate. This changes allocation timing
for malformed inputs, not native string precedence or tag interpretation.
