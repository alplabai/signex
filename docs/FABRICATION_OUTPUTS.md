# Fabrication outputs

File ▸ Fabrication Outputs (#2388), the PCB Active Bar's Fabrication Outputs
button and the palette row all run `fabrication_outputs`. It opens one
dialog on the board in view: which files to write, the file-name stem, and
the output folder (default: a `fabrication` folder beside the board). Nothing
is written until Generate. Generate copies the board as it is in memory
(unsaved edits included), builds the files on a worker with
`signex_output::fab::generate`, and writes each file with the crash-safe
atomic write. A file of the same name in the folder is replaced. The dialog
stays open with what was written, or why nothing was, and the same line goes
to Messages. When the set leaves something out (see "Not drawn yet") the
Messages line carries every note and is a warning, not an info line, so a
set without its zone copper is never logged as a clean one.

Generate never deletes. Files in the folder that start with `<name>-`, end
in `.gbr`, `.drl`, `.csv`, `.ipc` or `.gbrjob`, and were not written by this
run (an inner layer removed since, a kind unticked, a span no longer
drilled) are listed in the dialog and in the Messages warning as older
files to remove before the folder goes to a board house. The job file
lists only the files of this run.

The writers live in `crates/signex-output/src/fab/` (no UI); the dialog in
`crates/signex-app/src/app/fab_outputs/`.

## Files

| File | Holds |
|---|---|
| `<name>-copper-l<n>-<top\|innerK\|bottom>.gbr` | one per copper layer of the stack |
| `<name>-legend-<top\|bottom>.gbr` | silkscreen graphics, when there are any |
| `<name>-soldermask-<top\|bottom>.gbr` | mask openings (negative polarity) |
| `<name>-paste-<top\|bottom>.gbr` | paste apertures |
| `<name>-profile.gbr` | board outline and through cutouts |
| `<name>-vscore.gbr` | V-score lines, when there are any (#2410) |
| `<name>-pth.drl`, `<name>-npth.drl` | plated and non-plated holes and slots through the whole stack |
| `<name>-blind-l<i>-l<j>.drl`, `<name>-buried-l<i>-l<j>.drl` | plated holes of a blind or buried via span, one file per span |
| `<name>-drill-table.csv` | per file and tool: diameter, hole and slot counts |
| `<name>-centroid.csv` | pick and place |
| `<name>-netlist.ipc` | IPC-D-356 bare-board test netlist |
| `<name>-ipc2581.xml` | IPC-2581 revision C: the whole job in one file (#2484) |
| `<name>-odb.tar`, or the folder `<name>-odb/` | the ODB++ job (#2485), as one archive or as its folder tree |
| `<name>-job.gbrjob` | every file written, with its X2 file function |

The copper stack is top and bottom always, every inner layer the board's
layer table lists, and any inner layer an item is drawn on. Legend, mask and
paste images are written only when they hold something.

## Frame

All files share one frame: x to the right and y up, from the board origin
(#2384, `BoardOptions::origin`), as seen from the top: the coordinates the
board shows. Gerber, drill, pick-and-place, IPC-D-356, IPC-2581 and ODB++ all read it. The
board model's y grows down the screen, so every file mirrors y about the
origin, and an angle that is counter-clockwise in the model is clockwise in
the files (`FabFrame` in `fab/frame.rs` is the one place this lives). A part
at model (10, 70) on a board whose origin is at model (0, 80) is written at
(10, 10). A board saved without an origin counts from the model's zero.

Output is deterministic: no date, no version, no hash-map order. The
IPC-2581 schema requires history dates, so that file carries the epoch
(`1970-01-01T00:00:00Z`) to mean "not recorded", and the ODB++ `misc/info`
file carries `19700101.000000` the same way. The same board gives the
same bytes, so a diff of two generations shows only real changes.

## Gerber

RS-274X with X2 attributes. Millimetres, `%FSLAX46Y46*%`: six decimals, so a
coordinate is the board's integer nanometre value itself, with no rounding.
Header attributes: `.GenerationSoftware`, `.FileFunction` (`Copper,L1,Top`,
`Soldermask,Bot`, `Profile,NP`, ...) and `.FilePolarity`. Each aperture
carries `.AperFunction`; on copper each pad, track, via and zone fill carries
`.N` (net), and each pad `.P` and `.C` (designator and pad number). The X2
pad functions (`SMDPad,CuDef`, `ComponentPad`, ...) are for copper images;
a pad's flash on a mask or paste image is `Other,PadOpening`.

- Pads are flashed: a circle, an axis-aligned rectangle or stadium as a
  standard aperture, a turned rectangle or stadium and every round-rect as a
  macro (primitives 21 and 1, turned about the aperture origin), and a
  trapezoid or custom pad (#2242) as an outline macro (primitive 4) holding
  the corners `pcb_geom::pad_shape` places, already turned. The shape,
  centre and turn come from `pcb_geom` (`pad_world_center`,
  `pad_world_rotation_deg`), the same transform the canvas and DRC read. A
  pad with per-layer sizes (#2401) is flashed at its size on each copper
  image.
- Mask and paste: the mask and paste layers a pad lists; a pad that lists
  none opens the mask (and takes paste, for a surface pad) on its
  footprint's side, a hole on both faces. An opening takes the pad's size on
  the outer copper layer of its face. No expansion is applied: the board
  has no mask or paste rule yet. Vias are tented. A panel fiducial lists the
  top mask, so it opens the mask and takes no paste.
- Tracks are drawn with a round aperture of their width; vias flashed on
  every copper layer they span, at their diameter there (#2401).
- Board and footprint graphics on a fabrication layer are drawn: lines,
  rectangles, circles, arcs (`G75` multi-quadrant, with direction) and
  polygons; a filled one as a region.
- Zone copper is regions with their holes cleared (`%LPC*%`), largest first
  so a fill inside another's hole is drawn after the clear. The regions of
  one net on one layer are merged first, so a hole of one that a same-net
  region covers is copper, not cleared. Generate builds
  it with `FabExtras::of_board`: each pour's fill as `pcb_geom::zone_fills`
  computes it (#2216), the fill the canvas, the board PDF and the ratsnest
  read, and every other copper zone (Fill, Solid Region) solid. A zone's
  outline is never written in place of its fill: it would short every net it
  crosses. When no zone has any copper to write (a pour that fills nothing),
  the dialog and Messages note how many zones were left out. A keepout (a
  zone with `fill_type` `keepout`, #2373) has no copper and is not counted.
- Text on a fabrication layer is written in the board stroke font
  (`pcb_geom::text_strokes`, `docs/RENDERING_RULES.md`, "pcb-stroke-font")
  as strokes with a round aperture of `TEXT_STROKE_EM` of the text height
  (#2368): each footprint's shown designator and comment, placed and
  justified as the canvas places it (mirrored on the bottom side), a
  footprint's text drawings, board strings and text frame lines. A dot is a
  stroke of no length. A text the geometry cannot hold is counted with the
  skipped items.

## V-score lines and panels (#2410)

The board sketch's V-score lines are written on their own image,
`<name>-vscore.gbr` (`.FileFunction` `Vcut`), one stroke per line with the
profile's pen and `.AperFunction` `Other,VScore`. A board without one writes
no such file. When the board has a panel (`docs/PANELISATION.md`) the dialog
shows a "Whole panel" box, ticked by default: Generate then writes the panel
as one board (every copy, the frame as the profile, the routed edges, the
fiducials, the tooling and bite holes in the non-plated drill file, and the
panel's V-scores). Unticked, the board alone is written. A panel that does
not lay out on the board's outline is refused with the reason.

## Drill

Excellon, metric, absolute, explicit decimal point with three decimals
(micrometres), `FMAT,2`. One file per drilled span and plating, each with
X2 comments (`; #@! TF.FileFunction,Plated,1,2,PTH`): plated holes through
the whole stack, then one file per blind or buried via span top first
(`Plated,1,2,Blind` from an outer layer, `Plated,2,3,Buried` between inner
layers), then the non-plated holes. A blind or micro via is drilled only
between the outermost stack layers it names; one that names fewer than two
stack layers is not drilled and is counted as a skipped item. Tools are the distinct
diameters, smallest first. Holes come from pads (plated unless the pad is
non-plated), vias (plated) and the board sketch's mounting holes (as set).

A pad's oval drill (`drill.shape = "oval"`) is a slot (`G85`) along the pad's
longer side, keeping the ring the pad has across it: its length is the drill
width plus the pad's length minus its width. A slot on a non-plated pad goes
to the non-plated file.

## Pick and place

CSV with a header: `Designator,Value,Footprint,X (mm),Y (mm),Rotation,Side`.
One row per footprint with a designator whose component type is listed in
the BOM, sorted by designator. A part the BOM lists with no designator has
no row; the dialog and Messages note how many. X and Y are the footprint origin in the
frame above, in mm to three decimals; the rotation is counter-clockwise in
that frame, the turn its pads are flashed with, so a bottom-side part's row
and its Gerber flash agree.

## Test netlist

IPC-D-356, fixed 80-column records, `UNITS CUST 1` (millimetres, coordinates
and sizes in micrometres). Record 317 for a hole (pad or via, `VIA` for a
via), 327 for a surface pad; access `A00` for a hole, `A01` top, `A<n>`
bottom; `S0`–`S3` from the mask openings. A net name longer than 14
characters gets an `NNAME<k>` alias header. An unconnected pad is `N/C`. A
record's size is the pad's or via's size on the probed layer (#2401): the
top copper for a hole (`A00`) or a top pad, the bottom copper for a bottom
pad.

## Job file

The Gerber job JSON shape: generation software, board size from the outline,
copper layer count, thickness, and `FilesAttributes` (path, file function,
polarity) for every image and drill file; the table, centroid and test
netlist, the IPC-2581 file and the ODB++ archive or folder (listed once, by
its name) under `OtherFiles`.

## Layer roles

The whole-job exports (IPC-2581 #2484, ODB++ #2485) read one table,
`fab/roles.rs`, keyed by the typed layer id, never by a display name, so
renaming a layer row changes no output. Copper is a plane when the board's
stack (stored, or derived from the layer table's `plane` type) pours it as
one.

| Typed layer | IPC-2581 `layerFunction` | ODB++ matrix type |
|---|---|---|
| Copper | `SIGNAL`, or `PLANE` | `SIGNAL`, or `POWER_GROUND` |
| Silkscreen top / bottom | `LEGEND` | `SILK_SCREEN` |
| Solder mask top / bottom | `SOLDERMASK` | `SOLDER_MASK` |
| Paste top / bottom | `SOLDERPASTE` | `SOLDER_PASTE` |
| Adhesive top / bottom | `GLUE` | `DOCUMENT` |
| Board outline (`EdgeCuts`) | `BOARD_OUTLINE` | `ROUT`; the outline is also the step profile |
| Assembly top / bottom | `ASSEMBLY` | `DOCUMENT` |
| Courtyard top / bottom | `COURTYARD` | `DOCUMENT` |
| Mechanical 1–32, and every other layer | `DOCUMENT` | `DOCUMENT` |
| A drilled span (not a board layer) | `DRILL` | `DRILL` |

A layer's side comes from the typed id too: top, bottom, inner copper
(`INTERNAL`), or board-wide (`ALL`). A layer is named by its file token
(`TopCopper`, `Inner1Copper`, `TopSolderMask`); a mechanical layer keeps the
name its layer table gives it, or `Mechanical<n>` when it has none (and when
that name is another layer's).

## IPC-2581

Revision C, millimetres, one XML file, one step named after the file stem.
It holds:

- **Layers and stackup.** Every stack row top to bottom (overlay, paste,
  mask, copper, dielectric as `Dielectric<k>`, `DIELCORE`), then any other
  layer that carries a drawing or that the layer table lists, the V-score
  layer (`V_CUT`) when there are V-scores, and one drill layer per drilled
  span (`Drill_PTH`, `Drill_NPTH`, `Drill_BLIND_L1_L2`, ...) with its span.
  The stackup lists each stack row with its thickness.
- **Features.** Every image's marks, on the same frame as the Gerber
  images: pads as `Pad` with their pin (`padUsage` `TERMINATION`, `VIA`,
  `MASK` for a mask or paste opening), tracks and drawings as round-pen
  polylines, zone fills as contours with cutouts. Flashed shapes are written
  once in the standard dictionary, unturned, with the turn on the feature.
  Drawings on layers no Gerber image carries (fab, assembly, courtyard,
  documentation, mechanical) are written on their document layers.
- **Holes.** Each drill layer's holes and slots (`SlotCavity`, an oval from
  end to end) with their plating.
- **Profile.** The board outline and the board sketch's through cutouts.
- **Parts.** One package per footprint kind with its pins in the
  footprint's own frame, a component per part with a designator (rotation as
  in the pick-and-place file, mirrored on the bottom), a BOM item per value
  and footprint, and a logical net per net with the pins its pads carry.

No person, company or date is recorded. The file is written with every
generation; the dialog's own tick box for it is #2489.

## ODB++

Version 7.0, millimetres, one step `pcb`. `FabOptions::odb` picks the form:
`OdbForm::Archive` writes one uncompressed POSIX tar, `<name>-odb.tar`, whose
root folder is the job name; `OdbForm::Directory` writes the same tree under
`<name>-odb/`, so the job folder is `<name>-odb/<job name>/`. The dialog
writes the archive. Entity names (job, layers) are lowercase ASCII letters,
digits and `_ + - .`, at most 64 characters: a layer is named by its export
name from the table above, lowercased (`topcopper`, `inner1copper`, a
mechanical layer's own name such as `fold_lines`), and a name another layer
already has falls back to the layer's token, then takes `_2`, `_3`, ...
Record words (nets, designators, parts, pins) turn spaces, `;` and
non-ASCII characters into `_`; a net whose word another net already has
takes `_2`, `_3`, ..., since net names are unique. The job holds:

- **Matrix.** The layers in physical order: `comp_+_top` when parts sit on
  the top, top legend, paste and mask, the stack's copper and dielectric
  rows (`dielectric<k>`, type `DIELECTRIC`) with any copper the stack does
  not list put in copper order, the bottom's mask, paste and legend,
  `comp_+_bot`, then one drill layer per drilled span (`drill_pth`,
  `drill_npth`, `drill_blind_l1_l2`, ...) from its first to its last copper
  layer, then the V-score layer and every drawing layer as `MISC`
  `DOCUMENT` layers. The outline layer is a `BOARD` `ROUT` layer from the
  top copper to the bottom, holding everything the profile image holds:
  the outline, the through cutouts and every cut drawn on `EdgeCuts` (a
  slot in a footprint), which the step profile cannot carry.
  The matrix gives the stack order; layer thicknesses are not written.
- **Features.** Each layer's marks in the same frame as the Gerber images,
  every feature positive (a mask layer holds its openings): pads with their
  standard symbol (`r`, `rect`, `oval`, `rect…xr…`, sizes in micrometres)
  unturned with the clockwise turn on the record, SMD copper pads with the
  `.smd` attribute, tracks and drawings as lines and arcs, zone fills as
  surfaces (island clockwise, holes counter-clockwise). A full circle is two
  half arcs.
- **Drills.** Each drill layer's holes as pads and slots as lines, with a
  `tools` file listing each size, plated or not, hole or slot.
- **Profile.** The outline as one surface with the through cutouts as
  holes.
- **Components and nets.** `eda/data` holds one package per footprint kind
  with its pins in the footprint's own frame, and one net per net name.
  Each net's subnets point at the copper features it is made of: one
  toeprint subnet per pin, then traces, vias, and one plane subnet per
  filled area. A pin whose pad carries no net is listed under `$NONE$`.
  Each toeprint takes the copper of its own pad, so two pads numbered
  alike and a panel's copies of one designator each keep their net.
  Each side's `components` file places every designated part that has pads
  (rotation clockwise, `M` on the bottom) with a toeprint per pin naming
  its net and subnet. A toeprint is its package pin mirrored about the y
  axis when `M`, then turned clockwise by the rotation, then moved to the
  part's position. A part with no pads has no package and is left out.

No date or user is recorded. The archive is written with every generation;
the dialog's own choice of ODB++ and of its form is #2489.

## Refusals

Generate writes nothing, and says why, when no kind of file is chosen, the
name is empty or holds `/ \ : * ? " < > |`, or the board has no outline.
