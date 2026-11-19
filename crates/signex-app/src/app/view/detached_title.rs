//! Native title-bar text of a detached modal window.

use signex_i18n::{MessageId, ids};

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
                .map(|p| crate::library::picker::title(&p.mode, &self.ui_state.i18n)),
            ModalId::LibraryPrimitivePicker => library
                .primitive_picker
                .as_ref()
                .map(|p| crate::library::primitive_picker::title(p.kind, &self.ui_state.i18n)),
            ModalId::LibraryRecovery => library
                .recovery
                .as_ref()
                .map(crate::library::recovery::title),
            _ => None,
        }
        .unwrap_or_else(|| {
            self.ui_state
                .i18n
                .message_plain(detached_modal_title(modal), &[])
        })
    }

    /// The subtitle a dialog's header carries beside its title: the same
    /// line in the in-window header and in a borderless window's title bar.
    pub(crate) fn detached_modal_subtitle(&self, modal: ModalId) -> Option<String> {
        let i18n = &self.ui_state.i18n;
        match modal {
            ModalId::AnnotateDialog => Some(i18n.message_plain(ids::DIALOG_SUBTITLE_ANNOTATE, &[])),
            ModalId::ErcDialog => Some(self.document_state.active_document_project().map_or_else(
                || i18n.message_plain(ids::DIALOG_SUBTITLE_ERC_MACHINE, &[]),
                |p| p.data.name.clone(),
            )),
            ModalId::BomPreview => Some(i18n.message_plain(ids::DIALOG_SUBTITLE_BOM, &[])),
            _ => None,
        }
    }
}

/// Native title-bar text of a detached modal window. Matches the header
/// the same dialog shows when it is painted inside the main window, so the
/// OS window switcher names it the way the user knows it.
pub(crate) fn detached_modal_title(modal: ModalId) -> MessageId {
    match modal {
        ModalId::AnnotateDialog => ids::DIALOG_TITLE_ANNOTATE,
        ModalId::ErcDialog => ids::DIALOG_TITLE_ERC,
        ModalId::AnnotateResetConfirm => ids::DIALOG_TITLE_ANNOTATE_RESET,
        ModalId::ReprefixConfirm => ids::DIALOG_TITLE_REPREFIX,
        ModalId::MoveSelection => ids::DIALOG_TITLE_MOVE_SELECTION,
        ModalId::NetColorPalette => ids::DIALOG_TITLE_NET_COLORS,
        ModalId::ParameterManager => ids::DIALOG_TITLE_PARAMETERS,
        ModalId::Preferences => ids::DIALOG_TITLE_PREFERENCES,
        ModalId::FindReplace => ids::DIALOG_TITLE_FIND_REPLACE,
        ModalId::RenameDialog => ids::DIALOG_TITLE_RENAME,
        ModalId::RemoveDialog => ids::DIALOG_TITLE_REMOVE,
        ModalId::PrintPreview => ids::DIALOG_TITLE_EXPORT_PDF,
        ModalId::BomPreview => ids::DIALOG_TITLE_BOM,
        ModalId::EnableVersionControl => ids::DIALOG_TITLE_ENABLE_VERSION_CONTROL,
        ModalId::GridProperties => ids::DIALOG_TITLE_GRID_PROPERTIES,
        ModalId::SelectionFilterCustom => ids::DIALOG_TITLE_SELECTION_FILTER,
        ModalId::NewProject => ids::DIALOG_TITLE_NEW_PROJECT,
        ModalId::About => ids::DIALOG_TITLE_ABOUT,
        ModalId::KeyboardShortcuts => ids::DIALOG_TITLE_KEYBOARD_SHORTCUTS,
        ModalId::PassiveCalculator => ids::DIALOG_TITLE_PASSIVE_CALCULATOR,
        ModalId::FirstRunTour => ids::DIALOG_TITLE_FIRST_RUN_TOUR,
        ModalId::CreateChildSheet => ids::DIALOG_CREATE_CHILD_SHEET_TITLE,
        ModalId::ErrorNotice => ids::DIALOG_TITLE_ERROR_NOTICE,
        ModalId::NetlistIncompletePrompt => ids::DIALOG_TITLE_NETLIST_INCOMPLETE,
        ModalId::AutosaveRecovery => ids::DIALOG_TITLE_AUTOSAVE_RECOVERY,
        ModalId::ProjectCloseConfirm => ids::DIALOG_TITLE_CLOSE_PROJECT,
        ModalId::AppQuitConfirm => {
            if cfg!(target_os = "macos") {
                ids::DIALOG_TITLE_QUIT
            } else {
                ids::DIALOG_TITLE_EXIT
            }
        }
        ModalId::LibraryPicker => ids::DIALOG_TITLE_OPEN_LIBRARY,
        ModalId::LibraryEditRow => ids::DIALOG_TITLE_EDIT_COMPONENT,
        ModalId::LibraryPrimitivePicker => ids::DIALOG_TITLE_PICK_PRIMITIVE,
        ModalId::LibraryDocumentOptions => ids::DIALOG_TITLE_DOCUMENT_OPTIONS,
        ModalId::LibraryCreateOptions => ids::DIALOG_TITLE_LIBRARY_OPTIONS,
        ModalId::LibraryStockCopy => ids::DIALOG_TITLE_COPY_TO_LIBRARY,
        ModalId::LibraryUpdates => ids::DIALOG_TITLE_LIBRARY_UPDATES,
        ModalId::LibraryCloseConfirm => ids::DIALOG_TITLE_CLOSE_LIBRARY,
        ModalId::LibraryDeleteConfirm => ids::DIALOG_TITLE_DELETE_COMPONENT,
        ModalId::LibraryRecovery => ids::DIALOG_TITLE_LIBRARY_RECOVERY,
        ModalId::NetColorCustom => ids::DIALOG_TITLE_NET_COLOR_CUSTOM,
    }
}

#[cfg(test)]
mod tests {
    use signex_i18n::{I18n, LanguagePref};

    use super::{ModalId, detached_modal_title};
    use crate::app::{Message, Signex, UiMsg};

    fn en(modal: ModalId) -> String {
        I18n::en_us().message_plain(detached_modal_title(modal), &[])
    }

    #[test]
    fn detached_modal_title_names_the_dialog() {
        assert_eq!(en(ModalId::Preferences), "Preferences");
        assert_eq!(en(ModalId::ErcDialog), "Electrical Rules Check");
        assert!(!en(ModalId::BomPreview).is_empty());
    }

    /// #2058: a dialog window's title follows the language in force.
    #[test]
    fn a_dialog_window_title_follows_the_language() {
        let (mut app, _task) = Signex::new();
        let switch = |app: &mut Signex, tag: &str| {
            let pref = LanguagePref::Locale(tag.to_owned());
            let _ = app.update(Message::Ui(UiMsg::LanguageChanged(pref)));
        };
        switch(&mut app, "en-US");
        let english = app.detached_modal_title(ModalId::CreateChildSheet);
        assert_eq!(english, "Create Sheet");
        switch(&mut app, "en-XA");
        assert_ne!(app.detached_modal_title(ModalId::CreateChildSheet), english);
        switch(&mut app, "en-US");
        assert_eq!(app.detached_modal_title(ModalId::CreateChildSheet), english);
    }
}
