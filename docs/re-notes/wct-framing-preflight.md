# Custom-script container framing preflight

`Wct::parse` validates the complete length-word walk and exact input consumption
before reserving block metadata or copying text. Impossible counts are checked
against the physical number of minimum length words, without guessing a count
cap. The subsequent materialization uses fallible reservations with WCT region
and element-count context. Raw header words, text bytes and empty block positions
retain their existing representation.

Fresh native instrumentation compares the current baseline with the correction.
An eight-byte forged count previously requested 1,572,864 bytes; a later bad span
requested 4,144 bytes, and a trailing-byte case requested 51 bytes. All three now
fail with zero allocation requests. A valid two-slot synthetic input retains its
two requests and exact raw bytes. These are compiled allocation measurements,
not elapsed-time or gameplay performance claims.

Eight parser groups also execute as WASM. The explicit local archive-set gate
for `Maps\Campaign\NightElf06.w3m` compiles its 69 WCT slots with caller-selected
preludes, trigger catalog and authored map context: two custom blocks and six
custom functions. Archive release identity remains unestablished. This gate is
compilation and record preservation, not GUI-trigger desugaring, native execution
or a chapter playthrough.

Valid-input memory still needs caller policy. Preflight does not impose an
arbitrary global byte/record limit, bound total compiler memory, or establish
original-game malformed-input rejection behavior.
