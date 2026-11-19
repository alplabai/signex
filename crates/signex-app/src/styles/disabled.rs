//! Disabled and read-only styling. A disabled control fades as a whole to
//! [`DISABLED_OPACITY`]; disabled text inside an enabled surface uses
//! `ink4` ([`super::tokens::disabled_text`]); a read-only field keeps its
//! frame on a sunken fill with `ink3` text. A feature's dependent fields use
//! these when its enable is off.

use iced::widget::container;
use iced::{Background, Border, Color, Theme};
use signex_types::theme::ThemeTokens;

pub use signex_widgets::theme_ext::{DISABLED_OPACITY, dimmed, dimmed_button};

use super::radii::Radii;
use super::ti;
use super::tokens::resolve;

/// Alpha of the sunken fill behind a read-only property row. A wash, not a
/// disabled fade, so not [`DISABLED_OPACITY`]: the Graphite panels mockup
/// mixes the row as `color-mix(in srgb, var(--sunken) 55%, transparent)`
/// (`.prm.ro`, panels.html).
const READ_ONLY_ROW_ALPHA: f32 = 0.55;

/// Read-only field frame: sunken fill, `ink3` text, the field's hairline
/// border and control radius.
pub fn read_only_field(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let r = resolve(tokens);
    let (bg, ink, line) = (r.sunken, r.ink3, ti(tokens.border));
    move |_| container::Style {
        background: Some(Background::Color(bg)),
        text_color: Some(ink),
        border: Border {
            width: 1.0,
            color: line,
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// Read-only property row: a light sunken wash under `ink3` text.
pub fn read_only_row(tokens: &ThemeTokens) -> impl Fn(&Theme) -> container::Style + 'static {
    let r = resolve(tokens);
    let bg = Color {
        a: r.sunken.a * READ_ONLY_ROW_ALPHA,
        ..r.sunken
    };
    let ink = r.ink3;
    move |_| container::Style {
        background: Some(Background::Color(bg)),
        text_color: Some(ink),
        ..container::Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::super::tokens::{contrast_ratio, disabled_text};
    use super::*;
    use signex_types::theme::{ThemeId, theme_tokens};

    /// Disabled text is `ink4` and a read-only field sits on `sunken` with
    /// `ink3` text, on both themes.
    #[test]
    fn disabled_and_read_only_use_the_faint_tokens() {
        for &id in ThemeId::BUILTINS {
            let t = theme_tokens(id);
            let r = resolve(&t);
            assert_eq!(disabled_text(&t), r.ink4, "{id:?}");
            let field = read_only_field(&t)(&Theme::Dark);
            assert_eq!(
                field.background,
                Some(Background::Color(r.sunken)),
                "{id:?}"
            );
            assert_eq!(field.text_color, Some(r.ink3), "{id:?}");
            let row = read_only_row(&t)(&Theme::Dark);
            let Some(Background::Color(wash)) = row.background else {
                panic!("{id:?}: read-only row has no fill");
            };
            assert!(wash.a < r.sunken.a, "{id:?}");
        }
    }

    /// Disabled ink stays readable but fainter than running text.
    #[test]
    fn disabled_ink_is_fainter_than_body_text() {
        for &id in ThemeId::BUILTINS {
            let t = theme_tokens(id);
            let panel = ti(t.panel_bg);
            let body = contrast_ratio(ti(t.text), panel);
            let faint = contrast_ratio(disabled_text(&t), panel);
            assert!(faint < body, "{id:?}");
            assert!(faint > 1.5, "{id:?}");
        }
    }
}
