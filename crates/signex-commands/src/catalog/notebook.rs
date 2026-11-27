//! Design Notebook commands (#2795, mockup rows 195-202): open the
//! project's notebook, generate one from the project, export it as PDF,
//! the references of the notebook and where each stands, a reference's
//! hover card, the `@` picker, the schematic's Notebook badges and the fix
//! of a stale reference, View ▸ Notebook Badges (B), and the notebook tab's
//! own commands (compile and report, insert what the toolbar inserts, pick
//! the view and side panel). The commands that name the project with
//! `document` reach the tab the person sees from a CLI or an agent.

use super::{
    CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, Operand, OperandKind,
};
use signex_i18n_ids::command_ids;

/// The project (or one of its documents) every reference command names.
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

/// The operands of the reference commands (mockup rows 196 and 197).
mod reference_operands {
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
    // Project ▸ Design Notebook, Add New ▸ Design Notebook and a palette
    // search for "notebook" or "wiki" (mockup rows 195 and 60b).
    CommandMetadata {
        id: "open_design_notebook",
        category: "design",
        label: command_ids::CMD_OPEN_DESIGN_NOTEBOOK,
        menu_label: Some(command_ids::CMD_MENU_OPEN_DESIGN_NOTEBOOK),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // #2945, mockup row 201: Project ▸ Design Notebook ▸ Generate from
    // Project…, the design report from requirements, checks, decisions, BOM,
    // simulation and outputs, as its own notebook file. With no operands it
    // opens the options dialog for the project in view.
    CommandMetadata {
        id: "generate_design_notebook",
        category: "design",
        label: command_ids::CMD_GENERATE_DESIGN_NOTEBOOK,
        menu_label: Some(command_ids::CMD_MENU_GENERATE_DESIGN_NOTEBOOK),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::operands::GENERATE_DESIGN_NOTEBOOK,
        ..CommandMetadata::DEFAULT
    },
    // Mockup row 202: File ▸ Export ▸ Design Notebook PDF… and the notebook
    // toolbar's Export PDF… open the export dialog on the project in view;
    // headless (`signex-cli notebook export-pdf`) it writes the notebook of
    // `document`'s project to `path`.
    CommandMetadata {
        id: "export_notebook_pdf",
        category: "file",
        label: command_ids::CMD_EXPORT_NOTEBOOK_PDF,
        menu_label: Some(command_ids::CMD_MENU_EXPORT_NOTEBOOK_PDF),
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::operands::NOTEBOOK_PDF_EXPORT,
        ..CommandMetadata::DEFAULT
    },
    // References (mockup rows 196 and 197). Read-only: the notebook and the
    // design are read off the UI thread first, then answered from that read.
    CommandMetadata {
        id: "notebook_references",
        category: "design",
        label: command_ids::CMD_NOTEBOOK_REFERENCES,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: reference_operands::NOTEBOOK_REFERENCES,
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
        operands: reference_operands::NOTEBOOK_REFERENCE_CARD,
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
        operands: reference_operands::NOTEBOOK_REFERENCE_PICKER,
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
        operands: reference_operands::NOTEBOOK_BADGES,
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
        operands: reference_operands::NOTEBOOK_FIX_REFERENCE,
        ..CommandMetadata::DEFAULT
    },
    // View ▸ Notebook Badges (mockup row 197). The badges are on by default
    // (Caner, 2026-11-25); B shows or hides them on every sheet. Greyed
    // with no sheet in view, in the palette as in the menu (#3024).
    CommandMetadata {
        id: "toggle_notebook_badges",
        category: "view",
        label: command_ids::CMD_TOGGLE_NOTEBOOK_BADGES,
        menu_label: Some(command_ids::CMD_MENU_TOGGLE_NOTEBOOK_BADGES),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::GUI_ONLY,
        ..CommandMetadata::DEFAULT
    },
    // The notebook tab (mockup row 195).
    CommandMetadata {
        id: "notebook_compile",
        category: "reports",
        label: command_ids::CMD_NOTEBOOK_COMPILE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "notebook_insert",
        category: "edit",
        label: command_ids::CMD_NOTEBOOK_INSERT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: super::operands::NOTEBOOK_INSERT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "notebook_view",
        category: "view",
        label: command_ids::CMD_NOTEBOOK_VIEW,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::operands::NOTEBOOK_VIEW,
        ..CommandMetadata::DEFAULT
    },
    // The bench commands (mockup row 199): the readings, a target's
    // timeline and the attachments, the timeline written as CSV, Mark still
    // valid, and a file copied into `notebook/attachments/`. None has a key
    // or a menu row (the notebook's margin lane is their surface).
    CommandMetadata {
        id: "notebook_measurements",
        category: "design",
        label: command_ids::CMD_NOTEBOOK_MEASUREMENTS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "notebook_timeline",
        category: "design",
        label: command_ids::CMD_NOTEBOOK_TIMELINE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::operands::NOTEBOOK_TIMELINE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "notebook_attachments",
        category: "design",
        label: command_ids::CMD_NOTEBOOK_ATTACHMENTS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    // Writes the CSV `path` names: the export capability.
    CommandMetadata {
        id: "export_notebook_timeline",
        category: "reports",
        label: command_ids::CMD_EXPORT_NOTEBOOK_TIMELINE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::operands::NOTEBOOK_TIMELINE_EXPORT,
        ..CommandMetadata::DEFAULT
    },
    // Writes the notebook file: the save capability (signex-cli `SAVES`).
    CommandMetadata {
        id: "mark_notebook_attachment_valid",
        category: "design",
        label: command_ids::CMD_MARK_NOTEBOOK_ATTACHMENT_VALID,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::operands::NOTEBOOK_MARK_VALID,
        ..CommandMetadata::DEFAULT
    },
    // Writes the copy and the notebook file: the save capability.
    CommandMetadata {
        id: "attach_notebook_file",
        category: "design",
        label: command_ids::CMD_ATTACH_NOTEBOOK_FILE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: super::operands::NOTEBOOK_ATTACH,
        ..CommandMetadata::DEFAULT
    },
];
