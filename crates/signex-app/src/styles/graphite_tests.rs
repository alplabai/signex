//! Graphite helpers: only Graphite themes change; legacy themes keep their look.

use super::graphite::*;
use super::ti;
use super::tokens::resolve;
use iced::{Background, Color, Shadow, Theme, Vector};
use signex_types::theme::{ThemeId, theme_tokens};

fn legacy_shadow() -> Shadow {
    Shadow {
        color: Color::from_rgba(0.0, 0.0, 0.0, 0.4),
        offset: Vector::new(2.0, 3.0),
        blur_radius: 12.0,
    }
}

/// Legacy themes keep the shadow the call site passes; Graphite themes get
/// (0, 6) / 20 in their own shadow colour.
#[test]
fn only_graphite_themes_change_the_floating_shadow() {
    for id in ThemeId::BUILTINS {
        let t = theme_tokens(*id);
        let got = floating_shadow(&t, legacy_shadow());
        if is_graphite(&t) {
            assert_eq!(got.offset, Vector::new(0.0, 6.0), "{id:?}");
            assert_eq!(got.blur_radius, 20.0, "{id:?}");
            assert_eq!(got.color, resolve(&t).shadow, "{id:?}");
        } else {
            assert_eq!(got, legacy_shadow(), "{id:?}");
        }
    }
}

/// The legacy splitter is still the filled border colour; the Graphite one
/// is transparent with a single 1 px edge.
#[test]
fn the_splitter_is_a_one_px_line_only_in_graphite() {
    for id in ThemeId::BUILTINS {
        let t = theme_tokens(*id);
        for horizontal in [true, false] {
            let style = resize_handle_edge(&t, horizontal)(&Theme::Dark);
            if is_graphite(&t) {
                assert!(style.background.is_none(), "{id:?}");
                let b = style.border;
                let widths = [b.top.width, b.right.width, b.bottom.width, b.left.width];
                assert_eq!(widths.iter().flatten().count(), 1, "{id:?}");
            } else {
                assert_eq!(
                    style.background,
                    Some(Background::Color(ti(t.border))),
                    "{id:?}"
                );
            }
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
