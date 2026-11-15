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
to Messages.

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
| `<name>-pth.drl`, `<name>-npth.drl` | plated and non-plated holes and slots |
| `<name>-drill-table.csv` | per file and tool: diameter, hole and slot counts |
| `<name>-centroid.csv` | pick and place |
| `<name>-netlist.ipc` | IPC-D-356 bare-board test netlist |
| `<name>-job.gbrjob` | every file written, with its X2 file function |

The copper stack is top and bottom always, every inner layer the board's
layer table lists, and any inner layer an item is drawn on. Legend, mask and
paste images are written only when they hold something.

## Frame

All files share one frame: x to the right and y up, from the board origin,
as seen from the top. The board model's y grows down the screen, so every
file negates y, and an angle that is counter-clockwise in the model is
clockwise in the files (`fab/frame.rs` is the one place this lives). #2384 puts a new
board's origin at its bottom-left with y up; when it lands, `fab_point` and
`fab_angle_udeg` follow whatever axis direction the model then has, and the
golden files change with them.

Output is deterministic: no date, no version, no hash-map order. The same
board gives the same bytes, so a diff of two generations shows only real
changes.

## Gerber

RS-274X with X2 attributes. Millimetres, `%FSLAX46Y46*%`: six decimals, so a
coordinate is the board's integer nanometre value itself, with no rounding.
Header attributes: `.GenerationSoftware`, `.FileFunction` (`Copper,L1,Top`,
`Soldermask,Bot`, `Profile,NP`, ...) and `.FilePolarity`. Each aperture
carries `.AperFunction`; on copper each pad, track, via and zone fill carries
`.N` (net), and each pad `.P` and `.C` (designator and pad number).

- Pads are flashed: a circle, an axis-aligned rectangle or stadium as a
  standard aperture, a turned rectangle or stadium and every round-rect as a
  macro (primitives 21 and 1, turned about the aperture origin). The shape,
  centre and turn come from `pcb_geom` (`pad_world_center`,
  `pad_world_rotation_deg`), the same transform the canvas and DRC read.
- Mask and paste: the mask and paste layers a pad lists; a pad that lists
  none opens the mask (and takes paste, for a surface pad) on its
  footprint's side, a hole on both faces. No expansion is applied: the board
  has no mask or paste rule yet. Vias are tented.
- Tracks are drawn with a round aperture of their width; vias flashed on
  every copper layer they span.
- Board and footprint graphics on a fabrication layer are drawn: lines,
  rectangles, circles, arcs (`G75` multi-quadrant, with direction) and
  polygons; a filled one as a region.
- Zone fills are regions with their holes cleared (`%LPC*%`), largest first
  so a fill inside another's hole is drawn after the clear.

### Not drawn yet

- **Zone fills (#2216).** A zone's outline is never written in place of its
  fill: it would short every net it crosses. Fills enter through
  `FabExtras::zone_fills`; until zone filling lands the dialog notes how many
  zones were left out.
- **Text (#2387).** Board and footprint text needs the stroke font board text
  drawing brings; stroked glyphs enter through `FabExtras::strokes`. Until
  then the dialog notes how many texts were not drawn.
- V-score lines are not written; the dialog notes them.

## Drill

Excellon, metric, absolute, explicit decimal point with three decimals
(micrometres), `FMAT,2`. Plated and non-plated holes are separate files with
X2 comments (`; #@! TF.FileFunction,Plated,1,2,PTH`). Tools are the distinct
diameters, smallest first. Holes come from pads (plated unless the pad is
non-plated), vias (plated) and the board sketch's mounting holes (as set).

A pad's oval drill (`drill.shape = "oval"`) is a slot (`G85`) along the pad's
longer side, keeping the ring the pad has across it: its length is the drill
width plus the pad's length minus its width. A slot on a non-plated pad goes
to the non-plated file.

## Pick and place

CSV with a header: `Designator,Value,Footprint,X (mm),Y (mm),Rotation,Side`.
One row per footprint with a designator whose component type is listed in
the BOM, sorted by designator. X and Y are the footprint origin in the
frame above, in mm to three decimals; the rotation is counter-clockwise in
that frame, the turn its pads are flashed with, so a bottom-side part's row
and its Gerber flash agree.

## Test netlist

IPC-D-356, fixed 80-column records, `UNITS CUST 1` (millimetres, coordinates
and sizes in micrometres). Record 317 for a hole (pad or via, `VIA` for a
via), 327 for a surface pad; access `A00` for a hole, `A01` top, `A<n>`
bottom; `S0`–`S3` from the mask openings. A net name longer than 14
characters gets an `NNAME<k>` alias header. An unconnected pad is `N/C`.

## Job file

The Gerber job JSON shape: generation software, board size from the outline,
copper layer count, thickness, and `FilesAttributes` (path, file function,
polarity) for every image and drill file; the table, centroid and test
netlist under `OtherFiles`.

## Refusals

Generate writes nothing, and says why, when no kind of file is chosen, the
name is empty or holds `/ \ : * ? " < > |`, or the board has no outline.
