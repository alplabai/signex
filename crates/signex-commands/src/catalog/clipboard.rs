//! Clipboard commands: copy, cut, the paste variants, duplicate, delete
//! and rubber-stamp copies of the selection.

use super::operands::optional;
use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement, Operand, OperandKind};
use signex_i18n_ids::command_ids;

/// The Smart Paste dialog's answer (#2665, mockup row 155), placed at once:
/// `at` where the click lands (else the copy's own place), `delta` the offset
/// of copy 1 and the step between copies (`<x>,<y>` mm), `copies` an array's
/// count, `designators` `next_free` or `keep_if_free`, `nets` `keep` or
/// `rewrite` with `net_pattern` and `net_start`. Left out, a choice is the
/// dialog's default.
const SMART_PASTE: &[Operand] = &[
    optional("at", OperandKind::Point),
    optional("delta", OperandKind::Text),
    optional("copies", OperandKind::Text),
    optional("designators", OperandKind::Text),
    optional("nets", OperandKind::Text),
    optional("net_pattern", OperandKind::Text),
    optional("net_start", OperandKind::Text),
    optional("document", OperandKind::Document),
];

pub(super) const CLIPBOARD: &[CommandMetadata] = &[
    CommandMetadata {
        id: "copy",
        category: "edit",
        label: command_ids::CMD_COPY,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::RUNS_HEADLESS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "cut",
        category: "edit",
        label: command_ids::CMD_CUT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "paste",
        category: "edit",
        label: command_ids::CMD_PASTE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "paste_array",
        category: "edit",
        label: command_ids::CMD_PASTE_ARRAY,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "paste_special",
        category: "edit",
        label: command_ids::CMD_PASTE_SPECIAL,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "smart_paste",
        category: "edit",
        label: command_ids::CMD_SMART_PASTE,
        menu_label: Some(command_ids::CMD_MENU_SMART_PASTE),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: SMART_PASTE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "duplicate",
        category: "edit",
        label: command_ids::CMD_DUPLICATE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "delete_selection",
        category: "edit",
        label: command_ids::CMD_DELETE_SELECTION,
        menu_label: Some(command_ids::CMD_MENU_DELETE_SELECTION),
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::MUTATES,
        operands: super::operands::DOCUMENT_ITEMS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "rubber_stamp_copy",
        category: "edit",
        label: command_ids::CMD_RUBBER_STAMP_COPY,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::RequiresSelection,
        flags: CommandFlags::GUI_MUTATES,
        ..CommandMetadata::DEFAULT
    },
];
