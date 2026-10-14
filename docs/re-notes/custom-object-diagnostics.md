# Custom-object base resolution diagnostics

The existing map application report requires a custom record's original rawcode
to resolve in the supplied Editor tables. Previously the custom branch checked
only whether its new ID collided with an existing object. An absent base with no
modifications could then produce a clean report with `kind: None`, contradicting
the stock-base requirement used by `MetadataCatalog`.

Custom records with missing bases now use the existing `UnknownOriginal` refusal.
The diagnostic keeps the map, table and original rawcode; the enclosing report
keeps its member and source label. New-ID collision checks retain their existing
precedence. Raw object records and modifications remain available, including
unsupported selectors and values. This changes diagnostic admission, not native
inheritance rules or archive ordering.

Synthetic tests exercise the real nested map/archive application path for unit
and item members, valid custom definitions, collision precedence and exact raw
field preservation. They do not prove native custom-object behavior. Catalog
family selection, duplicate custom IDs and cumulative member-layer provenance
remain separate requirements.
