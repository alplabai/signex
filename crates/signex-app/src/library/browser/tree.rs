//! Library Browser — Graphite libraries tree (left column, above the tables).
//!
//! One row per mounted library with its total row count; the library this tab
//! shows is highlighted, a click opens another one's browser tab. Only real
//! mounts are listed.

use super::look;
use super::*;
use iced::widget::{Row, column};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

pub(super) fn view_libraries_tree<'a>(
    library_path: &'a std::path::Path,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let text_c = theme_ext::text_primary(tokens);
    let (muted, line, pill_fill) = (r.ink3, r.line_soft, r.sunken);
    let active_bg = look::active_fill(tokens);
    let hover_bg = look::row_hover_fill(tokens);
    let radius = crate::styles::Radii::GRAPHITE.button;

    let mut col: Column<Element<'a, LibraryMessage>> = column![
        container(
            text(raw("LIBRARIES"))
                .size(TS.section)
                .style(crate::styles::section_label(tokens))
        )
        .padding(iced::Padding {
            top: 6.0,
            right: 8.0,
            bottom: 4.0,
            left: 12.0,
        })
        .boxed()
    ]
    .spacing(SP.xs)
    .width(Length::Fill);

    for lib in &library_state.open_libraries {
        let is_current = lib.root == library_path;
        let rows: usize = lib.tables.values().map(Vec::len).sum();
        let mut label: Row<Element<'a, LibraryMessage>> = row![
            text(signex_i18n::UserText::new(lib.display_name.clone()))
                .size(BROWSER_TEXT_SIZE)
                .color(text_c)
                .wrapping(iced::widget::text::Wrapping::None)
                .width(Length::Fill)
                .boxed()
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center);
        if library_state.is_read_only(&lib.root) {
            label = label.push(
                container(text(raw("read-only")).size(TS.caption).color(muted))
                    .padding([0.0, SP.s])
                    .style(move |_: &Theme| iced::widget::container::Style {
                        background: Some(iced::Background::Color(pill_fill)),
                        border: Border {
                            width: 1.0,
                            color: line,
                            radius: crate::styles::Radii::GRAPHITE.pill.into(),
                            ..Border::default()
                        },
                        ..iced::widget::container::Style::default()
                    })
                    .boxed(),
            );
        }
        label = label.push(
            text(raw(rows.to_string()))
                .size(BROWSER_TEXT_SIZE)
                .color(muted)
                .boxed(),
        );
        col = col.push(
            button(label)
                .padding(iced::Padding {
                    top: 5.0,
                    right: 8.0,
                    bottom: 5.0,
                    left: 12.0,
                })
                .width(Length::Fill)
                .on_press(LibraryMessage::OpenLibraryBrowser(lib.root.clone()))
                .style(move |_: &Theme, status: iced::widget::button::Status| {
                    let hovered = matches!(status, iced::widget::button::Status::Hovered);
                    iced::widget::button::Style {
                        background: (is_current || hovered).then_some(iced::Background::Color(
                            if is_current { active_bg } else { hover_bg },
                        )),
                        text_color: text_c,
                        border: Border {
                            radius: radius.into(),
                            ..Border::default()
                        },
                        ..iced::widget::button::Style::default()
                    }
                })
                .boxed(),
        );
    }
    container(col.boxed())
        .width(Length::Fixed(sidebar::SIDEBAR_W))
        .boxed()
}
