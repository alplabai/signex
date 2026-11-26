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
    // #2552: a move's wire handling, given by the caller, not the settings.
    ("keep_wires", OperandKind::Bool),
    ("wiring", OperandKind::Text),
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
    // #2937: a library footprint, `<library>:<footprint>`.
    ("footprint", OperandKind::Text),
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
