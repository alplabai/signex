//! Electrical Rule Check (ERC) modal — per-rule severity override grid
//! and the pin-connection matrix.
//!
//! Extracted verbatim from `view/dialogs.rs` (ADR-0001, issue #164) as pure
//! code motion — no behaviour change. These are methods of the same
//! `Signex` view impl, split across sibling files.

use super::*;
use iced::widget::{Space, column, container, row, text, text_input};
use iced::{Element, Length, Widget};

use super::rule_editor::{RuleEditor, rule_editor};
use super::widgets::{
    close_x_button, draggable_header, fit_modal_size, os_title_bar_slot, wrap_modal,
};
use super::{MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE};
use crate::styles::{DialogButtonKind, dialog_button, dialog_footer_split};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

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

        let theme_id = self.ui_state.effective_theme_id();
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
        .boxed();
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
        let mut scope = column![text(scope_note).size(TS.caption).color(text_muted)].spacing(SP.xs);
        if stored.is_none()
            && let Some(notice) = self.machine_rules_notice()
        {
            scope = scope.push(
                text(notice)
                    .size(TS.caption)
                    .color(crate::styles::tokens::danger(tokens))
                    .boxed(),
            );
        }
        let secondary = |label: &str, message: Option<Message>| {
            dialog_button(label, DialogButtonKind::Secondary, message, tokens)
        };
        let mut extras: Vec<Element<'_, Message>> = vec![scope.boxed()];
        // A rule set from a newer Signex sits unread in the file; storing
        // would replace it on save, so the action is not offered.
        if has_project && stored.is_none() && !unreadable {
            extras.push(secondary(
                "Store ERC rules in this project",
                Some(Message::Erc(ErcMsg::StoreRulesInProject)),
            ));
        }
        if stored.is_some() {
            extras.push(secondary(
                "Save as profile",
                Some(Message::Erc(ErcMsg::ProfileSaveDialogOpened)),
            ));
            if self.active_project_has_profile() {
                extras.push(secondary(
                    "Update from profile",
                    Some(Message::Erc(ErcMsg::ProfileUpdateRequested)),
                ));
            }
            extras.push(secondary(
                "Undo",
                can_undo.then_some(Message::Erc(ErcMsg::RulesUndone)),
            ));
            extras.push(secondary(
                "Redo",
                can_redo.then_some(Message::Erc(ErcMsg::RulesRedone)),
            ));
        }
        let footer = dialog_footer_split(
            extras,
            vec![
                dialog_button(
                    "Run ERC",
                    DialogButtonKind::Primary,
                    Some(Message::Erc(ErcMsg::Run)),
                    tokens,
                ),
                secondary("Close", Some(Message::Erc(ErcMsg::CloseDialog))),
            ],
        );

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
                self.view_profile_update_panel(tokens),
                self.view_profile_save_panel(tokens),
                footer,
            ]
            .width(modal_w)
            .height(modal_h),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }
}

impl Signex {
    /// The diff an "Update from profile" would apply, with Apply / Cancel.
    /// Empty space when no update is pending.
    fn view_profile_update_panel(
        &self,
        tokens: &signex_types::theme::ThemeTokens,
    ) -> Element<'_, Message> {
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let Some((name, lines)) = self.pending_profile_update_lines() else {
            return Space::new().height(0).boxed();
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
        let mut list = column![text(heading).size(TS.body).color(text_c)].spacing(SP.xs);
        for line in lines {
            list = list.push(text(line).size(TS.caption).color(text_muted).boxed());
        }
        let buttons = row![
            dialog_button(
                "Apply update",
                DialogButtonKind::Primary,
                Some(Message::Erc(ErcMsg::ProfileUpdateConfirmed)),
                tokens,
            ),
            dialog_button(
                "Cancel",
                DialogButtonKind::Secondary,
                Some(Message::Erc(ErcMsg::ProfileUpdateCancelled)),
                tokens,
            ),
        ]
        .spacing(SP.s);
        container(
            column![
                container(iced::widget::scrollable(list)).height(Length::Fit.max(120)),
                buttons
            ]
            .spacing(6),
        )
        .padding([8, 14])
        .width(Length::Fill)
        .boxed()
    }
}

impl Signex {
    /// The "Save as profile" form: name, version, validation, and the
    /// overwrite question. Empty space when the dialog is closed.
    fn view_profile_save_panel(
        &self,
        tokens: &signex_types::theme::ThemeTokens,
    ) -> Element<'_, Message> {
        let text_c = crate::styles::ti(tokens.text);
        let text_muted = crate::styles::ti(tokens.text_secondary);
        let Some(dialog) = self.ui_state.save_profile_dialog.as_ref() else {
            return Space::new().height(0).boxed();
        };
        let mut panel = column![
            text("Save the project's ERC rules as a profile")
                .size(TS.body)
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
                    .size(TS.caption)
                    .color(text_muted)
                    .boxed(),
                )
                .push(
                    row![
                        dialog_button(
                            "Overwrite",
                            DialogButtonKind::Danger,
                            (!dialog.busy)
                                .then_some(Message::Erc(ErcMsg::ProfileSaveOverwriteConfirmed)),
                            tokens,
                        ),
                        dialog_button(
                            "Back",
                            DialogButtonKind::Secondary,
                            Some(Message::Erc(ErcMsg::ProfileSaveCancelled)),
                            tokens,
                        ),
                    ]
                    .spacing(SP.s)
                    .boxed(),
                );
        } else {
            let submit = || Message::Erc(ErcMsg::ProfileSaveSubmitted);
            let editable = !dialog.busy;
            let mut name_input = text_input("Profile name", &dialog.name).size(TS.body);
            let mut version_input = text_input("Version", &dialog.version)
                .size(TS.body)
                .width(90);
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
                    text("Name").size(TS.caption).color(text_muted),
                    name_input,
                    text("Version").size(TS.caption).color(text_muted),
                    version_input,
                ]
                .spacing(SP.m)
                .align_y(iced::Alignment::Center)
                .boxed(),
            );
            if let Some(error) = &dialog.error {
                panel = panel.push(
                    text(error.clone())
                        .size(TS.caption)
                        .color(crate::styles::tokens::danger(tokens))
                        .boxed(),
                );
            }
            panel = panel.push(
                row![
                    dialog_button(
                        "Save",
                        DialogButtonKind::Primary,
                        editable.then(submit),
                        tokens,
                    ),
                    dialog_button(
                        "Cancel",
                        DialogButtonKind::Secondary,
                        Some(Message::Erc(ErcMsg::ProfileSaveCancelled)),
                        tokens,
                    ),
                ]
                .spacing(SP.s)
                .boxed(),
            );
        }
        container(panel)
            .padding([8, 14])
            .width(Length::Fill)
            .boxed()
    }
}
