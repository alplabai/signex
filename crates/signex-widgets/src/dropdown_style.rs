//! The one Graphite look of every `pick_list` and its menu.
//!
//! Call sites add [`GraphiteDropdown::graphite`] right after `pick_list(..)`;
//! none keeps its own colours, font or size: the closed field reads in the UI
//! face at the menu label size, the same as every menu row. The look is found from the theme's name (the
//! Graphite themes only), so the style closures need no tokens. On any other
//! theme the stock iced style applies.

use std::borrow::Borrow;

use iced::overlay::menu;
use iced::widget::{PickList, pick_list};
use iced::{Background, Border, Color, Shadow, Theme, Vector};
use signex_types::theme::{ThemeId, ThemeTokens, theme_tokens};

use crate::active_bar::dropdown::MENU_LABEL_SIZE;
use crate::theme_ext::{accent, border_color, text_primary, text_secondary, to_color};

/// Corner radius of the closed field.
pub const FIELD_RADIUS: f32 = 7.0;
/// Corner radius of the open menu card.
pub const MENU_RADIUS: f32 = 10.0;
/// Family of the closed field's text: the bundled UI face.
pub const FIELD_FONT_FAMILY: &str = "Inter";
/// Text size of the closed field and its menu rows: the menu label token, so
/// a dropdown reads like the menus.
pub const FIELD_TEXT_SIZE: f32 = MENU_LABEL_SIZE;
/// Alpha of the row hover wash: the ink colour over the card.
const HOVER_ALPHA: f32 = 0.08;
/// Shadow strength when the theme sets none.
const SHADOW_FALLBACK_ALPHA: f32 = 0.35;

/// Colours of one Graphite dropdown, all from the theme tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DropdownLook {
    pub field: Color,
    pub ink: Color,
    pub ink3: Color,
    pub ink4: Color,
    pub line: Color,
    pub line_strong: Color,
    pub accent: Color,
    pub card: Color,
    pub hover: Color,
    pub shadow: Color,
}

impl DropdownLook {
    /// The look of `tokens`, or `None` for a theme that sets no extended token.
    pub fn of(tokens: &ThemeTokens) -> Option<Self> {
        let e = &tokens.extended;
        if e.is_empty() {
            return None;
        }
        let ink = text_primary(tokens);
        let panel = to_color(&tokens.panel_bg);
        let pick = |set: &Option<signex_types::theme::Color>, fallback: Color| {
            set.as_ref().map_or(fallback, to_color)
        };
        Some(Self {
            field: pick(&e.field, panel),
            ink,
            ink3: pick(&e.ink3, text_secondary(tokens)),
            ink4: pick(
                &e.ink4,
                Color {
                    a: 0.5,
                    ..text_secondary(tokens)
                },
            ),
            line: border_color(tokens),
            line_strong: pick(&e.line_strong, border_color(tokens)),
            accent: accent(tokens),
            card: pick(&e.raised, panel),
            hover: Color {
                a: HOVER_ALPHA,
                ..ink
            },
            shadow: pick(
                &e.shadow,
                Color::from_rgba(0.0, 0.0, 0.0, SHADOW_FALLBACK_ALPHA),
            ),
        })
    }

    /// The look of the Graphite theme `theme` names, else `None`.
    pub fn of_theme(theme: &Theme) -> Option<Self> {
        let name = theme.to_string();
        let id = if name.starts_with("Graphite Light") {
            ThemeId::GraphiteLight
        } else if name.starts_with("Graphite Dark") {
            ThemeId::GraphiteDark
        } else {
            return None;
        };
        Self::of(&theme_tokens(id))
    }

    /// The closed field: field fill and hairline, the accent ring while open.
    pub fn field_style(&self, status: pick_list::Status) -> pick_list::Style {
        let border = match status {
            pick_list::Status::Opened { .. } => self.accent,
            pick_list::Status::Hovered => self.line_strong,
            _ => self.line,
        };
        pick_list::Style {
            text_color: self.ink,
            placeholder_color: self.ink4,
            handle_color: self.ink3,
            background: Background::Color(self.field),
            border: Border {
                width: 1.0,
                radius: FIELD_RADIUS.into(),
                color: border,
                ..Border::default()
            },
        }
    }

    /// The open menu: a raised card; the row under the pointer gets the hover
    /// wash and accent text.
    pub fn menu_style(&self) -> menu::Style {
        menu::Style {
            background: Background::Color(self.card),
            border: Border {
                width: 1.0,
                radius: MENU_RADIUS.into(),
                color: self.line,
                ..Border::default()
            },
            text_color: self.ink,
            selected_text_color: self.accent,
            selected_background: Background::Color(self.hover),
            shadow: Shadow {
                color: self.shadow,
                offset: Vector::new(0.0, 6.0),
                blur_radius: 20.0,
            },
        }
    }
}

/// `pick_list` field style: Graphite, else the stock style.
pub fn pick_list_style(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    match DropdownLook::of_theme(theme) {
        Some(look) => look.field_style(status),
        None => pick_list::default(theme, status),
    }
}

/// `pick_list` menu style: Graphite, else the stock style.
pub fn pick_list_menu_style(theme: &Theme) -> menu::Style {
    match DropdownLook::of_theme(theme) {
        Some(look) => look.menu_style(),
        None => menu::default(theme),
    }
}

/// The face of every dropdown field.
#[must_use]
pub fn field_font() -> iced::Font {
    iced::Font::with_family(iced::font::Family::name(FIELD_FONT_FAMILY))
}

/// Gives a `pick_list` the shared look.
pub trait GraphiteDropdown {
    /// Field and menu styled by [`pick_list_style`] / [`pick_list_menu_style`],
    /// set in [`field_font`] at [`FIELD_TEXT_SIZE`].
    #[must_use]
    fn graphite(self) -> Self;
}

impl<'a, T, L, V, M> GraphiteDropdown for PickList<'a, T, L, V, M, Theme>
where
    T: PartialEq + Clone,
    L: Borrow<[T]> + 'a,
    V: Borrow<T> + 'a,
    M: Clone,
{
    fn graphite(self) -> Self {
        self.style(pick_list_style)
            .menu_style(pick_list_menu_style)
            .font(field_font())
            .text_size(FIELD_TEXT_SIZE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::widget::pick_list::Status;

    fn theme(id: ThemeId) -> Theme {
        let name = match id {
            ThemeId::GraphiteLight => "Graphite Light",
            _ => "Graphite Dark",
        };
        Theme::custom(
            name.to_string(),
            iced::theme::palette::Seed {
                background: Color::BLACK,
                text: Color::WHITE,
                primary: Color::WHITE,
                success: Color::WHITE,
                danger: Color::WHITE,
                warning: Color::WHITE,
            },
        )
    }

    #[test]
    fn a_graphite_dropdown_reads_in_the_ui_face_at_the_menu_label_size() {
        assert_eq!(FIELD_TEXT_SIZE, MENU_LABEL_SIZE);
        assert_eq!(FIELD_TEXT_SIZE, 12.0);
        assert_eq!(
            field_font(),
            iced::Font::with_family(iced::font::Family::name("Inter"))
        );
        // `.graphite()` applies both to the widget.
        let _: PickList<'_, u8, Vec<u8>, u8, (), Theme> =
            pick_list(Some(1u8), vec![1u8], |n: &u8| n.to_string()).graphite();
    }

    #[test]
    fn a_graphite_dropdown_paints_with_the_theme_tokens() {
        for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
            let tokens = theme_tokens(id);
            let e = &tokens.extended;
            let t = theme(id);

            let field = pick_list_style(&t, Status::Active);
            assert_eq!(
                field.background,
                Background::Color(to_color(e.field.as_ref().expect("field token")))
            );
            assert_eq!(field.border.color, border_color(&tokens));
            assert_eq!(field.text_color, text_primary(&tokens));
            let open = pick_list_style(&t, Status::Opened { is_hovered: false });
            assert_eq!(
                open.border.color,
                accent(&tokens),
                "focus ring is the accent"
            );

            let menu = pick_list_menu_style(&t);
            assert_eq!(
                menu.background,
                Background::Color(to_color(e.raised.as_ref().expect("raised token")))
            );
            assert_eq!(menu.border.color, border_color(&tokens));
            assert_eq!(menu.selected_text_color, accent(&tokens));
            assert_eq!(
                menu.selected_background,
                Background::Color(Color {
                    a: HOVER_ALPHA,
                    ..text_primary(&tokens)
                }),
                "the selected row is a wash, not an accent fill"
            );
        }
    }
}
