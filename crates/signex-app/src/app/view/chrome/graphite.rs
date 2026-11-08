//! Graphite title and menu bar: 38 px strip with the mark and menus, a
//! centred search field, the Signex AI button, a light/dark toggle and the
//! window controls. Legacy themes keep `view_main_window_chrome`.

use std::sync::LazyLock;

use iced::widget::{Space, button, container, mouse_area, row, svg, text, text_input, tooltip};
use iced::{Alignment, Background, Border, Color, Element, Length, Widget as _};

use super::{SP, TS, buttons};
use crate::app::view::{
    CHROME_SEARCH_BAR_MIN_WIDTH, CHROME_SEARCH_LEFT_GAP, CHROME_SEARCH_RIGHT_GAP,
};
use crate::app::{CommandPaletteMsg, Message, Signex, UiMsg, WindowMsg};
use crate::menu_bar::MenuMessage;
use crate::styles::{self, ChromeMetrics, Radii, ti, tokens::resolve};

/// Width of the Signex AI button, in px.
const SIGNAL_W: f32 = 96.0;
/// Side of the square theme toggle.
const THEME_W: f32 = 28.0;
/// Gap between the two buttons and padding after them.
const CLUSTER_GAP: f32 = 4.0;
const CLUSTER_PAD: f32 = 6.0;
/// Widest the search field gets (the mock's 380 px).
const SEARCH_MAX_W: f32 = 380.0;
/// Height of the "Ctrl K" key cap.
const KEY_CAP_H: f32 = 18.0;

/// Room the Signex AI and theme buttons take left of the window controls.
pub(super) const RIGHT_CLUSTER_W: f32 = SIGNAL_W + CLUSTER_GAP + THEME_W + CLUSTER_PAD;

static MOON: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(
        include_bytes!("../../../../assets/icons/chrome/theme_moon.svg").as_slice(),
    )
});
static SUN: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(
        include_bytes!("../../../../assets/icons/chrome/theme_sun.svg").as_slice(),
    )
});

/// `(x, width)` of the Graphite search field: at most [`SEARCH_MAX_W`],
/// centred on the window when that clears the menu row, otherwise flush
/// right of the menu. The field only shrinks when the room between the menu
/// row and the right cluster plus window controls is really short.
pub(super) fn search_geometry(
    window_w: f32,
    leading_w: f32,
    trailing_w: f32,
    menu_w: f32,
) -> (f32, f32) {
    let menu_end = leading_w + menu_w + CHROME_SEARCH_LEFT_GAP;
    let right_limit = window_w - (trailing_w + RIGHT_CLUSTER_W + CHROME_SEARCH_RIGHT_GAP);
    let width = (right_limit - menu_end).clamp(CHROME_SEARCH_BAR_MIN_WIDTH, SEARCH_MAX_W);
    let centred = (window_w - width) / 2.0;
    let x = centred.min(right_limit - width).max(menu_end);
    (x, width)
}

/// Whether a theme-bar colour is dark (picks the sun or moon glyph).
fn is_dark(c: signex_types::theme::Color) -> bool {
    0.2126 * c.r as f32 + 0.7152 * c.g as f32 + 0.0722 * c.b as f32 <= 127.5
}

impl Signex {
    /// The Graphite main-window strip; see the module docs.
    pub(super) fn view_graphite_window_chrome<'a>(
        &'a self,
        menu_row: Option<Element<'a, Message>>,
        tokens: &signex_types::theme::ThemeTokens,
    ) -> Element<'a, Message> {
        let theme_id = self.ui_state.effective_theme_id();
        let r = resolve(tokens);
        let (ink, ink3, line) = (ti(tokens.text), r.ink3, ti(tokens.border));
        let palette = buttons::Palette {
            theme_id,
            text: ink3,
            hover: ti(tokens.hover),
            close_hover: Color::from_rgba(0.78, 0.22, 0.22, 1.0),
            graphite: true,
        };
        let layout = self.window_controls_layout();
        let fullscreen = self.ui_state.main_window_is_fullscreen();
        let leading_controls = buttons::leading(&layout, palette, fullscreen);
        let trailing_controls = buttons::trailing(&layout, palette);

        let menu_padded = menu_row.map(|row| {
            container(row).padding(iced::Padding {
                left: 10.0,
                ..iced::Padding::ZERO
            })
        });
        let (search_x, search_w) = self.chrome_search_bar_geometry();
        let search = self.graphite_search_field(tokens, search_w);

        let drag_zone = || -> Element<'static, Message> {
            mouse_area(
                container(Space::new())
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .on_press(Message::Window(WindowMsg::StartMainWindowDrag))
            .on_double_click(Message::Window(WindowMsg::ToggleMaximizeMainWindow))
            .boxed()
        };
        let dark = is_dark(tokens.toolbar_bg);
        let cluster = container(
            row![
                signal_button(tokens, ink, line),
                theme_toggle(tokens, theme_id.light_dark_counterpart(), dark, ink3),
            ]
            .spacing(CLUSTER_GAP)
            .align_y(Alignment::Center),
        )
        .padding(iced::Padding {
            right: CLUSTER_PAD,
            ..iced::Padding::ZERO
        });

        let strip = row![leading_controls]
            .push(menu_padded.boxed())
            .push(drag_zone())
            .push(drag_zone())
            .push(cluster.boxed())
            .push(trailing_controls)
            .width(Length::Fill)
            .align_y(Alignment::Center);
        let centered_search = container(search)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(iced::Padding {
                left: search_x,
                ..iced::Padding::ZERO
            })
            .align_y(iced::alignment::Vertical::Center);
        let inner = iced::widget::Stack::new()
            .push(strip.boxed())
            .push(centered_search.boxed());
        container(inner)
            .width(Length::Fill)
            .height(crate::menu_bar::MENU_BAR_HEIGHT)
            .style(styles::toolbar_strip(tokens))
            .boxed()
    }

    /// The command-palette field: icon, input and the palette's key cap, on
    /// the `field` fill with a radius-7 hairline.
    fn graphite_search_field<'a>(
        &'a self,
        tokens: &signex_types::theme::ThemeTokens,
        width: f32,
    ) -> Element<'a, Message> {
        let r = resolve(tokens);
        let (ink, ink4) = (ti(tokens.text), r.ink4);
        let icon = svg(crate::icons::icon_chrome_search(
            self.ui_state.effective_theme_id(),
        ))
        .width(13)
        .height(13)
        .style(move |_: &iced::Theme, _| svg::Style { color: Some(ink4) });
        let query: &'a str = if self.ui_state.command_palette.window.is_none() {
            &self.ui_state.command_palette.query
        } else {
            ""
        };
        let input = text_input("Search commands, parts, nets", query)
            .id(crate::app::command_palette::COMMAND_PALETTE_INPUT_ID.clone())
            .on_input(|q| Message::CommandPalette(CommandPaletteMsg::QueryChanged(q)))
            .on_submit(Message::CommandPalette(CommandPaletteMsg::ExecuteSelected))
            .padding(iced::Padding::ZERO)
            .size(TS.dense)
            .width(Length::Fill)
            .style(move |_: &iced::Theme, _| text_input::Style {
                background: Background::Color(Color::TRANSPARENT),
                border: Border::default(),
                placeholder: ink4,
                value: ink,
                selection: Color { a: 0.4, ..ink },
            });
        // The key cap names the real binding, so it never promises a chord
        // the keymap does not have.
        let hint = self
            .ui_state
            .active_keymap
            .shortcut_label_preferring("open_command_palette", "Ctrl+K");
        let mut content = row![icon, input].spacing(SP.l).align_y(Alignment::Center);
        if !hint.is_empty() {
            let cap = container(text(hint).size(TS.caption).font(
                crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT),
            ))
            .padding([0.0, SP.m])
            .height(KEY_CAP_H)
            .align_y(iced::alignment::Vertical::Center)
            .style(styles::key_cap(tokens));
            content = content.push(cap.boxed());
        }
        container(content)
            .padding(iced::Padding {
                top: 0.0,
                right: 6.0,
                bottom: 0.0,
                left: 10.0,
            })
            .width(width)
            .height(ChromeMetrics::DEFAULT.field)
            .align_y(iced::alignment::Vertical::Center)
            .style(styles::field(tokens, false))
            .boxed()
    }
}

/// "Signex AI": copper dot and label; opens the Signex AI panel like View ▸ Signex AI.
fn signal_button(
    tokens: &signex_types::theme::ThemeTokens,
    ink: Color,
    line: Color,
) -> Element<'static, Message> {
    let r = resolve(tokens);
    let (raised, hover, accent) = (r.raised, ti(tokens.hover), ti(tokens.accent));
    let dot = container(Space::new())
        .width(7)
        .height(7)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(accent)),
            border: Border {
                radius: Radii::GRAPHITE.pill.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    button(
        row![dot, text("Signex AI").size(TS.dense)]
            .spacing(SP.l)
            .align_y(Alignment::Center),
    )
    .width(SIGNAL_W)
    .height(ChromeMetrics::DEFAULT.field)
    .padding([0.0, 10.0])
    .on_press(Message::Menu(MenuMessage::OpenSignexAiPanel))
    .style(move |_: &iced::Theme, status| button::Style {
        background: Some(Background::Color(
            if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                hover
            } else {
                raised
            },
        )),
        text_color: ink,
        border: Border {
            width: 1.0,
            color: line,
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..button::Style::default()
    })
    .boxed()
}

/// Light/dark toggle: sends the existing `ThemeChanged` with the Graphite
/// twin of the opposite lightness. Moon on a light bar, sun on a dark one.
fn theme_toggle(
    tokens: &signex_types::theme::ThemeTokens,
    target: signex_types::theme::ThemeId,
    dark_bar: bool,
    ink: Color,
) -> Element<'static, Message> {
    let hover = ti(tokens.hover);
    let border = ti(tokens.border);
    let label = if dark_bar {
        "Light theme"
    } else {
        "Dark theme"
    };
    let glyph = svg(if dark_bar { SUN.clone() } else { MOON.clone() })
        .width(15)
        .height(15)
        .style(move |_: &iced::Theme, _| svg::Style { color: Some(ink) });
    let btn = button(
        container(glyph)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(THEME_W)
    .height(ChromeMetrics::DEFAULT.field)
    .padding(0)
    .on_press(Message::Ui(UiMsg::ThemeChanged(target)))
    .style(move |_: &iced::Theme, status| button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(Background::Color(hover)),
        text_color: ink,
        border: Border {
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..button::Style::default()
    });
    let hint = container(text(label).size(TS.caption).color(ink))
        .padding([3, 8])
        .style(signex_widgets::active_bar_dropdown::tooltip_style(
            hover, border,
        ));
    tooltip(btn, hint, tooltip::Position::Bottom).gap(4).boxed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_field_is_at_most_380_and_centred_on_a_wide_window() {
        let (x, w) = search_geometry(1600.0, 0.0, 132.0, 440.0);
        assert_eq!(w, 380.0);
        assert_eq!(x, (1600.0 - 380.0) / 2.0);
    }

    #[test]
    fn ten_menu_roots_do_not_shrink_the_field_on_a_laptop_window() {
        let (x, w) = search_geometry(1440.0, 0.0, 138.0, 586.0);
        assert_eq!(w, 380.0);
        assert!(x >= 586.0);
        assert!(x + w <= 1440.0 - 138.0 - RIGHT_CLUSTER_W);
    }

    #[test]
    fn the_field_clears_the_right_cluster_on_a_narrow_window() {
        let (x, w) = search_geometry(900.0, 0.0, 132.0, 440.0);
        assert!(x + w <= 900.0 - 132.0 - RIGHT_CLUSTER_W);
        assert!(x >= 440.0);
    }

    /// The field is centred on the window, shrinks monotonically as the
    /// window narrows, and never runs under the menu items or the right
    /// cluster, for every window-control layout.
    #[test]
    fn search_bar_stays_centred_and_clamped() {
        let in_window = crate::menu_bar::approx_graphite_menu_bar_width();
        // Windows-style: nothing leading, 138 px of controls trailing.
        search_bar_invariants(0.0, 138.0, in_window);
        // Native macOS lights: room leading, nothing trailing.
        search_bar_invariants(78.0, 0.0, in_window);
        // Round buttons on both sides.
        search_bar_invariants(56.0, 40.0, in_window);
        // The system menu bar carries the menus: nothing to clear.
        search_bar_invariants(78.0, 0.0, 0.0);
    }

    fn search_bar_invariants(leading_w: f32, trailing_w: f32, menu_w: f32) {
        let menu_end = leading_w + menu_w + CHROME_SEARCH_LEFT_GAP;
        for window_w in [1000.0_f32, 1280.0, 1440.0, 1920.0, 3840.0] {
            let (x, w) = search_geometry(window_w, leading_w, trailing_w, menu_w);
            let right_limit = window_w - (trailing_w + RIGHT_CLUSTER_W + CHROME_SEARCH_RIGHT_GAP);
            assert!(
                (CHROME_SEARCH_BAR_MIN_WIDTH..=SEARCH_MAX_W).contains(&w),
                "bar width {w} out of range at {window_w}"
            );
            assert!(
                x >= menu_end,
                "bar slid under the menu at {window_w}: x={x}"
            );
            assert!(x + w <= window_w, "bar overflows the window at {window_w}");
            assert!(
                x + w <= right_limit.max(menu_end + w),
                "bar runs under the right cluster at {window_w}: x={x} w={w}"
            );
            // Centred whenever the window is wide enough for it.
            let centred = (window_w - w) / 2.0;
            if centred >= menu_end && centred + w <= right_limit {
                assert!(
                    (x + w / 2.0 - window_w / 2.0).abs() < 0.01,
                    "bar off window centre at {window_w}: x={x} w={w}"
                );
            }
        }
        // Narrowing the window never widens the bar.
        let (_, wide) = search_geometry(1920.0, leading_w, trailing_w, menu_w);
        let (_, narrow) = search_geometry(1100.0, leading_w, trailing_w, menu_w);
        assert!(narrow <= wide, "bar grew as the window shrank");
    }

    #[test]
    fn the_toggle_glyph_follows_the_bar_lightness() {
        use signex_types::theme::Color as C;
        let c = |v| C {
            r: v,
            g: v,
            b: v,
            a: 255,
        };
        assert!(is_dark(c(0x17)));
        assert!(!is_dark(c(0xFB)));
    }
}
