# Authored campaign index

`pcx_objects::editor::campaign_index::parse_campaign_index` reads one
caller-selected CampaignStrings profile through the existing object-profile
parser. `[Index] CampaignList` supplies screen order, including blank slots.
Each named section supplies contiguous `MissionN` rows containing a header,
mission name and map filename. Quoted commas and literal path backslashes use
the shared profile lexer. Additional mission fields remain uninterpreted.

The result retains the selected member label. Missing sections, duplicate
campaigns or assignments, malformed quotes, noncanonical or sparse mission
indices and empty map filenames return contextual errors. An older `FileN`
table without `CampaignList` is unsupported; no map path is manufactured.

Callers supply byte, campaign-slot and per-campaign mission limits. The byte
limit is checked before profile parsing; output slot and mission growth is
checked before reservation. These are resource policies, not native limits or
a bound on all allocator overhead. The common profile parser validates fields
incrementally instead of allocating a temporary field vector per assignment.

`first_index_entry()` means the first nonblank list entry. It does not establish
unlock state, `DefaultOpen`, completion, prerequisites or playability. Archive
selection and base-game versus expansion availability remain caller policy;
reading an expansion profile does not grant expansion availability. No menu or
map-execution caller is added by this parser.

## Local source gate

The explicit selected archive was `War3xLocal.mpq` in the supplied Legacy
installation. This selects an input, not a native archive precedence rule or
release attribution. The checked archive reader extracted these members in
memory without saving their contents:

| Member | Bytes | FNV-1a 64 | Slots | Missions |
| --- | ---: | --- | ---: | ---: |
| `UI\CampaignStrings.txt` | 6963 | `ce62ddd295bb496c` | 5 | 42 |
| `UI\CampaignStrings_exp.txt` | 6774 | `2c8357dc05f9cbae` | 9 | 38 |

These fingerprints identify the measured content and are not cryptographic
release identities. The explicit gate can be rerun with:

```sh
PEACECRAFT3_CAMPAIGN_ARCHIVE=/path/to/selected/archive.mpq \
  cargo test --locked -p pcx-objects --test campaign_index -- --ignored --nocapture
```

Missing originals or members fail that gate. Eight synthetic index tests cover
ordering, quoted fields, extra fields, duplicates, malformed indices and exact
or zero limits; all eight also execute under WASI. The complete object suite,
the existing two-install stock-default sweep, strict native/WASM Clippy and
downstream native/WASM application checks pass. These are parser and source
checks, not rendered campaign-menu or gameplay acceptance.
