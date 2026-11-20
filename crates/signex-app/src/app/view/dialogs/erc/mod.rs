//! Electrical Rules Check (ERC) modal (#2413): the source strip (where the
//! rules come from, plus severity count tiles), the shared rule editor body
//! (per-rule severities and the pin-connection matrix), the profile forms and
//! the footer (Profile menu, Undo / Redo, Close, Run ERC).

use super::*;
use iced::widget::{Stack, column, container};
use iced::{Element, Length, Widget};
use signex_i18n::ids;

use super::rule_editor::{RuleEditor, rule_editor, shipped_rules};
use super::widgets::{draggable_header, fit_modal_size, os_title_bar_slot};

mod footer;
mod profile_forms;
mod strip;

impl Signex {
    pub(in crate::app::view) fn view_erc_dialog(&self) -> Element<'_, Message> {
        let dialog = self.view_erc_dialog_body_inner(true);
        self.wrap_dialog(
            crate::app::state::ModalId::ErcDialog,
            dialog,
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

        let header_content: Element<'_, Message> = super::graphite::titled_header(
            self.ui_state.i18n.message_plain(ids::DIALOG_TITLE_ERC, &[]),
            self.detached_modal_subtitle(crate::app::state::ModalId::ErcDialog),
            Message::Erc(ErcMsg::CloseDialog),
            tokens,
        );
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

        // The shared rule editor: rules table on the left, pin-connection
        // matrix on the right. Preferences ▸ Rule Profiles draws the same one.
        let body = rule_editor(RuleEditor {
            i18n: &self.ui_state.i18n,
            tokens,
            severity: severity_map,
            pin_matrix: pin_map,
            shipped: shipped_rules(),
            editable: true,
            changed_only: self.ui_state.erc_rule_list.changed_only,
            filter: &self.ui_state.erc_rule_list.query,
            on_severity: |rule, severity| Message::Erc(ErcMsg::SeverityChanged(rule, severity)),
            on_pin_cell: |row, col| Message::Erc(ErcMsg::PinMatrixCellCycled { row, col }),
            on_changed_only: |on| Message::Erc(ErcMsg::RulesChangedOnlyToggled(on)),
            on_filter: |query| Message::Erc(ErcMsg::RuleFilterChanged(query)),
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
        let content = column![
            header,
            self.view_erc_source_strip(stored.is_some(), tokens),
            container(iced::widget::scrollable(
                container(body).padding([SP.l, SP.l]).width(Length::Fill)
            ))
            .width(Length::Fill)
            .height(Length::Fill),
            self.view_profile_update_panel(tokens),
            self.view_profile_save_panel(tokens),
            self.view_erc_footer(stored.is_some(), tokens),
        ]
        .width(modal_w)
        .height(modal_h);

        // The open Profile menu floats over the body, just above its button.
        let mut layers: Stack<Element<'_, Message>> = Stack::new()
            .width(modal_w)
            .height(modal_h)
            .push(content.boxed());
        if let Some(menu) = self.view_erc_profile_menu(tokens) {
            layers = layers.push(menu);
        }
        container(layers)
            .style(crate::styles::modal_card(tokens))
            .clip(true)
            .boxed()
    }
}

const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;
