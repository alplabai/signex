//! Gerber Viewer commands: the states of mockup row 193 (a pad or a plane
//! selected, the board flipped, the holes on top, a net highlighted, the
//! Open menu), set by id so the palette, the CLI and the screenshot run
//! reach each one. Opening the viewer itself is `open_gerber_viewer`
//! (`documents.rs`). Each answers with what the viewer shows; none writes a
//! file or the document.

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement, Operand, OperandKind};
use signex_i18n_ids::command_ids;

const fn optional(name: &'static str, kind: OperandKind) -> Operand {
    Operand {
        name,
        kind,
        required: false,
    }
}

/// A point on the board in the files' own frame, and the layer (a file
/// name) to pick on; with neither, nothing is selected.
const SELECT: &[Operand] = &[
    optional("at", OperandKind::Point),
    optional("layer", OperandKind::Text),
];
/// A switch: set to `on`, or turned over when left out.
const SWITCH: &[Operand] = &[optional("on", OperandKind::Bool)];
/// The net to keep at full strength, or the selected object's; `on=false`
/// lets the highlight go.
const NET: &[Operand] = &[
    optional("net", OperandKind::Text),
    optional("on", OperandKind::Bool),
];

pub(super) const GERBER_VIEWER: &[CommandMetadata] = &[
    CommandMetadata {
        id: "select_gerber_object",
        category: "select",
        label: command_ids::CMD_SELECT_GERBER_OBJECT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: SELECT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "flip_gerber_view",
        category: "view",
        label: command_ids::CMD_FLIP_GERBER_VIEW,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: SWITCH,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "show_gerber_drills_on_top",
        category: "view",
        label: command_ids::CMD_SHOW_GERBER_DRILLS_ON_TOP,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: SWITCH,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "highlight_gerber_net",
        category: "view",
        label: command_ids::CMD_HIGHLIGHT_GERBER_NET,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: NET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "show_gerber_open_menu",
        category: "file",
        label: command_ids::CMD_SHOW_GERBER_OPEN_MENU,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: SWITCH,
        ..CommandMetadata::DEFAULT
    },
];
