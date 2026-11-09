//! Graphite helpers.

use super::graphite::*;
use super::tokens::resolve;
use iced::{Theme, Vector};
use signex_types::theme::{ThemeId, theme_tokens};

/// Floating surfaces get (0, 6) / 20 in the theme's own shadow colour.
#[test]
fn the_floating_shadow_uses_the_graphite_token() {
    for id in ThemeId::BUILTINS {
        let t = theme_tokens(*id);
        let got = floating_shadow(&t);
        assert_eq!(got.offset, Vector::new(0.0, 6.0), "{id:?}");
        assert_eq!(got.blur_radius, 20.0, "{id:?}");
        assert_eq!(got.color, resolve(&t).shadow, "{id:?}");
    }
}

/// The splitter is transparent with a single 1 px edge.
#[test]
fn the_splitter_is_a_one_px_line() {
    for id in ThemeId::BUILTINS {
        let t = theme_tokens(*id);
        for horizontal in [true, false] {
            let style = resize_handle_edge(&t, horizontal)(&Theme::Dark);
            assert!(style.background.is_none(), "{id:?}");
            let b = style.border;
            let widths = [b.top.width, b.right.width, b.bottom.width, b.left.width];
            assert_eq!(widths.iter().flatten().count(), 1, "{id:?}");
        }
    }
}

/// Every tone has a visible ink on every theme.
#[test]
fn every_pill_tone_has_a_visible_ink() {
    for id in ThemeId::BUILTINS {
        let t = theme_tokens(*id);
        for tone in [
            PillTone::Neutral,
            PillTone::Accent,
            PillTone::Ok,
            PillTone::Warn,
            PillTone::Err,
            PillTone::Info,
        ] {
            assert!(pill_colors(&t, tone).1.a > 0.0, "{id:?} {tone:?}");
        }
    }
}
