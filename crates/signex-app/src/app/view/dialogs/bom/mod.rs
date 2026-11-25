//! The Bill of Materials export dialog (mockup row 112): header, toolbar,
//! the data grid (`table`) beside the Properties sidebar (`sidebar`,
//! `rules`), and the footer (`footer`).

use super::*;
use iced::widget::{Space, column, container, row};
use iced::{Element, Length, Widget};
use signex_i18n::{ids, t};

use super::widgets::{draggable_header, os_title_bar_slot};

#[cfg(test)]
mod computed_tests;
mod footer;
#[cfg(test)]
mod names_tests;
mod rules;
mod sidebar;
mod table;
mod toolbar;
#[cfg(test)]
mod view_tests;

pub(in crate::app) use table::{default_column_width, sorted_row_order};

/// Width of the Properties sidebar, as drawn.
const SIDEBAR_W: f32 = 300.0;

impl Signex {
    pub(in crate::app::view) fn view_bom_preview(&self) -> Element<'_, Message> {
        let dialog = self.view_bom_preview_body_inner(true);
        self.wrap_dialog(
            crate::app::state::ModalId::BomPreview,
            dialog,
            self.bom_modal_size(),
        )
    }

    /// The in-window BOM modal: 1000 x 700 where the window allows, capped
    /// to the window otherwise (#1227).
    pub(in crate::app::view) fn bom_modal_size(&self) -> (f32, f32) {
        super::widgets::fit_modal_size(self.ui_state.window_size, (1000.0, 700.0))
    }

    pub(in crate::app::view) fn view_bom_preview_body(&self) -> Element<'_, Message> {
        self.view_bom_preview_body_inner(false)
    }

    /// The project file the BOM is built for (the document's own name when
    /// it has no project): the header's subtitle.
    fn bom_subject(&self, source: &std::path::Path) -> Option<String> {
        let project = self.document_state.document_project(source);
        let path = project.map_or(source, |p| p.path.as_path());
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    }

    fn view_bom_preview_body_inner(&self, draggable: bool) -> Element<'_, Message> {
        let Some(ref preview) = self.document_state.bom_preview else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let line = crate::styles::tokens::resolve(tokens).line_soft;

        let header_content: Element<'_, Message> = super::graphite::titled_header(
            t!(i18n, ids::DIALOG_TITLE_BOM).into_string(),
            self.bom_subject(&preview.source),
            Message::BomPreview(BomPreviewMsg::Close),
            tokens,
        );
        let header = if draggable {
            draggable_header(
                header_content,
                super::super::super::state::ModalId::BomPreview,
                self.interaction_state.last_mouse_pos,
            )
        } else {
            os_title_bar_slot()
        };

        // Table, a hairline, then the Properties sidebar.
        let main_row = row![
            container(self.bom_table())
                .width(Length::Fill)
                .height(Length::Fill),
            hairline(line, true),
            container(self.bom_sidebar())
                .width(Length::Fixed(SIDEBAR_W))
                .height(Length::Fill),
        ]
        .height(Length::Fill);

        let (modal_w, modal_h): (Length, Length) = if draggable {
            let (w, h) = self.bom_modal_size();
            (Length::Fixed(w), Length::Fixed(h))
        } else {
            (Length::Fill, Length::Fill)
        };
        let dialog = container(
            column![
                header,
                hairline(line, false),
                self.bom_toolbar(),
                hairline(line, false),
                main_row,
                hairline(line, false),
                self.bom_footer(),
            ]
            .spacing(0)
            .width(modal_w)
            .height(modal_h),
        )
        .style(crate::styles::modal_card(tokens))
        .clip(true);
        dialog.boxed()
    }
}

/// A 1 px rule in `color`: across the dialog, or down it when `vertical`.
fn hairline<'a>(color: iced::Color, vertical: bool) -> Element<'a, Message> {
    let (w, h) = if vertical {
        (Length::Fixed(crate::styles::HAIRLINE), Length::Fill)
    } else {
        (Length::Fill, Length::Fixed(crate::styles::HAIRLINE))
    };
    container(Space::new().width(Length::Fill).height(Length::Fill))
        .width(w)
        .height(h)
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(color)),
            ..iced::widget::container::Style::default()
        })
        .boxed()
}
