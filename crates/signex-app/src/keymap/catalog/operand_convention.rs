//! Test-only views of the catalog operands (#1934): the naming convention a
//! test holds every catalog row to, and the golden column text.

use signex_commands::catalog::{Operand, OperandKind};

/// The allowed operand names and the one kind each has, on every command.
pub const CONVENTION: &[(&str, OperandKind)] = &[
    ("at", OperandKind::Point),
    ("start", OperandKind::Point),
    ("end", OperandKind::Point),
    ("rotation", OperandKind::Angle),
    ("degrees", OperandKind::Turn),
    ("text", OperandKind::Text),
    ("net_name", OperandKind::Text),
    ("class", OperandKind::Text),
    ("lib_id", OperandKind::Text),
    ("path", OperandKind::Path),
    ("overwrite", OperandKind::Bool),
    ("color", OperandKind::Color),
    ("document", OperandKind::Document),
    ("id", OperandKind::Text),
    ("kind", OperandKind::Text),
    ("project", OperandKind::Text),
    ("name", OperandKind::Text),
    ("dirty", OperandKind::Bool),
    ("allow_merge", OperandKind::Bool),
    ("allow_open", OperandKind::Bool),
    ("dry_run", OperandKind::Bool),
    ("designator", OperandKind::Text),
    ("value", OperandKind::Text),
    // Delete what the command names instead of setting it.
    ("remove", OperandKind::Bool),
    ("net", OperandKind::Text),
    // #2326: the items a command acts on, by uuid; see `IDS_STILL_TEXT`.
    ("ids", OperandKind::Targets),
    ("kinds", OperandKind::Text),
    ("region", OperandKind::Text),
    ("fields", OperandKind::Text),
    ("offset", OperandKind::Text),
    ("limit", OperandKind::Text),
    ("expect_revision", OperandKind::Text),
    ("expect_revisions", OperandKind::Text),
    ("edits", OperandKind::Text),
    ("summary", OperandKind::Text),
    ("group", OperandKind::Text),
    ("documents", OperandKind::Text),
    ("location", OperandKind::Path),
    ("make_folder", OperandKind::Bool),
    ("profile", OperandKind::Text),
    ("overrides", OperandKind::Text),
    ("library", OperandKind::Text),
    ("part", OperandKind::Text),
    ("unit", OperandKind::Text),
    ("mirror", OperandKind::Text),
    ("sheet", OperandKind::Text),
    ("size", OperandKind::Point),
    ("file", OperandKind::Text),
    ("direction", OperandKind::Text),
    // #1276: a sheet symbol by id or name.
    ("symbol", OperandKind::Text),
    ("apply", OperandKind::Bool),
    ("baseline", OperandKind::Text),
    ("layer", OperandKind::Text),
    ("style", OperandKind::Text),
    ("layers", OperandKind::Text),
    ("pages", OperandKind::Text),
    ("scale", OperandKind::Text),
    ("monochrome", OperandKind::Bool),
    ("mirror_bottom", OperandKind::Bool),
    ("outline", OperandKind::Bool),
    ("drill_marks", OperandKind::Bool),
    ("title_block", OperandKind::Bool),
    // #2486: the assembly drawings by side.
    ("side", OperandKind::Text),
    ("sides", OperandKind::Text),
    // #2488: the STEP schema of a board export.
    ("schema", OperandKind::Text),
    // #2470: the fabrication outputs of a board's panel, not the board alone.
    ("whole_panel", OperandKind::Bool),
    ("field", OperandKind::Text),
    ("requirement", OperandKind::Text),
    ("on", OperandKind::Bool),
    ("scope", OperandKind::Text),
    ("values", OperandKind::Text),
    ("pair", OperandKind::Text),
    ("severity", OperandKind::Text),
    ("enabled", OperandKind::Bool),
    ("violation", OperandKind::Text),
    ("rule", OperandKind::Text),
    ("waived", OperandKind::Bool),
    ("width", OperandKind::Text),
    ("clearance", OperandKind::Text),
    ("via_diameter", OperandKind::Text),
    ("via_drill", OperandKind::Text),
    ("diff_pair_gap", OperandKind::Text),
    ("diff_pair_width", OperandKind::Text),
    ("stack", OperandKind::Text),
    ("sizes", OperandKind::Text),
    ("pitch", OperandKind::Length),
    ("exclude", OperandKind::Text),
    ("class_a", OperandKind::Text),
    ("class_b", OperandKind::Text),
    ("object_pair", OperandKind::Text),
    ("set", OperandKind::Text),
    ("preset", OperandKind::Text),
    ("pins", OperandKind::Text),
    ("units", OperandKind::Text),
    ("shape", OperandKind::Text),
    ("delta", OperandKind::Text),
    ("corners", OperandKind::Text),
    // The shared waiver form's Author (#2449).
    ("author", OperandKind::Text),
    // #1699: the ERC rule ids a No-ERC directive silences, comma separated.
    ("rules", OperandKind::Text),
    // #1489: a Net Parameters directive's diff-pair class, via hole and
    // current (A).
    ("diff_pair_class", OperandKind::Text),
    ("via_hole", OperandKind::Text),
    ("current", OperandKind::Text),
    // #1283: the template a new sheet or project starts from; #1988: a BOM
    // template's text and the BOM grouping (`variant` is declared with the
    // priced BOM below).
    ("template", OperandKind::Text),
    ("grouping", OperandKind::Text),
    // #2665: the Smart Paste dialog's array count, numbering and net names
    // (`nets` is declared with the routes below).
    ("copies", OperandKind::Text),
    ("designators", OperandKind::Text),
    ("net_pattern", OperandKind::Text),
    ("net_start", OperandKind::Text),
    // #2495: a key binding: the command id it runs, its key sequence and
    // the shortcut context it applies in.
    ("command", OperandKind::Text),
    ("keys", OperandKind::Text),
    ("context", OperandKind::Text),
    // #2552: a move's wire handling, given by the caller, not the settings.
    ("keep_wires", OperandKind::Bool),
    ("wiring", OperandKind::Text),
    // #3038: a part's fit in a variant, and an alternate part's part number
    // (its `footprint` is declared with #2937 below).
    ("fitted", OperandKind::Bool),
    ("mpn", OperandKind::Text),
    // #2470: Re-Annotate Board's numbering order and bottom side.
    ("order", OperandKind::Text),
    ("bottom", OperandKind::Text),
    // #2470: a board route's corners between its ends (`x,y;x,y`), how each
    // leg bends, walk around or push, and the nets of a Multi-Route.
    ("through", OperandKind::Text),
    ("corner", OperandKind::Text),
    ("mode", OperandKind::Text),
    ("nets", OperandKind::Text),
    // #2936: one board point `x,y` per id of `ids`, `;` separated.
    ("points", OperandKind::Text),
    // #2937: a library footprint, `<library>:<footprint>`, and what a swap
    // does with an old pad no new pad takes over (`refuse` or `keep`).
    ("footprint", OperandKind::Text),
    ("unmatched", OperandKind::Text),
    // #2457, #2941: rooms and Auto Place in Rooms.
    ("margin", OperandKind::Length),
    ("move_parts", OperandKind::Bool),
    ("keep_parts_inside", OperandKind::Bool),
    ("replace", OperandKind::Bool),
    ("room", OperandKind::Text),
    ("rooms", OperandKind::Text),
    ("weights", OperandKind::Text),
    ("thermal_gap", OperandKind::Length),
    ("separation_gap", OperandKind::Length),
    // #2942: a stage of the board's layout plan and its settings, each a
    // comma list (`weights`, `rooms` and `nets` as above).
    ("stage", OperandKind::Text),
    ("keep", OperandKind::Text),
    ("skip", OperandKind::Text),
    ("pours", OperandKind::Text),
    ("requirements", OperandKind::Text),
    // #2938: a BOM line by its key, an alternate's place in the line's
    // order, and the build a priced BOM is for.
    ("line", OperandKind::Text),
    ("position", OperandKind::Text),
    ("quantity", OperandKind::Text),
    ("currency", OperandKind::Text),
    ("variant", OperandKind::Text),
    // #1297: the part search's query, parameters and stock, and whether
    // it asks for fresh supplier data.
    ("query", OperandKind::Text),
    ("package", OperandKind::Text),
    ("tolerance", OperandKind::Text),
    ("rating", OperandKind::Text),
    ("min_stock", OperandKind::Text),
    ("refresh", OperandKind::Bool),
    // #2943: a rail's voltage and tolerance, as typed.
    ("voltage", OperandKind::Text),
    ("tolerance_percent", OperandKind::Text),
    // #2462: the difference ids taken to the board and to the schematic.
    ("to_board", OperandKind::Text),
    ("to_schematic", OperandKind::Text),
];

/// The commands whose `ids` is still text, not targets (#2326): a design
/// reuse block takes designators too, and `read_document` filters by ids
/// rather than acting on them. The list only shrinks; #2709 empties it.
pub const IDS_STILL_TEXT: &[&str] = &["read_document", "save_board_reuse_block"];

/// The golden column:`name:kind` with `/unit`, `?` when optional, comma
/// separated; `-` when the command takes none.
pub fn describe(operands: &[Operand]) -> String {
    if operands.is_empty() {
        return "-".into();
    }
    let parts: Vec<String> = operands
        .iter()
        .map(|op| {
            let unit = op.kind.unit().map_or(String::new(), |u| format!("/{u}"));
            let opt = if op.required { "" } else { "?" };
            format!("{}:{}{unit}{opt}", op.name, op.kind.name())
        })
        .collect();
    parts.join(",")
}
