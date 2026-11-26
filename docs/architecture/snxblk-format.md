# `.snxblk` block diagram format

A `.snxblk` file holds one block diagram (mockup row 171, #2796): blocks,
their ports, signal-flow connections with protocol labels, groups, and the
link from a block to the schematic sheet it stands for. The model is
`signex_types::block_diagram`; the reader is `block_diagram::file`, the
writer `block_diagram::writer`.

## Layout

TOML. The first two keys are the format token and the document id, then
the arrays of tables in a fixed order: every `[[groups]]`, every
`[[blocks]]` with its `[[blocks.ports]]` after it, every `[[connections]]`.
A blank line goes before each table header.

```toml
format = "snxblk/1"
diagram_id = "0193a6c0-0000-7000-8000-000000000000"

[[groups]]
id = "…"
name = "Power"
color = "#d0453aff"

[[blocks]]
id = "…"
name = "DC/DC Buck"
function = "Buck converter"
note = "3 A buck, 570 kHz"
shape = "rounded_rectangle"
x_nm = 70000000
y_nm = 30000000
width_nm = 40000000
height_nm = 25000000
color = "#2f6fdbff"
group = "…"
rails = ["VIN_12V", "VOUT_3V3"]
specs = { "Fsw" = "570 kHz", "Vout" = "3.30 V" }
sheet = "…"
sheet_path = "dcdc.snxsch"
sheet_state = "linked"

[[blocks.ports]]
id = "…"
name = "VIN_12V"
side = "left"
offset_nm = 8000000
direction = "input"

[[connections]]
id = "…"
from_block = "…"
from_port = "…"
to_block = "…"
interface = "power"
label = "VIN_12V 12.0 V"
derived_from = "VIN_12V"
```

| Key | Values | Written |
|---|---|---|
| `shape` | `rectangle`, `rounded_rectangle`, `ellipse`, `hexagon` | always |
| `x_nm`, `y_nm` | the block's top-left corner, integer nm | always |
| `width_nm`, `height_nm` | integer nm, above zero | always |
| `function`, `note`, `label` | text | when not empty |
| `color` | `#rrggbbaa` | when the person picked one |
| `group`, `from_port`, `to_port` | an id in the same file | when set |
| `rails` | net names, in the block's order | when not empty |
| `specs` | an inline table, name to value, sorted by name | when not empty |
| `sheet`, `sheet_path`, `sheet_state` | the sheet's document id, its project-relative path with `/`, `linked` or `unlinked` | together, when the block is linked |
| `side` | `left`, `right`, `top`, `bottom` | always |
| `offset_nm` | along the side from its top or left end | always |
| `direction` | `input`, `output`, `bidirectional` | always |
| `interface` | `signal`, `bus`, `power`, `analog`, `spi`, `i2c`, `uart`, `usb`, `lvds`, or any other name as given | always |
| `derived_from` | the net, bus or harness a generated connection came from | for a generated connection |

Lengths are integer nanometres on disk and `f64` mm in memory, converted
with the same helpers as the schematic and board formats
(`format::mm_as_stored_nm`, `format::nm_to_mm`).

## Determinism

The writer builds the text itself instead of handing the model to a
serializer, so one diagram always gives the same bytes: keys in the order of
the table above, objects in diagram order, specs sorted, an optional key left
out when it is empty, strings as TOML basic strings with `"`, `\` and the
control characters escaped. The corpus under
`crates/signex-types/tests/corpus/block_diagram/` pins it: every fixture
must re-save byte for byte.

## Consistency

Every id is unique in the file, every `group`, `from_*` and `to_*` names an
object in the same file, a connection's two ends differ, names are not
blank and sizes are above zero. `BlockDiagram::fault` is the one check: the
writer refuses a diagram that fails it and the reader refuses such a file,
naming the object. The edit methods (`add_block`, `connect`, …) run the same
check on the diagram they would leave and change nothing when it fails.

An interface name this build does not know reads as `Interface::Other` and
is written back as it was, so a newer build's interface survives a save.

## Generate and regenerate

`generate_block_diagram` and `regenerate_block_diagram` (#2968, `docs/CLI.md`,
"Block diagrams from the hierarchy") make the diagram from the sheet
hierarchy. `signex_net::derive_block_diagram` reads the compiled design into a
`DerivedDiagram`; `signex_types::block_diagram::regenerate_block_diagram`
merges it into a diagram (generate merges it into an empty one). No new file
key: a generated connection is the one with `derived_from`, a linked block
the one with `sheet`.

**Blocks.** One per sheet file the hierarchy reaches, in walk order, named
after the sheet symbol that first places it (the file stem for a page no
symbol places). A sheet that places other sheets and has no parts of its own
on any occurrence is the frame the blocks sit in, not a block. A repeated
sheet is one block. A sheet whose file carries no document id cannot be
linked and is left out. Ports are the sheet symbol's entries, a
`Repeat(<net>)` entry by its net (a page no symbol places: its Hierarchical
labels); inputs sit on the left side, outputs and the rest on the right,
`PORT_PITCH_MM` apart, the block growing to fit. Rails are the power nets the
sheet touches.

**Connections.** Each net touching two or more blocks joins a signal group,
the first rule that applies naming it:

| Group | When | Interface | Label |
|---|---|---|---|
| the bus text | the net is a member of a bus an entry or a label writes (`D[0..7]`) | `bus` | the bus text |
| the net | a power net | `power` | the net |
| the harness type | the net is named `TYPE.ENTRY` | `bus` | the type |
| the text before the last `_` | any other net (`I2C_SDA` is `I2C`, `VOUT_3V3_CH1` is `VOUT_3V3`) | the protocol the prefix names, trailing digits ignored (`SPI1` is `spi`), else `signal` | the signals after the prefix (`SCL, SDA`) for a protocol, the prefix for another group of several nets, the net for one |

A group of one net goes by the net's own name. The flow starts at the first
block whose port for the group is an output, else the first block, and runs
to each other block: one connection per pair. A port matches the group by
name, or as a bus entry that carries one of its nets or shares its prefix.

**Regenerate.** A sheet with no block gets one, laid out in rows by depth
below every existing block. A linked block follows its sheet: a renamed or
moved file updates `sheet_path` (the document id stays), a sheet that left
the project marks the block `unlinked` (it stays, with its ports and
hand-drawn connections), a sheet that comes back links it again. Rails and
ports follow the sheet; a hand-drawn connection on a port the sheet lost
ends on the block's edge. Generated connections are matched by
`derived_from` and their two blocks: changed in place, added or removed.
Positions, sizes, names, colours, notes, groups and every hand-drawn block
and connection stay. The changes are listed (`DiagramChange`) and written
only when asked. A sheet that is there but does not read refuses the run.

## Version

`snxblk/1` (`SNXBLK_VERSION`). A newer minor opens read-only and its saves
are refused, as for the other document formats; a newer major, a foreign
token and a file with no token are refused on load
(`docs/FORMAT_VERSIONS.md`). A file with no `diagram_id`, or the nil one, is
given a new id on read and the next save writes it (`docs/DOCUMENT_IDS.md`).
