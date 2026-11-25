//! Design Notebook references (mockup rows 196 and 197): the references of
//! the project's notebook and where each stands, a reference's hover card,
//! the `@` picker, the schematic's Notebook badges and the fix of a stale
//! reference, each from the command line with a JSON answer; and View ▸
//! Notebook Badges (B).

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement, Operand, OperandKind};
use signex_i18n_ids::command_ids;

/// The project (or one of its documents) every command names.
const DOCUMENT: Operand = Operand {
    name: "document",
    kind: OperandKind::Document,
    required: true,
};

const fn text(name: &'static str, required: bool) -> Operand {
    Operand {
        name,
        kind: OperandKind::Text,
        required,
    }
}

/// The operands of each command.
mod operands {
    use super::{DOCUMENT, Operand, text};

    /// `notebook_references`: one notebook file only, by its path inside
    /// `notebook/`.
    pub(super) const NOTEBOOK_REFERENCES: &[Operand] = &[DOCUMENT, text("file", false)];
    /// `notebook_reference_card`: the reference by `kind` (`component`,
    /// `pin`, `net`) and `name` (`R7`, `U3.3`, `+3V3`).
    pub(super) const NOTEBOOK_REFERENCE_CARD: &[Operand] = &[
        DOCUMENT,
        text("kind", true),
        text("name", true),
        text("file", false),
    ];
    /// `notebook_reference_picker`: what is typed after the `@`.
    pub(super) const NOTEBOOK_REFERENCE_PICKER: &[Operand] =
        &[DOCUMENT, text("text", false), text("limit", false)];
    /// `notebook_badges`: the one badge of a part or a net.
    pub(super) const NOTEBOOK_BADGES: &[Operand] =
        &[DOCUMENT, text("part", false), text("net", false)];
    /// `notebook_fix_reference`: `fix` is `update`, `remove`, `keep_text`,
    /// `pick_pin` or `update_all`.
    pub(super) const NOTEBOOK_FIX_REFERENCE: &[Operand] = &[
        DOCUMENT,
        text("fix", true),
        text("kind", false),
        text("name", false),
        text("to", false),
        text("pin", false),
        text("file", false),
    ];
}

pub(super) const NOTEBOOK: &[CommandMetadata] = &[
    // Read-only: the notebook and the design are read off the UI thread
    // first, then answered from that read.
    CommandMetadata {
        id: "notebook_references",
        category: "design",
        label: command_ids::CMD_NOTEBOOK_REFERENCES,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::NOTEBOOK_REFERENCES,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "notebook_reference_card",
        category: "design",
        label: command_ids::CMD_NOTEBOOK_REFERENCE_CARD,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::NOTEBOOK_REFERENCE_CARD,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "notebook_reference_picker",
        category: "design",
        label: command_ids::CMD_NOTEBOOK_REFERENCE_PICKER,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::NOTEBOOK_REFERENCE_PICKER,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "notebook_badges",
        category: "design",
        label: command_ids::CMD_NOTEBOOK_BADGES,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::NOTEBOOK_BADGES,
        ..CommandMetadata::DEFAULT
    },
    // Edits the notebook tab's text, written on Save like any notebook edit.
    CommandMetadata {
        id: "notebook_fix_reference",
        category: "design",
        label: command_ids::CMD_NOTEBOOK_FIX_REFERENCE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::NOTEBOOK_FIX_REFERENCE,
        ..CommandMetadata::DEFAULT
    },
    // The badges are on by default (Caner, 2026-11-25); B shows or hides
    // them on every sheet.
    CommandMetadata {
        id: "toggle_notebook_badges",
        category: "view",
        label: command_ids::CMD_TOGGLE_NOTEBOOK_BADGES,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_NOTEBOOK_BADGES),
        group: CommandGroup::Schematic,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
];
