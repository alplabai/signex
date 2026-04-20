//! Preferences dialog — Altium-style modal with left navigation + right content.
//!
//! Opened via Tools > Preferences (or keyboard shortcut).
//! Left side: tree of settings categories.
//! Right side: settings panel for the selected category.

use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_render::PowerPortStyle;
use signex_types::theme::ThemeId;

use crate::fonts;

// ─── Navigation Items ─────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefNav {
    Appearance,
    /// Electrical Rule Check — per-rule severity override.
    Erc,
    // Future: Editor, Shortcuts, ...
}

impl PrefNav {
    pub const ALL: &'static [PrefNav] = &[PrefNav::Appearance, PrefNav::Erc];

    pub fn label(self) -> &'static str {
        match self {
            PrefNav::Appearance => "Appearance",
            PrefNav::Erc => "Electrical Rules",
        }
    }

    pub fn group(self) -> &'static str {
        match self {
            PrefNav::Appearance => "System",
            PrefNav::Erc => "Validation",
        }
    }
}

// ─── Messages ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum PrefMsg {
    /// Navigate to a category.
    Nav(PrefNav),
    /// Close without saving (only if not dirty; app ignores if dirty).
    Close,
    /// Discard all unsaved changes and close.
    DiscardAndClose,
    /// Commit the current draft and keep the dialog open.
    Save,
    /// Update the draft theme (not applied until Save).
    DraftTheme(ThemeId),
    /// Update the draft UI font (not applied until Save).
    DraftFont(String),
    /// Update draft power port drawing style (applies as live preview).
    DraftPowerPortStyle(PowerPortStyle),
    /// Open a file picker to import a custom theme JSON.
    ImportTheme,
    /// Save the current draft theme as a JSON file.
    ExportTheme,
    /// Loaded JSON content from an import pick dialog.
    ThemeFileLoaded(String),
    /// Set the severity override for an ERC rule. Setting the override
    /// to the default value clears the entry instead.
    DraftErcSeverity(signex_erc::RuleKind, signex_erc::Severity),
    /// Clear every ERC severity override — reset to defaults.
    ResetErcSeverities,
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
const WARN_YELLOW: Color = Color::from_rgb(0.95, 0.72, 0.15);
const BTN_IMPORT: Color = Color::from_rgb(0.18, 0.26, 0.40);
const BTN_IMPORT_HOV: Color = Color::from_rgb(0.24, 0.34, 0.52);
const BTN_DANGER: Color = Color::from_rgb(0.38, 0.16, 0.16);
const BTN_DANGER_HOV: Color = Color::from_rgb(0.50, 0.22, 0.22);

// ─── Public view ──────────────────────────────────────────────

/// Build the full-screen backdrop + centred dialog.
///
/// * `draft_theme`      — theme currently selected in the dialog (not yet saved)
/// * `saved_theme`      — the committed theme (used to detect unsaved changes)
/// * `draft_font`       — UI font name pending save
/// * `custom_name`      — name of the loaded custom theme (if any)
/// * `dirty`            — whether there are unsaved changes
#[allow(clippy::too_many_arguments)]
pub fn view<'a>(
    nav: PrefNav,
    draft_theme: ThemeId,
    saved_theme: ThemeId,
    draft_font: &str,
    draft_power_port_style: PowerPortStyle,
    custom_name: Option<&'a str>,
    dirty: bool,
    erc_overrides: &'a std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
) -> Element<'a, PrefMsg> {
    let dialog = build_dialog(
        nav,
        draft_theme,
        saved_theme,
        draft_font,
        draft_power_port_style,
        custom_name,
        dirty,
        erc_overrides,
    );

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

#[allow(clippy::too_many_arguments)]
fn build_dialog<'a>(
    nav: PrefNav,
    draft_theme: ThemeId,
    saved_theme: ThemeId,
    draft_font: &str,
    draft_power_port_style: PowerPortStyle,
    custom_name: Option<&'a str>,
    dirty: bool,
    erc_overrides: &'a std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
) -> Element<'a, PrefMsg> {
    // ── Header ──
    let header = container(
        row![
            text("Preferences").size(14).color(TEXT_PRI),
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
        build_content(
            nav,
            draft_theme,
            saved_theme,
            draft_font,
            draft_power_port_style,
            custom_name,
            erc_overrides,
        ),
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    // ── Footer ──
    let footer = build_footer(dirty);

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
                container(text(group.to_uppercase()).size(9).color(TEXT_MUT))
                    .padding(iced::Padding {
                        top: 10.0,
                        right: 12.0,
                        bottom: 4.0,
                        left: 12.0,
                    })
                    .width(Length::Fill),
            );
        }
        col = col.push(nav_item(item, active));
    }

    container(scrollable(col).width(Length::Fill))
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
    let bg = if is_active {
        Some(Background::Color(ROW_ACTIVE))
    } else {
        None
    };
    let tc = if is_active { Color::WHITE } else { TEXT_PRI };

    button(
        container(row![text(item.label()).size(12).color(tc),].align_y(iced::Alignment::Center))
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

fn build_content<'a>(
    nav: PrefNav,
    draft_theme: ThemeId,
    saved_theme: ThemeId,
    draft_font: &str,
    draft_power_port_style: PowerPortStyle,
    custom_name: Option<&'a str>,
    erc_overrides: &'a std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
) -> Element<'a, PrefMsg> {
    let inner = match nav {
        PrefNav::Appearance => content_appearance(
            draft_theme,
            saved_theme,
            draft_font,
            draft_power_port_style,
            custom_name,
        ),
        PrefNav::Erc => content_erc(erc_overrides),
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

fn content_appearance<'a>(
    draft_theme: ThemeId,
    _saved_theme: ThemeId,
    draft_font: &str,
    draft_power_port_style: PowerPortStyle,
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

    // ── Section: Power Port Symbols ──
    col = col.push(section_title("Power Ports"));
    col = col.push(Space::new().height(10));
    col = col.push(
        row![
            column![
                text("Power Port Style").size(12).color(TEXT_PRI),
                text("Choose how power symbols are rendered on canvas.")
                    .size(10)
                    .color(TEXT_MUT),
            ]
            .spacing(3)
            .width(200),
            Space::new().width(Length::Fill),
            iced::widget::pick_list(
                [PowerPortStyle::Altium, PowerPortStyle::KiCad],
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
        text("Altium changes only rendering view. KiCad preserves library symbol appearance.")
            .size(10)
            .color(TEXT_MUT),
    );
    col = col.push(Space::new().height(20));

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
    name: &str,
    desc: &'static str,
    current: ThemeId,
) -> Element<'a, PrefMsg> {
    let is_active = id == current;
    let border_c = if is_active {
        Color::from_rgb(0.30, 0.55, 0.90)
    } else {
        SEP
    };
    let label = format!("{}{name}", if is_active { "✓ " } else { "" });
    let card_bg = if is_active {
        Color::from_rgb(0.13, 0.21, 0.35)
    } else {
        Color::from_rgb(0.17, 0.17, 0.20)
    };
    let text_color = if is_active { Color::WHITE } else { TEXT_PRI };
    let hover_bg = Color::from_rgb(0.20, 0.20, 0.24);
    let msg = PrefMsg::DraftTheme(id);

    button(
        container(
            column![
                text(label).size(12).color(text_color),
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
    .on_press(msg)
    .style(move |_: &Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered | button::Status::Pressed => Background::Color(hover_bg),
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
    button(text("✕").size(14).color(TEXT_MUT))
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

/// Dynamic footer: Save + Close (clean) or ⚠ + Discard + Save (dirty).
fn build_footer<'a>(dirty: bool) -> Element<'a, PrefMsg> {
    let footer_row: Element<'a, PrefMsg> = if dirty {
        row![
            text("● Unsaved changes").size(11).color(WARN_YELLOW),
            Space::new().width(Length::Fill),
            discard_btn(),
            Space::new().width(8),
            save_btn(),
        ]
        .align_y(iced::Alignment::Center)
        .into()
    } else {
        row![Space::new().width(Length::Fill), close_footer_btn(),]
            .align_y(iced::Alignment::Center)
            .into()
    };

    container(footer_row)
        .width(Length::Fill)
        .height(FOOTER_H)
        .padding([0, 16])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(HDR_BG)),
            border: Border {
                width: 1.0,
                color: SEP,
                radius: 0.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn save_btn<'a>() -> Element<'a, PrefMsg> {
    button(text("Save").size(12).color(Color::WHITE))
        .padding([6, 20])
        .on_press(PrefMsg::Save)
        .style(|_: &Theme, status: button::Status| {
            let bg = match status {
                button::Status::Hovered => Color::from_rgb(0.18, 0.52, 0.30),
                _ => Color::from_rgb(0.14, 0.42, 0.24),
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

fn discard_btn<'a>() -> Element<'a, PrefMsg> {
    button(
        text("Discard & Close")
            .size(12)
            .color(Color::from_rgb(0.85, 0.60, 0.60)),
    )
    .padding([6, 16])
    .on_press(PrefMsg::DiscardAndClose)
    .style(|_: &Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered => BTN_DANGER_HOV,
            _ => BTN_DANGER,
        };
        button::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                radius: 3.0.into(),
                ..Border::default()
            },
            text_color: Color::from_rgb(0.90, 0.70, 0.70),
            ..button::Style::default()
        }
    })
    .into()
}

fn close_footer_btn<'a>() -> Element<'a, PrefMsg> {
    button(text("Close").size(12).color(Color::WHITE))
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

fn import_btn<'a>() -> Element<'a, PrefMsg> {
    button(text("⬆ Import Theme…").size(11).color(TEXT_PRI))
        .padding([5, 12])
        .on_press(PrefMsg::ImportTheme)
        .style(|_: &Theme, status: button::Status| {
            let bg = match status {
                button::Status::Hovered => BTN_IMPORT_HOV,
                _ => BTN_IMPORT,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: SEP,
                },
                text_color: TEXT_PRI,
                ..button::Style::default()
            }
        })
        .into()
}

fn export_btn<'a>() -> Element<'a, PrefMsg> {
    button(text("⬇ Export Theme…").size(11).color(TEXT_MUT))
        .padding([5, 12])
        .on_press(PrefMsg::ExportTheme)
        .style(|_: &Theme, status: button::Status| {
            let bg = match status {
                button::Status::Hovered => BTN_IMPORT_HOV,
                _ => BTN_IMPORT,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: SEP,
                },
                text_color: TEXT_MUT,
                ..button::Style::default()
            }
        })
        .into()
}

// === ERC Severity content ===

fn content_erc<'a>(
    overrides: &'a std::collections::HashMap<signex_erc::RuleKind, signex_erc::Severity>,
) -> Element<'a, PrefMsg> {
    use signex_erc::{RuleKind, Severity};
    const RULES: &[RuleKind] = &[
        RuleKind::UnusedPin,
        RuleKind::DuplicateRefDesignator,
        RuleKind::HierPortDisconnected,
        RuleKind::DanglingWire,
        RuleKind::NetLabelConflict,
        RuleKind::OrphanLabel,
        RuleKind::BusBitWidthMismatch,
        RuleKind::BadHierSheetPin,
        RuleKind::MissingPowerFlag,
        RuleKind::PowerPortShort,
        RuleKind::SymbolOutsideSheet,
    ];
    const CHOICES: &[Severity] = &[
        Severity::Error,
        Severity::Warning,
        Severity::Info,
        Severity::Off,
    ];

    let header = column![
        text("Electrical Rules Severity").size(15).color(TEXT_PRI),
        Space::new().height(4),
        text("Per-rule severity override. Errors show red, Warnings yellow, Info blue; Off silences the rule entirely.")
            .size(11)
            .color(TEXT_MUT),
    ]
    .padding([16, 20]);

    let mut rows_col = column![].spacing(0).padding([0, 20]);
    for rule in RULES {
        let current = overrides
            .get(rule)
            .copied()
            .unwrap_or_else(|| rule.default_severity());
        let default_sev = rule.default_severity();
        let mut row_ui = row![
            text(rule.label())
                .size(12)
                .color(TEXT_PRI)
                .width(Length::Fill)
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center);
        for &sev in CHOICES {
            let active = current == sev;
            let r = *rule;
            let s = sev;
            let d = default_sev;
            row_ui = row_ui.push(
                button(text(severity_label(sev)).size(11).color(if active {
                    Color::from_rgb(1.0, 1.0, 1.0)
                } else {
                    TEXT_MUT
                }))
                .padding([4, 10])
                .on_press(PrefMsg::DraftErcSeverity(r, if s == d { d } else { s }))
                .style(move |_: &Theme, status: button::Status| {
                    let bg = if active {
                        severity_bg(sev)
                    } else if matches!(status, button::Status::Hovered) {
                        ROW_HOVER
                    } else {
                        NAV_BG
                    };
                    button::Style {
                        background: Some(Background::Color(bg)),
                        border: Border {
                            width: 1.0,
                            radius: 3.0.into(),
                            color: if active { severity_bg(sev) } else { SEP },
                        },
                        ..button::Style::default()
                    }
                }),
            );
        }
        rows_col = rows_col.push(container(row_ui).padding([6, 4]).width(Length::Fill).style(
            |_: &Theme| container::Style {
                background: None,
                border: Border {
                    width: 1.0,
                    color: SEP,
                    radius: 0.0.into(),
                },
                ..container::Style::default()
            },
        ));
    }

    let reset_row = container(
        row![
            Space::new().width(Length::Fill),
            button(text("Reset to defaults").size(11).color(TEXT_MUT))
                .padding([5, 12])
                .on_press(PrefMsg::ResetErcSeverities)
                .style(|_: &Theme, status: button::Status| {
                    let bg = match status {
                        button::Status::Hovered => BTN_IMPORT_HOV,
                        _ => BTN_IMPORT,
                    };
                    button::Style {
                        background: Some(Background::Color(bg)),
                        border: Border {
                            width: 1.0,
                            radius: 3.0.into(),
                            color: SEP,
                        },
                        text_color: TEXT_MUT,
                        ..button::Style::default()
                    }
                }),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([12, 20])
    .width(Length::Fill);

    column![header, rows_col, reset_row].spacing(0).into()
}

fn severity_label(sev: signex_erc::Severity) -> &'static str {
    match sev {
        signex_erc::Severity::Error => "Error",
        signex_erc::Severity::Warning => "Warning",
        signex_erc::Severity::Info => "Info",
        signex_erc::Severity::Off => "Off",
    }
}

fn severity_bg(sev: signex_erc::Severity) -> Color {
    match sev {
        signex_erc::Severity::Error => Color::from_rgb(0.58, 0.20, 0.22),
        signex_erc::Severity::Warning => Color::from_rgb(0.55, 0.45, 0.12),
        signex_erc::Severity::Info => Color::from_rgb(0.20, 0.36, 0.58),
        signex_erc::Severity::Off => Color::from_rgb(0.28, 0.28, 0.32),
    }
}
