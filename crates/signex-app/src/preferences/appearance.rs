//! Appearance section — theme picker cards, custom-theme import/export,
//! UI font, and the schematic / symbol-editor render-style pick-lists.
//!
//! Moved verbatim from the former single-file `preferences` module —
//! pure view code, zero behaviour change.

use super::*;
use crate::fonts;
use crate::render_config::{GridStyle, LabelStyle, MultisheetStyle, PinSelectionMode, PowerPortStyle};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Background, Border, Element, Length, Theme};
use signex_types::theme::ThemeId;

pub(super) fn content_appearance<'a>(
    draft_theme: ThemeId,
    _saved_theme: ThemeId,
    draft_font: &str,
    draft_power_port_style: PowerPortStyle,
    draft_label_style: LabelStyle,
    draft_multisheet_style: MultisheetStyle,
    draft_grid_style: GridStyle,
    draft_symbol_grid_size_mm: f32,
    draft_symbol_grid_style: GridStyle,
    draft_symbol_pin_selection: PinSelectionMode,
    custom_name: Option<&'a str>,
) -> Element<'a, PrefMsg> {
    let mut col = column![].spacing(0).padding([16, 20]);

    // ── Section: Theme ──
    col = col.push(section_title("Theme"));
    col = col.push(Space::new().height(10));

    // Built-in theme data: (id, display name, description)
    let builtins: &[(ThemeId, &str, &str)] = &[
        (
            ThemeId::Signex,
            "Signex",
            "Default Signex schematic palette",
        ),
        (
            ThemeId::Alplab,
            "Alp Lab",
            "Alp Lab brand cyan accent on the Signex chrome",
        ),
        (
            ThemeId::VsCodeDark,
            "VS Code Dark",
            "VS Code inspired dark theme",
        ),
        (
            ThemeId::CatppuccinMocha,
            "Catppuccin Mocha",
            "Warm soft dark with pastels",
        ),
        (
            ThemeId::GitHubDark,
            "GitHub Dark",
            "GitHub's dark mode colors",
        ),
        (
            ThemeId::SolarizedLight,
            "Solarized Light",
            "Warm light tone-on-tone",
        ),
        (ThemeId::Nord, "Nord", "Arctic blue cool dark theme"),
    ];

    // Build list of all theme entries including optional custom
    let mut entries: Vec<(ThemeId, String, &'static str)> = builtins
        .iter()
        .map(|&(id, name, desc)| (id, name.to_string(), desc))
        .collect();
    if let Some(name) = custom_name {
        entries.push((
            ThemeId::Custom,
            format!("\u{2728} {name}"),
            "Custom imported theme",
        ));
    }

    // Rows of 2
    let mut i = 0;
    while i < entries.len() {
        let (id_a, ref name_a, desc_a) = entries[i];
        let card_a = theme_card(id_a, name_a, desc_a, draft_theme);
        let row_elem: Element<'_, PrefMsg> = if i + 1 < entries.len() {
            let (id_b, ref name_b, desc_b) = entries[i + 1];
            let card_b = theme_card(id_b, name_b, desc_b, draft_theme);
            row![card_a, Space::new().width(12), card_b]
                .width(Length::Fill)
                .into()
        } else {
            row![card_a, Space::new().width(Length::Fill)]
                .width(Length::Fill)
                .into()
        };
        col = col.push(row_elem);
        col = col.push(Space::new().height(10));
        i += 2;
    }

    // ── Custom theme import/export ──
    col = col.push(Space::new().height(4));
    col = col.push(
        row![
            import_btn(),
            Space::new().width(8),
            export_btn(),
            Space::new().width(Length::Fill),
        ]
        .align_y(iced::Alignment::Center),
    );

    // ── Divider ──
    col = col.push(Space::new().height(16));
    col = col.push(h_sep());
    col = col.push(Space::new().height(16));

    // ── Section: UI Font ──
    col = col.push(section_title("Font"));
    col = col.push(Space::new().height(10));
    col = col.push(
        row![
            column![
                text("UI Font").size(12).style(text_primary),
                text("Applies to all panels and menus. Requires restart.")
                    .size(10)
                    .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            {
                let families = fonts::system_font_families();
                let current_owned = draft_font.to_string();
                iced::widget::pick_list(
                    families.as_slice(),
                    Some(current_owned),
                    PrefMsg::DraftFont,
                )
                .text_size(12)
                .width(200)
            },
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(Space::new().height(20));

    // ── Section: Schematic Editor ──
    col = col.push(h_sep());
    col = col.push(Space::new().height(16));
    col = col.push(section_title("Schematic Editor"));
    col = col.push(Space::new().height(10));
    col = col.push(
        row![
            column![
                text("Grid Style").size(12).style(text_primary),
                text("Appearance of snap points on the schematic canvas.")
                    .size(10)
                    .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                GridStyle::ALL,
                Some(draft_grid_style),
                PrefMsg::DraftGridStyle,
            )
            .text_size(12)
            .width(200),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(Space::new().height(20));

    // ── Section: Power Port Symbols ──
    col = col.push(section_title("Power Ports"));
    col = col.push(Space::new().height(10));
    col = col.push(
        row![
            column![
                text("Power Port Style").size(12).style(text_primary),
                text("Choose how power symbols are rendered on canvas.")
                    .size(10)
                    .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                [PowerPortStyle::Altium, PowerPortStyle::Standard],
                Some(draft_power_port_style),
                PrefMsg::DraftPowerPortStyle,
            )
            .text_size(12)
            .width(200),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(Space::new().height(12));
    col = col.push(
        text("Restyled mode reshapes pin labels for rendering only. Standard preserves the library symbol's authored appearance.")
            .size(10)
            .style(text_muted),
    );
    col = col.push(Space::new().height(16));
    col = col.push(
        row![
            column![
                text("Global/Hier Label Style").size(12).style(text_primary),
                text("Controls multi-sheet and global label appearance.")
                    .size(10)
                    .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                [LabelStyle::Standard, LabelStyle::Altium],
                Some(draft_label_style),
                PrefMsg::DraftLabelStyle,
            )
            .text_size(12)
            .width(200),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(Space::new().height(16));
    col = col.push(
        row![
            column![
                text("Multisheet Style").size(12).style(text_primary),
                text(
                    "Controls hierarchical sheet body fill defaults. \
                     Per-sheet colours from the source file always win."
                )
                .size(10)
                .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                [MultisheetStyle::Standard, MultisheetStyle::Altium],
                Some(draft_multisheet_style),
                PrefMsg::DraftMultisheetStyle,
            )
            .text_size(12)
            .width(200),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(Space::new().height(20));

    // ── Section: Symbol Editor ──
    col = col.push(h_sep());
    col = col.push(Space::new().height(16));
    col = col.push(section_title("Symbol Editor"));
    col = col.push(Space::new().height(10));
    col = col.push(
        row![
            column![
                text("Default Grid Size").size(12).style(text_primary),
                text(
                    "Applied when a symbol library is first opened. \
                      Can be changed per-library from the canvas status bar."
                )
                .size(10)
                .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                crate::canvas::grid::GRID_SIZE_LABELS,
                crate::canvas::grid::GRID_SIZES_MM
                    .iter()
                    .zip(crate::canvas::grid::GRID_SIZE_LABELS.iter())
                    .find(|(sz, _)| (**sz - draft_symbol_grid_size_mm).abs() < 0.001)
                    .map(|(_, lbl)| *lbl),
                |lbl: &'static str| {
                    let mm = crate::canvas::grid::GRID_SIZES_MM
                        .iter()
                        .zip(crate::canvas::grid::GRID_SIZE_LABELS.iter())
                        .find(|(_, l)| **l == lbl)
                        .map(|(sz, _)| *sz)
                        .unwrap_or(1.27);
                    PrefMsg::DraftSymbolGridSize(mm)
                },
            )
            .text_size(12)
            .width(200),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(Space::new().height(16));
    col = col.push(
        row![
            column![
                text("Grid Style").size(12).style(text_primary),
                text("Appearance of snap points on the symbol editor canvas.")
                    .size(10)
                    .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                GridStyle::ALL,
                Some(draft_symbol_grid_style),
                PrefMsg::DraftSymbolGridStyle,
            )
            .text_size(12)
            .width(200),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(Space::new().height(16));
    col = col.push(
        row![
            column![
                text("Pin Selection").size(12).style(text_primary),
                text("How a click selects a pin on the symbol editor canvas — the pin body only, or its name and number labels too.")
                    .size(10)
                    .style(text_muted),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                PinSelectionMode::ALL,
                Some(draft_symbol_pin_selection),
                PrefMsg::DraftSymbolPinSelection,
            )
            .text_size(12)
            .width(200),
        ]
        .align_y(iced::Alignment::Center),
    );
    col = col.push(Space::new().height(20));

    col.into()
}

fn theme_card<'a>(
    id: ThemeId,
    name: &str,
    desc: &'static str,
    current: ThemeId,
) -> Element<'a, PrefMsg> {
    let is_active = id == current;
    let label = format!("{}{name}", if is_active { "✓ " } else { "" });
    let msg = PrefMsg::DraftTheme(id);

    // Card title / description resolve their colour against the card's own
    // (active vs inactive) background so they stay legible when the theme
    // flips light — active cards sit on a primary tint, inactive cards on
    // the weak background surface.
    let title_style = move |theme: &Theme| text::Style {
        color: Some(if is_active {
            theme.extended_palette().primary.weak.text
        } else {
            theme.extended_palette().background.base.text
        }),
    };
    let desc_style = move |theme: &Theme| text::Style {
        color: Some(if is_active {
            theme.extended_palette().primary.weak.text
        } else {
            theme.extended_palette().secondary.base.color
        }),
    };

    button(
        container(
            column![
                text(label).size(12).style(title_style),
                text(desc)
                    .size(10)
                    .style(desc_style)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .spacing(4),
        )
        .padding([10, 12])
        .width(Length::Fill),
    )
    .padding(0)
    .width(Length::Fill)
    .on_press(msg)
    .style(move |theme: &Theme, status: button::Status| {
        let palette = theme.extended_palette();
        let bg = if is_active {
            palette.primary.weak.color
        } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
            palette.background.strong.color
        } else {
            palette.background.weak.color
        };
        button::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                width: 1.0,
                radius: 4.0.into(),
                color: if is_active {
                    palette.primary.base.color
                } else {
                    palette.background.strong.color
                },
            },
            ..button::Style::default()
        }
    })
    .into()
}

fn import_btn<'a>() -> Element<'a, PrefMsg> {
    button(text("⬆ Import Theme…").size(11))
        .padding([5, 12])
        .on_press(PrefMsg::ImportTheme)
        .style(primary_button_style)
        .into()
}

fn export_btn<'a>() -> Element<'a, PrefMsg> {
    button(text("⬇ Export Theme…").size(11))
        .padding([5, 12])
        .on_press(PrefMsg::ExportTheme)
        .style(primary_button_style)
        .into()
}
