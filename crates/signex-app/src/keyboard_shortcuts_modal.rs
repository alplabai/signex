//! Help ▸ Keyboard Shortcuts modal — single-page reference grouped by
//! category. Reachable from the Help menu and from F1 (which also closes
//! the modal when it's already open).
//!
//! Closes UX_IMPROVEMENTS_OVER_ALTIUM §4.2 ("Hotkey discoverability").

use iced::widget::{Column, Space, column, container, row, scrollable, text};
use iced::{Border, Element, Length, Theme};
use signex_types::theme::{ThemeId, ThemeTokens};

use crate::app::{Message, OverlayMsg};
use crate::app::view::dialogs::{
    MODAL_HEADER_HEIGHT, MODAL_HEADER_PADDING, MODAL_HEADER_TITLE_SIZE, close_x_button,
};
use crate::keymap::{KeymapEditorModel, KeymapEditorRow, ShortcutProfileSet};
use crate::styles::{self, MODAL_CORNER_RADIUS};

const MODAL_W: f32 = 520.0;
const MODAL_H: f32 = 600.0;
const KEY_COL_W: f32 = 140.0;
const SECTION_HEADER_GAP_TOP: f32 = 14.0;

pub fn view<'a>(
    tokens: &'a ThemeTokens,
    theme_id: ThemeId,
    profiles: &ShortcutProfileSet,
) -> Element<'a, Message> {
    let text_primary = styles::ti(tokens.text);
    let text_secondary = styles::ti(tokens.text_secondary);
    let border = styles::ti(tokens.border);
    // Row dividers should be barely visible — half the alpha of the
    // border colour. Subtle horizontal rhythm without noise.
    let row_divider_color = iced::Color {
        a: border.a * 0.35,
        ..border
    };
    let section_divider_color = iced::Color {
        a: border.a * 0.7,
        ..border
    };

    // ── Header — canonical modal chrome ─────────────────────────
    let header = container(
        row![
            text("Keyboard Shortcuts")
                .size(MODAL_HEADER_TITLE_SIZE)
                .color(text_primary),
            Space::new().width(Length::Fill),
            close_x_button(
                Message::Overlay(OverlayMsg::CloseKeyboardShortcuts),
                theme_id,
                text_secondary,
            ),
        ]
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .height(MODAL_HEADER_HEIGHT)
    .padding(MODAL_HEADER_PADDING)
    .style(styles::modal_header_strip(tokens));

    // ── Body — sections per category ────────────────────────────
    // Column header (Action | Shortcut). Action goes on the LEFT
    // because it's what users scan for; Shortcut on the right.
    let column_header = container(
        row![
            text("Action").size(10).color(text_secondary),
            Space::new().width(Length::Fill),
            text("Shortcut")
                .size(10)
                .color(text_secondary)
                .width(Length::Fixed(KEY_COL_W)),
        ]
        .padding([0, 0]),
    )
    .padding([6, 0]);

    let editor = KeymapEditorModel::new(profiles.clone());
    let rows = editor.rows();
    let mut categories = Vec::<String>::new();
    for row in &rows {
        if !categories.iter().any(|category| category == &row.category) {
            categories.push(row.category.clone());
        }
    }

    let mut body = Column::new().spacing(0).push(column_header);
    body = body.push(thin_divider(border));

    let mut first_section = true;
    for category in categories {
        let section_shortcuts: Vec<&KeymapEditorRow> = rows
            .iter()
            .filter(|row| row.category == category && !row.trigger.trim().is_empty())
            .collect();

        if section_shortcuts.is_empty() {
            continue;
        }

        // Subtle section divider above every section after the first.
        if !first_section {
            body = body.push(thin_divider(section_divider_color));
        }
        first_section = false;

        body = body.push(
            container(
                text(title_case(&category).to_uppercase())
                    .size(9)
                    .color(text_secondary),
            )
            .padding(iced::Padding {
                top: SECTION_HEADER_GAP_TOP,
                right: 0.0,
                bottom: 4.0,
                left: 0.0,
            }),
        );

        let last_idx = section_shortcuts.len().saturating_sub(1);
        let mut section_col = Column::new().spacing(0);
        for (idx, s) in section_shortcuts.iter().enumerate() {
            let row_el: Element<'a, Message> = row![
                text(s.label.clone()).size(11).color(text_primary),
                Space::new().width(Length::Fill),
                text(s.trigger.clone())
                    .size(11)
                    .color(text_primary)
                    .width(Length::Fixed(KEY_COL_W)),
            ]
            .padding([3, 0])
            .into();
            section_col = section_col.push(row_el);
            // Subtle row divider between rows (not after the last row
            // in a section — section divider handles that gap).
            if idx < last_idx {
                section_col = section_col.push(thin_divider(row_divider_color));
            }
        }
        body = body.push(section_col);
    }

    let body_container = container(scrollable(body).height(Length::Fill))
        .padding([10, 16])
        .width(Length::Fill)
        .height(Length::Fill);

    // ── Card ────────────────────────────────────────────────────
    let card = container(column![header, body_container].spacing(0))
        .width(Length::Fixed(MODAL_W))
        .height(Length::Fixed(MODAL_H))
        .clip(true)
        .style(styles::modal_card(tokens));

    container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}

fn title_case(value: &str) -> String {
    value
        .split(['_', '-', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn thin_divider<'a, M: 'a>(color: iced::Color) -> Element<'a, M> {
    container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(move |_: &Theme| container::Style {
            background: Some(iced::Background::Color(color)),
            border: Border::default(),
            ..container::Style::default()
        })
        .into()
}

// `MODAL_CORNER_RADIUS` is currently consumed implicitly through
// `styles::modal_card`. The const re-export keeps the reference
// available for any future direct use (custom footer, etc.) without
// forcing a re-import.
#[allow(dead_code)]
const _MODAL_CORNER_RADIUS_REEXPORT: f32 = MODAL_CORNER_RADIUS;
