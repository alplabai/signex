//! The Design Checklist commands (#1291, mockup row 206): open the tab, read
//! the checklist, its edits (each one General step of the project) and the
//! run of the checks its items link to.

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement, Operand, OperandKind};
use signex_i18n_ids::command_ids;

const fn required(name: &'static str, kind: OperandKind) -> Operand {
    Operand {
        name,
        kind,
        required: true,
    }
}

const fn optional(name: &'static str, kind: OperandKind) -> Operand {
    Operand {
        name,
        kind,
        required: false,
    }
}

/// `project_checklist` and `project_run_checklist`: the project, or one of
/// its documents.
const DOCUMENT: &[Operand] = &[required("document", OperandKind::Document)];
/// `project_start_checklist`: `release_review`, `prototype_build` or `blank`.
const START: &[Operand] = &[
    required("document", OperandKind::Document),
    required("template", OperandKind::Text),
];
/// `project_add_checklist_item`: the item's `stage` and its title (`text`).
const ADD_ITEM: &[Operand] = &[
    required("document", OperandKind::Document),
    required("stage", OperandKind::Text),
    required("text", OperandKind::Text),
];
/// `project_set_checklist_item`: one `field` of item `item`, as typed in
/// `value`; empty clears the owner, the link and the notes.
const SET_ITEM: &[Operand] = &[
    required("document", OperandKind::Document),
    required("item", OperandKind::Text),
    required("field", OperandKind::Text),
    required("value", OperandKind::Text),
];
/// `project_set_checklist_state`: `open`, `done` or `waived`, which needs a
/// `reason`.
const SET_STATE: &[Operand] = &[
    required("document", OperandKind::Document),
    required("item", OperandKind::Text),
    required("state", OperandKind::Text),
    optional("reason", OperandKind::Text),
];
/// `project_remove_checklist_item`: the item by id.
const ITEM: &[Operand] = &[
    required("document", OperandKind::Document),
    required("item", OperandKind::Text),
];

pub(super) const CHECKLIST: &[CommandMetadata] = &[
    // Project ▸ Design Checklist: the active document's project, as a tab.
    CommandMetadata {
        id: "open_design_checklist",
        category: "design",
        label: command_ids::CMD_OPEN_DESIGN_CHECKLIST,
        menu_label: Some(command_ids::CMD_MENU_OPEN_DESIGN_CHECKLIST),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_checklist",
        category: "design",
        label: command_ids::CMD_PROJECT_CHECKLIST,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_start_checklist",
        category: "design",
        label: command_ids::CMD_PROJECT_START_CHECKLIST,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: START,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_add_checklist_item",
        category: "design",
        label: command_ids::CMD_PROJECT_ADD_CHECKLIST_ITEM,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: ADD_ITEM,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_set_checklist_item",
        category: "design",
        label: command_ids::CMD_PROJECT_SET_CHECKLIST_ITEM,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: SET_ITEM,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_set_checklist_state",
        category: "design",
        label: command_ids::CMD_PROJECT_SET_CHECKLIST_STATE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: SET_STATE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_remove_checklist_item",
        category: "design",
        label: command_ids::CMD_PROJECT_REMOVE_CHECKLIST_ITEM,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: ITEM,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "project_run_checklist",
        category: "validation",
        label: command_ids::CMD_PROJECT_RUN_CHECKLIST,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
];
