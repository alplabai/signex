//! Chrome catalog — a visual sandbox for Signex chrome widgets.
//!
//! Run via `cargo run -p chrome-catalog` (dev profile, ~5–15 s
//! incremental rebuild). Mounts every chrome variant we care about
//! side-by-side so UI iteration doesn't require launching the full
//! app + opening a project + arranging panels just to see whether a
//! tab style looks right.
//!
//! What's in here today:
//!   - Document tabs (active / inactive / hovered / dragging,
//!     first / middle / last) on every shipped theme
//!   - Panel tabs (same widget — should look identical to doc tabs)
//!   - Modal card chrome (rounded corners, header strip, close X)
//!   - Project tree leaf row indicators (open / dirty / active)
//!
//! How to extend: add a new "section" — a `column!` of one row per
//! variant — and append it to `view`. Keep sections grouped by widget
//! family; theme switching applies globally.
//!
//! Workflow tip: pair this with the spec-first request convention.
//! When you want a chrome change, drop an annotated screenshot
//! (radius / colour / spacing in pixels). I implement here, you
//! check, then we promote into the app.

use std::sync::OnceLock;

use iced::widget::{Column, Row, Space, button, column, container, row, scrollable, svg, text};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_types::theme::{Color as TokColor, ThemeId, ThemeTokens, theme_tokens};
use signex_widgets::tab_pill::{AccentPosition, TabPill, TabPillStyle};

/// Minimal X-mark SVG matched to the chrome window-close icon so
/// the modal's X renders at exactly 14×14 instead of as a Unicode
/// character whose font metrics inflate it past 14 px.
const X_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><path d="M3 3l8 8M11 3l-8 8" stroke="currentColor" stroke-width="1.2" fill="none"/></svg>"#;

fn x_handle() -> svg::Handle {
    static H: OnceLock<svg::Handle> = OnceLock::new();
    H.get_or_init(|| svg::Handle::from_memory(X_SVG)).clone()
}

fn main() -> iced::Result {
    iced::application(Catalog::new, Catalog::update, Catalog::view)
        .title("Signex Chrome Catalog")
        .theme(|state: &Catalog| state.iced_theme())
        .window_size((1200.0, 900.0))
        .run()
}

struct Catalog {
    theme: ThemeId,
}

#[derive(Debug, Clone)]
enum Message {
    SelectTheme(ThemeId),
}

impl Catalog {
    fn new() -> Self {
        Self {
            theme: ThemeId::Signex,
        }
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::SelectTheme(id) => self.theme = id,
        }
    }

    fn iced_theme(&self) -> Theme {
        // Only the canvas colour palette in signex-types maps cleanly
        // to iced's built-in Themes. The catalog uses generic dark /
        // light surfaces; the chrome we render below pulls from
        // ThemeTokens directly so theme accuracy is on the chrome
        // surfaces, not the iced backdrop.
        match self.theme {
            ThemeId::SolarizedLight => Theme::Light,
            _ => Theme::Dark,
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let tokens = theme_tokens(self.theme);
        let panel_bg = ti(tokens.bg);

        let header = self.theme_picker(&tokens);
        let body = column![
            section(
                "Document tabs (3 visible, middle is active)",
                &tokens,
                doc_tab_strip(&tokens)
            ),
            section(
                "Document tabs — every state side-by-side",
                &tokens,
                tab_state_matrix(&tokens),
            ),
            section(
                "Panel tabs (same widget; should match doc tabs)",
                &tokens,
                panel_tab_strip(&tokens),
            ),
            section("Modal card chrome", &tokens, modal_card_demo(&tokens)),
            section(
                "Project tree leaf indicators",
                &tokens,
                tree_row_demo(&tokens),
            ),
            section(
                "BOM modal — Altium-style layout (table + properties sidebar)",
                &tokens,
                bom_modal_demo(&tokens),
            ),
        ]
        .spacing(20)
        .padding(20);

        // Body grew past the window when the BOM section landed —
        // 660 px tall + every other section above it overflowed
        // the catalog's 900 px window. Wrap in a scrollable so
        // every variant stays reachable at any window size.
        let scrollable_body = scrollable(body).width(Length::Fill).height(Length::Fill);
        container(column![header, scrollable_body].spacing(0))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(panel_bg)),
                ..container::Style::default()
            })
            .into()
    }

    fn theme_picker(&self, tokens: &ThemeTokens) -> Element<'_, Message> {
        let toolbar_bg = ti(tokens.toolbar_bg);
        let text_c = ti(tokens.text);
        let mut pills: Row<'_, Message> = Row::new().spacing(6);
        pills = pills.push(text("Theme:").size(11).color(text_c));
        for &id in ThemeId::BUILTINS {
            let is_on = id == self.theme;
            pills = pills.push(theme_pill(id, is_on, tokens));
        }
        container(pills.align_y(iced::Alignment::Center))
            .width(Length::Fill)
            .padding([10, 14])
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(toolbar_bg)),
                border: Border {
                    width: 0.0,
                    radius: 0.0.into(),
                    color: Color::TRANSPARENT,
                },
                ..container::Style::default()
            })
            .into()
    }
}

fn theme_pill<'a>(id: ThemeId, is_on: bool, tokens: &ThemeTokens) -> Element<'a, Message> {
    let accent = ti(tokens.accent);
    let text_c = ti(tokens.text);
    let border = ti(tokens.border);
    button(text(id.label().to_string()).size(11).color(text_c))
        .padding([4, 10])
        .on_press(Message::SelectTheme(id))
        .style(move |_: &Theme, status: button::Status| {
            let bg = match (is_on, status) {
                (true, _) => accent,
                (false, button::Status::Hovered | button::Status::Pressed) => {
                    Color { a: 0.18, ..accent }
                }
                _ => Color::from_rgba(1.0, 1.0, 1.0, 0.04),
            };
            button::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                text_color: text_c,
                ..button::Style::default()
            }
        })
        .into()
}

fn section<'a>(
    title: &'static str,
    tokens: &ThemeTokens,
    body: Element<'a, Message>,
) -> Element<'a, Message> {
    let text_c = ti(tokens.text);
    let muted = ti(tokens.text_secondary);
    let border = ti(tokens.border);
    let panel = ti(tokens.panel_bg);
    container(column![
        text(title).size(13).color(text_c),
        Space::new().height(8),
        container(body)
            .padding(16)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(panel)),
                border: Border {
                    width: 1.0,
                    radius: 4.0.into(),
                    color: border,
                },
                ..container::Style::default()
            }),
    ])
    .padding([0, 0])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        text_color: Some(muted),
        ..container::Style::default()
    })
    .into()
}

// ─── Tab demo helpers ─────────────────────────────────────────

fn doc_tab_strip<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let tabs_row = Row::new()
        .spacing(0)
        .push(tab(
            "MCU_IO",
            false,
            false,
            false,
            false,
            AccentPosition::Bottom,
            tokens,
        ))
        .push(tab(
            "Loratis-SN",
            true,
            false,
            false,
            false,
            AccentPosition::Bottom,
            tokens,
        ))
        .push(tab(
            "Power",
            false,
            false,
            false,
            true,
            AccentPosition::Bottom,
            tokens,
        ));
    strip_with_baseline(tabs_row, AccentPosition::Bottom, tokens)
}

fn tab_state_matrix<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    // Show every state combination on a single row so the user can
    // eyeball drag/hover/active styling without having to drive the
    // app through every gesture.
    let mut r: Row<'a, Message> = Row::new().spacing(0);
    for (label, active, dragging, hovered, last) in [
        ("Inactive", false, false, false, false),
        ("Hovered", false, false, true, false),
        ("Active", true, false, false, false),
        ("Dragging", false, true, false, false),
        ("Last", false, false, false, true),
    ] {
        r = r.push(tab(
            label,
            active,
            dragging,
            hovered,
            last,
            AccentPosition::Bottom,
            tokens,
        ));
    }
    strip_with_baseline(r, AccentPosition::Bottom, tokens)
}

fn panel_tab_strip<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let mut r: Row<'a, Message> = Row::new().spacing(0);
    for (i, label) in [
        "Components",
        "Manufacturer Part Search",
        "PCB CoDesign",
        "Messages",
        "Properties",
    ]
    .iter()
    .enumerate()
    {
        let active = i == 0;
        let last = i == 4;
        r = r.push(tab(
            label,
            active,
            false,
            false,
            last,
            AccentPosition::Top,
            tokens,
        ));
    }
    strip_with_baseline(r, AccentPosition::Top, tokens)
}

/// Wrap a row of tabs in the same toolbar bg + 1 px theme-border
/// baseline the real app uses (`tab_bar::view`). The baseline sits
/// at the OPPOSITE edge from the rounded corners — bottom for
/// `Bottom`-accented doc tabs, top for `Top`-accented panel tabs —
/// so the active tab's accent stripe is always overlaying the
/// strip's baseline at the same y.
fn strip_with_baseline<'a>(
    tabs_row: Row<'a, Message>,
    accent_position: AccentPosition,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let toolbar_bg = ti(tokens.toolbar_bg);
    let baseline_color = ti(tokens.border);
    let baseline = container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(baseline_color)),
            ..container::Style::default()
        });
    let row_padding = match accent_position {
        AccentPosition::Bottom => iced::Padding {
            top: 2.0,
            right: 6.0,
            bottom: 0.0,
            left: 6.0,
        },
        AccentPosition::Top => iced::Padding {
            top: 0.0,
            right: 6.0,
            bottom: 2.0,
            left: 6.0,
        },
    };
    let row_container = container(tabs_row).width(Length::Fill).padding(row_padding);
    let inner: Column<'a, Message> = match accent_position {
        AccentPosition::Bottom => column![row_container, baseline],
        AccentPosition::Top => column![baseline, row_container],
    };
    container(inner)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(toolbar_bg)),
            ..container::Style::default()
        })
        .into()
}

fn tab<'a>(
    label: &str,
    is_active: bool,
    is_dragging: bool,
    is_hovered: bool,
    is_last: bool,
    accent_position: AccentPosition,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let text_primary = ti(tokens.text);
    let text_muted = ti(tokens.text_secondary);
    let tab_active = ti(tokens.hover);
    let accent = ti(tokens.accent);
    let fill = if is_dragging {
        Color { a: 0.22, ..accent }
    } else if is_active {
        tab_active
    } else if is_hovered {
        Color {
            a: tab_active.a * 0.70,
            ..tab_active
        }
    } else {
        Color {
            a: tab_active.a * 0.35,
            ..tab_active
        }
    };
    let pill_style = TabPillStyle {
        fill,
        border: ti(tokens.border),
        accent,
        is_active,
        is_last,
        accent_position,
    };
    let txt_c = if is_active { text_primary } else { text_muted };
    let inner = container(text(label.to_string()).size(11).color(txt_c)).padding([4, 10]);
    TabPill::new(inner, pill_style).into()
}

// ─── Modal card demo ──────────────────────────────────────────

fn modal_card_demo<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    // Constants matched to `app/view/dialogs.rs` (the real app's
    // modal chrome). MODAL_CLOSE_X_HIT_W = 46, body padding = 16.
    // Title padding also = 16 so the title's left edge aligns with
    // the body's first text glyph.
    //
    // Header height is 28 — Altium-style compact modal header.
    // The chrome menu bar is 36, but the modal header doesn't need
    // to carry that much vertical weight; matching MENU_BAR_HEIGHT
    // makes the modal feel taller than its body, especially on
    // small dialogs (Reset / Confirm / Rename).
    const HEADER_HEIGHT: f32 = 28.0;
    const CLOSE_W: f32 = 46.0;
    const X_ICON: f32 = 14.0;
    const BODY_PAD: f32 = 16.0;

    let panel_bg = ti(tokens.panel_bg);
    let toolbar_bg = ti(tokens.toolbar_bg);
    let text_c = ti(tokens.text);
    let border = ti(tokens.border);

    // Use the same 14×14 X SVG the chrome window-close uses
    // (`view::view_main_window_chrome::chrome_btn`). Unicode "×" at
    // size 14 was rendering at ~16-18 px because its font metrics
    // include glyph ascenders — visibly bigger than the chrome X.
    let icon_text_c = text_c;
    let x_icon = svg(x_handle())
        .width(X_ICON)
        .height(X_ICON)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(icon_text_c),
        });
    let header = container(
        row![
            text("Modal title").size(13).color(text_c),
            Space::new().width(Length::Fill),
            container(x_icon)
                .width(CLOSE_W)
                .height(HEADER_HEIGHT)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.78, 0.22, 0.22, 1.0))),
                    border: Border {
                        radius: iced::border::Radius::default().top_right(8.0),
                        ..Border::default()
                    },
                    ..container::Style::default()
                }),
        ]
        .align_y(iced::Alignment::Center),
    )
    .height(HEADER_HEIGHT)
    .padding(iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        // Title left = body left so the modal reads as a single
        // column. The previous 12 px inset put the title 4 px
        // left of the body text — visible misalignment.
        left: BODY_PAD,
    })
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(toolbar_bg)),
        border: Border {
            width: 0.0,
            radius: iced::border::Radius::default().top_left(8.0).top_right(8.0),
            color: Color::TRANSPARENT,
        },
        ..container::Style::default()
    });

    let body = container(
        column![
            text("Body content goes here.").size(11).color(text_c),
            Space::new().height(8),
            text("Use this card style for every modal — same 8 px corners.")
                .size(11)
                .color(text_c),
        ]
        .padding(BODY_PAD),
    );

    container(column![header, body].width(Length::Fixed(420.0)))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(panel_bg)),
            border: Border {
                width: 1.0,
                radius: 8.0.into(),
                color: border,
            },
            shadow: iced::Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
                offset: iced::Vector::new(0.0, 4.0),
                blur_radius: 12.0,
            },
            ..container::Style::default()
        })
        .clip(true)
        .padding(0)
        .into()
}

// ─── BOM modal demo ───────────────────────────────────────────
//
// Static layout sketch only — no interactivity. Goal is to nail the
// Altium-style structure: top toolbar row, table on the left,
// properties sidebar on the right with `General | Columns` tabs,
// status line + button row at the bottom. We can wire interactivity
// in the actual app once the layout is signed off.

fn bom_modal_demo<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    const MODAL_W: f32 = 1100.0;
    const MODAL_H: f32 = 660.0;
    const HEADER_H: f32 = 28.0;
    const SIDEBAR_W: f32 = 320.0;

    let panel_bg = ti(tokens.panel_bg);
    let toolbar_bg = ti(tokens.toolbar_bg);
    let text_c = ti(tokens.text);
    let muted = ti(tokens.text_secondary);
    let border = ti(tokens.border);
    let accent = ti(tokens.accent);

    // Title bar — sits above the toolbar; same pattern as the
    // simple modal_card_demo above.
    let icon_text_c = text_c;
    let x_icon = svg(x_handle())
        .width(14)
        .height(14)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(icon_text_c),
        });
    let header = container(
        row![
            text("Bill of Materials for Variant [Production] of Project [Loratis-SN]")
                .size(13)
                .color(text_c),
            Space::new().width(Length::Fill),
            container(x_icon)
                .width(46)
                .height(HEADER_H)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.78, 0.22, 0.22, 1.0))),
                    border: Border {
                        radius: iced::border::Radius::default().top_right(8.0),
                        ..Border::default()
                    },
                    ..container::Style::default()
                }),
        ]
        .align_y(iced::Alignment::Center),
    )
    .height(HEADER_H)
    .padding(iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 16.0,
    })
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(toolbar_bg)),
        border: Border {
            width: 0.0,
            radius: iced::border::Radius::default().top_left(8.0).top_right(8.0),
            color: Color::TRANSPARENT,
        },
        ..container::Style::default()
    });

    // Toolbar row — just the variant dropdown on the left and the
    // (i) info icon on the right. Removed: 3 view-mode icons +
    // Preview button (per user feedback — the variant picker is
    // the only persistent control needed at this level; view modes
    // and preview are noise for our flow).
    let toolbar = container(
        row![
            dropdown_stub("Production", tokens),
            Space::new().width(Length::Fill),
            info_icon_demo(tokens),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(4),
    )
    .padding([8, 12])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(toolbar_bg)),
        ..container::Style::default()
    });

    // Main content row: table on left, properties sidebar on right.
    let table = bom_table_demo(tokens);
    let sidebar = bom_properties_sidebar(tokens);
    let main = row![
        container(table)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding([6, 6])
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(panel_bg)),
                ..container::Style::default()
            }),
        container(Space::new())
            .width(1)
            .height(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(border)),
                ..container::Style::default()
            }),
        container(sidebar)
            .width(Length::Fixed(SIDEBAR_W))
            .height(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(panel_bg)),
                ..container::Style::default()
            }),
    ]
    .height(Length::Fill);

    // Status line at the bottom-left.
    let status = container(
        row![
            text("84 of 84 lines visible").size(11).color(muted),
            Space::new().width(16),
            text("|").size(11).color(muted),
            Space::new().width(16),
            text("Current variant: Production").size(11).color(muted),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([6, 14])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(toolbar_bg)),
        border: Border {
            width: 0.0,
            radius: 0.0.into(),
            color: Color::TRANSPARENT,
        },
        ..container::Style::default()
    });

    // Button row — Export... | OK | Cancel.
    let buttons = container(
        row![
            Space::new().width(Length::Fill),
            secondary_btn_demo("Export…", tokens),
            Space::new().width(8),
            primary_btn_demo("OK", accent, text_c),
            Space::new().width(8),
            secondary_btn_demo("Cancel", tokens),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 14])
    .width(Length::Fill);

    container(
        column![header, toolbar, main, status, buttons]
            .spacing(0)
            .width(Length::Fixed(MODAL_W))
            .height(Length::Fixed(MODAL_H)),
    )
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(panel_bg)),
        border: Border {
            width: 1.0,
            radius: 8.0.into(),
            color: border,
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
            offset: iced::Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        },
        ..container::Style::default()
    })
    .clip(true)
    .into()
}

fn view_mode_btn<'a>(glyph: &str, is_on: bool, tokens: &ThemeTokens) -> Element<'a, Message> {
    let text_c = ti(tokens.text);
    let muted = ti(tokens.text_secondary);
    let border = ti(tokens.border);
    let bg = if is_on {
        Color {
            a: 0.18,
            ..ti(tokens.accent)
        }
    } else {
        Color::TRANSPARENT
    };
    let txt_c = if is_on { text_c } else { muted };
    container(text(glyph.to_string()).size(14).color(txt_c))
        .width(28)
        .height(24)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..container::Style::default()
        })
        .into()
}

fn dropdown_stub<'a>(label: &str, tokens: &ThemeTokens) -> Element<'a, Message> {
    let text_c = ti(tokens.text);
    let border = ti(tokens.border);
    // Layout: equal-width spacers either side of the label so the
    // text reads as centred while the chevron stays hard-right.
    container(
        row![
            Space::new().width(Length::Fill),
            text(label.to_string()).size(11).color(text_c),
            Space::new().width(Length::Fill),
            text("▾").size(10).color(text_c),
        ]
        .align_y(iced::Alignment::Center),
    )
    .width(140)
    .height(24)
    .padding([0, 10])
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
        border: Border {
            width: 1.0,
            radius: 3.0.into(),
            color: border,
        },
        ..container::Style::default()
    })
    .into()
}

fn secondary_btn_demo<'a>(label: &str, tokens: &ThemeTokens) -> Element<'a, Message> {
    let text_c = ti(tokens.text);
    let border = ti(tokens.border);
    container(text(label.to_string()).size(11).color(text_c))
        .padding([5, 14])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
            border: Border {
                width: 1.0,
                radius: 3.0.into(),
                color: border,
            },
            ..container::Style::default()
        })
        .into()
}

fn primary_btn_demo<'a>(label: &str, accent: Color, text_c: Color) -> Element<'a, Message> {
    container(text(label.to_string()).size(11).color(text_c))
        .padding([5, 18])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(accent)),
            border: Border {
                width: 0.0,
                radius: 3.0.into(),
                color: Color::TRANSPARENT,
            },
            ..container::Style::default()
        })
        .into()
}

fn info_icon_demo<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let accent = ti(tokens.accent);
    container(text("i").size(11).color(Color::WHITE))
        .width(20)
        .height(20)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(Color { a: 0.7, ..accent })),
            border: Border {
                width: 0.0,
                radius: 10.0.into(),
                color: Color::TRANSPARENT,
            },
            ..container::Style::default()
        })
        .into()
}

fn bom_table_demo<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let text_c = ti(tokens.text);
    let muted = ti(tokens.text_secondary);
    let toolbar_bg = ti(tokens.toolbar_bg);

    // Header row — # | Name | Description | Designator | Footprint | LibRef | Quantity
    let header_cell = |label: &'static str, w: f32| -> Element<'a, Message> {
        container(text(label.to_string()).size(11).color(text_c))
            .width(Length::Fixed(w))
            .padding([4, 8])
            .into()
    };
    let header = container(
        row![
            header_cell("#", 32.0),
            header_cell("Name", 140.0),
            header_cell("Description", 200.0),
            header_cell("Designator", 160.0),
            header_cell("Footprint", 140.0),
            header_cell("LibRef", 120.0),
            header_cell("Quantity", 70.0),
        ]
        .spacing(0),
    )
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(toolbar_bg)),
        ..container::Style::default()
    });

    // Sample rows
    let rows = [
        (
            "1",
            "SBR1M100BLP-7",
            "BRIDGE RECT 1P 100V",
            "BR1, BR2",
            "DIODES U-DFN303…",
            "SBR1M100BLP-7",
            "2",
        ),
        (
            "2",
            "10µF",
            "CAP CER 10UF 10V",
            "C1, C28, C29, C34…",
            "CAP 0603/1608",
            "GRM188Z71A106K…",
            "5",
        ),
        (
            "3",
            "0.1µF",
            "CAP CER 0.1UF 10V…",
            "C2, C3, C7, C8, C17…",
            "CAP 0603/1608",
            "C0603X7S1A104K03…",
            "35",
        ),
        (
            "4",
            "4.7µF",
            "CAP CER 4.7UF 6.3…",
            "C4, C61, C65",
            "CAP 0603/1608",
            "CL10B475K6JNQNC",
            "3",
        ),
        (
            "5",
            "30pF",
            "CAP CER 30PF 50V…",
            "C5, C6",
            "CAP 0402/1005",
            "GRM1555C1H300J…",
            "2",
        ),
        (
            "6",
            "12nF",
            "CAP CER 0.012UF 1…",
            "C9, C10, C11, C12",
            "CAP 0402/1005",
            "06031C123KAT2A",
            "4",
        ),
        (
            "7",
            "1nF",
            "CAP CER 1000PF 2K…",
            "C13, C14",
            "CAP 1206/3216",
            "CL31B102KJHNNNE",
            "2",
        ),
        (
            "8",
            "100µF",
            "CAP ALUM POLY 10…",
            "C15, C25",
            "WURTH WCAP-PH…",
            "875015119003",
            "2",
        ),
        (
            "9",
            "10µF",
            "CAP CER 10UF 10V…",
            "C16, C52",
            "CAP 0805/2012",
            "C2012X7R1A106K1…",
            "2",
        ),
        (
            "10",
            "10µF",
            "CAP CER 10UF 6.3V…",
            "C19, C20, C21, C22…",
            "CAP 0402/1005",
            "C0402X5R1A106K…",
            "16",
        ),
    ];
    let mut row_col: Column<'a, Message> = Column::new().spacing(0);
    for (i, r) in rows.iter().enumerate() {
        let alt_bg = if i % 2 == 0 {
            Color::from_rgba(1.0, 1.0, 1.0, 0.0)
        } else {
            Color::from_rgba(1.0, 1.0, 1.0, 0.025)
        };
        let cell = |s: &str, w: f32| -> Element<'a, Message> {
            container(text(s.to_string()).size(10).color(text_c))
                .width(Length::Fixed(w))
                .padding([3, 8])
                .into()
        };
        let num_cell = container(text(r.0.to_string()).size(10).color(muted))
            .width(Length::Fixed(32.0))
            .padding([3, 8])
            .align_x(iced::alignment::Horizontal::Right);
        let row_el = container(
            row![
                num_cell,
                cell(r.1, 140.0),
                cell(r.2, 200.0),
                cell(r.3, 160.0),
                cell(r.4, 140.0),
                cell(r.5, 120.0),
                cell(r.6, 70.0),
            ]
            .spacing(0),
        )
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(alt_bg)),
            ..container::Style::default()
        });
        row_col = row_col.push(row_el);
    }
    column![header, row_col].spacing(0).into()
}

fn bom_properties_sidebar<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let text_c = ti(tokens.text);
    let muted = ti(tokens.text_secondary);
    let border = ti(tokens.border);
    let accent = ti(tokens.accent);

    let sidebar_title = container(text("Properties").size(13).color(text_c))
        .padding([8, 12])
        .width(Length::Fill);

    // Tabs row — General | Columns. General is active.
    let tab_pill = |label: &'static str, is_active: bool| -> Element<'a, Message> {
        let bg = if is_active {
            Color { a: 0.18, ..accent }
        } else {
            Color::TRANSPARENT
        };
        container(text(label.to_string()).size(11).color(text_c))
            .padding([4, 12])
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(bg)),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                ..container::Style::default()
            })
            .into()
    };
    let tabs = row![
        tab_pill("General", true),
        Space::new().width(4),
        tab_pill("Columns", false),
    ]
    .padding([0, 12]);

    // Collapsible sections — represent with a header row + body.
    let section_block = |title: &'static str, body: Element<'a, Message>| -> Element<'a, Message> {
        let header = container(
            row![
                text("▾").size(10).color(muted),
                Space::new().width(6),
                text(title.to_string()).size(11).color(text_c),
            ]
            .align_y(iced::Alignment::Center),
        )
        .padding([6, 12])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.025))),
            border: Border {
                width: 0.0,
                radius: 0.0.into(),
                color: Color::TRANSPARENT,
            },
            ..container::Style::default()
        });
        column![header, container(body).padding([8, 12])]
            .spacing(0)
            .into()
    };

    // BOM Items section
    let checkbox_row = |label: &'static str, on: bool| -> Element<'a, Message> {
        let pip_bg = if on {
            Color::from_rgb(0.00, 0.47, 0.84)
        } else {
            Color::from_rgba(1.0, 1.0, 1.0, 0.04)
        };
        let pip = container(if on {
            text("✓").size(9).color(Color::WHITE)
        } else {
            text(" ").size(9)
        })
        .width(12)
        .height(12)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(pip_bg)),
            border: Border {
                width: 1.0,
                radius: 2.0.into(),
                color: border,
            },
            ..container::Style::default()
        });
        row![
            pip,
            Space::new().width(8),
            text(label.to_string()).size(11).color(text_c)
        ]
        .align_y(iced::Alignment::Center)
        .into()
    };
    let bom_items = column![
        checkbox_row("Show Not Fitted", false),
        Space::new().height(6),
        checkbox_row("Include DB Parameters in Variations", false),
    ]
    .spacing(0);

    // Export Options section
    let label_field = |label: &'static str, value: &'static str| -> Element<'a, Message> {
        row![
            container(text(label.to_string()).size(11).color(muted)).width(Length::Fixed(80.0)),
            container(
                row![
                    text(value.to_string()).size(11).color(text_c),
                    Space::new().width(Length::Fill),
                    text("▾").size(10).color(text_c),
                ]
                .align_y(iced::Alignment::Center),
            )
            .padding([4, 8])
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.04))),
                border: Border {
                    width: 1.0,
                    radius: 3.0.into(),
                    color: border,
                },
                ..container::Style::default()
            }),
        ]
        .align_y(iced::Alignment::Center)
        .spacing(8)
        .into()
    };
    let export_opts = column![
        label_field("File Format", "MS-Excel (*.xls, *.xlsx, *.xlsm)"),
        Space::new().height(8),
        label_field("Template", "No Template"),
        Space::new().height(8),
        checkbox_row("Add to Project", false),
        Space::new().height(6),
        checkbox_row("Open Exported", false),
    ]
    .spacing(0);

    column![
        sidebar_title,
        tabs,
        Space::new().height(8),
        section_block("BOM Items", bom_items.into()),
        Space::new().height(4),
        section_block("Export Options", export_opts.into()),
    ]
    .spacing(0)
    .into()
}

// ─── Project tree leaf demo ───────────────────────────────────

fn tree_row_demo<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let mut col: Column<'a, Message> = Column::new().spacing(2);
    for (label, open, dirty, active) in [
        ("clean.standard_sch", false, false, false),
        ("open.standard_sch", true, false, false),
        ("dirty.standard_sch", true, true, false),
        ("active.standard_sch", true, false, true),
        ("active+dirty.standard_sch", true, true, true),
    ] {
        col = col.push(tree_row(label, open, dirty, active, tokens));
    }
    col.into()
}

fn tree_row<'a>(
    label: &str,
    is_open: bool,
    is_dirty: bool,
    is_active: bool,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let text_c = ti(tokens.text);
    let selection = ti(tokens.selection);
    let active_bg = Color {
        a: 0.45,
        ..selection
    };
    let dirty_red = Color::from_rgba(0.85, 0.30, 0.30, 1.0);
    let dot_size = 6.0;
    let mut r: Row<'a, Message> = Row::new().spacing(8).align_y(iced::Alignment::Center);
    r = r.push(text(label.to_string()).size(11).color(text_c));
    r = r.push(Space::new().width(Length::Fill));
    if is_open || is_dirty {
        let dot_color = if is_dirty { dirty_red } else { Color::WHITE };
        r = r.push(
            container(Space::new().width(dot_size).height(dot_size))
                .width(dot_size)
                .height(dot_size)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(dot_color)),
                    border: Border {
                        radius: (dot_size / 2.0).into(),
                        ..Border::default()
                    },
                    ..container::Style::default()
                }),
        );
    }
    let row_bg = if is_active {
        Some(Background::Color(active_bg))
    } else {
        None
    };
    container(r)
        .width(Length::Fixed(360.0))
        .padding([4, 8])
        .style(move |_: &Theme| container::Style {
            background: row_bg,
            border: Border {
                radius: 2.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .into()
}

// ─── Local helpers ────────────────────────────────────────────

fn ti(c: TokColor) -> Color {
    Color::from_rgba8(c.r, c.g, c.b, c.a as f32 / 255.0)
}
