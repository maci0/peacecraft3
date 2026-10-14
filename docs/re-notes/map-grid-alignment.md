# Terrain/pathing grid alignment

The public `pcx_map::grid_alignment::terrain_pathing_alignment` helper and
`pcx_map::GridAlignment::from_dimensions` compare authored
W3E terrain dimensions with authored WPM pathing dimensions. It derives integer
scale ratios from those input dimensions: `pathing_width / terrain_width` and
`pathing_height / terrain_height`. It accepts only nonzero dimensions where both
quotients are whole numbers and identical, then rejects the comparison otherwise.

Provenance carried by `GridAlignment`:

- terrain tile grid (`width`, `height`) from W3E;
- terrain corner grid (`width + 1`, `height + 1`) from W3E;
- WPM raster grid (`width`, `height`) from WPM;
- derived width/height/uniform ratios;
- checked products for terrain tile, terrain corner and pathing cell counts.

This is explicit dimension validation; map importers and simulation do not
automatically invoke it. It does not define native world
units, pathing cell world size, height scaling, terrain walkability, or the
meaning of any WPM flag bit. Runtime map importers may use it to reject grids
whose authored dimensions do not form one uniform integer raster relationship,
but it is not evidence that any gameplay behavior follows from that relationship.

Synthetic public-API tests cover distinct ratios, mismatched axes, zero dimensions
and corner overflow. The optional original gate requires a caller-supplied local
`Maps` tree, refuses missing or malformed members, and checks loose maps only.
