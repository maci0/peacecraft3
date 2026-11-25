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

Map admission and campaign placement use this check to reject grids without
a uniform integer raster ratio. The calculation validates dimensions. World
units, cell pitch, height scaling, walkability and WPM bit meanings require
separate evidence.

Synthetic public-API tests cover distinct ratios, mismatched axes, zero dimensions
and corner overflow. The optional original gate requires a caller-supplied local
`Maps` tree, refuses missing or malformed members, and checks loose maps only.
