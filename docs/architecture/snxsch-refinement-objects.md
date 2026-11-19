# `.snxsch` Schematic Refinement objects (`snxsch/1.3`)

Schematic Refinement adds harnesses (#1279), unions (#1300), Bezier, dimension
and table drawings (#1267, #1300) and new directive fields (#1281). A sheet
that holds any of them is written as `snxsch/1.3`; a sheet without them is
written exactly as before (`snxsch/1.1`, or `1.2` with a No-ERC rule list).
The rule is `format::snxsch_token`; the version policy is in
`docs/FORMAT_VERSIONS.md`.

## What raises the token

Any one of:

- a harness connector or a harness wire;
- a harness entry type whose sheet pin exists (one for a missing pin is not
  written, so it does not count);
- a union;
- a `bezier`, `dimension` or `table` drawing (`SchDrawing::needs_format_1_3`);
- a `net_parameters` directive, a Blanket with `track_width`, a `pour` other
  than `normal` or `include_edge_nets`, or a `diff_pair` with a `class`
  (`SchDirective::needs_format_1_3`).

The token follows the content, not the token the file was read with, so a
sheet whose last such object is removed goes back to the lower token.

## Keys in `[extras.sheet]`

Four new keys, each omitted when empty:

| Key | Shape | Holds |
|---|---|---|
| `harness_connectors` | array of tables | `HarnessConnector` |
| `harness_wires` | array of tables | `HarnessWire` |
| `harness_entry_types` | table | sheet-pin uuid to harness type |
| `unions` | array of tables | `SchUnion` |

The new drawings and directive fields have no key of their own: they are
items of the existing `drawings` and `directives` arrays, tagged by `type`.
A new drawing's lock lives in `locked_objects`, like every drawing's.

```toml
[[extras.sheet.harness_connectors]]
uuid = "0192a8c0-0000-7013-8000-000000000011"
harness_type = "SPI_BUS"
width = 10.16
entry_pitch = 2.54
rotation = 0.0

[extras.sheet.harness_connectors.position]
x = 50.8
y = 25.4

[[extras.sheet.harness_connectors.entries]]
uuid = "0192a8c0-0000-7013-8000-000000000012"
name = "SCK"

[[extras.sheet.harness_wires]]
uuid = "0192a8c0-0000-7013-8000-000000000019"

[extras.sheet.harness_wires.start]
x = 63.5
y = 31.75

[extras.sheet.harness_wires.end]
x = 101.6
y = 31.75

[extras.sheet.harness_entry_types]
0192a8c0-0000-7013-8000-000000000002 = "SPI_BUS"

[[extras.sheet.unions]]
uuid = "0192a8c0-0000-7013-8000-000000000051"
name = "Flash interface"
tags = ["spi"]
members = [
    "0192a8c0-0000-7013-8000-000000000001",
    "0192a8c0-0000-7013-8000-000000000011",
]
locked = true
```

### Harness connector

- `position`: the top-left corner of the body (mm), whatever the rotation.
- `width`: from the flat side to the base of the tip (mm).
- `entry_pitch`: distance between entries (mm); default 2.54 when absent.
- `rotation`: 0, 90, 180 or 270 degrees, counter-clockwise on screen. At 0 the
  flat side faces left and the tip points right.
- `entries`: the signals in order, from the top of the flat side at rotation 0.
  The first entry sits one pitch below the top; the body is one pitch longer
  than its entries (`HarnessConnector::height`).
- The tip reaches `HARNESS_TIP_MM` (2.54 mm) past the body, centred on the side
  opposite the flat side; the harness wire leaves there
  (`HarnessConnector::tip_point`).
- `locked` is written only when set.

### Harness entry types

`harness_entry_types` marks a sheet entry (a sheet pin of a sheet symbol on
this sheet) as harness-typed. Keys are sheet-pin uuids. On save, an entry whose
sheet pin no longer exists is not written; on load it is dropped with a load
repair, so Messages says so. A key that is not a uuid refuses the file, naming
the key, rather than dropping the entry.

### Unions

A union lists its members by uuid: placed objects, harness connectors and
harness wires (`schematic::union_member_kinds`, the rule the engine keeps
too). A sheet pin, a harness entry and another union are not members. `tags`
is omitted when empty, `locked` when not set. On load, a member that is no
such object is dropped with a load repair, so Messages says so; a union left
with no members is kept.

### Harness definitions

The harness types themselves (`SPI_BUS`: SCK, MOSI, MISO, CS) are project data,
in `.snxprj` under `schematic_design.harness_definitions`
(`docs/architecture/snxprj-format.md`).

## Repeated uuids

Harness connectors, harness entries, harness wires and unions share the
sheet's one id space and are repaired as every other object is
(`format::uuid_repair`, #1830): an exact copy of an earlier object of the same
kind is dropped, a different object with a taken id gets a new derived uuid,
and the repair is reported in Messages. They are checked after every older
kind, so a file without them repairs exactly as before. A harness entry names
its connector's harness type as its owner. When a sheet pin is re-minted, its
harness entry type is copied to the new id, like a wire's net colour.

## What an older build does

A build that reads up to `snxsch/1.2` sees `snxsch/1.3` as a newer minor: it
opens the sheet read-only and refuses every save, so it never drops the new
content. A build from before the new drawing kinds or directive fields may
instead refuse to parse the sheet (an unknown drawing `type`, an unknown
directive `type`), which loses nothing either. Were an older build to save,
the four new `[extras.sheet]` keys would survive as unknown keys, but the new
directive fields would not, which is why the token changes.

## Tests

`format::refinement_tests` (each object alone raises the token, a full round
trip, the dropping rules, uuid repair), `format::version_tests`
(`snxsch_1_3_is_read_and_1_4_opens_read_only`), `schematic::harness_tests`
(connector geometry) and the corpus fixture
`crates/signex-types/tests/corpus/schematic/refinement-objects.snxsch`.
