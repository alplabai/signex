//! Preferences dialog — Altium-style modal with left navigation + right content.
//!
//! Opened via Tools > Preferences (or keyboard shortcut).
//! Left side: tree of settings categories.
//! Right side: settings panel for the selected category.

use iced::widget::{button, column, container, row, scrollable, text, Space};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_types::theme::ThemeId;

use crate::fonts;

// ─── Navigation Items ─────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefNav {
    Appearance,
    // Future: Editor, Shortcuts, ...
}

impl PrefNav {
    pub const ALL: &'static [PrefNav] = &[PrefNav::Appearance];

    pub fn label(self) -> &'static str {
        match self {
            PrefNav::Appearance => "Appearance",
        }
    }

    pub fn group(self) -> &'static str {
        match self {
            PrefNav::Appearance => "System",
        }
    }
}

// ─── Messages ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum PrefMsg {
    /// Navigate to a category.
    Nav(PrefNav),
    /// Close the dialog.
    Close,
    /// Change the application theme.
    SetTheme(ThemeId),
    /// Change the UI font (restart to apply).
    SetUiFont(String),
}

// ─── Dialog sizes ─────────────────────────────────────────────

const DLG_W: f32 = 760.0;
const DLG_H: f32 = 520.0;
const NAV_W: f32 = 190.0;
const HDR_H: f32 = 40.0;
const FOOTER_H: f32 = 44.0;

// ─── Colors ───────────────────────────────────────────────────

const DLG_BG: Color = Color::from_rgb(0.13, 0.13, 0.15);
const NAV_BG: Color = Color::from_rgb(0.10, 0.10, 0.12);
const CONTENT_BG: Color = Color::from_rgb(0.15, 0.15, 0.17);
const HDR_BG: Color = Color::from_rgb(0.11, 0.11, 0.13);
const ROW_ACTIVE: Color = Color::from_rgb(0.17, 0.30, 0.50);
const ROW_HOVER: Color = Color::from_rgb(0.18, 0.18, 0.21);
const SEP: Color = Color::from_rgb(0.22, 0.22, 0.25);
const TEXT_PRI: Color = Color::from_rgb(0.90, 0.90, 0.92);
const TEXT_MUT: Color = Color::from_rgb(0.50, 0.50, 0.55);

// ─── Public view ──────────────────────────────────────────────

/// Build the full-screen backdrop + centred dialog.
pub fn view<'a>(
    nav: PrefNav,
    theme_id: ThemeId,
    ui_font_name: &str,
) -> Element<'a, PrefMsg> {
    let dialog = build_dialog(nav, theme_id, ui_font_name);

    container(
        column![
            Space::new().height(Length::Fill),
            row![
                Space::new().width(Length::Fill),
                dialog,
                Space::new().width(Length::Fill),
            ],
            Space::new().height(Length::Fill),
        ]
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_: &Theme| container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.65))),
        ..container::Style::default()
    })
    .into()
}

// ─── Dialog shell ─────────────────────────────────────────────

fn build_dialog<'a>(nav: PrefNav, theme_id: ThemeId, ui_font_name: &str) -> Element<'a, PrefMsg> {
    // ── Header ──
    let header = container(
        row![
            text("Preferences")
                .size(14)
                .color(TEXT_PRI),
            Space::new().width(Length::Fill),
            close_btn(),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(8),
    )
    .width(Length::Fill)
    .height(HDR_H)
    .padding([0, 14])
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(HDR_BG)),
        border: Border {
            width: 0.0,
            ..Border::default()
        },
        ..container::Style::default()
    });

    // ── Body: nav | divider | content ──
    let body = row![
        build_nav(nav),
        // Vertical divider
        container(Space::new())
            .width(1)
            .height(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(SEP)),
                ..container::Style::default()
            }),
        build_content(nav, theme_id, ui_font_name),
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    // ── Footer ──
    let footer = container(
        row![
            Space::new().width(Length::Fill),
            close_footer_btn(),
        ]
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .height(FOOTER_H)
    .padding([0, 14])
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(HDR_BG)),
        border: Border {
            width: 1.0,
            color: SEP,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    });

    // ── Assemble ──
    container(
        column![
            header,
            // Horizontal divider under header
            container(Space::new())
                .width(Length::Fill)
                .height(1)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(SEP)),
                    ..container::Style::default()
                }),
            body,
            // Horizontal divider above footer
            container(Space::new())
                .width(Length::Fill)
                .height(1)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(SEP)),
                    ..container::Style::default()
                }),
            footer,
        ]
        .spacing(0),
    )
    .width(DLG_W)
    .height(DLG_H)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(DLG_BG)),
        border: Border {
            width: 1.0,
            radius: 6.0.into(),
            color: SEP,
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.7),
            offset: iced::Vector::new(0.0, 8.0),
            blur_radius: 24.0,
        },
        ..container::Style::default()
    })
    .into()
}

// ─── Left navigation ──────────────────────────────────────────

fn build_nav<'a>(active: PrefNav) -> Element<'a, PrefMsg> {
    let mut col = column![].spacing(0).width(NAV_W);

    // Group headers + items
    let mut last_group = "";
    for &item in PrefNav::ALL {
        let group = item.group();
        if group != last_group {
            last_group = group;
            col = col.push(
                container(
                    text(group.to_uppercase())
                        .size(9)
                        .color(TEXT_MUT),
                )
                .padding(iced::Padding { top: 10.0, right: 12.0, bottom: 4.0, left: 12.0 })
                .width(Length::Fill),
            );
        }
        col = col.push(nav_item(item, active));
    }

    container(
        scrollable(col).width(Length::Fill),
    )
    .width(NAV_W)
    .height(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(NAV_BG)),
        ..container::Style::default()
    })
    .into()
}

fn nav_item<'a>(item: PrefNav, active: PrefNav) -> Element<'a, PrefMsg> {
    let is_active = item == active;
    let bg = if is_active { Some(Background::Color(ROW_ACTIVE)) } else { None };
    let tc = if is_active { Color::WHITE } else { TEXT_PRI };

    button(
        container(
            row![
                text(item.label())
                    .size(12)
                    .color(tc),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([6, 12])
        .width(Length::Fill),
    )
    .padding(0)
    .width(Length::Fill)
    .on_press(PrefMsg::Nav(item))
    .style(move |_: &Theme, status: button::Status| {
        let bg = match (is_active, status) {
            (true, _) => Some(Background::Color(ROW_ACTIVE)),
            (false, button::Status::Hovered) => Some(Background::Color(ROW_HOVER)),
            _ => bg,
        };
        button::Style {
            background: bg,
            border: Border::default(),
            text_color: tc,
            ..button::Style::default()
        }
    })
    .into()
}

// ─── Right content ────────────────────────────────────────────

fn build_content<'a>(nav: PrefNav, theme_id: ThemeId, ui_font_name: &str) -> Element<'a, PrefMsg> {
    let inner = match nav {
        PrefNav::Appearance => content_appearance(theme_id, ui_font_name),
    };

    container(scrollable(inner).width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(0)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(CONTENT_BG)),
            ..container::Style::default()
        })
        .into()
}

// ─── Appearance page ──────────────────────────────────────────

fn content_appearance<'a>(current_theme: ThemeId, ui_font_name: &str) -> Element<'a, PrefMsg> {
    let mut col = column![].spacing(0).padding([16, 20]);

    // ── Section: Theme ──
    col = col.push(section_title("Theme"));
    col = col.push(Space::new().height(10));

    // 2-column grid of theme cards
    let themes = [
        (ThemeId::AltiumDark,      "Altium Dark",     "Default Altium dark palette"),
        (ThemeId::VsCodeDark,      "VS Code Dark",    "VS Code inspired dark theme"),
        (ThemeId::CatppuccinMocha, "Catppuccin Mocha","Warm soft dark with pastels"),
        (ThemeId::GitHubDark,      "GitHub Dark",     "GitHub's dark mode colors"),
        (ThemeId::SolarizedLight,  "Solarized Light", "Warm light tone-on-tone"),
        (ThemeId::Nord,            "Nord",            "Arctic blue cool dark theme"),
    ];

    // Rows of 2
    let mut i = 0;
    while i < themes.len() {
        let (id_a, name_a, desc_a) = themes[i];
        let card_a = theme_card(id_a, name_a, desc_a, current_theme);
        let row_elem: Element<'_, PrefMsg> = if i + 1 < themes.len() {
            let (id_b, name_b, desc_b) = themes[i + 1];
            let card_b = theme_card(id_b, name_b, desc_b, current_theme);
            row![card_a, Space::new().width(12), card_b]
                .width(Length::Fill)
                .into()
        } else {
            row![card_a].width(Length::Fill).into()
        };
        col = col.push(row_elem);
        col = col.push(Space::new().height(10));
        i += 2;
    }

    // ── Divider ──
    col = col.push(h_sep());
    col = col.push(Space::new().height(16));

    // ── Section: UI Font ──
    col = col.push(section_title("Font"));
    col = col.push(Space::new().height(10));
    col = col.push(
        row![
            column![
                text("UI Font").size(12).color(TEXT_PRI),
                text("Applies to all panels and menus. Requires restart.")
                    .size(10)
                    .color(TEXT_MUT),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            {
                let families = fonts::system_font_families();
                let current_owned = ui_font_name.to_string();
                iced::widget::pick_list(
                    families.as_slice(),
                    Some(current_owned),
                    PrefMsg::SetUiFont,
                )
                .text_size(12)
                .width(200)
            },
        ]
        .align_y(iced::Alignment::Center),
    );

    col.into()
}

// ─── Widget helpers ───────────────────────────────────────────

fn section_title<'a>(title: &str) -> Element<'a, PrefMsg> {
    column![
        text(title.to_owned()).size(13).color(TEXT_PRI),
        container(Space::new())
            .width(Length::Fill)
            .height(1)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(SEP)),
                ..container::Style::default()
            }),
    ]
    .spacing(6)
    .into()
}

fn h_sep<'a>() -> Element<'a, PrefMsg> {
    container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(SEP)),
            ..container::Style::default()
        })
        .into()
}

fn theme_card<'a>(
    id: ThemeId,
    name: &'static str,
    desc: &'static str,
    current: ThemeId,
) -> Element<'a, PrefMsg> {
    let is_active = id == current;
    let border_c = if is_active { Color::from_rgb(0.30, 0.55, 0.90) } else { SEP };
    let check = if is_active { "✓ " } else { "" };

    let card_bg = if is_active {
        Color::from_rgb(0.13, 0.21, 0.35)
    } else {
        Color::from_rgb(0.17, 0.17, 0.20)
    };
    let hover_bg = Color::from_rgb(0.20, 0.20, 0.24);

    button(
        container(
            column![
                text(format!("{check}{name}"))
                    .size(12)
                    .color(if is_active { Color::WHITE } else { TEXT_PRI }),
                text(desc)
                    .size(10)
                    .color(TEXT_MUT)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .spacing(4),
        )
        .padding([10, 12])
        .width(Length::Fill),
    )
    .padding(0)
    .width(Length::Fill)
    .on_press(PrefMsg::SetTheme(id))
    .style(move |_: &Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered | button::Status::Pressed => {
                Background::Color(hover_bg)
            }
            _ => Background::Color(card_bg),
        };
        button::Style {
            background: Some(bg),
            border: Border {
                width: 1.0,
                radius: 4.0.into(),
                color: border_c,
            },
            ..button::Style::default()
        }
    })
    .into()
}

fn close_btn<'a>() -> Element<'a, PrefMsg> {
    button(
        text("✕").size(14).color(TEXT_MUT),
    )
    .padding([2, 8])
    .on_press(PrefMsg::Close)
    .style(move |_: &Theme, status: button::Status| {
        let tc = match status {
            button::Status::Hovered => Color::WHITE,
            _ => TEXT_MUT,
        };
        button::Style {
            text_color: tc,
            ..button::Style::default()
        }
    })
    .into()
}

fn close_footer_btn<'a>() -> Element<'a, PrefMsg> {
    button(
        text("Close").size(12).color(Color::WHITE),
    )
    .padding([6, 20])
    .on_press(PrefMsg::Close)
    .style(|_: &Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered => Color::from_rgb(0.28, 0.42, 0.65),
            _ => Color::from_rgb(0.22, 0.36, 0.58),
        };
        button::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                radius: 3.0.into(),
                ..Border::default()
            },
            text_color: Color::WHITE,
            ..button::Style::default()
        }
    })
    .into()
}
