# Map-info authored record spans

The player, force and table-list byte reports now retain the cursor spans of the
successfully parsed input. Their names keep the existing lossy UTF-8 decoding,
which can expand an invalid sequence into more bytes than the source occupied.
Summing decoded string lengths therefore misreported these record sections and
the residual head shown by `mapx`. Its arithmetic line now uses record byte
lengths rather than record counts; the separate count rows remain available.

The record spans exclude their counts. The complete raw/canonical input, decoded
fields, supported versions and truncation refusals retain their existing paths.
The public `TableList::wire_len` remains available as the length implied by its
current decoded fields. It is not the authored span after lossy decoding or caller
mutation.

Framing counts and cross-reference lengths describe current decoded fields;
cached record spans and `exact` describe the original parse. This view does not
validate or serialize later mutations, and its head is a residual including
sections not separately itemized. Synthetic tests cover invalid/valid UTF-8,
empty sections, nested table words, truncation and mutation. These checks establish
source-span reporting, not original text encoding, player selection or force and
random-table semantics.
