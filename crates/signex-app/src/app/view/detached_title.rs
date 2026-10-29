//! Native title-bar text of a detached modal window.

use crate::app::state::ModalId;

/// Native title-bar text of a detached modal window. Matches the header
/// the same dialog shows when it is painted inside the main window, so the
/// OS window switcher names it the way the user knows it.
/// "Quit" on macOS (the app menu's word), "Exit" elsewhere.
pub(crate) fn quit_word() -> &'static str {
    if cfg!(target_os = "macos") {
        "Quit"
    } else {
        "Exit"
    }
}

pub(crate) fn detached_modal_title(modal: ModalId) -> &'static str {
    match modal {
        ModalId::AnnotateDialog => "Annotate",
        ModalId::ErcDialog => "Electrical Rules Check",
        ModalId::AnnotateResetConfirm => "Reset All Annotations",
        ModalId::ReprefixConfirm => "Re-prefix to Standard",
        ModalId::MoveSelection => "Move Selection",
        ModalId::NetColorPalette => "Net Colors",
        ModalId::ParameterManager => "Parameters",
        ModalId::Preferences => "Preferences",
        ModalId::FindReplace => "Find and Replace",
        ModalId::RenameDialog => "Rename",
        ModalId::RemoveDialog => "Remove",
        ModalId::PrintPreview => "Export PDF",
        ModalId::BomPreview => "Bill of Materials",
        ModalId::EnableVersionControl => "Enable Version Control",
        ModalId::GridProperties => "Grid Properties",
        ModalId::SelectionFilterCustom => "Custom Selection Filter",
        ModalId::About => "About Signex",
        ModalId::KeyboardShortcuts => "Keyboard Shortcuts",
        ModalId::PassiveCalculator => "Passive Network Calculator",
        ModalId::FirstRunTour => "Welcome to Signex",
        ModalId::CreateChildSheet => "Create Sheet",
        ModalId::ErrorNotice => "Signex",
        ModalId::NetlistIncompletePrompt => "Netlist is incomplete",
        ModalId::AutosaveRecovery => "Recover Unsaved Edits",
        ModalId::ProjectCloseConfirm => "Close Project",
        ModalId::AppQuitConfirm => {
            if cfg!(target_os = "macos") {
                "Quit Signex"
            } else {
                "Exit Signex"
            }
        }
        ModalId::LibraryPicker => "Open Library",
        ModalId::LibraryEditRow => "Edit Component Details",
        ModalId::LibraryPrimitivePicker => "Pick Primitive",
        ModalId::LibraryDocumentOptions => "Document Options",
        ModalId::LibraryCreateOptions => "Library Options",
        ModalId::LibraryStockCopy => "Copy to Library",
        ModalId::LibraryUpdates => "Library Updates Available",
        ModalId::LibraryCloseConfirm => "Close Library",
        ModalId::LibraryDeleteConfirm => "Delete Component",
        ModalId::LibraryRecovery => "Library Recovery",
        ModalId::NetColorCustom => "Custom Net Colour",
    }
}

#[cfg(test)]
mod tests {
    use super::{ModalId, detached_modal_title};

    #[test]
    fn detached_modal_title_names_the_dialog() {
        assert_eq!(detached_modal_title(ModalId::Preferences), "Preferences");
        assert_eq!(
            detached_modal_title(ModalId::ErcDialog),
            "Electrical Rules Check"
        );
        assert!(!detached_modal_title(ModalId::BomPreview).is_empty());
    }
}
