//! Block diagram commands (mockup row 171, #2796): a new diagram in the
//! project, the edits of its blocks, ports, connections and groups, and the
//! query that answers the whole diagram. Every edit names its diagram by
//! `document` and answers with the id of what it made or changed.

use super::operands::{optional, required};
use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement, Operand, OperandKind};
use signex_i18n_ids::command_ids;

/// `new_block_diagram`: `name` is the file stem in the project folder
/// (default: the project's name).
pub const NEW_BLOCK_DIAGRAM: &[Operand] = &[optional("name", OperandKind::Text)];

/// A block's properties, each optional: `at` the top-left corner and `size`
/// the width and height in mm, `shape` (`rectangle`, `rounded_rectangle`,
/// `ellipse`, `hexagon`), `group` a group id, `rails` comma separated net
/// names, `specs` `name=value` pairs separated by `;`.
const BLOCK_PROPERTIES: [Operand; 9] = [
    optional("at", OperandKind::Point),
    optional("size", OperandKind::Point),
    optional("function", OperandKind::Text),
    optional("note", OperandKind::Text),
    optional("shape", OperandKind::Text),
    optional("color", OperandKind::Color),
    optional("group", OperandKind::Text),
    optional("rails", OperandKind::Text),
    optional("specs", OperandKind::Text),
];

/// `block_add`: a block named `name`, with any of its properties.
pub const BLOCK_ADD: &[Operand] = &[
    required("document", OperandKind::Document),
    required("name", OperandKind::Text),
    BLOCK_PROPERTIES[0],
    BLOCK_PROPERTIES[1],
    BLOCK_PROPERTIES[2],
    BLOCK_PROPERTIES[3],
    BLOCK_PROPERTIES[4],
    BLOCK_PROPERTIES[5],
    BLOCK_PROPERTIES[6],
    BLOCK_PROPERTIES[7],
    BLOCK_PROPERTIES[8],
];

/// `block_set`: block `id`, the properties to change, and in `clear` the
/// optional ones to clear (`color`, `group`), comma separated.
pub const BLOCK_SET: &[Operand] = &[
    required("document", OperandKind::Document),
    required("id", OperandKind::Text),
    optional("name", OperandKind::Text),
    BLOCK_PROPERTIES[0],
    BLOCK_PROPERTIES[1],
    BLOCK_PROPERTIES[2],
    BLOCK_PROPERTIES[3],
    BLOCK_PROPERTIES[4],
    BLOCK_PROPERTIES[5],
    BLOCK_PROPERTIES[6],
    BLOCK_PROPERTIES[7],
    BLOCK_PROPERTIES[8],
    optional("clear", OperandKind::Text),
];

/// One object of a diagram by its `id`: `block_remove`,
/// `block_port_remove`, `block_unlink`, `block_ungroup`.
pub const DIAGRAM_OBJECT: &[Operand] = &[
    required("document", OperandKind::Document),
    required("id", OperandKind::Text),
];

/// `block_port_add`: a port named `name` on `block`; `side` (`left`,
/// `right`, `top`, `bottom`, default `left`), `offset` along it in mm from
/// its top or left end, `direction` (`input`, `output`, `bidirectional`).
pub const BLOCK_PORT_ADD: &[Operand] = &[
    required("document", OperandKind::Document),
    required("block", OperandKind::Text),
    required("name", OperandKind::Text),
    optional("side", OperandKind::Text),
    optional("offset", OperandKind::Text),
    optional("direction", OperandKind::Text),
];

/// `block_link`: from one block or port to another, by id, with the
/// `interface` it carries (`signal`, `bus`, `power`, `analog`, `spi`,
/// `i2c`, `uart`, `usb`, `lvds` or any other name) and its `label`.
pub const BLOCK_LINK: &[Operand] = &[
    required("document", OperandKind::Document),
    required("from", OperandKind::Text),
    required("to", OperandKind::Text),
    optional("interface", OperandKind::Text),
    optional("label", OperandKind::Text),
];

/// `block_link_set`: a connection's interface and label.
pub const BLOCK_LINK_SET: &[Operand] = &[
    required("document", OperandKind::Document),
    required("id", OperandKind::Text),
    optional("interface", OperandKind::Text),
    optional("label", OperandKind::Text),
];

/// `block_group`: a group named `name` holding the blocks `ids`.
pub const BLOCK_GROUP: &[Operand] = &[
    required("document", OperandKind::Document),
    required("name", OperandKind::Text),
    optional("ids", OperandKind::Targets),
    optional("color", OperandKind::Color),
];

/// `block_group_set`: a group's name and colour; `clear=color` returns the
/// frame to the theme's colour.
pub const BLOCK_GROUP_SET: &[Operand] = &[
    required("document", OperandKind::Document),
    required("id", OperandKind::Text),
    optional("name", OperandKind::Text),
    optional("color", OperandKind::Color),
    optional("clear", OperandKind::Text),
];

/// `query_block_diagram`: the diagram `document` names.
pub const QUERY_BLOCK_DIAGRAM: &[Operand] = &[required("document", OperandKind::Document)];

pub(super) const BLOCK_DIAGRAM: &[CommandMetadata] = &[
    // File ▸ New ▸ Block Diagram: an empty diagram in the project folder,
    // opened unsaved; nothing is written until it is saved.
    CommandMetadata {
        id: "new_block_diagram",
        category: "file",
        label: command_ids::CMD_NEW_BLOCK_DIAGRAM,
        menu_label: Some(command_ids::CMD_MENU_NEW_BLOCK_DIAGRAM),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES_NOT_UNDOABLE,
        operands: NEW_BLOCK_DIAGRAM,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "block_add",
        category: "edit",
        label: command_ids::CMD_BLOCK_ADD,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: BLOCK_ADD,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "block_set",
        category: "edit",
        label: command_ids::CMD_BLOCK_SET,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: BLOCK_SET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "block_remove",
        category: "edit",
        label: command_ids::CMD_BLOCK_REMOVE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: DIAGRAM_OBJECT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "block_port_add",
        category: "edit",
        label: command_ids::CMD_BLOCK_PORT_ADD,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: BLOCK_PORT_ADD,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "block_port_remove",
        category: "edit",
        label: command_ids::CMD_BLOCK_PORT_REMOVE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: DIAGRAM_OBJECT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "block_link",
        category: "edit",
        label: command_ids::CMD_BLOCK_LINK,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: BLOCK_LINK,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "block_link_set",
        category: "edit",
        label: command_ids::CMD_BLOCK_LINK_SET,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: BLOCK_LINK_SET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "block_unlink",
        category: "edit",
        label: command_ids::CMD_BLOCK_UNLINK,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: DIAGRAM_OBJECT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "block_group",
        category: "edit",
        label: command_ids::CMD_BLOCK_GROUP,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: BLOCK_GROUP,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "block_group_set",
        category: "edit",
        label: command_ids::CMD_BLOCK_GROUP_SET,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: BLOCK_GROUP_SET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "block_ungroup",
        category: "edit",
        label: command_ids::CMD_BLOCK_UNGROUP,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: DIAGRAM_OBJECT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "query_block_diagram",
        category: "search",
        label: command_ids::CMD_QUERY_BLOCK_DIAGRAM,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: QUERY_BLOCK_DIAGRAM,
        ..CommandMetadata::DEFAULT
    },
];
