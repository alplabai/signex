//! Native title-bar text of a detached modal window.

use crate::app::Signex;
use crate::app::state::ModalId;

impl Signex {
    /// Native title of a detached modal. Where one `ModalId` shows several
    /// dialogs, the title follows the dialog's state, as the in-window
    /// header does.
    pub(crate) fn detached_modal_title(&self, modal: ModalId) -> String {
        let library = &self.library;
        match modal {
            ModalId::NewProject => Some(
                signex_i18n::t!(
                    self.ui_state.i18n.as_ref(),
                    signex_i18n::ids::DIALOG_NEW_PROJECT_TITLE
                )
                .into_string(),
            ),
            ModalId::LibraryPicker => library
                .picker
                .as_ref()
                .map(|p| crate::library::picker::title(&p.mode).to_owned()),
            ModalId::LibraryPrimitivePicker => library
                .primitive_picker
                .as_ref()
                .map(|p| crate::library::primitive_picker::title(p.kind).to_owned()),
            ModalId::LibraryRecovery => library
                .recovery
                .as_ref()
                .map(crate::library::recovery::title),
            _ => None,
        }
        .unwrap_or_else(|| detached_modal_title(modal).to_owned())
    }

    /// The subtitle a dialog's header carries beside its title: the same
    /// line in the in-window header and in a borderless window's title bar.
    pub(crate) fn detached_modal_subtitle(&self, modal: ModalId) -> Option<String> {
        match modal {
            ModalId::AnnotateDialog => Some("Schematic Annotation Configuration".to_owned()),
            ModalId::ErcDialog => Some(
                self.document_state
                    .active_document_project()
                    .map_or_else(|| "Machine rules".to_string(), |p| p.data.name.clone()),
            ),
            ModalId::BomPreview => Some("BOM preview".to_owned()),
            _ => None,
        }
    }
}

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
        ModalId::NewProject => "New Project",
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
