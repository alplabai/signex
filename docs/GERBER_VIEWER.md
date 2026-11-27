# Gerber viewer

The Gerber viewer (#2539) shows a fabrication package the way a board house
reads it: Gerber images, Excellon drill files and the Gerber job file. It
checks what the fab will receive, or what came back from it, without a
second tool. The viewer is read-only: it never changes or writes a file.

The reading lives in the UI-free domain crate `signex-gerber`. It opens no
file and reads no clock: each reader takes a file name and its bytes. The
viewer itself is a document tab on the shared tab shell (`app/gerber_viewer`,
see "The viewer tab").

## What is read

- **Gerber**: RS-274X with X2 attributes, per the public Gerber format
  specification. Signex's own exporter writes it
  ([FABRICATION_OUTPUTS.md](FABRICATION_OUTPUTS.md), "Gerber").
- **Excellon drill**: the common subset of the format. That is an `M48`
  header with `METRIC` or `INCH`, the zero mode (`LZ`, `TZ`), an optional
  digit pattern (`000.000`) and tools (`T1C0.300`). The header ends with `%`
  or `M95`. The body holds tool selections, holes and `G85` slots, and ends
  with `M30`. X2 attributes in comments (`; #@! TF.FileFunction,...`) are
  read as Signex writes them.
- **Gerber job file** (`.gbrjob`): the JSON keys Signex writes. These are
  `Header`, `GeneralSpecs.ProjectId.Name`, `GeneralSpecs.Size.X`/`.Y` (mm),
  `GeneralSpecs.LayerNumber`, `FilesAttributes` (`Path`, `FileFunction`,
  `FilePolarity`) and `OtherFiles` (`Path`).

`read_file` sorts a file by its content first: `%FS`, `%TF`, `%MO` or a
`G04` line means Gerber, an `M48` line means Excellon, and a JSON object
with `Header` or `FilesAttributes` means a job file. Only then does it fall
back to the extension: `.gbr .gtl .gbl .gts .gbs .gto .gbo .gtp .gbp .gm1
.gko .art .pho` for Gerber, `.drl .xln` for Excellon and `.gbrjob` for a
job file. Every other file of a package is listed as **Not read**, with an
Info problem. It is never skipped without a word.

## Units and frame

Every coordinate is an exact integer nanometre (`i64`) in the file's own
frame: x to the right and y up, from the file origin. For a Signex file that
origin is the board origin. Digits are converted in `i128` and rounded half
away from zero once: millimetres times 10^6, inches times 25,400,000,
divided by the power of ten of the decimals. A coordinate is never passed
through a float. Aperture macro arithmetic is the one place a float is
used; its result is rounded to the nm once.

## X2 attributes used

- File: `.FileFunction` gives the layer (copper by position L1..Ln, mask,
  paste, legend, profile, drill span and plating; any other value is kept
  verbatim). The reader also keeps `.FilePolarity`, `.GenerationSoftware`
  and `.ProjectId`.
- Aperture: `.AperFunction`, kept with each aperture definition or drill
  tool.
- Object: `.N` (net), `.P` (designator and pin) and `.C` (component), with
  `\uXXXX` escapes decoded. This is the inverse of the exporter's escaping.
- `%TD` deletes attributes as the specification says. Other attributes are
  valid X2 and change no drawing, so they are accepted without a problem.
  Examples are `.Part`, `.MD5` and user attributes.

## What is drawn

Each object keeps its kind (flash, line, arc, region, hole), its load
polarity, its D code or drill tool, its X2 attributes, its file and line,
and its shapes as `pcb_geom::Shape`:

- A circle flash is a disc, a rectangle a rectangle, an obround a capsule
  and a polygon aperture a polygon. A macro flash is its primitives.
- A draw with a round aperture is a capsule. An arc is a chain of capsules
  whose chords stay within 1 µm of the true curve. A rectangle aperture
  draws a straight line as the hull of the rectangle at both ends.
- A region is one polygon per contour, arcs flattened. A drill hole is a
  disc and a slot a capsule.
- A clear (`%LPC`) object keeps `Polarity::Clear`; it is not subtracted
  from what lies under it.
- Two parts of an aperture are not cut out of its flash: the hole of a
  standard aperture and the exposure-off primitives of a macro. The flash
  is drawn solid, and Properties shows the full definition.

## Read problems

Nothing is dropped silently. Every command that is not understood or not
supported, and every object that cannot be built, becomes a `ReadProblem`.
It holds the file, the 1-based line, the line's text and one kind. What can
still be drawn is drawn. An object whose shape cannot be built keeps an
empty shape list, so the viewer draws a marker where it is:

- A flash with a macro the file never defines gets a red dashed box with
  "?". The macro is reported once, at its `%AD` line, with the count of
  objects that use it.
- A hole whose tool has no size gets a red cross. The tool is reported once,
  at its definition line, with the count of its holes.
- An aperture or tool the file never defines is reported at its first
  selection, with its count.

The severity follows one rule (`ProblemKind::severity`). A file with no X2
file function, whose name then comes from the file name, is a warning. So
is a file with no end command (`M02`, `M30`). A file that is not
fabrication data is information. Everything else is an error. The same rows
go to the app's Messages with the file and line.

### Reported as unsupported

Read, reported with their line, and the rest of the file still drawn:

- `%LM`, `%LR` and `%LS` with a value other than their identity. The
  objects are drawn untransformed.
- The deprecated `%IP`, `%MI`, `%OF`, `%SF`, `%AS` and `%IR` with a value
  other than their identity.
- Incremental coordinates (`%FS..I..`, `G91`).
- Macro primitive 6 (moiré) and any unknown primitive code.
- A draw (`D01`) with an aperture that cannot draw, `D03` inside a region,
  and an unknown G, D or M code.
- A step and repeat that would add more than 1,000,000 objects, and a block
  flash that would take an image past 10,000,000 objects. The objects
  already read are kept.
- In drill files: routing (`G00`-`G03`, `M15`-`M17`), `FMAT,1`, `ICI,ON` and
  anything else the body does not know.

Read in full: `%SR` step and repeat, `%AB` block apertures, `G74`
single-quadrant arcs, and `G70`/`G71` (the deprecated unit codes). A modal
coordinate word without an operation repeats the last operation. When a
file gives no format or units, the problem is reported once and the file is
read as 3.6 millimetres. A drill file with no units is read as inches, and
one with no zero mode drops leading zeros; both are reported once.

## Fixtures

`crates/signex-gerber/tests/fixtures/signex-export/` holds byte copies of
the exporter's golden files (`crates/signex-output/src/fab/testdata/`). The
test `every_fixture_is_a_copy_of_its_exporter_golden` fails when a fixture
differs from its golden, so a golden refresh forces a fixture refresh and
the reader is always checked against what the exporter writes today. The
round-trip test (`tests/round_trip.rs`) generates a board with every kind
of copper the exporter writes and reads back every image, drill file and
the job file, expecting no problem. The fixtures in `tests/fixtures/problems/`
are written by hand. Each one shows one problem a package from a board
house can have.

## Package

`signex_gerber::assemble` turns the files read together (a folder, a zip
or a pick of files) into a `Package`. Each image is one layer:

- **Name.** A layer's function comes from the file's own X2 file function
  (`%TF.FileFunction` or `; #@! TF.FileFunction` in a drill file). A file
  without one takes the function of the job file entry whose path names the
  same file. A file in neither is named from its file name: the layer keeps
  `file_label` ("fab-notes.gbr" is "Fab notes"), and a name that says what
  the layer is (a side and `silk`, `mask` or `paste`; `L3`; the usual
  three-letter extensions; any drill file) gives it a function too.
  Otherwise the layer has no function and is drawn as a drawing. The label is
  the user's own file name, shown as it is. The reader's "no X2 file
  function" warning stays in the problems whichever way the layer was
  named.
- **Copper by position.** Copper is numbered L1..Ln from the top, as in the
  PCB editor; n is the job file's layer number, else the highest copper
  number in the package.
- **Pairs.** Top and bottom legend, solder mask and paste pair with each
  other, and copper Lk pairs with L(n+1-k) (L1 with L4 and L2 with L3 on four
  layers). A layer whose partner is not in the package has none. Pairs are
  always mutual.
- **Default drawing order** (index 0 draws on top). The board profile, the
  drill files (plated, then non-plated) and the drawings come first, so they
  always draw above the copper. Then the stack from the top: top legend,
  paste and mask, copper L1..Ln, bottom mask, paste and legend. This is the
  stack order whether or not there is a job file: the Gerber job file does
  not have to list its files in stack order (Signex's own lists the copper
  first), so its list only breaks ties between files at the same place, then
  the order the files were given. Reset order goes back to it.
- **Nothing dropped.** A file that cannot be read at all (not text, not a
  Gerber, drill or job file, a job file that does not parse) is listed in
  `not_read` with its problem. With several job files the first one given is
  used. `Package::problems` lists every problem: the files not read in the
  order given, then each layer's by file name and line.

## Compare with board

`signex_gerber::compare_with_board` compares a package with the board it
should come from (mockup row 193d).

- **The board side is the exporter's.** The board is not drawn from its
  model here. Signex's own Gerber and drill writer
  (`signex_output::fab::generate`, Gerber and drill only) makes the board's
  files, and they are read back through the same reader as the package.
  The frame (`signex_output::fab::FabFrame`: x right, y up from the board
  origin), the pad and track shapes, the zone fills and the layer functions
  are therefore decided once, by the exporter, and a package Signex wrote for
  an unchanged board compares with no difference at all.
- **Layer matching.** Copper matches by its number L1..Ln, solder mask,
  paste and legend by side, the profile with the board shape, and a drill
  file with the holes of the same plating and span. A file whose function
  the board has no layer for (a drawing, L5 on a four-layer board) is
  unmatched: drawn, not compared, and listed once as an unmatched layer. A
  layer can be matched by hand with any board layer, or left as a drawing;
  that choice wins over its function. A board layer the board draws nothing
  on is compared as empty, so everything in the file is extra.
- **Pairing objects.** Two objects are the same when they are the same kind
  (flash, line, arc, region, hole) at the same place: each placing point
  (a line's ends in either order, an arc's centre, a region's box, a hole's
  position and slot end) and the box of their shapes within the tolerance
  on each axis. The default tolerance is 0.010 mm. Only dark objects pair;
  clear ones cut out of a region and are drawn, not compared.
- **Differences.** A board object with no partner is missing copper (on the
  board, not in the file); a package object with no partner is extra copper
  (in the file, not on the board). On mask, paste, legend and drill layers
  the same two kinds name that layer's material; the viewer filters them by
  layer. Extra copper is marked inside a keep-out when its shape meets a
  board keep-out of its layer (drawn keep-outs and the board sketch's);
  a keep-out on an inner layer is checked against every inner layer, since
  the file numbers inner layers by position. Two holes at the same place
  whose sizes differ by more than the tolerance are a drill size mismatch,
  with both sizes. A row carries the X2 net and pin of the object; a hole
  carries the pad flashed where it is drilled.
- **Not readable, never missing.** An object the reader could not build
  (an unknown aperture macro, a tool without a size) is listed as not
  readable, and the board object at its place is not listed as missing
  copper.
- **Order.** Differences are sorted by package layer, kind, then position,
  so the same inputs give the same list.
- **Overlay.** The comparison carries the board outline and cut-outs, its
  copper keep-outs and its copper shapes per copper layer, all in the file
  frame, for the viewer to draw over the package.
- **No outline.** A board with no outline makes no fabrication files, so
  every layer is unmatched and there is no other difference.
- Not decided here: the banner that says the board was saved after the
  package (it needs the files' times, so the viewer tab owns it).

## Read problems in Messages

`signex_app::app::gerber_read::report_read_problems` writes one Messages
row per read problem (mockup row 193e), in the order given (`Package::problems`), so
the rows outlive the viewer tab. Each row is a catalog sentence
(`locales/en-US/gerber_viewer.ftl`, one message per kind of problem) that
starts "Gerber Viewer:", names the file and the line, and says what was not
read and what is drawn instead: an aperture macro that is not defined draws
its flashes as markers, a tool with no size draws its holes as crosses. The
level is the problem's severity: errors as errors, the lost X2 file function
and a missing end-of-file command as warnings, a file that is not
fabrication data as information. The rows are logged under the target
`signex::gerber_viewer`, whose Messages code is `SIGNEX-GERBER_VIEWER`.
The Messages panel's Source column shows them as **Gerber Viewer**
(`panels::MessageSource::GerberViewer`, #2612), and its Source filter keeps
or hides them as one source. A line the reader crate `signex_gerber` logs
itself has the same Source; the exporter's Gerber writer stays under
Project. A package that cannot be read at all is one Messages error under
the same target, naming the package (the file name of its first path) and
the reason.

## The viewer tab

`signex_app::app::gerber_viewer` (mockup rows 193 and 193f) is the tab. One
Gerber Viewer tab is open at a time, in the main window only. It holds no
document, so nothing saves, undoes or restores from History through it,
and it does not undock. Its path is the package's first
path and its title the package's name ("Gerber Viewer" with nothing open).
Its state is `DocumentViewState::gerber_viewer`; closing the tab drops the
package and keeps the recent list.

- **Open paths.** A double-click on a Projects row named like fabrication
  data; files dropped on any window (the `file_drops` subscription); the
  tab's Open menu (the toolbar's Open, or Ctrl+O / ⌘O while the tab is
  shown) with Open folder…, Open zip package…, Open files… and the recent
  packages. A press off the menu closes it. A drop on a window that shows
  the tab opens whatever it holds; elsewhere it opens only when a file is
  named like fabrication data. One rule (`read::is_fabrication_name`)
  decides that name for Projects and for drops: `.zip`, or an extension
  `read_file` takes for Gerber, drill or job (`.gbr`, `.gtl`…, `.drl`,
  `.xln`, `.gbrjob`). The drops of one gesture open as one package: the
  window reports each hovered file, and the read starts when the last of
  them lands (with no hover report, after the batch of drops).
- **Reading.** Every read runs on a worker (`read.rs`): a folder gives its
  files (not its subfolders), a zip its entries, read in memory and never
  unpacked to disk, any other path itself. A file whose bytes cannot be
  had (a permission, a lock, a folder entry the listing cannot give, a zip
  entry that does not decompress) or that is over 256 MiB (a zip entry is
  read at most one byte past that, whatever its header says) is a Not read
  row with its problem, and the rest of the package opens. The entries of
  one zip may together decompress to at most 1 GiB, counted from the bytes
  actually read rather than the headers. Only a path given at the top that
  is not there, a zip that is not one, or a zip past that total, fails the
  read, as one Messages error. A read a later open superseded is dropped.
  Read problems go to Messages as above.
- **Layer list.** One row per file in drawing order, the top of the list
  drawing on top: a grip (drag to reorder), an eye, a swatch (opens a row
  of colours), the name, the file name and a badge, X2 (hover for the file
  function), job or name. Copper reads L1..Ln by position. Files that are
  not fabrication data close the list as Not read. A held row is let go
  wherever the button comes up on the tab, and by a click on the board.
- **Board view.** The file frame, x right and y up, mirrored while flipped.
  The near face draws at full strength, the far face and inner copper
  darker. Flip also swaps each pair's drawing order. The board outline,
  the drill files and the drawings (a file with any other function or
  none) always draw above the copper, whatever their place in the list;
  the list order decides within the copper stack and within that band.
  Drills draws the holes last of all as rings, a non-plated hole's ring
  dashed. Highlight net dims what is not on the selected object's X2 net;
  it is greyed with its reason when the files carry no X2 nets or nothing
  on a net is selected. Wheel zooms at the pointer, a drag (left or middle
  button) pans, a left click selects the topmost visible object. The
  picture is cached and drawn again only when the package's `revision`
  moves (any change to looks, order, flip, drills, net, selection or
  camera), the size changes or the theme colours do; the board's box is
  found once when the package opens.
- **Known drawing gaps.** A clear (`%LPC`) object is painted in the
  background colour, so it also hides the layers drawn under it: a
  cut-out in L1 shows as a hole through L2. A file whose image polarity
  is negative (`%TF.FilePolarity,Negative`) is drawn as if positive, so a
  negative plane reads inverted. Both need a cut-out within the object's
  own layer.
- **Esc.** On the tab, Esc closes the Open menu, else lets go of a held
  row where it was, else clears the selection. An overlay over the tab
  takes Esc first; with nothing to undo, Esc passes on as elsewhere.
- **Properties.** Read only: type, layer, file and line, D-code (T-code for
  a hole), aperture definition and X2 aperture function, X2 net, pin and
  component, position, hole size and polarity, in mm or mil. With nothing
  selected, the layer last clicked: file function, units, object count and
  creator.
- **Empty state.** Open folder… (primary), Open zip package…, Open files…,
  the accepted files in one line, and the last three of the recent
  packages. While files are dragged over a window the frame turns to the
  accent and names how many will open. Tools that need files are greyed
  with "Open files first".
- **Recent packages.** App-wide, ten kept, the empty state shows three
  (maintainer, 2026-11-24). A package whose files are gone stays listed,
  greyed, with "Files not found". The check runs on a worker when a path
  opens, when the Open menu opens, after a read fails and after the tab
  closes, so a moved package is marked before anyone clicks it; its answer
  also sets the clock the "opened … ago" text reads. The list is kept for
  the session; it is not written to the preferences yet.
- **Not in this change.** The menu rows File ▸ Open Gerber / Drill and
  Tools ▸ Gerber Viewer (the catalog command is below; menu content is the
  maintainers' call), Measure, Compare, in-tab Messages, the From this
  project link, the Open menu's "Add files to this view…", the layer
  right-click menu, pair hover, Alt+↑ / Alt+↓, Alt-click on an eye, Reset
  order, Reload and the cursor read-out.

## Opening from the catalog

`open_gerber_viewer [path=<folder, zip or file>]` (#2896) is the catalog
command that reaches the tab: the command palette, a key a profile binds,
the CLI and the screenshot run that compares the app with mockup rows 193
and 193f. It is in the File category, General group, enabled everywhere
(the start screen too), with no default key and no menu row yet.

- **Without `path`.** The tab opens, or is focused, showing what it holds:
  the empty state, or the package already open. The recent packages are
  checked as when the Open menu opens. A palette or key call runs this
  form; a headless call answers at once.
- **With `path`.** The folder, zip or file opens as one package, read as
  a drop or the Open menu reads it (`read.rs`); its problems go to
  Messages as above. The call is answered when the read lands. A read that
  fails is refused with the Messages sentence; a read a later open
  replaced, or that lands after the tab closed, is refused with
  "a later open replaced this read".
- **The answer.** JSON with `tab` (`"gerber_viewer"`), `reading` (a read
  still in flight), `paths` (what is open), `layers` (the file of each
  layer in the list's order, top first), `not_read` (the files not read)
  and `problems` (each with `severity`, `file`, `line` and `message`, the
  Messages sentence in the app's language). With nothing open the lists
  are empty. `view` says how the tab shows it; see the next section.
- **Who may run it.** The CLI runs it with the `read` capability; its
  `path` is read, never written. The agent channel refuses it, as every
  command that takes a file path. It changes no document, so it makes no
  undo step and no dirty mark.

## Setting the view by id

Every state of mockup row 193 is reachable by id, so the screenshot run
and a script can set it (`view_commands.rs`). All five are in the General
group, enabled everywhere and offered on the start screen, with no default
key and no menu row. A palette or key call sends the toolbar's own message
(Flip, Drills, Highlight net, Open) or clears the selection; a call with
operands, a headless call and an agent's call run the operand handler and
answer with the `open_gerber_viewer` JSON. They change session state only:
no undo step, no dirty mark, the `read` capability on the CLI.

| Command | Operands | What it sets |
| --- | --- | --- |
| `select_gerber_object` | `at` (x,y in mm, the files' frame), `layer` (a file name) | `at` selects the topmost visible object there, as a click does; with `layer` too, the topmost object of that layer there, even under another layer ("Plane selected (L2)"); `layer` alone picks the layer row; neither clears the selection |
| `flip_gerber_view` | `on` | the board seen from the bottom; without `on` it turns over |
| `show_gerber_drills_on_top` | `on` | the holes drawn over everything; without `on` it turns over |
| `highlight_gerber_net` | `net`, `on` | `net` keeps that X2 net at full strength; `on=false` lets the highlight go; otherwise the selected object's net, and without `on` a highlight already on is let go |
| `show_gerber_open_menu` | `on` | the Open menu shows or closes; without `on` it toggles; showing it checks the recent packages |

- **Tolerance.** A point given by id lands within 10 µm of an object: a
  point inside a pad or a region selects it; a marker (an object with no
  shape) needs its own point. A point on nothing selects nothing.
- **Refusals,** from the catalog: no package is open (every command but
  the Open menu); `layer` names no layer of the package, or a hidden one;
  `at` is a pin reference, not a point; `net` is on no object; Highlight
  net with no net to take gives the toolbar's own reason.
- **The answer's `view`:** `flipped`, `drills_on_top`, `unit` (`mm` or
  `mil`), `net` (the highlighted net or null), `open_menu`,
  `current_layer` (the file of the layer row picked, or null) and
  `selected` (null, or `layer` (its file), `object` (its place in that
  file's objects), `kind` (`flash`, `line`, `arc`, `region` or `hole`)
  and `net`).
- **Not by id yet.** The mm / mil switch, a layer's eye, colour and place
  in the list, and the camera keep their toolbar and list controls only.

The check of which recent packages are still on disk runs on a worker.
When the worker cannot run, Messages shows a Gerber Viewer warning worded
by the catalog (`gerber-viewer-recent-check-failed`), and the list and its
clock stay as they were.
