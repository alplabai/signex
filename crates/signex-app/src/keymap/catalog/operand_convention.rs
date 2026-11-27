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
    // #2795: the Design Notebook tab's line (from 1), view, side panel and
    // next problem.
    ("line", OperandKind::Text),
    ("view", OperandKind::Text),
    ("panel", OperandKind::Text),
    ("next_problem", OperandKind::Bool),
    // Mockup rows 196 and 199: the fix of a stale notebook reference, the
    // name it is updated to and the pin (`U3.3`) it or a timeline names.
    ("fix", OperandKind::Text),
    ("to", OperandKind::Text),
    ("pin", OperandKind::Text),
    // Mockup row 199: a notebook timeline's quantity.
    ("quantity", OperandKind::Text),
    // #2945, mockup row 201: Generate design notebook's sections (a comma
    // list or `all`), variant, page template, document number, revision,
    // check source (`run`, `last`), the `--fail-on` conditions, and
    // whether it writes the report and replaces one that is there.
    ("sections", OperandKind::Text),
    ("variant", OperandKind::Text),
    ("template", OperandKind::Text),
    ("document_number", OperandKind::Text),
    ("revision", OperandKind::Text),
    ("checks", OperandKind::Text),
    ("fail_on", OperandKind::Text),
    ("save", OperandKind::Bool),
    ("update", OperandKind::Bool),
    // Mockup row 202: the notebook PDF export's notebook file, paper, stale
    // rule, annotation kinds and what it prints.
    ("notebook", OperandKind::Text),
    ("paper", OperandKind::Text),
    ("stale", OperandKind::Text),
    ("annotations", OperandKind::Text),
    ("contents", OperandKind::Bool),
    ("bookmarks", OperandKind::Bool),
    ("python_source", OperandKind::Bool),
    // #2552: a move's wire handling, given by the caller, not the settings.
    ("keep_wires", OperandKind::Bool),
    ("wiring", OperandKind::Text),
    // #1267, #2838: the curve shapes of a sheet and of a symbol body.
    ("center", OperandKind::Point),
    ("radius", OperandKind::Length),
    ("radius_x", OperandKind::Length),
    ("radius_y", OperandKind::Length),
    ("start_angle", OperandKind::Turn),
    ("end_angle", OperandKind::Turn),
    ("corner_radius", OperandKind::Length),
    ("control_1", OperandKind::Point),
    ("control_2", OperandKind::Point),
    // #2817: the Pin table's Paste rows text and its paste mode.
    ("paste", OperandKind::Text),
    ("mode", OperandKind::Text),
    // #1114: a text note's format and a rich note's Frame, Size, source,
    // editor cursor and link.
    ("format", OperandKind::Text),
    ("frame", OperandKind::Text),
    ("border", OperandKind::Text),
    ("padding", OperandKind::Text),
    ("sizing", OperandKind::Text),
    ("height", OperandKind::Text),
    ("source", OperandKind::Text),
    ("column", OperandKind::Text),
    ("target", OperandKind::Text),
    // #1178: a project font by family.
    ("family", OperandKind::Text),
    // #2836: a dimension's decimals and arrow style.
    ("precision", OperandKind::Text),
    ("arrows", OperandKind::Text),
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
