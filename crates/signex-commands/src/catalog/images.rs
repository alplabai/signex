//! The image commands (#1116, mockup row 204): one field of a placed image,
//! Relink and Replace, Show in folder, and the project's image assets.
//! `place_graphic` places an image (`placement.rs`).

use super::operands::{optional, required};
use super::{
    CommandFlags, CommandGroup, CommandMetadata, DocumentKind, Enablement, Operand, OperandKind,
};
use signex_i18n_ids::command_ids;

/// `set_image`: one `field` (`width`, `height`, `scale`, `keep_aspect`,
/// `opacity`, `border`, `placement`, `bounds`) of the image `id` set to
/// `value`. One undo step; the Properties ▸ Image rows and the resize
/// handles run the same edit.
pub const SET_IMAGE: &[Operand] = &[
    required("id", OperandKind::Text),
    required("field", OperandKind::Text),
    required("value", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// `relink_image`: the image file `path` replaces the file `file` names in
/// every image of the project (Relink…), or the file of the one image `id`
/// (Replace…).
pub const RELINK_IMAGE: &[Operand] = &[
    required("path", OperandKind::Path),
    optional("file", OperandKind::Text),
    optional("id", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// An image `id` on the sheet in view or the named one.
pub const IMAGE_ON_DOCUMENT: &[Operand] = &[
    required("id", OperandKind::Text),
    optional("document", OperandKind::Document),
];
/// The image assets of the named sheet's project.
pub const IMAGE_ASSETS: &[Operand] = &[required("document", OperandKind::Document)];

pub(super) const IMAGES: &[CommandMetadata] = &[
    CommandMetadata {
        id: "set_image",
        category: "edit",
        label: command_ids::CMD_SET_IMAGE,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: SET_IMAGE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "relink_image",
        category: "edit",
        label: command_ids::CMD_RELINK_IMAGE,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::MUTATES,
        operands: RELINK_IMAGE,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "show_image_in_folder",
        category: "edit",
        label: command_ids::CMD_SHOW_IMAGE_IN_FOLDER,
        menu_label: None,
        group: CommandGroup::Schematic,
        enable: Enablement::RequiresDocument(DocumentKind::Schematic),
        flags: CommandFlags::NONE,
        operands: IMAGE_ON_DOCUMENT,
        ..CommandMetadata::DEFAULT
    },
    CommandMetadata {
        id: "list_image_assets",
        category: "file",
        label: command_ids::CMD_LIST_IMAGE_ASSETS,
        menu_label: None,
        group: CommandGroup::General,
        enable: Enablement::Always,
        flags: CommandFlags::NONE,
        operands: IMAGE_ASSETS,
        ..CommandMetadata::DEFAULT
    },
];
