//! Electrical Rule Check (ERC) modal — per-rule severity override grid
//! and the pin-connection matrix.
//!
//! Extracted verbatim from `view/dialogs.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;
use iced::widget::{Space, column, container, row, text, text_input};
use iced::{Color, Element, Length};

use super::rule_editor::{RuleEditor, rule_editor};
use super::widgets::{
    close_x_button, draggable_header, fit_modal_size, os_title_bar_slot, primary_button,
    secondary_button, secondary_button_maybe, wrap_modal,
};
use super::{MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE};

impl Signex {
    pub(in crate::app::view) fn view_erc_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_erc_dialog_body_inner(true);
        let offset = self
            .ui_state
            .modal_offsets
            .get(&super::super::super::state::ModalId::ErcDialog)
            .copied()
            .unwrap_or((0.0, 0.0));
        wrap_modal(
            dialog,
            offset,
            self.ui_state.window_size,
            self.erc_modal_size(),
        )
    }

    /// The in-window ERC modal: 1000 x 600 where the window allows, capped
    /// to the window otherwise (#1227).
    pub(in crate::app::view) fn erc_modal_size(&self) -> (f32, f32) {
        fit_modal_size(self.ui_state.window_size, (1000.0, 600.0))
    }

    pub(in crate::app::view) fn view_erc_dialog_body(&self) -> Element<'_, Message> {
        self.view_erc_dialog_body_inner(false)
    }

    fn view_erc_dialog_body_inner(&self, draggable: bool) -> Element<'_, Message> {
        let tokens = &self.document_state.panel_ctx.tokens;
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let border_c = crate::styles::ti(tokens.border);

        let theme_id = self.ui_state.theme_id;
        let header_content: Element<'_, Message> = container(
            row![
                text("Electrical Rules Check")
                    .size(MODAL_HEADER_TITLE_SIZE)
                    .color(text_c),
                Space::new().width(Length::Fill),
                close_x_button(Message::Erc(ErcMsg::CloseDialog), theme_id, text_muted),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding(MODAL_HEADER_PADDING)
        .height(MODAL_HEADER_HEIGHT)
        .style(crate::styles::modal_header_strip(tokens))
        .into();
        let header = if draggable {
            draggable_header(
                header_content,
                super::super::super::state::ModalId::ErcDialog,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            os_title_bar_slot()
        };

        // A project that stores its own rules shows (and edits) those; the
        // machine preferences only back a project without a rule set.
        let stored = self
            .document_state
            .active_document_project()
            .and_then(|p| p.data.rules.as_ref())
            .map(|rules| signex_erc::project_rules::resolve(&rules.erc));
        let (severity_map, pin_map) = match &stored {
            Some(resolved) => (&resolved.severity, &resolved.pin_matrix),
            None => (
                &self.ui_state.erc_severity_override,
                &self.ui_state.pin_matrix_overrides,
            ),
        };

        let history = self
            .document_state
            .active_document_project()
            .and_then(|p| self.document_state.rules_history.get(&p.id));
        let can_undo = history.is_some_and(|h| h.can_undo());
        let can_redo = history.is_some_and(|h| h.can_redo());
        let has_project = self.document_state.active_document_project().is_some();
        let unreadable = self.active_project_has_unreadable_rules();
        let scope_note = match (has_project, stored.is_some(), unreadable) {
            (_, true, _) => "Editing the rules stored in this project.",
            (true, false, true) => {
                "This project's rule set is from a newer Signex and is kept unchanged; \
                 editing the \"User Default\" rule profile (this machine's rules)."
            }
            (true, false, false) => {
                "Editing the \"User Default\" rule profile (this machine's rules); \
                 this project stores no rules."
            }
            (false, false, _) => {
                "Editing the \"User Default\" rule profile (this machine's rules)."
            }
        };
        // Editing the machine's rules while they are loading, or while the
        // User Default file cannot be read or written: say so here, not only
        // in the log.
        let mut scope = column![text(scope_note).size(10).color(text_muted)].spacing(2);
        if stored.is_none()
            && let Some(notice) = self.machine_rules_notice()
        {
            scope = scope.push(
                text(notice)
                    .size(10)
                    .color(Color::from_rgb(0.9, 0.35, 0.35)),
            );
        }
        let mut footer = row![scope].spacing(8).align_y(iced::Alignment::Center);
        // A rule set from a newer Signex sits unread in the file; storing
        // would replace it on save, so the action is not offered.
        if has_project && stored.is_none() && !unreadable {
            footer = footer.push(secondary_button(
                "Store ERC rules in this project",
                Message::Erc(ErcMsg::StoreRulesInProject),
                text_c,
                border_c,
            ));
        }
        if stored.is_some() {
            footer = footer.push(secondary_button(
                "Save as profile",
                Message::Erc(ErcMsg::ProfileSaveDialogOpened),
                text_c,
                border_c,
            ));
            if self.active_project_has_profile() {
                footer = footer.push(secondary_button(
                    "Update from profile",
                    Message::Erc(ErcMsg::ProfileUpdateRequested),
                    text_c,
                    border_c,
                ));
            }
            footer = footer
                .push(secondary_button_maybe(
                    "Undo",
                    can_undo.then_some(Message::Erc(ErcMsg::RulesUndone)),
                    if can_undo { text_c } else { text_muted },
                    border_c,
                ))
                .push(secondary_button_maybe(
                    "Redo",
                    can_redo.then_some(Message::Erc(ErcMsg::RulesRedone)),
                    if can_redo { text_c } else { text_muted },
                    border_c,
                ));
        }
        let footer = footer
            .push(Space::new().width(Length::Fill))
            .push(primary_button(
                "Run ERC",
                Some(Message::Erc(ErcMsg::Run)),
                border_c,
            ))
            .push(Space::new().width(8))
            .push(secondary_button(
                "Close",
                Message::Erc(ErcMsg::CloseDialog),
                text_c,
                border_c,
            ));

        // The shared rule editor: rules table on the left, pin-connection
        // matrix on the right. Preferences ▸ Rule Profiles draws the same one.
        let body = rule_editor(RuleEditor {
            tokens,
            severity: severity_map,
            pin_matrix: pin_map,
            editable: true,
            on_severity: |rule, severity| Message::Erc(ErcMsg::SeverityChanged(rule, severity)),
            on_pin_cell: |row, col| Message::Erc(ErcMsg::PinMatrixCellCycled { row, col }),
        });

        // In-window: sized to the window-capped modal. Detached: the OS
        // window owns the size, so fill it. The body scrolls; the footer
        // holds the buttons and stays outside the scrollable (#1227).
        let (modal_w, modal_h): (Length, Length) = if draggable {
            let (w, h) = self.erc_modal_size();
            (Length::Fixed(w), Length::Fixed(h))
        } else {
            (Length::Fill, Length::Fill)
        };
        let dialog = container(
            column![
                header,
                container(iced::widget::scrollable(
                    container(body).padding([14, 14]).width(Length::Fill)
                ))
                .width(Length::Fill)
                .height(Length::Fill),
                self.view_profile_update_panel(text_c, text_muted, border_c),
                self.view_profile_save_panel(text_c, text_muted, border_c),
                container(footer).padding([10, 14]),
            ]
            .width(modal_w)
            .height(modal_h),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.into()
    }
}

impl Signex {
    /// The diff an "Update from profile" would apply, with Apply / Cancel.
    /// Empty space when no update is pending.
    fn view_profile_update_panel(
        &self,
        text_c: Color,
        text_muted: Color,
        border_c: Color,
    ) -> Element<'_, Message> {
        let Some((name, lines)) = self.pending_profile_update_lines() else {
            return Space::new().height(0).into();
        };
        let heading = if lines.is_empty() {
            format!(
                "Profile \"{name}\": rules already match; only the recorded hash is out of date."
            )
        } else {
            format!(
                "Update from profile \"{name}\" changes {} entries:",
                lines.len()
            )
        };
        let mut list = column![text(heading).size(11).color(text_c)].spacing(2);
        for line in lines {
            list = list.push(text(line).size(10).color(text_muted));
        }
        let buttons = row![
            primary_button(
                "Apply update",
                Some(Message::Erc(ErcMsg::ProfileUpdateConfirmed)),
                border_c,
            ),
            secondary_button(
                "Cancel",
                Message::Erc(ErcMsg::ProfileUpdateCancelled),
                text_c,
                border_c,
            ),
        ]
        .spacing(8);
        container(
            column![
                container(iced::widget::scrollable(list)).max_height(120),
                buttons
            ]
            .spacing(6),
        )
        .padding([8, 14])
        .width(Length::Fill)
        .into()
    }
}

impl Signex {
    /// The "Save as profile" form: name, version, validation, and the
    /// overwrite question. Empty space when the dialog is closed.
    fn view_profile_save_panel(
        &self,
        text_c: Color,
        text_muted: Color,
        border_c: Color,
    ) -> Element<'_, Message> {
        let Some(dialog) = self.ui_state.save_profile_dialog.as_ref() else {
            return Space::new().height(0).into();
        };
        let mut panel = column![
            text("Save the project's ERC rules as a profile")
                .size(11)
                .color(text_c)
        ]
        .spacing(6);
        if dialog.confirm_overwrite {
            let name = dialog.name.trim();
            panel = panel
                .push(
                    text(format!(
                        "A profile named \"{name}\" already exists. Overwriting replaces its \
                         rules and version; projects that took their rules from it will show \
                         the change as an update."
                    ))
                    .size(10)
                    .color(text_muted),
                )
                .push(
                    row![
                        primary_button(
                            "Overwrite",
                            (!dialog.busy)
                                .then_some(Message::Erc(ErcMsg::ProfileSaveOverwriteConfirmed)),
                            border_c,
                        ),
                        secondary_button(
                            "Back",
                            Message::Erc(ErcMsg::ProfileSaveCancelled),
                            text_c,
                            border_c,
                        ),
                    ]
                    .spacing(8),
                );
        } else {
            let submit = || Message::Erc(ErcMsg::ProfileSaveSubmitted);
            let editable = !dialog.busy;
            let mut name_input = text_input("Profile name", &dialog.name).size(11);
            let mut version_input = text_input("Version", &dialog.version).size(11).width(90);
            if editable {
                name_input = name_input
                    .on_input(|v| Message::Erc(ErcMsg::ProfileSaveNameChanged(v)))
                    .on_submit(submit());
                version_input = version_input
                    .on_input(|v| Message::Erc(ErcMsg::ProfileSaveVersionChanged(v)))
                    .on_submit(submit());
            }
            panel = panel.push(
                row![
                    text("Name").size(10).color(text_muted),
                    name_input,
                    text("Version").size(10).color(text_muted),
                    version_input,
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            );
            if let Some(error) = &dialog.error {
                panel = panel.push(
                    text(error.clone())
                        .size(10)
                        .color(Color::from_rgb(0.9, 0.35, 0.35)),
                );
            }
            panel = panel.push(
                row![
                    primary_button("Save", editable.then(submit), border_c),
                    secondary_button(
                        "Cancel",
                        Message::Erc(ErcMsg::ProfileSaveCancelled),
                        text_c,
                        border_c,
                    ),
                ]
                .spacing(8),
            );
        }
        container(panel).padding([8, 14]).width(Length::Fill).into()
    }
}
