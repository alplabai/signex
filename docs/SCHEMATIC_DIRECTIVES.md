# Schematic directives

Three objects a sheet can carry that are not circuit (#2420). None of them
makes a connection; the netlist and ERC read them.

| Directive | Shape | What it does |
|---|---|---|
| Blanket | a drawn region | Gives every net inside it a net class, and drops the ERC rules it names inside it. |
| Compile Mask | a drawn region | ERC and the netlist skip what it covers. |
| Differential Pair | a point on a wire | Pairs the net it sits on (`X_P` or `X_N`) with its partner, for the differential-pair net classes (#1278) and the board router (#2389). |

The Generic No-ERC directive (`no_erc_directives`, a point) is older and
stays as it is.

## Model and file

`signex_types::schematic::SchDirective`, one enum tagged by `type`, held in
`SchematicSheet::directives` in placement order. A region is two opposite
corners (`start`, `end`, any order), a Differential Pair a `position`; all in
mm like every other schematic point.

On disk the list is `[[extras.sheet.directives]]`, a TOML array of tables:

```toml
[[extras.sheet.directives]]
type = "blanket"
uuid = "0192a8c0-0004-7000-8000-000000000001"
net_class = "HighSpeed"
erc_exclusions = ["builtin::unused_pin"]

[extras.sheet.directives.start]
x = 10.16
y = 12.7

[extras.sheet.directives.end]
x = 50.8
y = 38.1
```

(The writer's table layout, as for `[[extras.sheet.drawings]]`.)

`net_class` and `erc_exclusions` are left out when unset, and the whole key
is left out when a sheet has no directive, so a sheet without one re-saves
byte-identical. An older build keeps the key as an unknown `[extras.sheet]`
key and writes it back. A directive whose uuid repeats another object's is
re-minted on load like any other object (`kind::DIRECTIVE`).

## Meaning

All of it is derived once, in `signex-net` (`directives.rs`):

- **Compile Mask** (`compile_masked`). Covered means: a symbol's origin, both
  ends of a wire or bus, the position of a label, junction, no-connect,
  No-ERC directive or bus entry, or the whole box of a sheet symbol lies
  inside the mask, its edge included. `build_netlist`, the project stitch
  (`build_project_netlist`, `multi_named_nets`, the net index) and the ERC
  projection (`ErcContext::project`) all read the masked sheet, so they
  agree. A finding located inside a mask (on the sheet, or a net finding
  anchored there) is dropped too, which covers DSL rules.
- **Blanket** (`blanket_net_classes`). A net is inside a Blanket when one of
  its wires lies wholly inside it or one of its junctions does. The first
  Blanket in sheet order that sets a class wins, and `build_netlist` writes
  it to `Net::class`. `assign_net_classes` still sets the class from the
  project's table alone. ERC drops a finding located inside a Blanket whose
  `erc_exclusions` holds the finding's rule id (`builtin::...`, `user::...`)
  or `*`. The project netlist does not apply Blanket classes yet.
- **Differential Pair** (`diff_pairs`). The directive's wire is found the way
  a label finds its wire (`wire_at_point`); its net name minus `_P` or `_N`
  is the pair name, and both halves are looked up by name. A directive on no
  wire, or on a net without the suffix, pairs nothing.

## Editing

Every placement and edit is one engine command: `Command::PlaceDirective`
and `Command::UpdateDirective`, patch `DocumentPatch::DIRECTIVES` (part of
`ELECTRICAL`, so connectivity caches refresh). Move, rotate, mirror and
delete work on a selected directive like on any other item
(`SelectedKind::Directive`). Consecutive updates of one directive share one
undo step, like typing a field; a command or a checkbox click is a step of
its own.

| Surface | Blanket | Compile Mask | Differential Pair |
|---|---|---|---|
| Active Bar (Directives) and the canvas Place flyout | two corner clicks | two corner clicks | one click |
| Command | `place_blanket start end [class] [exclude] [document]` | `place_compile_mask start end [document]` | `place_diff_pair at [document]` |
| Edit | Properties (Net Class field, one checkbox per rule) or `edit_blanket id [start] [end] [class] [exclude] [document]` | move or delete | move or delete |

`exclude` is a comma-separated list of ERC rule ids; an unknown id is
refused. The commands answer with the directive as stored (its `id` for a
later `edit_blanket`); `place_diff_pair` also answers with the pair it found.
`read_document kinds=directive` lists them.

## Drawing

All three use the directive colour the No-ERC diamond uses (`no_connect`):
a Blanket is a dashed outline, a Compile Mask a dotted one, a Differential
Pair two short parallel strokes. A region is picked by a click near its
edge, so a click inside it still reaches what it covers
(`schematic_runtime/directives.rs`).

## Not yet

Copy and paste do not carry directives. Blanket classes are not applied to
the project netlist, and nothing on the board reads `diff_pairs` yet; #1278
and the router's differential pairs build on it.
