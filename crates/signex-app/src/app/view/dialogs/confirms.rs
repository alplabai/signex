//! Confirmation modals — Remove-from-Project, Close-Project confirm, and
//! Quit-app confirm.
//!
//! Extracted verbatim from `view/dialogs.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;
use iced::widget::{Space, column, container};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_widgets::{button, raw, text};

use super::widgets::modal_title_header;
use crate::app::path_ops::PlacingSymbol;
use crate::styles::{
    DialogButtonKind, DialogIcon, dialog_body, dialog_button, dialog_footer, dialog_hint,
    dialog_intro, dialog_list,
};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

impl Signex {
    pub(in crate::app::view) fn view_remove_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_remove_dialog_body();
        self.wrap_dialog(
            crate::app::state::ModalId::RemoveDialog,
            dialog,
            super::sizes::REMOVE,
        )
    }

    pub(in crate::app::view) fn view_remove_dialog_body(&self) -> Element<'_, Message> {
        let Some(ref st) = self.ui_state.remove_dialog else {
            return container(Space::new()).boxed();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let i18n = self.ui_state.i18n.as_ref();
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let header_content: Element<'_, Message> = modal_title_header(
            t!(
                i18n,
                if st.unsaved_prompt {
                    ids::DIALOG_REMOVE_UNSAVED_TITLE
                } else {
                    ids::DIALOG_REMOVE_TITLE
                }
            )
            .into_string(),
            Message::Remove(RemoveMsg::Close),
            tokens,
        );
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::RemoveDialog,
        );

        if st.unsaved_prompt {
            let (body, can_discard) = self.exclude_dirty_prompt(st);
            let width = self.dialog_width(
                crate::app::state::ModalId::RemoveDialog,
                super::sizes::REMOVE,
            );
            return Self::exclude_dirty_card(i18n, body, can_discard, header, width, tokens);
        }

        let option_card =
            |title: MessageId, subtitle: MessageId, msg: Message| -> Element<'_, Message> {
                button(
                    column![
                        text(t!(i18n, title)).size(TS.label).color(text_c),
                        text(t!(i18n, subtitle)).size(TS.caption).color(text_muted),
                    ]
                    .spacing(SP.s)
                    .padding([SP.xs, 0.0]),
                )
                .on_press(msg)
                .padding([10, 14])
                .width(Length::Fill)
                .style(move |_: &Theme, status: button::Status| {
                    let bg = match status {
                        button::Status::Hovered | button::Status::Pressed => {
                            Color::from_rgba(1.0, 1.0, 1.0, 0.06)
                        }
                        _ => Color::from_rgba(1.0, 1.0, 1.0, 0.02),
                    };
                    button::Style {
                        background: Some(Background::Color(bg)),
                        border: Border {
                            width: 1.0,
                            radius: 4.0.into(),
                            color: border_c,
                            ..Border::default()
                        },
                        text_color: text_c,
                        ..button::Style::default()
                    }
                })
                .boxed()
            };

        let exclude_card = option_card(
            ids::DIALOG_REMOVE_EXCLUDE,
            if st.is_outside_project {
                ids::DIALOG_REMOVE_EXCLUDE_OUTSIDE
            } else {
                ids::DIALOG_REMOVE_EXCLUDE_INSIDE
            },
            Message::Remove(RemoveMsg::Confirm(
                crate::app::RemoveChoice::ExcludeFromProject,
            )),
        );
        // Shared / Global libraries are owned by other projects too:
        // never offer to delete them from disk (#139). A sheet other sheets
        // place is not deleted either, and a sheet that is not a project page
        // has nothing to exclude (#1618).
        let blockers = self.remove_blockers(st);
        // A `.snxlib` is a directory package: it is never a placed sheet.
        let is_library = crate::app::file_extension::has_extension(&st.target_path, "snxlib");
        let mut remove_options = column![].spacing(SP.m);
        if !st.is_outside_project && !blockers.blocks_delete(is_library) {
            remove_options = remove_options.push(option_card(
                ids::DIALOG_REMOVE_DELETE,
                if st.is_directory_package {
                    ids::DIALOG_REMOVE_DELETE_PACKAGE
                } else if is_library {
                    ids::DIALOG_REMOVE_DELETE_LIBRARY
                } else {
                    ids::DIALOG_REMOVE_DELETE_FILE
                },
                Message::Remove(RemoveMsg::Confirm(crate::app::RemoveChoice::DeleteFile)),
            ));
        }
        if !blockers.unlisted {
            remove_options = remove_options.push(exclude_card);
        }
        let remove_options: Element<'_, Message> = remove_options.boxed();

        let mut prompt = column![dialog_intro(
            DialogIcon::Warn,
            t!(i18n, ids::DIALOG_REMOVE_QUESTION, "name" => st.display_name.as_str()).into_string(),
            Some(
                t!(i18n, ids::DIALOG_REMOVE_QUESTION_DETAIL, "name" => st.display_name.as_str())
                    .into_string(),
            ),
            tokens,
        )]
        .spacing(SP.s);
        if !blockers.other_parents.is_empty() {
            prompt = prompt.push(
                text(t!(i18n, ids::DIALOG_REMOVE_PLACED_BY, "parents" => blockers.parent_names()))
                    .size(TS.caption)
                    .color(text_muted)
                    .boxed(),
            );
        }
        if let Some(why) = blockers.unknown.as_deref().filter(|_| !is_library) {
            prompt = prompt.push(
                text(t!(i18n, ids::DIALOG_REMOVE_PLACEMENT_UNKNOWN, "reason" => why))
                    .size(TS.caption)
                    .color(text_muted)
                    .boxed(),
            );
        }
        // Delete is only offered when no other sheet places the file
        // (`blocks_delete`), so every listed symbol sits on the row's own parent.
        if !blockers.blocks_delete(is_library)
            && !st.is_outside_project
            && blockers.own_parent.is_some()
        {
            prompt = prompt.push(
                text(t!(
                    i18n,
                    ids::DIALOG_REMOVE_DANGLING,
                    "count" => blockers.symbols.len(),
                    "symbols" => PlacingSymbol::describe_all(&blockers.symbols)
                ))
                .size(TS.caption)
                .color(text_muted)
                .boxed(),
            );
        }
        if blockers.placed && !blockers.unlisted {
            prompt = prompt.push(
                text(t!(
                    i18n,
                    ids::DIALOG_REMOVE_STILL_PLACED,
                    "symbols" => PlacingSymbol::describe_all(&blockers.symbols)
                ))
                .size(TS.caption)
                .color(text_muted)
                .boxed(),
            );
        }
        if blockers.unlisted {
            prompt = prompt.push(
                text(t!(i18n, ids::DIALOG_REMOVE_NOT_A_PAGE))
                    .size(TS.caption)
                    .color(text_muted)
                    .boxed(),
            );
        }
        // Library names can repeat across folders; the full path says which
        // one is affected. Sheets and PCBs get no extra line.
        if is_library {
            prompt = prompt.push(
                text(signex_i18n::UserText::new(
                    st.target_path.display().to_string(),
                ))
                .size(TS.caption)
                .color(text_muted)
                .boxed(),
            );
            if let Some(line) = self.library_usage_line(st) {
                // raw: the usage line is built in English by the Remove
                // handler, which its own tests read (#2061).
                prompt = prompt.push(text(raw(line)).size(TS.caption).color(text_muted).boxed());
            }
        }

        let dialog = container(
            column![
                header,
                dialog_body(column![prompt, remove_options].spacing(SP.m)),
                dialog_footer(vec![dialog_button(
                    t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                    DialogButtonKind::Cancel,
                    Some(Message::Remove(RemoveMsg::Close)),
                    tokens,
                )]),
            ]
            .width(self.dialog_width(
                super::super::super::state::ModalId::RemoveDialog,
                super::sizes::REMOVE,
            )),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }

    /// Unsaved-edits stage of the Remove modal (#738): Save / Discard / Cancel.
    /// Cancel is `RemoveMsg::Close`, the same message Esc resolves to.
    fn exclude_dirty_card<'a>(
        i18n: &I18n,
        body: String,
        can_discard: bool,
        header: Element<'a, Message>,
        width: Length,
        tokens: &signex_types::theme::ThemeTokens,
    ) -> Element<'a, Message> {
        // Discard is withheld when another open project shares the target.
        let mut buttons = vec![dialog_button(
            t!(i18n, ids::DIALOG_BUTTON_CANCEL),
            DialogButtonKind::Cancel,
            Some(Message::Remove(RemoveMsg::Close)),
            tokens,
        )];
        if can_discard {
            buttons.push(dialog_button(
                t!(i18n, ids::DIALOG_BUTTON_DISCARD),
                DialogButtonKind::Danger,
                Some(Message::Remove(RemoveMsg::ExcludeDirty(
                    crate::app::ExcludeDirtyChoice::Discard,
                ))),
                tokens,
            ));
        }
        buttons.push(dialog_button(
            t!(i18n, ids::DIALOG_BUTTON_SAVE),
            DialogButtonKind::Primary,
            Some(Message::Remove(RemoveMsg::ExcludeDirty(
                crate::app::ExcludeDirtyChoice::Save,
            ))),
            tokens,
        ));
        container(
            column![
                header,
                dialog_body(dialog_intro(
                    DialogIcon::Warn,
                    t!(i18n, ids::DIALOG_EXCLUDE_DIRTY_QUESTION).into_string(),
                    Some(body),
                    tokens,
                )),
                dialog_footer(buttons),
            ]
            .width(width),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }

    pub(in crate::app::view) fn view_project_close_confirm(&self) -> Element<'_, Message> {
        let dialog = self.view_project_close_confirm_body();
        self.wrap_centred_dialog(dialog, super::sizes::PROJECT_CLOSE_CONFIRM)
    }

    pub(in crate::app::view) fn view_project_close_confirm_body(&self) -> Element<'_, Message> {
        use crate::app::ProjectCloseChoice;
        let Some(ref st) = self.ui_state.project_close_confirm else {
            return container(Space::new()).boxed();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let i18n = self.ui_state.i18n.as_ref();

        let header_content: Element<'_, Message> = modal_title_header(
            t!(i18n, ids::DIALOG_CLOSE_PROJECT_UNSAVED_TITLE).into_string(),
            Message::Project(ProjectMsg::CloseConfirm(ProjectCloseChoice::Cancel)),
            tokens,
        );
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::ProjectCloseConfirm,
        );

        // Filenames only: paths would overflow narrow modals and the project
        // context already implies the parent dir.
        let file_rows = self.dirty_entry_rows(&st.dirty_paths, &st.dirty_rows, text_c);

        // An unreadable sheet hides its children: no Discard All, and why.
        let discard_blocked = st.sheet_refs.discard_blocked_reason();

        let mut body = column![
            dialog_intro(
                DialogIcon::Warn,
                t!(i18n, ids::DIALOG_CLOSE_PROJECT_QUESTION).into_string(),
                Some(unsaved_line(
                    i18n,
                    ids::DIALOG_CLOSE_PROJECT_DETAIL,
                    st.dirty_paths.len(),
                    st.dirty_rows.len(),
                    Some(&st.project_name),
                )),
                tokens,
            ),
            container(dialog_list(file_rows, tokens)).height(Length::Fit.max(
                crate::app::handlers::modal_sizing::LIST_MAX_HEIGHT
                    + crate::app::handlers::modal_sizing::LIST_BOX_CHROME,
            )),
        ]
        .spacing(SP.m);
        // Kept documents stay open and are never discarded; say so, by name.
        if !st.kept_paths.is_empty() {
            let names: Vec<String> = st
                .kept_paths
                .iter()
                .map(|p| crate::app::state::sheet_refs::file_label(p))
                .collect();
            body = body.push(dialog_hint(
                t!(
                    i18n,
                    ids::DIALOG_CLOSE_PROJECT_KEPT,
                    "names" => crate::app::state::sheet_refs::bounded_names(&names)
                )
                .into_string(),
                tokens,
            ));
        }
        if let Some(reason) = &discard_blocked {
            body = body.push(dialog_hint(reason.clone(), tokens));
        }

        let mut buttons = vec![dialog_button(
            t!(i18n, ids::DIALOG_BUTTON_CANCEL),
            DialogButtonKind::Cancel,
            Some(Message::Project(ProjectMsg::CloseConfirm(
                ProjectCloseChoice::Cancel,
            ))),
            tokens,
        )];
        if discard_blocked.is_none() {
            buttons.push(dialog_button(
                t!(i18n, ids::DIALOG_UNSAVED_DISCARD_ALL),
                DialogButtonKind::Danger,
                Some(Message::Project(ProjectMsg::CloseConfirm(
                    ProjectCloseChoice::DiscardAll,
                ))),
                tokens,
            ));
        }
        buttons.push(dialog_button(
            t!(i18n, ids::DIALOG_BUTTON_SAVE_ALL),
            DialogButtonKind::Primary,
            Some(Message::Project(ProjectMsg::CloseConfirm(
                ProjectCloseChoice::SaveAll,
            ))),
            tokens,
        ));

        let dialog = container(
            column![header, dialog_body(body), dialog_footer(buttons)].width(self.dialog_width(
                super::super::super::state::ModalId::ProjectCloseConfirm,
                super::sizes::PROJECT_CLOSE_CONFIRM,
            )),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }

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
fn unsaved_line(
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
            "'Demo' has 2 unsaved documents. What would you like to do?"
        );
    }

    /// The multi-line dialog texts keep the English the views built before
    /// the catalog, line break included.
    #[test]
    fn multi_line_dialog_texts_keep_their_english() {
        let en = I18n::en_us();
        let undo = || signex_i18n::Arg::from("Ctrl+Z");
        // #76 / AV-05: the reset detail names its scope; undo is stated once,
        // in the confirm's banner, the same way as the Annotate footer.
        let scope = |renumber: &'static str| {
            en.message_plain(
                ids::DIALOG_ANNOTATE_RESET_DETAIL,
                &[
                    ("sheets", signex_i18n::Arg::from(5_usize)),
                    ("symbols", signex_i18n::Arg::from(42_usize)),
                    ("renumber", signex_i18n::Arg::from(renumber)),
                ],
            )
        };
        assert_eq!(
            scope("yes"),
            "Every reference designator on the 5 ticked sheets (42 symbols) is reset to '?', \
             then renumbered from 1."
        );
        assert_eq!(
            scope("no"),
            "Every reference designator on the 5 ticked sheets (42 symbols) is reset to '?'."
        );
        assert_eq!(
            en.message_plain(ids::DIALOG_ANNOTATE_RESET_ONE_STEP, &[("undo", undo())]),
            "One undo step: Ctrl+Z restores every sheet."
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
