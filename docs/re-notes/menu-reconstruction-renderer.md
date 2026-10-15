# Glue backdrop reconstruction renderer

The current glue renderer implements a recreation convention; it does not require
an identical native rendering algorithm. Appearance, state behavior and resizing
still need comparison with the selected local edition. No original asset data is
embedded by this change.

## Asset-derived inputs

`glue_menu::backdrop` reads inherited FDF background/edge paths, tile flag,
`BackdropBackgroundSize`, equal background insets, mirror flag, corner size and
corner flags. Decoding supplies actual texture dimensions and RGBA texels.
`spawn_backdrop` derives relative rectangles from the resolved FDF frame and its
insets. Patterned backgrounds repeat independently by
`(inner_width, inner_height) / BackgroundSize`. The shader maps normalized node
coordinates to these repeats; each tile consequently occupies the authored fraction
of the *actual* laid-out UI rectangle on each axis, including an anisotropic resize.
No physical-pixel size or measured original tile size is copied into code.

## Explicit renderer conventions

- An absent background inset means zero inset. The parsed absence stays distinct
  from an authored zero. Negative/nonfinite or frame-consuming insets are reported.
- An edge file must be a horizontal strip of exactly eight square slices, each the
  decoded texture height. Fixed slots are `L,R,T,B,UL,UR,BL,BR`, independently of
  enabled flag count or written flag order. All four side slices are vertical strips. Top and bottom destinations use a
  clockwise UV rotation; vertical destinations retain their source orientation.
  This is a reconstruction convention, not a proven native atlas rule. Other shapes are reported rather than silently partitioned.
- Corner dimensions derive from `BackdropCornerSize`; small destination rectangles
  shrink all corners together to fit. Side pieces stretch between those corners.
- `BackdropMirrored` is horizontal reflection of the whole backdrop: background
  UVs reflect; edge destinations reflect and their source UVs reflect. It is not
  a claim about the original implementation's exact mirroring operation.
- Patterned tiled backgrounds use a repeating sampler plus an explicit per-axis
  UV transform. A missing tile size uses the decoded image's intrinsic logical-pixel dimensions
  via Bevy tiling; invalid authored sizes are reported. A measured uniform image
  can stretch equivalently. Existing Bevy filter defaults
  remain renderer policy, not source-authored sampler flags.

The shader uses ordinary alpha blending. Existing additive highlights keep their
separate shader. This does not implement `BackdropBlendAll`, asymmetric inset
ordering, arbitrary edge files, native focus/hover precedence or every FDF widget.

## Texture identity correction

`mainmenu_scene::spawn_geosets`, also used by the hanging sprite panels, now obtains
sampler wrap flags through the selected first material layer's TEXS **index**.
Two TEXS records may share an image path and differ in flags; path equality must
not merge their sampler policies. A missing referenced TEXS record is contextual
failure. This does not implement additional material layers or animated texture
selection.

## Validation and remaining acceptance

Synthetic tests exercise all eight source rectangles, shape/overflow refusals,
horizontal mirroring, asymmetric texel-marker orientation, missing-inset full-frame geometry, authored tile repeats,
patterned cached-art admission and two equal-path TEXS records with different flags.
These tests establish data flow and component construction, not visual fidelity.
Root-owned native/WASM application checks and rendered original-menu comparisons
remain the integration acceptance gates. Compare normal/pushed/disabled, mirrored
plates, corners and long edges at multiple aspect ratios; keep atlas/orientation
choices visible until that comparison succeeds. The same backdrop/material path
is reusable for Options and other FDF-selected screens.

## Settings-control decoration

`decorate_setting` takes an existing node, FDF identity and resolved frame size;
it does not create backend values or settings. Control backdrops reuse button
state layers. Slider thumbs use the referenced frame's Width/Height and authored
orientation, starting from the authored initial value and then reading the actual
SliderValue/SliderRange. Travel subtracts thumb extent; cross-axis art can overhang
and remains centered. Checkmarks follow Checked and select disabled highlight art
when supplied. Popup arrows use their referenced dimensions with a right-centered
placement convention. These art choices require rendered fidelity comparison;
no hardcoded source dimensions or independent widget values are supplied.
