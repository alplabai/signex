//! The Design Checklist commands (#1291, mockup row 206): open the tab, read
//! the checklist and its link picker's rows, its edits (each one General
//! step of the project: start, import, reset, add, set, remove), the run of
//! the checks its items link to, the user's saved templates, the CSV export
//! and the link's Open, which cross-probes a design object.

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

/// `project_reset_checklist`, `project_checklist_templates` and
/// `project_checklist_link_choices`: the project, or one of its documents.
const DOCUMENT: &[Operand] = &[required("document", OperandKind::Document)];
/// `project_checklist`: `filter` is `all`, `open`, `done`, `waived` or
/// `mine`; Mine is the items `owner` owns, else the user's.
const READ: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("filter", OperandKind::Text),
    optional("owner", OperandKind::Text),
];
/// `project_start_checklist`: `release_review`, `prototype_build`, `blank`
/// or `user:<name>`.
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
/// `project_remove_checklist_item` and `project_open_checklist_link`: the
/// item by id.
const ITEM: &[Operand] = &[
    required("document", OperandKind::Document),
    required("item", OperandKind::Text),
];
/// `project_run_checklist`: every linked check, or only item `item`'s.
const RUN: &[Operand] = &[
    required("document", OperandKind::Document),
    optional("item", OperandKind::Text),
];
/// `project_save_checklist_template`: the template's `name`.
const SAVE_TEMPLATE: &[Operand] = &[
    required("document", OperandKind::Document),
    required("name", OperandKind::Text),
];
/// `project_export_checklist_csv` (the CSV file) and
/// `project_import_checklist` (the `.snxprj` to import from).
const FILE: &[Operand] = &[
    required("document", OperandKind::Document),
    required("path", OperandKind::Path),
];

pub(super) const CHECKLIST: &[CommandMetadata] = &[
    // Project ▸ Design Checklist: the active document's project, as a tab;
    // from a Projects panel row, that row's project.
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
        operands: READ,
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
        operands: RUN,
        ..CommandMetadata::DEFAULT
    },
    // More ▸ Reset from template: the items of the template it started
    // from put back, as one undo step.
    CommandMetadata {
        id: "project_reset_checklist",
        category: "design",
        label: command_ids::CMD_PROJECT_RESET_CHECKLIST,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    // The empty tab's Import from another project…: one undo step.
    CommandMetadata {
        id: "project_import_checklist",
        category: "design",
        label: command_ids::CMD_PROJECT_IMPORT_CHECKLIST,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: FILE,
        ..CommandMetadata::DEFAULT
    },
    // The built-in templates and the user's saved ones.
    CommandMetadata {
        id: "project_checklist_templates",
        category: "design",
        label: command_ids::CMD_PROJECT_CHECKLIST_TEMPLATES,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    // More ▸ Save as template…: written to the user's config folder, not
    // the project.
    CommandMetadata {
        id: "project_save_checklist_template",
        category: "design",
        label: command_ids::CMD_PROJECT_SAVE_CHECKLIST_TEMPLATE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: SAVE_TEMPLATE,
        ..CommandMetadata::DEFAULT
    },
    // More ▸ Export CSV….
    CommandMetadata {
        id: "project_export_checklist_csv",
        category: "reports",
        label: command_ids::CMD_PROJECT_EXPORT_CHECKLIST_CSV,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: FILE,
        ..CommandMetadata::DEFAULT
    },
    // The link picker's rows.
    CommandMetadata {
        id: "project_checklist_link_choices",
        category: "design",
        label: command_ids::CMD_PROJECT_CHECKLIST_LINK_CHOICES,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    // The link card's Open: a requirement, a check's results, the title
    // block, or a design object cross-probed.
    CommandMetadata {
        id: "project_open_checklist_link",
        category: "design",
        label: command_ids::CMD_PROJECT_OPEN_CHECKLIST_LINK,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: ITEM,
        ..CommandMetadata::DEFAULT
    },
];
