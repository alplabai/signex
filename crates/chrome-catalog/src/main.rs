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

use iced::widget::{Column, Row, Space, button, column, container, row, text};
use iced::{Background, Border, Color, Element, Length, Theme};
use signex_types::theme::{Color as TokColor, ThemeId, ThemeTokens, theme_tokens};
use signex_widgets::tab_pill::{TabPill, TabPillStyle};

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
            section("Document tabs (3 visible, middle is active)", &tokens, doc_tab_strip(&tokens)),
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
        ]
        .spacing(20)
        .padding(20);

        container(column![header, body].spacing(0))
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
                (false, button::Status::Hovered | button::Status::Pressed) => Color {
                    a: 0.18,
                    ..accent
                },
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
        .push(tab("MCU_IO", false, false, false, false, tokens))
        .push(tab("Loratis-SN", true, false, false, false, tokens))
        .push(tab("Power", false, false, false, true, tokens));
    strip_with_baseline(tabs_row, tokens)
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
        r = r.push(tab(label, active, dragging, hovered, last, tokens));
    }
    strip_with_baseline(r, tokens)
}

fn panel_tab_strip<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let mut r: Row<'a, Message> = Row::new().spacing(0);
    for (i, label) in ["Components", "Manufacturer Part Search", "PCB CoDesign", "Messages", "Properties"]
        .iter()
        .enumerate()
    {
        let active = i == 0;
        let last = i == 4;
        r = r.push(tab(label, active, false, false, last, tokens));
    }
    strip_with_baseline(r, tokens)
}

/// Wrap a row of tabs in the same toolbar bg + 1 px black baseline
/// the real app uses (`tab_bar::view`). Without this the tabs would
/// "float" against the section panel's bg with no continuous strip
/// underneath them — the strip + baseline are part of the chrome,
/// not just decoration.
fn strip_with_baseline<'a>(tabs_row: Row<'a, Message>, tokens: &ThemeTokens) -> Element<'a, Message> {
    let toolbar_bg = ti(tokens.toolbar_bg);
    let baseline_color = ti(tokens.border);
    let baseline = container(Space::new())
        .width(Length::Fill)
        .height(1)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(baseline_color)),
            ..container::Style::default()
        });
    container(column![
        container(tabs_row)
            .width(Length::Fill)
            .padding(iced::Padding {
                top: 2.0,
                right: 6.0,
                bottom: 0.0,
                left: 6.0,
            }),
        baseline,
    ])
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
        Color::TRANSPARENT
    };
    let pill_style = TabPillStyle {
        fill,
        border: ti(tokens.border),
        accent,
        is_active,
        is_last,
    };
    let txt_c = if is_active { text_primary } else { text_muted };
    let inner = container(text(label.to_string()).size(11).color(txt_c))
        .padding([4, 10]);
    TabPill::new(inner, pill_style).into()
}

// ─── Modal card demo ──────────────────────────────────────────

fn modal_card_demo<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let panel_bg = ti(tokens.panel_bg);
    let toolbar_bg = ti(tokens.toolbar_bg);
    let text_c = ti(tokens.text);
    let border = ti(tokens.border);

    let header = container(
        row![
            text("Modal title").size(13).color(text_c),
            Space::new().width(Length::Fill),
            container(text("\u{00D7}").size(14).color(text_c))
                .width(40)
                .height(28)
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
    .height(36)
    .padding(iced::Padding {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 12.0,
    })
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(toolbar_bg)),
        border: Border {
            width: 0.0,
            radius: iced::border::Radius::default()
                .top_left(8.0)
                .top_right(8.0),
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
        .padding(16),
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

// ─── Project tree leaf demo ───────────────────────────────────

fn tree_row_demo<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let mut col: Column<'a, Message> = Column::new().spacing(2);
    for (label, open, dirty, active) in [
        ("clean.kicad_sch", false, false, false),
        ("open.kicad_sch", true, false, false),
        ("dirty.kicad_sch", true, true, false),
        ("active.kicad_sch", true, false, true),
        ("active+dirty.kicad_sch", true, true, true),
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
    let mut r: Row<'a, Message> = Row::new()
        .spacing(8)
        .align_y(iced::Alignment::Center);
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
