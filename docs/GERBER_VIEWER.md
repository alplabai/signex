# Gerber viewer

The Gerber viewer (#2539) shows a fabrication package the way a board house
reads it: Gerber images, Excellon drill files and the Gerber job file. It
checks what the fab will receive, or what came back from it, without a
second tool. The viewer is read-only: it never changes or writes a file.

The reading lives in the UI-free domain crate `signex-gerber`. It opens no
file and reads no clock: each reader takes a file name and its bytes. The
viewer itself is a document tab on the shared tab shell, which is still to
come.

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
