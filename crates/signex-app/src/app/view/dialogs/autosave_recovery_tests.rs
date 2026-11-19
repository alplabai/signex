//! The recovery prompt as drawn (#2262, Graphite mockup m15).

use std::time::{Duration, SystemTime};

use super::{clock_time, restore_kind};
use crate::app::Signex;
use crate::autosave::RecoveryOffer;
use crate::layout_probe::{LaidOutText, find, lay_out_texts};
use crate::styles::DialogButtonKind;

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

/// The time of each version is local wall-clock time: the time of day
/// within a day of the check, the date and time beyond it.
#[test]
fn version_times_are_the_time_of_day_or_the_date_beyond_a_day() {
    let zone = chrono::FixedOffset::east_opt(3600).expect("offset");
    assert_eq!(clock_time(checked_at(), 0, &zone), "23:13");
    assert_eq!(clock_time(checked_at(), 360, &zone), "23:07");
    assert_eq!(clock_time(checked_at(), 86_399, &zone), "23:13");
    assert_eq!(clock_time(checked_at(), 2 * 86_400, &zone), "Nov 12 23:13");
    let utc = chrono::Utc;
    assert_eq!(clock_time(checked_at(), 3 * 86_400, &utc), "Nov 11 22:13");
}

/// Restore is the primary action only while the copy is the newer version;
/// when the saved file is newer the footer has no primary button.
#[test]
fn restore_is_primary_only_when_the_copy_is_newer() {
    assert_eq!(restore_kind(&offer(false)), DialogButtonKind::Primary);
    assert_eq!(restore_kind(&offer(true)), DialogButtonKind::Secondary);
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
/// warning instead of the explanation, the caption kept. Both rows give
/// their time.
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
    assert!(!has(&newer_file, restore));
    assert!(has(&newer_file, later), "the Decide later caption is kept");
    assert!(has(&newer_file, "autosaved 2 days ago · "));
}

/// Discard sits alone at the left; Decide later, then Restore, at the right.
#[test]
fn discard_leads_and_restore_ends_the_footer() {
    let texts = laid_out(&offer(false));
    let discard = find(&texts, "Discard").bounds;
    let later = find(&texts, "Decide later").bounds;
    let restore = find(&texts, "Restore").bounds;
    assert!(discard.x < later.x && later.x < restore.x);
    assert!(
        later.x - (discard.x + discard.width) > 100.0,
        "Discard stands apart"
    );
}
