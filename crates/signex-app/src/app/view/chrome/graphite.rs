//! Graphite title and menu bar: 38 px strip with the mark and menus, a
//! centred search field (a square search button on a narrow window), a
//! light/dark toggle and the window controls. While the command palette is
//! open in the main window the field is its query input. Legacy themes keep
//! `view_main_window_chrome`.

use std::sync::LazyLock;

use iced::widget::{Space, container, mouse_area, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Length, Widget as _};
use signex_i18n::{I18n, ids, shortcut_ids, t};
use signex_widgets::{button, raw, text, text_input, tooltip};

use super::{SP, TS, buttons};
use crate::app::view::{
    CHROME_SEARCH_BAR_MIN_WIDTH, CHROME_SEARCH_LEFT_GAP, CHROME_SEARCH_RIGHT_GAP,
};
use crate::app::{CommandPaletteMsg, Message, Signex, UiMsg, WindowMsg};
use crate::styles::{self, ChromeMetrics, Radii, ti, tokens::resolve};

/// Side of the square theme toggle.
const THEME_W: f32 = 28.0;
/// Padding after the theme toggle.
const CLUSTER_PAD: f32 = 6.0;
/// Widest the search field gets (mock #4: 400 px with eight menu roots).
const SEARCH_MAX_W: f32 = 400.0;
/// Height of the "Ctrl K" key cap.
const KEY_CAP_H: f32 = 18.0;
/// Side of the square search button that replaces the field on a narrow
/// window: the theme toggle's height.
const SEARCH_BUTTON_W: f32 = ChromeMetrics::DEFAULT.field;
/// Gap between the search button and the theme toggle.
const SEARCH_BUTTON_GAP: f32 = SP.s;

/// Room the theme toggle takes left of the window controls.
pub(super) const RIGHT_CLUSTER_W: f32 = THEME_W + CLUSTER_PAD;

/// Where the title bar offers the command palette's search.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::app::view) enum SearchSlot {
    /// The search field, its left edge at `x`, `w` wide.
    Field { x: f32, w: f32 },
    /// Too little room between the menu roots and the right cluster for the
    /// field: a square search button at `x`, first in the right cluster.
    Button { x: f32 },
}

impl SearchSlot {
    /// `(x, width)` of what the palette card centres under.
    pub(in crate::app::view) fn anchor(self) -> (f32, f32) {
        match self {
            Self::Field { x, w } => (x, w),
            Self::Button { x } => (x, SEARCH_BUTTON_W),
        }
    }
}

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

/// Where the Graphite search goes. The field is at most [`SEARCH_MAX_W`],
/// centred on the window when that clears the menu row, otherwise flush
/// right of the menu; it only shrinks when the room between the menu row and
/// the right cluster plus window controls is really short. Below
/// [`CHROME_SEARCH_BAR_MIN_WIDTH`] of room it becomes the search button, so
/// it never squeezes further or runs under the roots.
pub(super) fn search_geometry(
    window_w: f32,
    leading_w: f32,
    trailing_w: f32,
    menu_w: f32,
) -> SearchSlot {
    let menu_end = leading_w + menu_w + CHROME_SEARCH_LEFT_GAP;
    let right_limit = window_w - (trailing_w + RIGHT_CLUSTER_W + CHROME_SEARCH_RIGHT_GAP);
    let room = right_limit - menu_end;
    if room < CHROME_SEARCH_BAR_MIN_WIDTH {
        let x = window_w - trailing_w - RIGHT_CLUSTER_W - SEARCH_BUTTON_GAP - SEARCH_BUTTON_W;
        return SearchSlot::Button { x };
    }
    let w = room.min(SEARCH_MAX_W);
    let centred = (window_w - w) / 2.0;
    let x = centred.min(right_limit - w).max(menu_end);
    SearchSlot::Field { x, w }
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
        let ink3 = r.ink3;
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
        let hint = self.palette_key_hint();
        let slot = self.chrome_search_slot();
        let search_btn = matches!(slot, SearchSlot::Button { .. })
            .then(|| search_button(tokens, &self.ui_state.i18n, theme_id, &hint, ink3));

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
        let toggle = theme_toggle(
            tokens,
            &self.ui_state.i18n,
            theme_id.light_dark_counterpart(),
            dark,
            ink3,
        );
        let mut cluster_row = row![].align_y(Alignment::Center);
        if let Some(search_btn) = search_btn {
            cluster_row = cluster_row
                .push(search_btn)
                .push(Space::new().width(SEARCH_BUTTON_GAP).boxed());
        }
        let cluster = container(cluster_row.push(toggle)).padding(iced::Padding {
            right: CLUSTER_PAD,
            ..iced::Padding::ZERO
        });

        // The roots sit in the row, so the search button can never overlap
        // them; the field floats over the drag zones at its own x.
        let strip = row![leading_controls]
            .push(menu_padded.boxed())
            .push(drag_zone())
            .push(drag_zone())
            .push(cluster.boxed())
            .push(trailing_controls)
            .width(Length::Fill)
            .align_y(Alignment::Center);
        let mut inner = iced::widget::Stack::new().push(strip.boxed());
        if let SearchSlot::Field { x, w } = slot {
            let field = container(self.graphite_search_field(tokens, w, hint))
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(iced::Padding {
                    left: x,
                    ..iced::Padding::ZERO
                })
                .align_y(iced::alignment::Vertical::Center);
            inner = inner.push(field.boxed());
        }
        container(inner)
            .width(Length::Fill)
            .height(crate::menu_bar::MENU_BAR_HEIGHT)
            .style(styles::toolbar_strip(tokens))
            .boxed()
    }

    /// The palette's key binding as the live keymap writes it, empty when
    /// nothing binds it. The key cap and the search button's tooltip name it,
    /// so they never promise a chord the keymap does not have.
    fn palette_key_hint(&self) -> String {
        self.ui_state.active_keymap.shortcut_label_preferring(
            "open_command_palette",
            "Ctrl+K",
            &self.ui_state.i18n,
        )
    }

    /// The command-palette field: icon, input and a key cap, on the `field`
    /// fill with a radius-7 hairline. While the palette is open in the main
    /// window the field is its query input: an accent focus border and an
    /// Esc cap in place of the palette's binding `hint`.
    fn graphite_search_field<'a>(
        &'a self,
        tokens: &signex_types::theme::ThemeTokens,
        width: f32,
        hint: String,
    ) -> Element<'a, Message> {
        let r = resolve(tokens);
        let (ink, ink4) = (ti(tokens.text), r.ink4);
        let icon = svg(crate::icons::icon_chrome_search(
            self.ui_state.effective_theme_id(),
        ))
        // The glyph is as tall as title text (13 px at the default base).
        .width(TS.title)
        .height(TS.title)
        .style(move |_: &iced::Theme, _| svg::Style {
            color: Some(ink4),
            ..Default::default()
        });
        let palette = &self.ui_state.command_palette;
        let open_here = palette.open && palette.window.is_none();
        let query: &'a str = if palette.window.is_none() {
            &palette.query
        } else {
            ""
        };
        let input = text_input(
            t!(self.ui_state.i18n, ids::MENU_BAR_SEARCH_PLACEHOLDER),
            query,
        )
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
        let mut content = row![icon, input].spacing(SP.l).align_y(Alignment::Center);
        let cap_label = if open_here {
            Some(signex_widgets::Label::from(t!(
                self.ui_state.i18n,
                shortcut_ids::PALETTE_KEY_ESC
            )))
        } else if hint.is_empty() {
            None
        } else {
            // raw: the key chord, already rendered for the locale by the keymap.
            Some(raw(hint))
        };
        if let Some(label) = cap_label {
            let cap = container(text(label).size(TS.caption).font(
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
            .style(styles::field(tokens, open_here))
            .boxed()
    }
}

/// The square search button a narrow window shows instead of the field:
/// a hairline square at the theme toggle's height that opens the palette,
/// as the field does, with a "Search (Ctrl+K)" tooltip from the live keymap.
fn search_button(
    tokens: &signex_types::theme::ThemeTokens,
    i18n: &I18n,
    theme_id: signex_types::theme::ThemeId,
    hint: &str,
    ink: Color,
) -> Element<'static, Message> {
    let hover = ti(tokens.hover);
    let border = ti(tokens.border);
    let label = if hint.is_empty() {
        t!(i18n, ids::MENU_BAR_SEARCH_BUTTON)
    } else {
        t!(i18n, ids::MENU_BAR_SEARCH_BUTTON_KEY, "key" => hint)
    };
    // The same glyph size as the field's search icon.
    let glyph = svg(crate::icons::icon_chrome_search(theme_id))
        .width(TS.title)
        .height(TS.title)
        .style(move |_: &iced::Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        });
    let btn = button(
        container(glyph)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(SEARCH_BUTTON_W)
    .height(SEARCH_BUTTON_W)
    .padding(0)
    .on_press(Message::CommandPalette(CommandPaletteMsg::Open))
    .style(move |_: &iced::Theme, status| button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(Background::Color(hover)),
        text_color: ink,
        border: Border {
            width: 1.0,
            color: border,
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..button::Style::default()
    });
    chrome_tooltip(btn.boxed(), label, hover, border, ink)
}

/// A title-bar button with its tooltip below it.
fn chrome_tooltip(
    btn: Element<'static, Message>,
    label: signex_i18n::Localized,
    hover: Color,
    border: Color,
    ink: Color,
) -> Element<'static, Message> {
    let hint = container(text(label).size(TS.caption).color(ink))
        .padding([3, 8])
        .style(signex_widgets::active_bar_dropdown::tooltip_style(
            hover, border,
        ));
    tooltip(btn, hint, tooltip::Position::Bottom).gap(4).boxed()
}

/// Light/dark toggle: sends the existing `ThemeChanged` with the Graphite
/// twin of the opposite lightness. Moon on a light bar, sun on a dark one.
fn theme_toggle(
    tokens: &signex_types::theme::ThemeTokens,
    i18n: &I18n,
    target: signex_types::theme::ThemeId,
    dark_bar: bool,
    ink: Color,
) -> Element<'static, Message> {
    let hover = ti(tokens.hover);
    let border = ti(tokens.border);
    let label = t!(
        i18n,
        if dark_bar {
            ids::MENU_BAR_LIGHT_THEME
        } else {
            ids::MENU_BAR_DARK_THEME
        }
    );
    let glyph = svg(if dark_bar { SUN.clone() } else { MOON.clone() })
        .width(15)
        .height(15)
        .style(move |_: &iced::Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        });
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
    chrome_tooltip(btn.boxed(), label, hover, border, ink)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `(x, w)` of a slot that must be the field.
    fn field(slot: SearchSlot) -> (f32, f32) {
        match slot {
            SearchSlot::Field { x, w } => (x, w),
            SearchSlot::Button { .. } => panic!("expected the field, got {slot:?}"),
        }
    }

    /// The en-US in-window menu row (eight roots once Reports and Window
    /// fold into Tools and View).
    fn menu_row_w() -> f32 {
        crate::menu_bar::approx_graphite_menu_bar_width(&signex_i18n::I18n::en_us())
    }

    #[test]
    fn the_field_is_at_most_400_and_centred_on_a_wide_window() {
        let (x, w) = field(search_geometry(1600.0, 0.0, 132.0, 440.0));
        assert_eq!(w, 400.0);
        assert_eq!(x, (1600.0 - 400.0) / 2.0);
    }

    #[test]
    fn eight_menu_roots_do_not_shrink_the_field_on_a_laptop_window() {
        let menu_w = menu_row_w();
        let (x, w) = field(search_geometry(1440.0, 0.0, 138.0, menu_w));
        assert_eq!(w, 400.0);
        assert!(x >= menu_w);
        assert!(x + w <= 1440.0 - 138.0 - RIGHT_CLUSTER_W);
    }

    #[test]
    fn the_field_clears_the_right_cluster_on_a_narrow_window() {
        let (x, w) = field(search_geometry(900.0, 0.0, 132.0, 440.0));
        assert!(x + w <= 900.0 - 132.0 - RIGHT_CLUSTER_W);
        assert!(x >= 440.0);
    }

    /// Mock #4, narrow window: below the field's minimum the title bar shows
    /// the square search button first in the right cluster, left of the
    /// theme toggle, and draws no field.
    #[test]
    fn a_window_too_narrow_for_the_field_shows_the_search_button() {
        let trailing_w = 138.0;
        let window_w = 700.0;
        let slot = search_geometry(window_w, 0.0, trailing_w, menu_row_w());
        let SearchSlot::Button { x } = slot else {
            panic!("expected the search button, got {slot:?}");
        };
        let toggle_x = window_w - trailing_w - RIGHT_CLUSTER_W;
        assert!(
            (x + SEARCH_BUTTON_W + SEARCH_BUTTON_GAP - toggle_x).abs() < 0.01,
            "the button does not sit just left of the theme toggle: x={x}"
        );
        assert_eq!(slot.anchor(), (x, SEARCH_BUTTON_W));
    }

    /// The field shows down to exactly its minimum width; one pixel less of
    /// room and it becomes the button.
    #[test]
    fn the_field_collapses_to_the_button_just_below_its_minimum() {
        // No controls: the right limit is the window less the cluster and gap.
        let right_limit = 1000.0 - RIGHT_CLUSTER_W - CHROME_SEARCH_RIGHT_GAP;
        let menu_w = right_limit - CHROME_SEARCH_LEFT_GAP - CHROME_SEARCH_BAR_MIN_WIDTH;
        let (_, w) = field(search_geometry(1000.0, 0.0, 0.0, menu_w));
        assert_eq!(w, CHROME_SEARCH_BAR_MIN_WIDTH);
        assert!(matches!(
            search_geometry(1000.0, 0.0, 0.0, menu_w + 1.0),
            SearchSlot::Button { .. }
        ));
    }

    /// The field is centred on the window, shrinks monotonically as the
    /// window narrows, and never runs under the menu items or the right
    /// cluster, for every window-control layout; where it would have to, the
    /// button stands in.
    #[test]
    fn search_bar_stays_centred_and_clamped() {
        let in_window = menu_row_w();
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
        for window_w in [700.0_f32, 1000.0, 1280.0, 1440.0, 1920.0, 3840.0] {
            let right_limit = window_w - (trailing_w + RIGHT_CLUSTER_W + CHROME_SEARCH_RIGHT_GAP);
            let (x, w) = match search_geometry(window_w, leading_w, trailing_w, menu_w) {
                SearchSlot::Field { x, w } => (x, w),
                SearchSlot::Button { .. } => {
                    assert!(
                        right_limit - menu_end < CHROME_SEARCH_BAR_MIN_WIDTH,
                        "the button replaced a field that had room at {window_w}"
                    );
                    continue;
                }
            };
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
        // Narrowing the window never widens the bar (the button is narrower
        // than any field).
        let width = |window_w| match search_geometry(window_w, leading_w, trailing_w, menu_w) {
            SearchSlot::Field { w, .. } => w,
            SearchSlot::Button { .. } => 0.0,
        };
        assert!(
            width(1100.0) <= width(1920.0),
            "bar grew as the window shrank"
        );
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
