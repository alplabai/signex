//! Snippets and templates (#1283): save a selection as a snippet of the
//! project or of the user's library, place one onto a sheet, and list the
//! snippets and the sheet and project templates kept in either place.

use super::operands::{Operand, OperandKind, optional, required};
use super::{CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement};
use signex_i18n_ids::command_ids;

/// The snippet `name`, kept in `scope` (`project`, the default, or
/// `library`), made of the items `ids` names on `document` (the selection on
/// the sheet in view when left out); `overwrite` replaces one of that name.
const SAVE_SNIPPET: &[Operand] = &[
    required("name", OperandKind::Text),
    optional("scope", OperandKind::Text),
    optional("ids", OperandKind::Targets),
    optional("document", OperandKind::Document),
    optional("overwrite", OperandKind::Bool),
];

/// The snippet `name` of `scope`, placed with its centre on `at`.
const PLACE_SNIPPET: &[Operand] = &[
    required("name", OperandKind::Text),
    optional("scope", OperandKind::Text),
    required("at", OperandKind::Point),
    optional("document", OperandKind::Document),
];

/// The templates of one `kind` (`sheet` or `project`); both when left out.
const LIST_TEMPLATES: &[Operand] = &[optional("kind", OperandKind::Text)];

pub(super) const REUSE: &[CommandMetadata] = &[
    // Writes the snippet's file and leaves the sheet as it is.
    CommandMetadata {
        id: "save_snippet",
        category: "design",
        label: command_ids::CMD_SAVE_SNIPPET,
        menu_label: Some(command_ids::CMD_MENU_SAVE_SNIPPET),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::NONE,
        operands: SAVE_SNIPPET,
        ..CommandMetadata::DEFAULT
    },
    // A snippet placed as a design block: one undo step.
    CommandMetadata {
        id: "place_snippet",
        category: "place",
        label: command_ids::CMD_PLACE_SNIPPET,
        menu_label: Some(command_ids::CMD_MENU_PLACE_SNIPPET),
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: PLACE_SNIPPET,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "list_snippets",
        category: "file",
        label: command_ids::CMD_LIST_SNIPPETS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "list_templates",
        category: "file",
        label: command_ids::CMD_LIST_TEMPLATES,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: LIST_TEMPLATES,
        ..CommandMetadata::DEFAULT
    },
];
