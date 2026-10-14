# Terrain/pathing grid alignment

Implements `crates/pcx-map/src/grid_alignment.rs`. The helper compares authored
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

This helper performs map import validation only. It does not define native world
units, pathing cell world size, height scaling, terrain walkability, or the
meaning of any WPM flag bit. Runtime map importers may use it to reject grids
whose authored dimensions do not form one uniform integer raster relationship,
but it is not evidence that any gameplay behavior follows from that relationship.
