use super::*;
use std::path::PathBuf;

use crate::library::part_sync::Choice;

/// `classify_bump` distinguishes patch / minor / major by the
/// leading two numeric segments. Anything that fails to parse
/// upgrades to Major — defensive default since mismatched-format
/// pairs likely indicate breaking schema drift.
#[test]
fn classify_bump_distinguishes_three_buckets() {
    assert_eq!(classify_bump("1.0.0", "1.0.1"), BumpKind::Patch);
    assert_eq!(classify_bump("1.0.0", "1.1.0"), BumpKind::Minor);
    assert_eq!(classify_bump("1.0.0", "2.0.0"), BumpKind::Major);
    // Unparseable → Major (defensive).
    assert_eq!(classify_bump("nightly", "main"), BumpKind::Major);
    // Mixed parse — only major component present in latest.
    assert_eq!(classify_bump("1", "2"), BumpKind::Major);
}

/// Mockup row 135 (accepted 2026-11-24): patch and minor bumps start
/// checked, since a minor bump is compatible under semver; a major bump
/// starts unchecked.
#[test]
fn default_checked_patches_and_minors_not_majors() {
    assert!(BumpKind::Patch.default_checked());
    assert!(BumpKind::Minor.default_checked());
    assert!(!BumpKind::Major.default_checked());
}

/// `LibraryUpdatesState::new` sorts by `ref_des` and pre-checks every
/// entry but the major bumps.
#[test]
fn new_sorts_by_ref_des_and_pre_checks_patches() {
    let path = PathBuf::from("/tmp/x.standard_sch");
    let library_path = PathBuf::from("/tmp/x.snxlib");
    let mk = |refdes: &str, kind: BumpKind| LibraryUpdateEntry {
        symbol_uuid: Uuid::now_v7(),
        ref_des: refdes.to_string(),
        internal_pn: String::new(),
        lib_id: format!("L:{refdes}"),
        library_id: Uuid::nil(),
        library_name: "Lib".to_string(),
        row_id: RowId::from_uuid(Uuid::nil()),
        library_path: library_path.clone(),
        current_version: "1.0.0".to_string(),
        latest_version: match kind {
            BumpKind::Patch => "1.0.1",
            BumpKind::Minor => "1.1.0",
            BumpKind::Major => "2.0.0",
        }
        .to_string(),
        bump_kind: kind,
        selected: false,
        conflicts: Vec::new(),
    };
    let state = LibraryUpdatesState::new(
        path,
        vec![
            mk("U3", BumpKind::Major),
            mk("R10", BumpKind::Patch),
            mk("R2", BumpKind::Patch),
            mk("R1", BumpKind::Patch),
            mk("C2", BumpKind::Minor),
        ],
    );
    // Sorted ascending by ref_des, naturally: R10 comes last, not
    // between R1 and R2 the way `str::cmp` would place it.
    let order: Vec<&str> = state.entries.iter().map(|e| e.ref_des.as_str()).collect();
    assert_eq!(order, vec!["C2", "R1", "R2", "R10", "U3"]);
    // Patches and minors are pre-checked, the major is not.
    assert!(state.entries[0].selected); // minor
    assert!(state.entries[1].selected); // patch
    assert!(state.entries[2].selected); // patch
    assert!(state.entries[3].selected); // patch
    assert!(!state.entries[4].selected); // major
    assert_eq!(state.selected_count(), 4);
}

/// `toggle` flips the checkbox for a matching `symbol_uuid` and
/// is a no-op for an unknown uuid.
#[test]
fn toggle_flips_only_matching_uuid() {
    let path = PathBuf::from("/tmp/x.standard_sch");
    let library_path = PathBuf::from("/tmp/x.snxlib");
    let id = Uuid::now_v7();
    let other = Uuid::now_v7();
    let mut state = LibraryUpdatesState::new(
        path,
        vec![LibraryUpdateEntry {
            symbol_uuid: id,
            ref_des: "R1".to_string(),
            internal_pn: String::new(),
            lib_id: "L:R1".to_string(),
            library_id: Uuid::nil(),
            library_name: "Lib".to_string(),
            row_id: RowId::from_uuid(Uuid::nil()),
            library_path: library_path.clone(),
            current_version: "1.0.0".to_string(),
            latest_version: "1.0.1".to_string(),
            bump_kind: BumpKind::Patch,
            selected: false,
            conflicts: Vec::new(),
        }],
    );
    // Started pre-checked because of the patch default.
    assert!(state.entries[0].selected);
    state.toggle(id);
    assert!(!state.entries[0].selected);
    // Unknown uuid is a no-op.
    state.toggle(other);
    assert!(!state.entries[0].selected);
}

/// Instances sharing a `lib_id` share one embedded copy: one checkbox
/// for the group, default on only if no member is a major bump.
#[test]
fn instances_of_one_lib_id_are_selected_together() {
    let mk = |refdes: &str, lib_id: &str, kind: BumpKind| LibraryUpdateEntry {
        symbol_uuid: Uuid::now_v7(),
        ref_des: refdes.to_string(),
        internal_pn: String::new(),
        lib_id: lib_id.to_string(),
        library_id: Uuid::nil(),
        library_name: "Lib".to_string(),
        row_id: RowId::from_uuid(Uuid::nil()),
        library_path: PathBuf::from("/tmp/x.snxlib"),
        current_version: "1.0.0".to_string(),
        latest_version: "1.0.1".to_string(),
        bump_kind: kind,
        selected: false,
        conflicts: Vec::new(),
    };
    let mut state = LibraryUpdatesState::new(
        PathBuf::from("/tmp/x.standard_sch"),
        vec![
            mk("R1", "L:R", BumpKind::Patch),
            mk("R2", "L:R", BumpKind::Major),
            mk("C1", "L:C", BumpKind::Patch),
        ],
    );
    let sel =
        |s: &LibraryUpdatesState| -> Vec<bool> { s.entries.iter().map(|e| e.selected).collect() };
    // C1 (own group, patch) on; R1/R2 off because R2 is a major bump.
    assert_eq!(sel(&state), vec![true, false, false]);
    let r1 = state.entries[1].symbol_uuid;
    state.toggle(r1);
    assert_eq!(sel(&state), vec![true, true, true]);
}

/// Answering a conflict ticks its entry and the group sharing the copy.
#[test]
fn answering_a_conflict_ticks_the_entry() {
    let mk = |refdes: &str, kind: BumpKind| LibraryUpdateEntry {
        symbol_uuid: Uuid::now_v7(),
        ref_des: refdes.to_string(),
        internal_pn: String::new(),
        lib_id: "L:R".to_string(),
        library_id: Uuid::nil(),
        library_name: "Lib".to_string(),
        row_id: RowId::from_uuid(Uuid::nil()),
        library_path: PathBuf::from("/tmp/x.snxlib"),
        current_version: "1.0.0".to_string(),
        latest_version: "2.0.0".to_string(),
        bump_kind: kind,
        selected: false,
        conflicts: vec![ConflictChoice {
            field: StampField::Mpn,
            mine: "MINE".into(),
            library: "LIB".into(),
            choice: Choice::KeepMine,
        }],
    };
    let mut state = LibraryUpdatesState::new(
        PathBuf::from("/tmp/x.standard_sch"),
        vec![mk("R1", BumpKind::Major), mk("R2", BumpKind::Major)],
    );
    assert_eq!(state.selected_count(), 0, "a major bump starts unticked");
    let r1 = state.entries[0].symbol_uuid;
    state.answer_conflict(r1, StampField::Mpn, Choice::UseLibrary);
    assert_eq!(state.entries[0].conflicts[0].choice, Choice::UseLibrary);
    assert_eq!(state.selected_count(), 2);
}

/// One entry of each bump kind (`R2` and `R3` share a `lib_id`), the major
/// one carrying an MPN conflict.
fn mixed_state() -> LibraryUpdatesState {
    let mk = |refdes: &str, lib_id: &str, pn: &str, latest: &str| {
        let bump_kind = classify_bump("1.0.0", latest);
        LibraryUpdateEntry {
            symbol_uuid: Uuid::now_v7(),
            ref_des: refdes.to_string(),
            internal_pn: pn.to_string(),
            lib_id: lib_id.to_string(),
            library_id: Uuid::nil(),
            library_name: "passives".to_string(),
            row_id: RowId::from_uuid(Uuid::nil()),
            library_path: PathBuf::from("/tmp/x.snxlib"),
            current_version: "1.0.0".to_string(),
            latest_version: latest.to_string(),
            bump_kind,
            selected: false,
            conflicts: if bump_kind == BumpKind::Major {
                vec![ConflictChoice {
                    field: StampField::Mpn,
                    mine: "TPS54331DR".into(),
                    library: "TPS54331DDAR".into(),
                    choice: Choice::KeepMine,
                }]
            } else {
                Vec::new()
            },
        }
    };
    LibraryUpdatesState::new(
        PathBuf::from("/tmp/dcdc.snxsch"),
        vec![
            mk("U3", "L:U", "TPS54331", "2.0.0"),
            mk("L1", "L:L", "IND-4040-10U", "1.1.0"),
            mk("R2", "L:R", "R-0402-10K-1%", "1.0.1"),
            mk("R3", "L:R", "R-0402-10K-1%", "1.0.1"),
            mk("C4", "L:C", "", "1.0.3"),
        ],
    )
}

/// `set_all` ticks or clears every entry; the header checkbox reads
/// `all_selected` and its click (`toggle_all`) flips between the two.
#[test]
fn set_all_ticks_or_clears_every_entry() {
    let mut state = mixed_state();
    let sel =
        |s: &LibraryUpdatesState| -> Vec<bool> { s.entries.iter().map(|e| e.selected).collect() };
    // Sorted C4, L1, R2, R3, U3: the three patches start ticked.
    assert_eq!(sel(&state), vec![true, false, true, true, false]);
    assert!(!state.all_selected());

    state.set_all(true);
    assert_eq!(sel(&state), vec![true; 5]);
    assert!(state.all_selected());
    assert_eq!(state.selected_count(), 5);

    state.set_all(false);
    assert_eq!(sel(&state), vec![false; 5]);
    assert!(!state.all_selected());

    // Some ticked: the header click ticks all; all ticked: it clears all.
    let r2 = state.entries[2].symbol_uuid;
    state.toggle(r2);
    assert_eq!(state.selected_count(), 2, "R2 and R3 share a copy");
    state.toggle_all();
    assert!(state.all_selected());
    state.toggle_all();
    assert_eq!(state.selected_count(), 0);

    // An empty list is never "all selected": the header box shows clear.
    let empty = LibraryUpdatesState::new(PathBuf::from("/tmp/x.snxsch"), Vec::new());
    assert!(!empty.all_selected());
}

/// The summary pills count each bump kind.
#[test]
fn count_of_counts_each_kind() {
    let state = mixed_state();
    assert_eq!(state.count_of(BumpKind::Major), 1);
    assert_eq!(state.count_of(BumpKind::Minor), 1);
    assert_eq!(state.count_of(BumpKind::Patch), 3);
}

/// The texts the card lays out, without the Fluent isolation marks.
fn card_texts(state: &LibraryUpdatesState, theme: signex_types::theme::ThemeId) -> Vec<String> {
    let tokens = signex_types::theme::theme_tokens(theme);
    let i18n = signex_i18n::I18n::en_us();
    crate::layout_probe::lay_out_texts(
        view(state, &tokens, &i18n, false),
        iced::Size::new(1000.0, 800.0),
    )
    .into_iter()
    .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
    .collect()
}

/// The card builds with every bump kind and a conflict, in both Graphite
/// themes: the summary, one pill per kind, the headings, each row's cells
/// and the conflict line.
#[test]
fn the_card_shows_every_kind_and_the_conflict_line() {
    use signex_types::theme::ThemeId;
    let state = mixed_state();
    for theme in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let texts = card_texts(&state, theme);
        let has = |wanted: &str| texts.iter().any(|t| t == wanted);
        for wanted in [
            "Library Updates Available",
            "5 placed components drifted from the library since this schematic was last saved.",
            "1 major",
            "1 minor",
            "3 patch",
            "PART",
            "LIBRARY",
            "VERSION",
            "CHANGE",
            "U3",
            "TPS54331",
            "C4",
            "passives",
            "1.0.0 → 2.0.0",
            "1.0.0 → 1.1.0",
            "⚠ Major",
            "Minor",
            "Patch",
            "MPN",
            "yours",
            "TPS54331DR",
            "library now",
            "TPS54331DDAR",
        ] {
            assert!(has(wanted), "{theme:?}: missing {wanted:?} in {texts:?}");
        }
        // The two R-0402 rows show their PN; C4 has none, so none is added.
        let pns = texts.iter().filter(|t| *t == "R-0402-10K-1%").count();
        assert_eq!(pns, 2, "{theme:?}: {texts:?}");
    }
}

/// A kind with no entry has no summary pill, and one entry reads singular.
#[test]
fn a_kind_without_entries_has_no_pill() {
    let mut state = mixed_state();
    state.entries.retain(|e| e.ref_des == "C4");
    let texts = card_texts(&state, signex_types::theme::ThemeId::GraphiteLight);
    assert!(texts.iter().any(|t| t == "1 patch"), "{texts:?}");
    assert!(
        texts.iter().any(|t| t
            == "1 placed component drifted from the library since this schematic was last saved."),
        "{texts:?}"
    );
    assert!(
        !texts
            .iter()
            .any(|t| t.ends_with(" major") || t.ends_with(" minor"))
    );
}
