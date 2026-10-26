# De Morgan body style (#1233)

A library symbol can carry two bodies. Each library pin and graphic is tagged
with a unit and a body-style number: `0` common to every body, `1` normal,
`2` alternate (De Morgan). A placed symbol shows one body.

## One predicate

`signex_types::schematic::item_shown(item_unit, item_body, unit, body)` is the
only answer to "is this library item shown?". `instance_pins` and
`lower_symbol` use it, and so do the netlist, ERC, rubber-band, autoplace,
`ReplaceLibSymbol`, the Properties preview and the symbol editor, so they can
not disagree about which pins a placed symbol has.

`LibSymbol::effective_body(unit, requested)` is the shared fallback: the
alternate body is used only when the unit has one (`has_alternate_body`),
otherwise the normal body, so an Alternate request on a symbol without one
draws and wires as Normal.

## `.snxsch`

`Symbol.body_style` is stored as `body_style = 2` in
`[extras.symbols.<uuid>]`, only for an alternate symbol. A Normal symbol writes
nothing, so every file that never uses the feature is byte-identical. The key
counts toward the extras table's `is_default`, otherwise an alternate-only
symbol would be elided. A value other than 1 or 2 loads as Normal and logs a
warning, and the raw value is kept and written back unchanged until the user
picks a body for that symbol. No format-token bump: an older build keeps the key as an unknown
extras key, but shows both bodies' pins.

## `.snxsym`

`SymbolPin` and `SymbolGraphic` gain `body_style` (`0`, `1` default, `2`).
Graphics are TOML and skip the key when it is `1`. Pins are a TSV block with a
fixed header, so the column is appended (`body_style`, 21 columns) only when a
pin is off the normal body. The reader accepts both layouts; a missing column
means body 1. A file holding any pin or graphic off the normal body (0 or 2) is written as
`snxsym/v3`. Builds that predate this accept only v1 and v2, so they refuse a
v3 file with `UnsupportedFormat`. Reusing v2 would not do that: those builds
accept v2, ignore the unknown graphic key, hard-code body 1 and drop every tag
on their next save. v2 stays the token for polygon-only files and v1 for
everything else, with their exact legacy bytes. This build reads v1, v2 and v3.

## Switching

`Command::SetSymbolBodyStyle` refuses with
`EngineError::SymbolBodyPinsConnected` when a connected pin would be lost,
moved or land on a new connection. It shares its comparison
(`exec/connected_pins.rs`) with `ReplaceLibSymbol`. Unconnected pins may move.
