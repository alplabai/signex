//! Query and edit-batch commands: read a document, its pins and its
//! objects without a window, and apply a batch of edits.

use super::{CommandFlags, CommandGroup, CommandMetadata, Enablement, operands};
use signex_i18n_ids::command_ids;

pub(super) const QUERY: &[CommandMetadata] = &[
    CommandMetadata {
        id: "document_info",
        category: "file",
        label: command_ids::CMD_DOCUMENT_INFO,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "list_documents",
        category: "file",
        label: command_ids::CMD_LIST_DOCUMENTS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::DOCUMENT_FILTERS,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "read_document",
        category: "file",
        label: command_ids::CMD_READ_DOCUMENT,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::READ_DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "query_pins",
        category: "file",
        label: command_ids::CMD_QUERY_PINS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::QUERY_PINS,
        ..CommandMetadata::DEFAULT
    },
    // #2944: the rows of an open library as the app holds them; reads no
    // file and reaches no library server, so it does not need the network.
    CommandMetadata {
        id: "query_library",
        category: "library",
        label: command_ids::CMD_QUERY_LIBRARY,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::QUERY_LIBRARY,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "resolve",
        category: "file",
        label: command_ids::CMD_RESOLVE,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: operands::RESOLVE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "apply_edits",
        category: "edit",
        label: command_ids::CMD_APPLY_EDITS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::MUTATES,
        operands: operands::APPLY_EDITS,
        ..CommandMetadata::DEFAULT
    },
];
