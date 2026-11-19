//! The Quit-app confirm, and the unsaved-count sentence it shares with the
//! Close Project confirm. Remove from Project is in `remove.rs`, the Close
//! Project confirm in `close_project.rs`.
//!
//! Extracted verbatim from `view/dialogs.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;
use iced::widget::{Space, column, container};
use iced::{Element, Length, Widget};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_widgets::text;

use super::widgets::modal_title_header;
use crate::styles::{
    DialogButtonKind, DialogIcon, dialog_body, dialog_button, dialog_footer, dialog_intro,
    dialog_list,
};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

impl Signex {
    pub(in crate::app::view) fn view_app_quit_confirm(&self) -> Element<'_, Message> {
        let dialog = self.view_app_quit_confirm_body();
        self.wrap_centred_dialog(dialog, super::sizes::APP_QUIT_CONFIRM)
    }

    /// One list row per unsaved entry in a confirm modal: file
    /// basenames first, then Component Preview rows by part number.
    fn dirty_entry_rows(
        &self,
        paths: &[std::path::PathBuf],
        rows: &[crate::library::state::EditorAddress],
        text_c: iced::Color,
    ) -> Vec<Element<'_, Message>> {
        let roots = self.draft_library_roots();
        let drafts: std::collections::HashSet<_> =
            self.document_state.unsaved_drafts().into_iter().collect();
        let path_labels = paths.iter().map(|path| {
            let dir = drafts.contains(path).then(|| {
                let fold = self.draft_fold(path);
                // Libraries sharing a folder share its relative label.
                crate::app::state::scope::owning_libraries(path, &roots, fold)
                    .first()
                    .and_then(|lib| lib.parent())
            });
            crate::library::close_prompt::dirty_path_label(path, dir.flatten())
        });
        let row_labels = rows
            .iter()
            .map(|address| self.library.editor_label(address));
        path_labels
            .chain(row_labels)
            .map(|name| {
                container(
                    text(signex_i18n::UserText::new(name))
                        .size(TS.body)
                        .color(text_c),
                )
                .padding([SP.s, SP.m])
                .boxed()
            })
            .collect()
    }

    pub(in crate::app::view) fn view_app_quit_confirm_body(&self) -> Element<'_, Message> {
        use crate::app::ProjectCloseChoice;
        let Some(ref st) = self.ui_state.app_quit_confirm else {
            return container(Space::new()).boxed();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let i18n = self.ui_state.i18n.as_ref();

        // macOS says "Quit", other systems "Exit".
        let macos = cfg!(target_os = "macos");
        let pick = |quit: MessageId, exit: MessageId| if macos { quit } else { exit };
        let header_content: Element<'_, Message> = modal_title_header(
            t!(
                i18n,
                pick(
                    ids::DIALOG_QUIT_UNSAVED_TITLE,
                    ids::DIALOG_EXIT_UNSAVED_TITLE
                )
            )
            .into_string(),
            Message::Project(ProjectMsg::AppQuitConfirm(ProjectCloseChoice::Cancel)),
            tokens,
        );
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::AppQuitConfirm,
        );

        // Basenames only: full paths would overflow the modal.
        let file_rows = self.dirty_entry_rows(&st.dirty_paths, &st.dirty_rows, text_c);

        let mut body: iced::widget::Column<Element<'_, Message>> = column![dialog_intro(
            DialogIcon::Warn,
            t!(
                i18n,
                pick(ids::DIALOG_QUIT_QUESTION, ids::DIALOG_EXIT_QUESTION)
            )
            .into_string(),
            Some(unsaved_line(
                i18n,
                pick(ids::DIALOG_QUIT_DETAIL, ids::DIALOG_EXIT_DETAIL),
                st.dirty_paths.len(),
                st.dirty_rows.len(),
                None,
            )),
            tokens,
        )]
        .spacing(SP.m);
        if self.ui_state.quit_confirm_refreshed {
            body = body.push(
                text(t!(i18n, ids::DIALOG_QUIT_LIST_CHANGED))
                    .size(TS.body)
                    .color(crate::styles::tokens::danger(tokens))
                    .boxed(),
            );
        }
        body = body.push(
            container(dialog_list(file_rows, tokens))
                .height(Length::Fit.max(
                    crate::app::handlers::modal_sizing::LIST_MAX_HEIGHT
                        + crate::app::handlers::modal_sizing::LIST_BOX_CHROME,
                ))
                .boxed(),
        );

        let choose = |choice| Message::Project(ProjectMsg::AppQuitConfirm(choice));
        let buttons = vec![
            dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                DialogButtonKind::Cancel,
                Some(choose(ProjectCloseChoice::Cancel)),
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::DIALOG_UNSAVED_DISCARD_ALL),
                DialogButtonKind::Danger,
                Some(choose(ProjectCloseChoice::DiscardAll)),
                tokens,
            ),
            dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_SAVE_ALL),
                DialogButtonKind::Primary,
                Some(choose(ProjectCloseChoice::SaveAll)),
                tokens,
            ),
        ];
        let dialog = container(
            column![header, dialog_body(body), dialog_footer(buttons)].width(self.dialog_width(
                super::super::super::state::ModalId::AppQuitConfirm,
                super::sizes::APP_QUIT_CONFIRM,
            )),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }
}

/// The sentence `id` naming the unsaved count: "1 unsaved document",
/// "2 unsaved documents"; with library rows in the mix, "3 unsaved items"
/// (singular "1 unsaved item"). `project` names the project when it says so.
pub(super) fn unsaved_line(
    i18n: &I18n,
    id: MessageId,
    documents: usize,
    rows: usize,
    project: Option<&str>,
) -> String {
    let (count, kind) = if rows == 0 {
        (documents, "documents")
    } else {
        (documents + rows, "items")
    };
    let project = project.unwrap_or_default();
    t!(i18n, id, "count" => count, "kind" => kind, "project" => project).into_string()
}

#[cfg(test)]
mod copy_tests {
    use signex_i18n::{I18n, LanguagePref, ids};

    use super::unsaved_line;

    fn plain(text: String) -> String {
        text.replace(['\u{2068}', '\u{2069}'], "")
    }

    #[test]
    fn the_unsaved_count_is_singular_or_plural_not_a_parenthesised_s() {
        let en = I18n::en_us();
        let line = |documents, rows| {
            plain(unsaved_line(
                &en,
                ids::DIALOG_QUIT_DETAIL,
                documents,
                rows,
                None,
            ))
        };
        assert_eq!(
            line(1, 0),
            "You have 1 unsaved document. Save before quitting?"
        );
        assert_eq!(
            line(2, 0),
            "You have 2 unsaved documents. Save before quitting?"
        );
        assert_eq!(
            line(1, 1),
            "You have 2 unsaved items. Save before quitting?"
        );
        assert_eq!(line(0, 1), "You have 1 unsaved item. Save before quitting?");
        assert_eq!(
            plain(unsaved_line(
                &en,
                ids::DIALOG_CLOSE_PROJECT_DETAIL,
                2,
                0,
                Some("Demo")
            )),
            "'Demo' has 2 unsaved documents."
        );
    }

    /// The multi-line dialog texts keep the English the views built before
    /// the catalog, line break included.
    #[test]
    fn multi_line_dialog_texts_keep_their_english() {
        let en = I18n::en_us();
        let undo = || signex_i18n::Arg::from("Ctrl+Z");
        assert_eq!(
            en.message_plain(ids::DIALOG_ANNOTATE_RESET_DETAIL, &[("undo", undo())]),
            "Every reference designator will be reset to '?', then the sheet will be renumbered from 1.
This cannot be undone through Ctrl+Z alone — consider saving first."
        );
        assert_eq!(
            en.message_plain(ids::DIALOG_RECOVERY_HINT, &[("undo", undo())]),
            "Restore loads the copy as unsaved edits; nothing is written to the file until \
             you save. Restoring replaces the open document and cannot be undone with \
             Ctrl+Z. Discard deletes the copy. Decide later keeps it for the next time \
             this file is opened, and pauses autosave for this document for the rest of the \
             session."
        );
    }

    /// #2058: the confirm sentences follow the language in force.
    #[test]
    fn the_unsaved_sentence_follows_the_language() {
        let xa = I18n::new(&LanguagePref::Locale("en-XA".to_owned()));
        let en = I18n::en_us();
        let english = unsaved_line(&en, ids::DIALOG_EXIT_DETAIL, 2, 0, None);
        assert_ne!(
            unsaved_line(&xa, ids::DIALOG_EXIT_DETAIL, 2, 0, None),
            english
        );
        assert_eq!(
            plain(english),
            "You have 2 unsaved documents. Save before exiting?"
        );
    }
}
