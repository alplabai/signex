//! The pure parts of the Project tab look: the glyph sources, the button
//! metrics and the pill tones on both Graphite themes.

use iced::Widget as _;
use signex_i18n::{I18n, ids, t};
use signex_types::theme::{ThemeId, theme_tokens};

use super::*;
use crate::styles::tokens::resolve;

/// Every glyph is its own well-formed 24 x 24 SVG (a broken one paints an
/// empty square), and `ALL` lists each glyph at its own index.
#[test]
fn every_glyph_is_its_own_well_formed_svg() {
    for (i, g) in Glyph::ALL.iter().enumerate() {
        assert_eq!(g.index(), i, "{g:?}");
        let src = std::str::from_utf8(g.source()).expect("utf-8");
        assert!(src.starts_with("<svg") && src.ends_with("</svg>"), "{g:?}");
        assert!(src.contains(r#"viewBox="0 0 24 24""#), "{g:?}");
        assert_eq!(src.matches('<').count(), src.matches('>').count(), "{g:?}");
        for other in &Glyph::ALL[i + 1..] {
            assert_ne!(g.source(), other.source(), "{g:?} / {other:?}");
        }
    }
}

/// The pill tones the sections use map to the token pairs of the mockup's
/// `.pill`, `.pill.ok` and `.pill.err` on both themes.
#[test]
fn pill_tones_use_the_mockup_fills() {
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let t = theme_tokens(id);
        let r = resolve(&t);
        assert_eq!(pill_colors(&t, PillTone::Neutral), (r.sunken, r.ink3));
        assert_eq!(pill_colors(&t, PillTone::Ok), (r.ok_soft, ti(t.success)));
        assert_eq!(pill_colors(&t, PillTone::Err), (r.err_soft, ti(t.error)));
    }
}

/// Button heights follow the mockup: 26 px, 30 px large, 22 px small and
/// ghost (with caption text).
#[test]
fn button_kinds_have_the_mockup_heights() {
    let close = |a: f32, b: f32| (a - b).abs() < 0.01;
    assert!(close(ButtonKind::Primary.metrics().0, 26.0));
    assert!(close(ButtonKind::Outline.metrics().0, 26.0));
    assert!(close(ButtonKind::PrimaryLarge.metrics().0, 30.0));
    assert!(close(ButtonKind::Small.metrics().0, 22.0));
    assert!(close(ButtonKind::Ghost.metrics().0, 22.0));
    assert!(close(ButtonKind::Small.metrics().2, TS.caption));
    assert!(close(ICON_BUTTON, 24.0));
}

/// Every piece builds on both themes, enabled and disabled.
#[test]
fn every_piece_builds_on_both_themes() {
    let i18n = I18n::en_us();
    let project_id = crate::app::state::ProjectId::mint(&mut 0);
    let pressed = || {
        crate::app::Message::Project(crate::app::ProjectMsg::Tab(
            super::super::ProjectTabMsg::RenameRequested { project_id },
        ))
    };
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let t = theme_tokens(id);
        let words = || t!(i18n, ids::PROJECT_MOVE_UP);
        let note = || text(words()).boxed();
        let _ = card(
            Some(CardHeader {
                title: words(),
                trailing: vec![note()],
            }),
            note(),
            &t,
        );
        let _ = card(None, note(), &t);
        let _ = pill(words(), PillTone::Neutral, Some(Glyph::Lock), &t);
        let _ = pill(words(), PillTone::Err, None, &t);
        let _ = info_line(note(), &t);
        let _ = warn_line(Glyph::Warn, note(), Some(note()), &t);
        let _ = err_box(note(), &t);
        let _ = kv_row(words(), note(), &t);
        let _ = option_row(words(), words(), note(), &t);
        let _ = field_label(words(), &t);
        let _ = icon_button(Glyph::Up, words(), Some(pressed()), &t);
        let _ = icon_button(Glyph::Down, words(), None, &t);
        for kind in [
            ButtonKind::Primary,
            ButtonKind::PrimaryLarge,
            ButtonKind::Outline,
            ButtonKind::Small,
            ButtonKind::Ghost,
        ] {
            let _ = text_button(Some(Glyph::Plus), words(), Some(pressed()), kind, &t);
            let _ = text_button(None, words(), None, kind, &t);
        }
    }
}
