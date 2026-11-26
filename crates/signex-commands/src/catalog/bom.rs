//! BOM and part search commands (#2938, #1297): a BOM line's alternate
//! parts, the priced BOM, and the parametric part search over the open
//! libraries' cached supplier data. Shapes: `docs/CLI.md`, "BOM and part
//! search".

use super::operands::{Operand, OperandKind, optional, required};
use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement};
use signex_i18n_ids::command_ids;

/// A BOM line of the named project (`line`, its key) and one library row
/// (`library` and `part`, as `place_component` names a row).
pub const BOM_ALTERNATE: &[Operand] = &[
    required("document", OperandKind::Document),
    required("line", OperandKind::Text),
    required("library", OperandKind::Text),
    required("part", OperandKind::Text),
];
/// As [`BOM_ALTERNATE`], and the alternate's new place in the line's order
/// (1 is the most preferred).
pub const BOM_ALTERNATE_ORDER: &[Operand] = &[
    required("document", OperandKind::Document),
    required("line", OperandKind::Text),
    required("library", OperandKind::Text),
    required("part", OperandKind::Text),
    required("position", OperandKind::Text),
];
/// `query_bom`: the BOM of the named project (or of the project a sheet or
/// board belongs to), priced for `quantity` boards in `currency`, for the
/// `variant` named (else the active one).
pub const QUERY_BOM: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("quantity", OperandKind::Text),
    optional("currency", OperandKind::Text),
    optional("variant", OperandKind::Text),
];
/// `search_parts`: the Components panel's query over the open libraries
/// (none matches every part), narrowed by parameters and stock, priced at
/// `quantity`.
pub const SEARCH_PARTS: &[Operand] = &[
    optional("query", OperandKind::Text),
    optional("value", OperandKind::Text),
    optional("package", OperandKind::Text),
    optional("tolerance", OperandKind::Text),
    optional("rating", OperandKind::Text),
    optional("min_stock", OperandKind::Text),
    optional("quantity", OperandKind::Text),
    optional("refresh", OperandKind::Bool),
    optional("limit", OperandKind::Text),
];
/// `change_component` from operands (#1297): the placed part `id` on the
/// sheet `document` swapped for a library row (`library` and `part`).
pub const CHANGE_COMPONENT: &[Operand] = &[
    required("document", OperandKind::Document),
    required("id", OperandKind::Text),
    required("library", OperandKind::Text),
    required("part", OperandKind::Text),
];

pub(super) const BOM: &[CommandMetadata] = &[
    // An edit is one step of the project's General history (#2938).
    CommandMetadata {
        id: "bom_add_alternate",
        category: "design",
        label: command_ids::CMD_BOM_ADD_ALTERNATE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: BOM_ALTERNATE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "bom_remove_alternate",
        category: "design",
        label: command_ids::CMD_BOM_REMOVE_ALTERNATE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: BOM_ALTERNATE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "bom_set_alternate_order",
        category: "design",
        label: command_ids::CMD_BOM_SET_ALTERNATE_ORDER,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: BOM_ALTERNATE_ORDER,
        ..CommandMetadata::DEFAULT
    },
    // Queries: they read the documents and the libraries, and change nothing.
    CommandMetadata {
        id: "query_bom",
        category: "design",
        label: command_ids::CMD_QUERY_BOM,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: QUERY_BOM,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "search_parts",
        // It reads the open libraries in memory and reaches no server
        // (`refresh=true` is refused), so it does not need the network.
        category: "library",
        label: command_ids::CMD_SEARCH_PARTS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: SEARCH_PARTS,
        ..CommandMetadata::DEFAULT
    },
];
