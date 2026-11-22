use std::path::Path;

use iced::Element;
use signex_i18n::{I18n, UserText, ids, t};
use signex_types::theme::{ThemeId, theme_tokens};
use signex_widgets::text;

use super::*;

fn body<'a>() -> Element<'a, ()> {
    text(UserText::new("power.snxsch".to_string())).boxed()
}

/// Every builder builds from the tokens alone, for every built-in theme.
#[test]
fn every_part_builds_for_every_theme() {
    let i18n = I18n::en_us_ref();
    for id in ThemeId::BUILTINS {
        let tokens = theme_tokens(*id);
        for tone in [CalloutTone::Info, CalloutTone::Warn, CalloutTone::Error] {
            let _: Element<'_, ()> = dialog_callout(tone, body(), &tokens);
        }
        let _: Element<'_, ()> = dialog_inline_error(body(), &tokens);
        let _: Element<'_, ()> = dialog_field_label(t!(i18n, ids::DIALOG_BUTTON_OK), &tokens);
        let _: Element<'_, ()> = dialog_section_label(t!(i18n, ids::DIALOG_BUTTON_OK), &tokens);
        let _: Element<'_, ()> = dialog_caption(UserText::new("8 of 9".to_string()), &tokens);
        let head = dialog_head_row(vec![body()], &tokens);
        let rows = vec![dialog_row(vec![body(), body()]), dialog_row(vec![body()])];
        let _: Element<'_, ()> = dialog_rows(Some(head), rows, Some(180.0), &tokens);
        let _: Element<'_, ()> = dialog_rows(None, vec![dialog_row(vec![body()])], None, &tokens);
        let _: Element<'_, ()> = dialog_rows(None, Vec::new(), Some(180.0), &tokens);
        for path in ["a.snxsch", "b.snxpcb", "l.snxlib/", "assets/", "x.txt"] {
            let _: Element<'_, ()> = file_kind_icon(Path::new(path));
        }
        let _: Element<'_, ()> = dirty_dot(&tokens);
    }
}

/// The callout tones take the soft fills and inks of their status: the
/// neutral note on the sunken fill in muted ink, warn and error on their
/// soft tints.
#[test]
fn callout_tones_take_their_status_colours() {
    for id in ThemeId::BUILTINS {
        let tokens = theme_tokens(*id);
        let r = resolve(&tokens);
        assert_eq!(CalloutTone::Info.colors(&tokens), (r.sunken, r.ink3));
        assert_eq!(
            CalloutTone::Warn.colors(&tokens),
            (r.warn_soft, ti(tokens.warning))
        );
        assert_eq!(
            CalloutTone::Error.colors(&tokens),
            (r.err_soft, ti(tokens.error))
        );
    }
}

/// The file kinds follow the Projects tree's kind tags: a sheet is a
/// schematic, the board a PCB, a library folder a library, a BOM and an
/// output job by their extensions, and a path ending in a separator is a
/// folder unless its name says more.
#[test]
fn file_kinds_follow_the_extension() {
    let kind = |p: &str| FileKind::of(Path::new(p)).0;
    assert_eq!(kind("dcdc-buck.snxprj"), FileKind::Project);
    assert_eq!(kind("power.snxsch"), FileKind::Schematic);
    assert_eq!(kind("POWER.SNXSCH"), FileKind::Schematic);
    assert_eq!(kind("dcdc-buck.snxpcb"), FileKind::Pcb);
    assert_eq!(kind("dcdc-buck.snxlib/"), FileKind::Library);
    assert_eq!(kind("parts.snxsym"), FileKind::Library);
    assert_eq!(kind("dcdc-buck.snxbom"), FileKind::Bom);
    assert_eq!(kind("fab.snxout"), FileKind::Output);
    assert_eq!(kind("assets/"), FileKind::Folder);
    assert_eq!(kind("assets\\"), FileKind::Folder);
    assert_eq!(kind("notes.txt"), FileKind::Other);
    assert_eq!(FileKind::of(Path::new("assets/")).1, TreeIcon::Folder);
}

/// #2756: a file icon draws in its asset's colours. Pixels are grouped by
/// their channel ratios, so edge smoothing of one colour stays one group; a
/// tint over the whole shape would leave a single real group.
#[test]
fn a_file_icon_keeps_its_asset_colours() {
    const MIN_PIXELS: usize = 3;
    let window = iced::Size::new(32.0, 32.0);
    for path in ["a.snxsch", "b.snxpcb", "l.snxlib/", "assets/", "x.txt"] {
        let pixels =
            crate::layout_probe::draw_pixels(file_kind_icon::<()>(Path::new(path)), window);
        let mut hues: Vec<[u8; 3]> = pixels
            .chunks_exact(4)
            .filter(|p| p[0].max(p[1]).max(p[2]) > 96)
            .map(|p| {
                let top = u16::from(p[0].max(p[1]).max(p[2]));
                [p[0], p[1], p[2]].map(|c| (u16::from(c) * 8 / (top + 1)) as u8)
            })
            .collect();
        hues.sort_unstable();
        let colours = hues
            .chunk_by(|a, b| a == b)
            .filter(|group| group.len() >= MIN_PIXELS)
            .count();
        assert!(colours > 1, "{path}: {colours} colour(s)");
    }
}

/// The violet pair follows the panel: the dark pair on Graphite Dark, the
/// light pair on Graphite Light.
#[test]
fn the_violet_pair_follows_the_panel() {
    let dark = violet_pair(&theme_tokens(ThemeId::GraphiteDark));
    let light = violet_pair(&theme_tokens(ThemeId::GraphiteLight));
    assert_eq!(dark.1, iced::Color::from_rgb8(0xC9, 0xB3, 0xEE));
    assert_eq!(light.1, iced::Color::from_rgb8(0x6B, 0x3F, 0xA0));
    assert!(relative_luminance(dark.0) < relative_luminance(light.0));
}

/// A long list stops at its maximum height and scrolls; a short one hugs
/// its rows.
#[test]
fn a_long_list_stops_at_its_maximum_height() {
    let tokens = theme_tokens(ThemeId::GraphiteLight);
    let rows = |n: usize| -> Vec<Element<'static, ()>> {
        (0..n)
            .map(|i| dialog_row(vec![text(UserText::new(format!("row {i}"))).boxed()]))
            .collect()
    };
    let room = iced::Size::new(400.0, 2000.0);
    let tall =
        crate::layout_probe::root_size(dialog_rows(None, rows(40), Some(180.0), &tokens), room);
    assert!(tall.height <= 180.0 + 2.0, "{tall:?}");
    let short =
        crate::layout_probe::root_size(dialog_rows(None, rows(2), Some(180.0), &tokens), room);
    assert!(short.height < 180.0, "{short:?}");
    assert!(
        short.height >= 2.0 * 30.0,
        "{short:?}: rows keep their least height"
    );
}
