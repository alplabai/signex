//! Library Browser — Graphite libraries tree (left column, above the tables).
//!
//! One row per mounted library with its total row count; the library this tab
//! shows is highlighted, a click opens another one's browser tab. Only real
//! mounts are listed. A right-click opens the row's menu under it, with View
//! import report (the Import Library dialog's loss report, #3121).

use super::look;
use super::*;
use crate::library::import_dialog::{self, ImportDialogMsg};
use iced::widget::{Row, column};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

pub(super) fn view_libraries_tree<'a>(
    library_path: &'a std::path::Path,
    library_state: &'a LibraryState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let text_c = theme_ext::text_primary(tokens);
    let (muted, line, pill_fill) = (r.ink3, r.line_soft, r.sunken);
    let active_bg = look::active_fill(tokens);
    let hover_bg = look::row_hover_fill(tokens);
    let radius = crate::styles::Radii::GRAPHITE.small;

    let mut col: Column<Element<'a, LibraryMessage>> = column![
        container(
            row![
                text(t!(i18n, ids::LIBRARY_BROWSER_LIBRARIES))
                    .size(TS.section)
                    .style(crate::styles::section_label(tokens))
                    .boxed(),
                Space::new().width(Length::Fill).boxed(),
                import_button(tokens, i18n),
            ]
            .align_y(iced::Alignment::Center)
        )
        .padding(iced::Padding {
            top: SP.m,
            right: SP.m,
            bottom: SP.xs,
            left: SP.m,
        })
        .boxed()
    ]
    .spacing(SP.xs)
    .width(Length::Fill);

    for lib in &library_state.open_libraries {
        let is_current = lib.root == library_path;
        let rows: usize = lib.tables.values().map(Vec::len).sum();
        let mut label: Row<Element<'a, LibraryMessage>> = row![
            super::header::tinted_icon(
                crate::icons::icon_chrome_library(signex_types::theme::ThemeId::GraphiteDark),
                BROWSER_TEXT_SIZE,
                muted
            ),
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
                container(
                    text(t!(i18n, ids::LIBRARY_BROWSER_READ_ONLY))
                        .size(TS.caption)
                        .color(muted),
                )
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
            text(t!(i18n, ids::LIBRARY_BROWSER_ROW_COUNT, "count" => rows))
                .size(TS.caption)
                .font(crate::fonts::iced_font_for_family(
                    crate::fonts::CHROME_MONO_FONT,
                ))
                .color(muted)
                .boxed(),
        );
        let entry = button(label)
            .padding([5.0, SP.m])
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
            });
        let menu = import_dialog::msg(ImportDialogMsg::LibraryMenuToggled(lib.root.clone()));
        col = col.push(mouse_area(entry).on_right_press(menu).boxed());
        if library_state.import_menu.as_deref() == Some(lib.root.as_path()) {
            col = col.push(library_menu(&lib.root, tokens, i18n));
        }
    }
    col.boxed()
}

/// The LIBRARIES header's Import…: the Import Library dialog (#3122), as a
/// small ghost button in the accent text like the TABLES header's.
fn import_button<'a>(tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, LibraryMessage> {
    use iced::widget::button::{Status, Style};
    let r = crate::styles::tokens::resolve(tokens);
    let accent_text = r.accent_text;
    let hover = look::hover_fill(tokens);
    button(text(t!(i18n, ids::LIBRARY_BROWSER_IMPORT)).size(TS.caption))
        .padding([0.0, SP.s])
        .on_press(import_dialog::msg(ImportDialogMsg::Requested))
        .style(move |_: &Theme, status: Status| Style {
            background: matches!(status, Status::Hovered)
                .then_some(iced::Background::Color(hover)),
            text_color: accent_text,
            border: Border {
                radius: crate::styles::Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..Style::default()
        })
        .boxed()
}

/// A library row's menu: View import report.
fn library_menu<'a>(
    root: &std::path::Path,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    use iced::widget::button::{Status, Style};
    let ink = theme_ext::text_primary(tokens);
    let hover = look::row_hover_fill(tokens);
    let item = button(text(t!(i18n, ids::LIBRARY_IMPORT_DIALOG_MENU_VIEW_REPORT)).size(TS.body))
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .on_press(import_dialog::msg(ImportDialogMsg::StoredReportRequested(
            root.to_path_buf(),
        )))
        .style(move |_: &Theme, status: Status| Style {
            background: matches!(status, Status::Hovered | Status::Pressed)
                .then_some(iced::Background::Color(hover)),
            text_color: ink,
            ..Style::default()
        });
    container(item)
        .padding(SP.xs)
        .width(Length::Fill)
        .style(crate::styles::context_menu(tokens))
        .boxed()
}
