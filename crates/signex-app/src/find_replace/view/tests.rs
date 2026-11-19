//! The Find and Replace view (#67, #2525): its English, the result groups,
//! the include count and the laid-out dialog in both Graphite themes.

use std::path::PathBuf;

use signex_engine::{FindHit, FindKind, TextTarget};
use signex_i18n::{Arg, I18n, LanguagePref, ids};
use signex_types::schematic::{Point, SelectedItem, SelectedKind};
use signex_types::theme::{ThemeId, theme_tokens};

use super::footer::{position_line, replace_label};
use super::form::pill_text;
use super::results::{ResultRow, groups, segments};
use crate::find_replace::{FindMatch, FindReplaceState, FindScope, ReplaceJob};

fn plain(text: impl ToString) -> String {
    text.to_string().replace(['\u{2068}', '\u{2069}'], "")
}

fn found(n: u128, kind: FindKind, text: &str, sheet: &str) -> FindMatch {
    let uuid = uuid::Uuid::from_u128(n);
    let (selected, target) = match kind {
        FindKind::NetLabel => (SelectedKind::Label, TextTarget::Label(uuid)),
        FindKind::TextNote => (SelectedKind::TextNote, TextTarget::TextNote(uuid)),
        FindKind::Designator => (
            SelectedKind::SymbolRefField,
            TextTarget::SymbolReference(uuid),
        ),
        FindKind::Value => (SelectedKind::SymbolValField, TextTarget::SymbolValue(uuid)),
    };
    FindMatch {
        sheet: Some(PathBuf::from(format!("{sheet}.snxsch"))),
        hit: FindHit {
            kind,
            item: SelectedItem::new(uuid, selected),
            target,
            text: text.into(),
            placement: None,
            single: false,
            position: Point::new(0.0, 0.0),
        },
        placement_label: None,
    }
}

/// The #67 sample: Replace mode, query VOUT -> VOUT_3V3, whole word off so
/// all five substring results are listed, the second one current.
fn sample() -> FindReplaceState {
    let mut state = FindReplaceState {
        open: true,
        replace_mode: true,
        query: "VOUT".into(),
        replacement: "VOUT_3V3".into(),
        scope: FindScope::Project,
        matches: vec![
            found(1, FindKind::NetLabel, "VOUT_3V3", "dcdc"),
            found(2, FindKind::NetLabel, "VOUT", "mcu"),
            found(3, FindKind::NetLabel, "VOUT_SENSE", "dcdc"),
            found(4, FindKind::TextNote, "Keep VOUT trace short", "dcdc"),
            found(5, FindKind::Value, "VOUT_ADJ", "dcdc"),
        ],
        selected_index: Some(1),
        ..FindReplaceState::default()
    };
    state.options.whole_word = false;
    state
}

#[test]
fn the_pill_keeps_its_english_and_follows_the_language() {
    let en = I18n::en_us();
    let pill = |state: &FindReplaceState| pill_text(state, &en).map(|(text, _)| plain(text));
    let mut state = sample();
    assert_eq!(pill(&state).as_deref(), Some("5 matches"));
    state.matches.truncate(1);
    assert_eq!(pill(&state).as_deref(), Some("1 match"));
    state.matches.clear();
    assert_eq!(pill(&state).as_deref(), Some("0 matches"));
    state.scanning = true;
    assert_eq!(pill(&state).as_deref(), Some("Searching..."));
    state.replace_job = Some(ReplaceJob {
        generation: 0,
        epoch: 0,
        child_sheets: Default::default(),
        find: Default::default(),
        replacement: String::new(),
        matches: Vec::new(),
    });
    assert_eq!(pill(&state).as_deref(), Some("Replacing..."));

    let mut empty = FindReplaceState::default();
    assert_eq!(pill(&empty), None, "no pill while the field is empty");
    empty.query = "VOUT(".into();
    empty.options.regex = true;
    assert_eq!(pill(&empty).as_deref(), Some("Invalid pattern"));

    let xa = I18n::new(&LanguagePref::Locale("en-XA".to_owned()));
    let shown = pill_text(&sample(), &xa).map(|(text, _)| plain(text));
    assert_ne!(shown.as_deref(), Some("5 matches"));
    assert_eq!(
        shown.unwrap(),
        plain(xa.message_plain(ids::FIND_MATCHES, &[("count", Arg::from(5_usize))]))
    );
}

/// #2058: the find, find-similar and paste-array texts keep their English
/// and follow the language in force.
#[test]
fn the_find_texts_follow_the_language() {
    let en = I18n::en_us();
    let matches =
        |i18n: &I18n| i18n.message_plain(ids::FIND_MATCHES, &[("count", Arg::from(3_usize))]);
    assert_eq!(matches(&en), "3 matches");
    assert_eq!(
        en.message_plain(ids::PASTE_ARRAY_X_SPACING, &[("unit", Arg::from("mm"))]),
        "X spacing (mm)"
    );
    assert_eq!(
        en.message_plain(ids::FIND_SIMILAR_SEED, &[("seed", Arg::from("Symbol R1"))]),
        "Seed: Symbol R1"
    );
    let xa = I18n::new(&LanguagePref::Locale("en-XA".to_owned()));
    assert_ne!(matches(&xa), matches(&en));
    assert_ne!(
        xa.message_plain(ids::FIND_SIMILAR_TITLE, &[]),
        "Find Similar Objects"
    );
}

#[test]
fn results_are_grouped_by_kind_with_the_sheet_beside_each() {
    let groups = groups(&sample());
    let kinds: Vec<_> = groups.iter().map(|g| (g.kind, g.rows.len())).collect();
    assert_eq!(kinds, [("Net Label", 3), ("Text Note", 1), ("Value", 1)]);
    assert_eq!(
        groups[0].rows[1],
        ResultRow {
            index: 1,
            text: "VOUT".into(),
            extra: None,
            sheet: "mcu".into(),
            mono: true,
        }
    );
    assert!(!groups[1].rows[0].mono, "a note reads in the body face");
}

#[test]
fn segments_cut_the_text_at_the_matches() {
    assert_eq!(
        segments("Keep VOUT trace", &[5..9]),
        [("Keep ", false), ("VOUT", true), (" trace", false)]
    );
    assert_eq!(segments("VOUT", &[0..4]), [("VOUT", true)]);
    assert_eq!(segments("R1", &[]), [("R1", false)]);
}

#[test]
fn the_primary_button_counts_the_included_results() {
    let en = I18n::en_us();
    let mut state = sample();
    assert_eq!(plain(replace_label(&state, &en)), "Replace 5 of 5");
    for index in [0, 2, 3, 4] {
        state.toggle_included(index);
    }
    assert_eq!(plain(replace_label(&state, &en)), "Replace 1 of 5");
    assert_eq!(state.included_matches().len(), 1);
    assert_eq!(state.included_matches()[0].hit.text, "VOUT");
    state.toggle_included(0);
    assert_eq!(plain(replace_label(&state, &en)), "Replace 2 of 5");
    assert_eq!(
        position_line(&state, &en).map(plain).as_deref(),
        Some("Match 2 of 5")
    );
    state.selected_index = None;
    assert_eq!(position_line(&state, &en), None);
}

/// The laid-out dialog in both Graphite themes: the mockup's texts are
/// there, and an excluded row shows no result after the arrow.
#[test]
fn the_dialog_lays_out_in_graphite_light_and_dark() {
    let en = I18n::en_us();
    let mut state = sample();
    for index in [0, 2, 3, 4] {
        state.toggle_included(index);
    }
    for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let tokens = theme_tokens(theme);
        let texts: Vec<String> = crate::layout_probe::lay_out_texts(
            super::view(&state, &tokens, &en),
            iced::Size::new(1200.0, 900.0),
        )
        .into_iter()
        .map(|t| plain(t.text))
        .collect();
        for expected in [
            "Find and Replace",
            "5 matches",
            "Searches labels, notes, designators, values",
            "Net Label",
            "Sheet",
            "Match 2 of 5",
            "Replace All is one undo step",
            "Replace Current",
            "Replace 1 of 5",
            "mcu",
        ] {
            assert!(
                texts.iter().any(|t| t == expected),
                "{theme:?}: {expected} in {texts:?}"
            );
        }
        let arrows = texts.iter().filter(|t| t.contains('\u{2192}')).count();
        assert_eq!(
            arrows, 1,
            "{theme:?}: only the included row shows its result"
        );
        assert!(texts.iter().any(|t| t == "VOUT_3V3"), "{theme:?}");
    }
}

#[test]
fn find_mode_shows_no_replace_controls() {
    let en = I18n::en_us();
    let mut state = sample();
    state.replace_mode = false;
    let tokens = theme_tokens(ThemeId::GraphiteDark);
    let texts: Vec<String> = crate::layout_probe::lay_out_texts(
        super::view(&state, &tokens, &en),
        iced::Size::new(1200.0, 900.0),
    )
    .into_iter()
    .map(|t| plain(t.text))
    .collect();
    assert!(texts.iter().any(|t| t == "Match 2 of 5"));
    for absent in [
        "Replace Current",
        "Replace 5 of 5",
        "Replace All is one undo step",
    ] {
        assert!(!texts.iter().any(|t| t == absent), "{absent}");
    }
    assert!(!texts.iter().any(|t| t.contains('\u{2192}')));
}
