//! Confirmation modals — Remove-from-Project, Close-Project confirm, and
//! Quit-app confirm.
//!
//! Extracted verbatim from `view/dialogs.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;
use iced::widget::{Space, button, column, container, text};
use iced::{Background, Border, Color, Element, Length, Theme, Widget};

use super::widgets::modal_title_header;
use crate::app::path_ops::PlacingSymbol;
use crate::styles::{
    DialogButtonKind, dialog_body, dialog_button, dialog_footer, dialog_hint, dialog_list,
    dialog_message,
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
            (560.0, 260.0),
        )
    }

    pub(in crate::app::view) fn view_remove_dialog_body(&self) -> Element<'_, Message> {
        let Some(ref st) = self.ui_state.remove_dialog else {
            return container(Space::new()).boxed();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let theme_id = self.ui_state.effective_theme_id();
        let header_content: Element<'_, Message> = modal_title_header(
            if st.unsaved_prompt {
                "Exclude from Project \u{2014} Unsaved Edits"
            } else {
                "Remove from Project"
            },
            Message::Remove(RemoveMsg::Close),
            theme_id,
            tokens,
        );
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::RemoveDialog,
        );

        if st.unsaved_prompt {
            let (body, can_discard) = self.exclude_dirty_prompt(st);
            return Self::exclude_dirty_card(body, can_discard, header, tokens);
        }

        let option_card =
            |title: &'static str, subtitle: &'static str, msg: Message| -> Element<'_, Message> {
                let title_owned = title.to_string();
                let subtitle_owned = subtitle.to_string();
                button(
                    column![
                        text(format!("\u{2192} {}", title_owned))
                            .size(TS.label)
                            .color(text_c),
                        text(subtitle_owned).size(TS.caption).color(text_muted),
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
            "Exclude from project",
            if st.is_outside_project {
                "Library is shared or lives outside this project's folder. It will be excluded from the project and is not deleted from disk."
            } else {
                "File will be excluded from project but left in local folder. Not recommended for projects stored in version control."
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
                "Delete file",
                if st.is_directory_package {
                    "The library package is deleted whole, with everything inside it, and the \
                     library is closed."
                } else if is_library {
                    "The library file is permanently deleted and the library closed. Its symbol, \
                     footprint, simulation and datasheet files are deleted too when the folder \
                     is the library's own; in a folder that also holds another library or a \
                     project or design file, only the library file goes. Other files are left."
                } else {
                    "File will be removed from project and permanently deleted."
                },
                Message::Remove(RemoveMsg::Confirm(crate::app::RemoveChoice::DeleteFile)),
            ));
        }
        if !blockers.unlisted {
            remove_options = remove_options.push(exclude_card);
        }
        let remove_options: Element<'_, Message> = remove_options.boxed();

        let mut prompt = column![dialog_message(
            format!("Please choose how to remove file \"{}\"", st.display_name),
            tokens,
        )]
        .spacing(SP.s);
        if !blockers.other_parents.is_empty() {
            prompt = prompt.push(
                text(format!(
                    "It is also placed by {}, so it cannot be deleted from here.",
                    blockers.parent_names()
                ))
                .size(TS.caption)
                .color(text_muted)
                .boxed(),
            );
        }
        if let Some(why) = blockers.unknown.as_deref().filter(|_| !is_library) {
            prompt = prompt.push(
                text(format!(
                    "It cannot be deleted from here: whether another sheet places it is not \
                     known yet ({why}). You can exclude it from the project."
                ))
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
                text(format!(
                    "Deleting it leaves {} sheet symbol(s) pointing at a missing file: {}. \
                     ERC and netlist export will report them until they are removed.",
                    blockers.symbols.len(),
                    PlacingSymbol::describe_all(&blockers.symbols)
                ))
                .size(TS.caption)
                .color(text_muted)
                .boxed(),
            );
        }
        if blockers.placed && !blockers.unlisted {
            prompt = prompt.push(
                text(format!(
                    "Excluding it does not take it out of the design: a sheet symbol still \
                         places it ({}).",
                    PlacingSymbol::describe_all(&blockers.symbols)
                ))
                .size(TS.caption)
                .color(text_muted)
                .boxed(),
            );
        }
        if blockers.unlisted {
            prompt = prompt.push(
                text("It is not a page of this project, so there is nothing to exclude.")
                    .size(TS.caption)
                    .color(text_muted)
                    .boxed(),
            );
        }
        // Library names can repeat across folders; the full path says which
        // one is affected. Sheets and PCBs get no extra line.
        if is_library {
            prompt = prompt.push(
                text(st.target_path.display().to_string())
                    .size(TS.caption)
                    .color(text_muted)
                    .boxed(),
            );
            if let Some(line) = self.library_usage_line(st) {
                prompt = prompt.push(text(line).size(TS.caption).color(text_muted).boxed());
            }
        }

        let dialog = container(
            column![
                header,
                dialog_body(column![prompt, remove_options].spacing(SP.m)),
                dialog_footer(vec![dialog_button(
                    "Cancel",
                    DialogButtonKind::Secondary,
                    Some(Message::Remove(RemoveMsg::Close)),
                    tokens,
                )]),
            ]
            .width(self.dialog_width(super::super::super::state::ModalId::RemoveDialog, 560.0)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }

    /// Unsaved-edits stage of the Remove modal (#738): Save / Discard / Cancel.
    /// Cancel is `RemoveMsg::Close`, the same message Esc resolves to.
    fn exclude_dirty_card<'a>(
        body: String,
        can_discard: bool,
        header: Element<'a, Message>,
        tokens: &signex_types::theme::ThemeTokens,
    ) -> Element<'a, Message> {
        // Discard is withheld when another open project shares the target.
        let mut buttons = vec![dialog_button(
            "Cancel",
            DialogButtonKind::Secondary,
            Some(Message::Remove(RemoveMsg::Close)),
            tokens,
        )];
        if can_discard {
            buttons.push(dialog_button(
                "Discard",
                DialogButtonKind::Danger,
                Some(Message::Remove(RemoveMsg::ExcludeDirty(
                    crate::app::ExcludeDirtyChoice::Discard,
                ))),
                tokens,
            ));
        }
        buttons.push(dialog_button(
            "Save",
            DialogButtonKind::Primary,
            Some(Message::Remove(RemoveMsg::ExcludeDirty(
                crate::app::ExcludeDirtyChoice::Save,
            ))),
            tokens,
        ));
        container(
            column![
                header,
                dialog_body(dialog_message(body, tokens)),
                dialog_footer(buttons),
            ]
            .width(560),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }

    pub(in crate::app::view) fn view_project_close_confirm(&self) -> Element<'_, Message> {
        let dialog = self.view_project_close_confirm_body();
        self.wrap_centred_dialog(dialog, (520.0, 360.0))
    }

    pub(in crate::app::view) fn view_project_close_confirm_body(&self) -> Element<'_, Message> {
        use crate::app::ProjectCloseChoice;
        let Some(ref st) = self.ui_state.project_close_confirm else {
            return container(Space::new()).boxed();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);

        let theme_id = self.ui_state.effective_theme_id();
        let header_content: Element<'_, Message> = modal_title_header(
            "Close Project — Unsaved Edits",
            Message::Project(ProjectMsg::CloseConfirm(ProjectCloseChoice::Cancel)),
            theme_id,
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
            dialog_message(
                format!(
                    "'{}' has {}. What would you like to do?",
                    st.project_name,
                    unsaved_phrase(st.dirty_paths.len(), st.dirty_rows.len())
                ),
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
                format!(
                    "Kept open, not provably this project's alone: {}",
                    crate::app::state::sheet_refs::bounded_names(&names)
                ),
                tokens,
            ));
        }
        if let Some(reason) = &discard_blocked {
            body = body.push(dialog_hint(reason.clone(), tokens));
        }

        let mut buttons = vec![dialog_button(
            "Cancel",
            DialogButtonKind::Secondary,
            Some(Message::Project(ProjectMsg::CloseConfirm(
                ProjectCloseChoice::Cancel,
            ))),
            tokens,
        )];
        if discard_blocked.is_none() {
            buttons.push(dialog_button(
                "Discard All",
                DialogButtonKind::Danger,
                Some(Message::Project(ProjectMsg::CloseConfirm(
                    ProjectCloseChoice::DiscardAll,
                ))),
                tokens,
            ));
        }
        buttons.push(dialog_button(
            "Save All",
            DialogButtonKind::Primary,
            Some(Message::Project(ProjectMsg::CloseConfirm(
                ProjectCloseChoice::SaveAll,
            ))),
            tokens,
        ));

        let dialog = container(
            column![header, dialog_body(body), dialog_footer(buttons)].width(self.dialog_width(
                super::super::super::state::ModalId::ProjectCloseConfirm,
                520.0,
            )),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }

    pub(in crate::app::view) fn view_app_quit_confirm(&self) -> Element<'_, Message> {
        let dialog = self.view_app_quit_confirm_body();
        self.wrap_centred_dialog(dialog, (520.0, 360.0))
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
            self.document_state.dirty_primitive_paths().collect();
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
                container(text(name).size(TS.body).color(text_c))
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

        let theme_id = self.ui_state.effective_theme_id();
        let header_content: Element<'_, Message> = modal_title_header(
            format!(
                "{} Signex — Unsaved Edits",
                super::super::detached_title::quit_word()
            ),
            Message::Project(ProjectMsg::AppQuitConfirm(ProjectCloseChoice::Cancel)),
            theme_id,
            tokens,
        );
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::AppQuitConfirm,
        );

        // Basenames only: full paths would overflow the modal.
        let file_rows = self.dirty_entry_rows(&st.dirty_paths, &st.dirty_rows, text_c);

        let mut body: iced::widget::Column<Element<'_, Message>> = column![].spacing(SP.m);
        if self.ui_state.quit_confirm_refreshed {
            body = body.push(
                text("The unsaved list changed; review and confirm again.")
                    .size(TS.body)
                    .color(crate::styles::tokens::danger(tokens))
                    .boxed(),
            );
        }
        body = body
            .push(dialog_message(
                format!(
                    "You have {}. Save before {}?",
                    unsaved_phrase(st.dirty_paths.len(), st.dirty_rows.len()),
                    if cfg!(target_os = "macos") {
                        "quitting"
                    } else {
                        "exiting"
                    }
                ),
                tokens,
            ))
            .push(
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
                "Cancel",
                DialogButtonKind::Secondary,
                Some(choose(ProjectCloseChoice::Cancel)),
                tokens,
            ),
            dialog_button(
                "Discard All",
                DialogButtonKind::Danger,
                Some(choose(ProjectCloseChoice::DiscardAll)),
                tokens,
            ),
            dialog_button(
                "Save All",
                DialogButtonKind::Primary,
                Some(choose(ProjectCloseChoice::SaveAll)),
                tokens,
            ),
        ];
        let dialog = container(
            column![header, dialog_body(body), dialog_footer(buttons)].width(
                self.dialog_width(super::super::super::state::ModalId::AppQuitConfirm, 520.0),
            ),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }
}

/// "1 unsaved document", "2 unsaved documents"; with library rows in the mix,
/// "3 unsaved items" (singular "1 unsaved item").
pub(in crate::app::view) fn unsaved_phrase(documents: usize, rows: usize) -> String {
    let (total, noun) = if rows == 0 {
        (documents, "document")
    } else {
        (documents + rows, "item")
    };
    let plural = if total == 1 { "" } else { "s" };
    format!("{total} unsaved {noun}{plural}")
}

#[cfg(test)]
mod copy_tests {
    use super::unsaved_phrase;

    #[test]
    fn the_unsaved_count_is_singular_or_plural_not_a_parenthesised_s() {
        assert_eq!(unsaved_phrase(1, 0), "1 unsaved document");
        assert_eq!(unsaved_phrase(2, 0), "2 unsaved documents");
        assert_eq!(unsaved_phrase(1, 1), "2 unsaved items");
        assert_eq!(unsaved_phrase(0, 1), "1 unsaved item");
    }
}
