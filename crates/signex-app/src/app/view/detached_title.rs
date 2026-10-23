//! Native title-bar text of a detached modal window.

use crate::app::state::ModalId;

/// Native title-bar text of a detached modal window. Matches the header
/// the same dialog shows when it is painted inside the main window, so the
/// OS window switcher names it the way the user knows it.
pub(crate) fn detached_modal_title(modal: ModalId) -> &'static str {
    match modal {
        ModalId::AnnotateDialog => "Annotate",
        ModalId::ErcDialog => "Electrical Rules Check",
        ModalId::AnnotateResetConfirm => "Reset All Annotations",
        ModalId::MoveSelection => "Move Selection",
        ModalId::NetColorPalette => "Net Colors",
        ModalId::ParameterManager => "Parameters",
        ModalId::Preferences => "Preferences",
        ModalId::FindReplace => "Find and Replace",
        ModalId::RenameDialog => "Rename",
        ModalId::RemoveDialog => "Remove",
        ModalId::PrintPreview => "Export PDF",
        ModalId::BomPreview => "Bill of Materials",
        ModalId::ProjectOptions => "Project Options",
        ModalId::EnableVersionControl => "Enable Version Control",
        ModalId::GridProperties => "Grid Properties",
        ModalId::SelectionFilterCustom => "Custom Selection Filter",
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
