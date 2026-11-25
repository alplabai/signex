//! The recovery prompt as drawn (#2262, Graphite mockup row 15).

use std::time::{Duration, SystemTime};

use signex_i18n::{DateLength, I18n, LanguagePref};

use super::{clock_time, elided_path, instant_before, saved_caption_chars};
use crate::app::Signex;
use crate::autosave::RecoveryOffer;
use crate::layout_probe::{LaidOutText, find, lay_out_texts};

/// 2023-11-14 22:13:20 UTC.
fn checked_at() -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000)
}

fn offer(older_than_file: bool) -> RecoveryOffer {
    RecoveryOffer {
        path: "power.snxsch".into(),
        age_secs: if older_than_file { 2 * 86_400 } else { 360 },
        older_than_file,
        saved_age_secs: Some(if older_than_file { 600 } else { 1_800 }),
        checked_at: checked_at(),
    }
}

fn plain(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

fn laid_out(offer: &RecoveryOffer) -> Vec<LaidOutText> {
    let (app, _boot) = Signex::new();
    lay_out_texts(
        app.view_autosave_recovery_window(offer),
        iced::Size::new(540.0, 600.0),
    )
}

/// The time of each version goes through the locale formatter: the time of
/// day within a day of the check, the date and time beyond it. Never a
/// fixed English pattern.
#[test]
fn version_times_are_the_time_of_day_or_the_date_beyond_a_day() {
    for tag in ["en-US", "de", "tr-TR"] {
        let i18n = I18n::new(&LanguagePref::Locale(tag.to_owned()));
        for age in [0, 360, 86_399] {
            assert_eq!(
                clock_time(&i18n, checked_at(), age),
                i18n.time(instant_before(checked_at(), age)),
                "{tag} {age}"
            );
        }
        for age in [86_400, 2 * 86_400] {
            assert_eq!(
                clock_time(&i18n, checked_at(), age),
                i18n.date_time(instant_before(checked_at(), age), DateLength::Medium),
                "{tag} {age}"
            );
        }
    }
    assert_eq!(
        instant_before(checked_at(), 360).timestamp(),
        1_700_000_000 - 360
    );
}

/// The "newer" pill sits on the row of the newer version: the copy's row
/// (first) or the saved file's row (second).
#[test]
fn the_newer_pill_marks_the_newer_version() {
    for older in [false, true] {
        let texts = laid_out(&offer(older));
        let copy = find(&texts, "Recovery copy").bounds;
        let saved = find(&texts, "Saved file").bounds;
        let pills: Vec<_> = texts.iter().filter(|t| t.text == "newer").collect();
        assert_eq!(pills.len(), 1, "one pill: {texts:#?}");
        let pill = pills[0].bounds;
        let on_copy_row = pill.y < saved.y && pill.y + pill.height > copy.y;
        assert_eq!(on_copy_row, !older, "older_than_file {older}: {pill:?}");
        if older {
            assert!(pill.y + pill.height > saved.y, "on the saved row: {pill:?}");
        }
    }
}

/// Copy newer: the Restore explanation (old hint, first part) in the info
/// callout, then the Discard / Decide later caption. Saved file newer: the
/// warning, then the same Restore explanation (its "cannot be undone"
/// sentence matters most here) and the caption. Both rows give their time.
#[test]
fn the_notes_follow_which_version_is_newer() {
    let restore = "Restore loads the copy as unsaved edits;";
    let warning = "power.snxsch was saved after this copy was written.";
    let later = "Discard deletes the copy. Decide later keeps it";
    let shown = |older| -> Vec<String> {
        laid_out(&offer(older))
            .iter()
            .map(|t| plain(&t.text))
            .collect()
    };
    let newer_copy = shown(false);
    let has = |texts: &[String], needle: &str| texts.iter().any(|t| t.contains(needle));
    assert!(has(&newer_copy, restore), "{newer_copy:#?}");
    assert!(!has(&newer_copy, warning));
    assert!(has(&newer_copy, later));
    assert!(has(
        &newer_copy,
        "Signex found a recovery copy of power.snxsch that differs from the saved file."
    ));
    assert!(has(&newer_copy, "autosaved 6 minutes ago · "));
    assert!(has(&newer_copy, "power.snxsch · "));

    let newer_file = shown(true);
    assert!(has(&newer_file, warning), "{newer_file:#?}");
    assert!(has(&newer_file, restore), "the undo warning is kept");
    assert!(has(&newer_file, "cannot be undone with"));
    assert!(has(&newer_file, later), "the Decide later caption is kept");
    assert!(has(&newer_file, "autosaved 2 days ago · "));
}

/// Mockup row 15: a long saved path gives up its middle, so the line fits
/// beside the pill and keeps the file name and the time.
#[test]
fn a_long_saved_path_keeps_its_file_name_and_time() {
    let path = format!("/home/user/{}/power.snxsch", "deep/folder/".repeat(12));
    let cut = elided_path(&path, 30);
    assert_eq!(cut.chars().count(), 30, "{cut}");
    assert!(cut.starts_with("/home") && cut.ends_with("power.snxsch"));
    assert_eq!(elided_path("power.snxsch", 30), "power.snxsch");

    let long = RecoveryOffer {
        path: path.into(),
        ..offer(true)
    };
    let shown: Vec<String> = laid_out(&long).iter().map(|t| plain(&t.text)).collect();
    let line = shown
        .iter()
        .find(|t| t.contains("power.snxsch · "))
        .expect("the saved row");
    assert!(line.contains('…'), "{line}");
    assert!(line.chars().count() <= saved_caption_chars(540.0), "{line}");
}

/// Mockup row 15: in a window narrower than the card the character estimate
/// assumes, the saved line still ends before the "newer" pill, and the pill
/// stays inside the window.
#[test]
fn a_long_saved_path_in_a_narrow_window_leaves_the_pill_clear() {
    let path = format!("/home/user/{}/power.snxsch", "deep/folder/".repeat(12));
    let long = RecoveryOffer {
        path: path.into(),
        ..offer(true)
    };
    let (app, _boot) = Signex::new();
    let window = iced::Size::new(360.0, 600.0);
    let texts = lay_out_texts(app.view_autosave_recovery_window(&long), window);
    let pill = find(&texts, "newer").bounds;
    let line = texts
        .iter()
        .find(|t| plain(&t.text).contains("power.snxsch · "))
        .expect("the saved row")
        .bounds;
    assert!(
        line.x + line.width <= pill.x + 0.5,
        "{line:?} under {pill:?}"
    );
    assert!(pill.x + pill.width <= window.width + 0.5, "{pill:?}");
}

/// Mockup row 15: a caption wider than its row gives up its middle to an
/// ellipsis and is never drawn past the row's edge.
#[test]
fn a_caption_wider_than_its_row_is_cut_inside_it() {
    use iced::widget::{Space, container, row};
    use iced::{Element, Length, Widget as _};
    use signex_i18n::UserText;
    use signex_types::theme::{ThemeId, theme_tokens};

    const ROOM: f32 = 120.0;
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    let window = iced::Size::new(4.0 * ROOM, 60.0);
    let label = "/home/user/deep/folder/deep/folder/power.snxsch · 14:02";
    let cell =
        container(super::caption(UserText::new(label), true, &tokens)).width(Length::Fixed(ROOM));
    let line: Element<'_, crate::app::Message> =
        row![cell, Space::new().width(Length::Fill)].boxed();
    let pixels = crate::layout_probe::draw_pixels(line, window);
    let width = window.width as usize;
    let inked = |xs: std::ops::Range<usize>| {
        pixels
            .chunks_exact(4)
            .enumerate()
            .filter(|(i, px)| xs.contains(&(i % width)) && px[..3].iter().any(|c| *c > 24))
            .count()
    };
    let room = ROOM as usize;
    assert!(inked(0..room) > 0, "the caption is drawn");
    assert_eq!(inked(room + 1..width), 0, "drawn past its row");
}

/// The footer order is unchanged by the redraw: Restore, then Discard, at
/// the left; Decide later alone at the right. The mockup row 15 split is an open
/// maintainer question.
#[test]
fn restore_and_discard_lead_and_decide_later_ends_the_footer() {
    for older in [false, true] {
        let texts = laid_out(&offer(older));
        let restore = find(&texts, "Restore").bounds;
        let discard = find(&texts, "Discard").bounds;
        let later = find(&texts, "Decide later").bounds;
        assert!(restore.x < discard.x && discard.x < later.x, "{texts:#?}");
        assert!(
            later.x - (discard.x + discard.width) > 100.0,
            "Decide later stands apart"
        );
    }
}
