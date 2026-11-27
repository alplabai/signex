//! Board commands' operands from the command-line batch (#2470, #2942,
//! #2944): the area tools, the fabrication outputs, the layout plan and an
//! open library's rows.

use super::{Operand, OperandKind, optional, required};

/// `place_board_fill` from its operands (#2470): the rectangle between
/// `start` and `end` (mm from the board origin, Y up) on the copper `layer`
/// (`TopCopper`) of the board `document` names. `net` names a board net, or
/// is empty for no net; left out, the net of the copper under its corners.
pub const BOARD_FILL: &[Operand] = &[
    required("document", OperandKind::Document),
    required("layer", OperandKind::Text),
    required("start", OperandKind::Point),
    required("end", OperandKind::Point),
    optional("net", OperandKind::Text),
];

/// `place_board_region` and `place_board_polygon_pour` from their operands
/// (#2470): the `corners` `<x>,<y>;<x>,<y>;..` (mm from the board origin, Y
/// up) on the copper `layer` of the board `document` names; `net` as for
/// [`BOARD_FILL`].
pub const BOARD_COPPER_AREA: &[Operand] = &[
    required("document", OperandKind::Document),
    required("layer", OperandKind::Text),
    required("corners", OperandKind::Text),
    optional("net", OperandKind::Text),
];

/// `place_board_keepout` (a copper or keep-out `layer`) and
/// `place_board_pour_cutout` (a copper `layer`) from their operands (#2470):
/// `corners` as for [`BOARD_COPPER_AREA`]. Neither has a net.
pub const BOARD_NETLESS_AREA: &[Operand] = &[
    required("document", OperandKind::Document),
    required("layer", OperandKind::Text),
    required("corners", OperandKind::Text),
];

/// The Fabrication Outputs dialog's Generate (#2470): which board and the
/// folder; `kinds` lists the files (the project's ticks when left out),
/// `name` the file-name stem, `whole_panel` whether a panel is written.
pub const FABRICATION_OUTPUTS: &[Operand] = &[
    required("document", OperandKind::Document),
    required("path", OperandKind::Path),
    optional("kinds", OperandKind::Text),
    optional("name", OperandKind::Text),
    optional("whole_panel", OperandKind::Bool),
];

/// `layout_plan_set` (#2942): one stage of the named board's layout plan
/// (`placement`, `routing`, `pours`, `checks`) and the settings of that
/// stage to change, each a comma list (empty clears it); `remove` clears
/// the whole stage.
pub const LAYOUT_PLAN_SET: &[Operand] = &[
    required("document", OperandKind::Document),
    required("stage", OperandKind::Text),
    optional("weights", OperandKind::Text),
    optional("rooms", OperandKind::Text),
    optional("keep", OperandKind::Text),
    optional("nets", OperandKind::Text),
    optional("skip", OperandKind::Text),
    optional("pours", OperandKind::Text),
    optional("rules", OperandKind::Text),
    optional("requirements", OperandKind::Text),
    optional("remove", OperandKind::Bool),
];

/// `query_library` (#2944): the rows of the open library `library` (its id
/// or path), or the row `part` names (a row id or a part number), or the
/// rows holding `text`.
pub const QUERY_LIBRARY: &[Operand] = &[
    required("library", OperandKind::Text),
    optional("part", OperandKind::Text),
    optional("text", OperandKind::Text),
];
