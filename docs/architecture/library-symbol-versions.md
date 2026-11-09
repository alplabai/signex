# Versioned library symbols on a sheet (#2209)

A sheet embeds the library symbols it draws (`SchematicSheet::lib_symbols`) so
it opens without any library mounted. Before #2209 it held one body per
`lib_id` and the first copy won: a library symbol edited after its first
placement was never drawn by later placements, and the sheet warned in the
Messages panel only.

## Model

- `Symbol.lib_id` is the identity of the library symbol,
  `"<library uuid>:<symbol uuid>"`. It is unchanged.
- `lib_symbols` is keyed by a **body ref**, `"<lib_id>#<8 hex>"`: one entry per
  distinct version of a body.
- A placed `Symbol` names the body it draws with in `Symbol.body_ref`.
  `SchematicSheet::body_of(&Symbol)` resolves it; `resolve_lib_symbol` (the
  netlist, ERC and output readers) goes through it. A symbol with an empty
  `body_ref` is drawn by `lib_id`, which is only what a symbol built in code
  without a library uses.
- Old and new versions coexist, so the netlist, ERC and output give each
  instance the pins and graphics of its own version.

## Hash spec

The 8 hex digits are FNV-1a (32-bit, offset basis `0x811c9dc5`, prime
`0x01000193`) over the canonical text of the body: the serde value tree,
object keys sorted bytewise, no whitespace, floats written with nine fixed
decimals and `-0` as `0`, arrays in order. Nothing depends on map iteration
order, the platform or the float printer. The ref is an identity, not a
checksum: `Engine` refuses two different bodies under one ref
(`LibSymbolBodyCollision`), and the sheet decoder never merges them silently.
Two bodies are the same version when their canonical text is equal (not
`PartialEq`: a float difference below the ninth decimal, or a NaN, must not
make a body collide with itself). Changing the spec changes every saved ref,
so it is a format change.

## Commands

- `RegisterLibSymbol` embeds a body under its own ref; the same version is a
  no-op, a different body under the ref is an error. `ReplaceLibSymbol`,
  `ChangeComponent` and the load-time upgrade run the same collision check
  (`SchematicSheet::check_body` / `embed_body`) before they change anything. A placement batch is
  `RegisterLibSymbol` + `PlaceSymbol`, so one undo removes both.
- `ReplaceLibSymbol { only }` ("Update from library") moves the instances of a
  library symbol to the new version: all on the sheet, or the parts of the
  `only` symbols. All units of a part move together (the part is the one
  `placed_unit_ids` names, so an unannotated `U?` unit is a part of its own). It is refused, changing
  nothing, when a connected pin would be lost or moved, or a unit would
  vanish; each instance is judged against the body it draws with now.
- `UpdateSymbolLibId` carries the new `body_ref`; `ChangeComponent` registers
  the replacement's own version.

## Drift

`DriftReport.sheets` still lists the `lib_id`s with a pending update (that
drives the Library Updates modal). `DriftReport.stale_bodies` lists, per sheet,
the embedded versions that differ from the library's current symbol: a placed
part is on an older version when its `body_ref` is among them. Properties shows
"N instances use an older version" with **Update from library** and, for an
instance that is itself behind, **Only this one**. Only bodies some symbol
references are judged: a version whose last instance was deleted is not drift. Nothing changes on an existing instance without that click.

## Pruning

An update drops the versions it leaves unreferenced. Undo restores them from
the history snapshot. A body whose last instance was deleted stays embedded
until the next update of that symbol; it is small, and keeping it makes undo of
the delete exact. Nothing is pruned on save.

## File format

`[extras.sheet.lib_symbols."<lib_id>#<hash>"]` holds the body and
`[extras.symbols.<uuid>] body_ref = "<lib_id>#<hash>"` names it, written only
when set.

### Format version

The writer is `snxsch/1.1`. A build that only knows `snxsch/1` reads
`lib_symbols` by `lib_id` and would find no body (parts without pins, a wrong
netlist). It sees `1.1` as a newer minor, so it opens the file read-only and
refuses to save it (`is_newer_minor`, `refuse_save`). A `snxsch/1` file loads
and is saved as `1.1`. The clipboard fragment marker is
`signex-clipboard-fragment/2` for the same reason: an older build ignores the
text instead of pasting pinless parts. This build still reads the `/1` marker
an older build wrote: the same parser upgrades its bodies to versions, so the
pasted parts resolve. It keeps writing `/2`.

### One-time upgrade of old files

A sheet saved before #2209 has bodies keyed by plain `lib_id` and no
`body_ref`. On load, `SchematicSheet::upgrade_unversioned_bodies` re-keys each
such body to its ref and sets `body_ref` on the instances that drew with it;
the next save writes the new form. The same pass re-keys any entry whose key
is not its body's ref (so stored keys heal after a later change to the body's
serde shape) and keeps the old key rather than overwrite a different body. This
is the single exception to the
no-legacy rule. It is removable: delete that function and its call in
`SnxSchematic::parse` when old files are no longer expected.

### A `body_ref` that names no body

A merge or a hand edit can leave a symbol with `body_ref = "L:A#00000000"`
while the table holds no such key. On load, after the upgrade,
`SchematicSheet::repair_dangling_body_refs` finds each such symbol and records
one `SheetLoadRepair::BodyRefDangling` per symbol (shown in Messages with the
file name, the designator, its position and the missing key; the sheet opens
modified). Fallback: when the sheet embeds exactly one body of that library
symbol, the symbol is re-pointed to it and the repair says so. With none or
several, the symbol is left dangling and only reported, because no version is
picked for the user; Update from library fixes it.

## Units on different versions

Update moves all units of a part together, but Annotate or a placement can still
join a unit on one version and a unit on another into one part. Nothing is
changed automatically. A Messages warning names the part (when Annotate runs and
after a placement) and suggests Update from library. Re-prefix reads the
reference letters of every unit's own version and leaves the part alone when
they disagree; it never reads only the first unit.

## Not covered

Sharing versions across sheets of a project is not done: each sheet embeds its
own versions, as before.
