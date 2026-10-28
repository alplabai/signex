//! Confirmation modals — Remove-from-Project, Close-Project confirm, and
//! Quit-app confirm.
//!
//! Extracted verbatim from `view/dialogs.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Theme};

use super::widgets::{close_x_button, primary_button, secondary_button, wrap_modal};
use super::{MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

impl Signex {
    pub(in crate::app::view) fn view_remove_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_remove_dialog_body();
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::super::state::ModalId::RemoveDialog)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(dialog, offset, self.ui_state.window_size, (560.0, 260.0))
    }

    pub(in crate::app::view) fn view_remove_dialog_body(&self) -> Element<'_, Message> {
        let Some(ref st) = self.ui_state.remove_dialog else {
            return container(Space::new()).into();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let theme_id = self.ui_state.theme_id;
        let header_content: Element<'_, Message> = container(
            row![
                text(if st.unsaved_prompt {
                    "Exclude from Project \u{2014} Unsaved Edits"
                } else {
                    "Remove from Project"
                })
                .size(MODAL_HEADER_TITLE_SIZE)
                .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(Message::Remove(RemoveMsg::Close), theme_id, text_muted),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
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
                        },
                        text_color: text_c,
                        ..button::Style::default()
                    }
                })
                .into()
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
        let is_dir = st.target_path.extension().and_then(|e| e.to_str()) == Some("snxlib");
        let mut remove_options = column![].spacing(SP.m);
        if !st.is_outside_project && !blockers.blocks_delete(is_dir) {
            remove_options = remove_options.push(option_card(
                "Delete file",
                "File will be removed from project and permanently deleted.",
                Message::Remove(RemoveMsg::Confirm(crate::app::RemoveChoice::DeleteFile)),
            ));
        }
        if !blockers.unlisted {
            remove_options = remove_options.push(exclude_card);
        }
        let remove_options: Element<'_, Message> = remove_options.into();

        let mut prompt = column![
            text(format!(
                "Please choose how to remove file \"{}\"",
                st.display_name
            ))
            .size(TS.body)
            .color(text_muted),
        ]
        .spacing(SP.s);
        if !blockers.other_parents.is_empty() {
            prompt = prompt.push(
                text(format!(
                    "It is also placed by {}, so it cannot be deleted from here.",
                    blockers.parent_names()
                ))
                .size(TS.caption)
                .color(text_muted),
            );
        }
        if let Some(why) = blockers.unknown.as_deref().filter(|_| !is_dir) {
            prompt = prompt.push(
                text(format!(
                    "It cannot be deleted from here: whether another sheet places it is not \
                     known yet ({why}). You can exclude it from the project."
                ))
                .size(TS.caption)
                .color(text_muted),
            );
        }
        if !blockers.blocks_delete(is_dir)
            && !st.is_outside_project
            && let Some(parent) = &blockers.own_parent
        {
            prompt = prompt.push(
                text(format!("{} will show it as missing.", parent.display()))
                    .size(TS.caption)
                    .color(text_muted),
            );
        }
        if blockers.placed && !blockers.unlisted {
            prompt = prompt.push(
                text(
                    "Excluding it does not take it out of the design: a sheet symbol still \
                     places it.",
                )
                .size(TS.caption)
                .color(text_muted),
            );
        }
        if blockers.unlisted {
            prompt = prompt.push(
                text("It is not a page of this project, so there is nothing to exclude.")
                    .size(TS.caption)
                    .color(text_muted),
            );
        }
        // Library names can repeat across folders; the full path says which
        // one is affected. Sheets and PCBs get no extra line.
        if st.display_name.ends_with(".snxlib") {
            prompt = prompt.push(
                text(st.target_path.display().to_string())
                    .size(TS.caption)
                    .color(text_muted),
            );
        }

        let dialog = container(
            column![
                header,
                container(prompt).padding([14, 14]),
                container(remove_options).padding([0, 14]),
                container(row![
                    Space::new().width(Length::Fill),
                    secondary_button(
                        "Cancel",
                        Message::Remove(RemoveMsg::Close),
                        text_c,
                        border_c,
                    ),
                ])
                .padding([14, 14]),
            ]
            .width(self.dialog_width(super::super::super::state::ModalId::RemoveDialog, 560.0)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }

    /// Unsaved-edits stage of the Remove modal (#738): Save / Discard / Cancel.
    /// Cancel is `RemoveMsg::Close`, the same message Esc resolves to.
    fn exclude_dirty_card<'a>(
        body: String,
        can_discard: bool,
        header: Element<'a, Message>,
        tokens: &signex_types::theme::ThemeTokens,
    ) -> Element<'a, Message> {
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);
        // Discard is withheld when another open project shares the target.
        let mut buttons = row![
            Space::new().width(Length::Fill),
            secondary_button(
                "Cancel",
                Message::Remove(RemoveMsg::Close),
                text_c,
                border_c
            ),
        ]
        .align_y(iced::Alignment::Center);
        if can_discard {
            buttons = buttons.push(Space::new().width(8)).push(secondary_button(
                "Discard",
                Message::Remove(RemoveMsg::ExcludeDirty(
                    crate::app::ExcludeDirtyChoice::Discard,
                )),
                text_c,
                border_c,
            ));
        }
        buttons = buttons.push(Space::new().width(8)).push(primary_button(
            "Save",
            Some(Message::Remove(RemoveMsg::ExcludeDirty(
                crate::app::ExcludeDirtyChoice::Save,
            ))),
            border_c,
        ));
        container(
            column![
                header,
                container(text(body).size(TS.body).color(text_muted)).padding([14, 14]),
                container(buttons).padding([14, 14]),
            ]
            .width(560),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .into()
    }

    pub(in crate::app::view) fn view_project_close_confirm(&self) -> Element<'_, Message> {
        let dialog = self.view_project_close_confirm_body();
        // Reuse the rename modal's offset slot — `RenameDialog` is
        // never up at the same time as project-close confirm, and
        // they're roughly the same footprint, so reusing the slot
        // keeps modal-offset state simpler.
        let offset = (0.0, 0.0);
        wrap_modal(dialog, offset, self.ui_state.window_size, (520.0, 360.0))
    }

    pub(in crate::app::view) fn view_project_close_confirm_body(&self) -> Element<'_, Message> {
        use crate::app::ProjectCloseChoice;
        let Some(ref st) = self.ui_state.project_close_confirm else {
            return container(Space::new()).into();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let theme_id = self.ui_state.theme_id;
        let header_content: Element<'_, Message> = container(
            row![
                text("Close Project — Unsaved Edits")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(
                    Message::Project(ProjectMsg::CloseConfirm(ProjectCloseChoice::Cancel)),
                    theme_id,
                    text_muted,
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::ProjectCloseConfirm,
        );

        // List of dirty filenames, scrollable in case the project
        // has dozens of unsaved sheets. Filenames only — paths would
        // overflow narrow modals and the project context already
        // implies the parent dir.
        let file_rows = self.dirty_entry_rows(&st.dirty_paths, &st.dirty_rows, text_c);
        let file_list = scrollable(column(file_rows).spacing(0))
            .height(Length::Shrink)
            .width(Length::Fill);

        // An unreadable sheet hides its children: no Discard All, and why.
        let discard_blocked = st.sheet_refs.discard_blocked_reason();
        let discard_note: Element<'_, Message> = match &discard_blocked {
            Some(reason) => container(text(reason.clone()).size(TS.body).color(text_muted))
                .padding([10, 14])
                .into(),
            None => Space::new().height(0).into(),
        };
        let discard_button: Element<'_, Message> = if discard_blocked.is_some() {
            Space::new().width(0).into()
        } else {
            secondary_button(
                "Discard All",
                Message::Project(ProjectMsg::CloseConfirm(ProjectCloseChoice::DiscardAll)),
                text_c,
                border_c,
            )
        };
        let discard_gap: Element<'_, Message> = if discard_blocked.is_some() {
            Space::new().width(0).into()
        } else {
            Space::new().width(8).into()
        };

        // Kept documents stay open and are never discarded; say so, by name.
        let kept_note: Element<'_, Message> = if st.kept_paths.is_empty() {
            Space::new().height(0).into()
        } else {
            let names: Vec<String> = st
                .kept_paths
                .iter()
                .map(|p| crate::app::state::sheet_refs::file_label(p))
                .collect();
            container(
                text(format!(
                    "Kept open, not provably this project's alone: {}",
                    crate::app::state::sheet_refs::bounded_names(&names)
                ))
                .size(TS.body)
                .color(text_muted),
            )
            .padding([10, 14])
            .into()
        };

        let dialog = container(
            column![
                header,
                container(
                    text(format!(
                        "'{}' has {}. What would you like to do?",
                        st.project_name,
                        unsaved_phrase(st.dirty_paths.len(), st.dirty_rows.len())
                    ))
                    .size(TS.body)
                    .color(text_muted)
                )
                .padding([14, 14]),
                container(file_list)
                    .max_height(crate::app::handlers::modal_sizing::LIST_MAX_HEIGHT)
                    .padding([0, 14]),
                kept_note,
                discard_note,
                container(
                    row![
                        Space::new().width(Length::Fill),
                        secondary_button(
                            "Cancel",
                            Message::Project(ProjectMsg::CloseConfirm(ProjectCloseChoice::Cancel)),
                            text_c,
                            border_c,
                        ),
                        Space::new().width(8),
                        discard_button,
                        discard_gap,
                        primary_button(
                            "Save All",
                            Some(Message::Project(ProjectMsg::CloseConfirm(
                                ProjectCloseChoice::SaveAll,
                            ))),
                            border_c,
                        ),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([14, 14]),
            ]
            .width(self.dialog_width(
                super::super::super::state::ModalId::ProjectCloseConfirm,
                520.0,
            )),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }

    pub(in crate::app::view) fn view_app_quit_confirm(&self) -> Element<'_, Message> {
        let dialog = self.view_app_quit_confirm_body();
        // Reuse the rename modal's centred offset slot — the app-quit
        // confirm is never up at the same time as the rename dialog.
        let offset = (0.0, 0.0);
        wrap_modal(dialog, offset, self.ui_state.window_size, (520.0, 360.0))
    }

    /// One list row per unsaved entry in a confirm modal: file
    /// basenames first, then Component Preview rows by part number.
    fn dirty_entry_rows(
        &self,
        paths: &[std::path::PathBuf],
        rows: &[crate::library::state::EditorAddress],
        text_c: iced::Color,
    ) -> Vec<Element<'_, Message>> {
        let path_labels = paths.iter().map(|path| {
            path.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("(unnamed)")
                .to_string()
        });
        let row_labels = rows
            .iter()
            .map(|address| self.library.editor_label(address));
        path_labels
            .chain(row_labels)
            .map(|name| {
                container(text(name).size(TS.body).color(text_c))
                    .padding([SP.s, SP.m])
                    .into()
            })
            .collect()
    }

    pub(in crate::app::view) fn view_app_quit_confirm_body(&self) -> Element<'_, Message> {
        use crate::app::ProjectCloseChoice;
        let Some(ref st) = self.ui_state.app_quit_confirm else {
            return container(Space::new()).into();
        };

        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let theme_id = self.ui_state.theme_id;
        let header_content: Element<'_, Message> = container(
            row![
                text(format!(
                    "{} Signex — Unsaved Edits",
                    super::super::detached_title::quit_word()
                ))
                .size(MODAL_HEADER_TITLE_SIZE)
                .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(
                    Message::Project(ProjectMsg::AppQuitConfirm(ProjectCloseChoice::Cancel)),
                    theme_id,
                    text_muted,
                ),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let header = self.modal_header(
            header_content,
            super::super::super::state::ModalId::AppQuitConfirm,
        );

        // List of dirty filenames across the whole workspace,
        // scrollable in case many files are unsaved. Basenames only —
        // full paths would overflow the modal.
        let file_rows = self.dirty_entry_rows(&st.dirty_paths, &st.dirty_rows, text_c);
        let file_list = scrollable(column(file_rows).spacing(0))
            .height(Length::Shrink)
            .width(Length::Fill);

        let refreshed_note: Element<'_, Message> = if self.ui_state.quit_confirm_refreshed {
            container(
                text("The unsaved list changed; review and confirm again.")
                    .size(11)
                    .color(iced::Color::from_rgb(0.90, 0.35, 0.30)),
            )
            .padding([10, 14])
            .into()
        } else {
            Space::new().height(0).into()
        };
        let dialog = container(
            column![
                header,
                refreshed_note,
                container(
                    text(format!(
                        "You have {}. Save before {}?",
                        unsaved_phrase(st.dirty_paths.len(), st.dirty_rows.len()),
                        if cfg!(target_os = "macos") {
                            "quitting"
                        } else {
                            "exiting"
                        }
                    ))
                    .size(TS.body)
                    .color(text_muted)
                )
                .padding([14, 14]),
                container(file_list)
                    .max_height(crate::app::handlers::modal_sizing::LIST_MAX_HEIGHT)
                    .padding([0, 14]),
                container(
                    row![
                        Space::new().width(Length::Fill),
                        secondary_button(
                            "Cancel",
                            Message::Project(ProjectMsg::AppQuitConfirm(
                                ProjectCloseChoice::Cancel
                            )),
                            text_c,
                            border_c,
                        ),
                        Space::new().width(8),
                        secondary_button(
                            "Discard All",
                            Message::Project(ProjectMsg::AppQuitConfirm(
                                ProjectCloseChoice::DiscardAll,
                            )),
                            text_c,
                            border_c,
                        ),
                        Space::new().width(8),
                        primary_button(
                            "Save All",
                            Some(Message::Project(ProjectMsg::AppQuitConfirm(
                                ProjectCloseChoice::SaveAll,
                            ))),
                            border_c,
                        ),
                    ]
                    .align_y(iced::Alignment::Center),
                )
                .padding([14, 14]),
            ]
            .width(self.dialog_width(super::super::super::state::ModalId::AppQuitConfirm, 520.0)),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
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
