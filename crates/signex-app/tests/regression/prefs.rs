//! `prefs.json` migration plus the read/write round-trip sweep.

use signex_app::render_config::GridStyle;
use signex_types::coord::Unit;
use signex_types::theme::ThemeId;

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// ─────────────────────────────────────────────────────────────────
// F1 — Prefs migration (Windows path bug)
// ─────────────────────────────────────────────────────────────────

#[test]
fn f1_legacy_prefs_path_copied_forward_when_canonical_empty() {
    let tmp = TempDir::new().unwrap();
    let canonical = tmp
        .path()
        .join("canonical")
        .join("signex")
        .join("prefs.json");
    let legacy = tmp.path().join("legacy").join("signex").join("prefs.json");

    fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    fs::write(
        &legacy,
        br#"{"ui_font":"Roboto","theme":"signex","label_style":"standard"}"#,
    )
    .unwrap();
    assert!(!canonical.exists(), "canonical absent before migration");

    signex_app::fonts::migrate_legacy_prefs(&canonical, &legacy);

    assert!(canonical.exists(), "canonical now exists (F1 copy)");
    let copied = fs::read_to_string(&canonical).unwrap();
    assert!(
        copied.contains("\"ui_font\""),
        "canonical contains the legacy file's content"
    );
    assert!(
        legacy.exists(),
        "legacy preserved (forward-copy, not move) — backward compat"
    );
}

#[test]
fn f1_canonical_present_blocks_legacy_copy() {
    let tmp = TempDir::new().unwrap();
    let canonical = tmp
        .path()
        .join("canonical")
        .join("signex")
        .join("prefs.json");
    let legacy = tmp.path().join("legacy").join("signex").join("prefs.json");

    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    fs::write(&canonical, br#"{"ui_font":"Iosevka"}"#).unwrap();
    fs::write(&legacy, br#"{"ui_font":"LegacyValue"}"#).unwrap();

    signex_app::fonts::migrate_legacy_prefs(&canonical, &legacy);

    let content = fs::read_to_string(&canonical).unwrap();
    assert!(
        content.contains("Iosevka"),
        "canonical content untouched when it already exists"
    );
    assert!(
        !content.contains("LegacyValue"),
        "legacy must NOT overwrite canonical when canonical exists"
    );
}

#[test]
fn f1_no_legacy_no_canonical_is_a_clean_noop() {
    let tmp = TempDir::new().unwrap();
    let canonical = tmp
        .path()
        .join("canonical")
        .join("signex")
        .join("prefs.json");
    let legacy = tmp.path().join("legacy").join("signex").join("prefs.json");

    // Neither exists. Migration should not panic, not create anything.
    signex_app::fonts::migrate_legacy_prefs(&canonical, &legacy);

    assert!(!canonical.exists(), "no canonical created from nothing");
    assert!(!legacy.exists(), "no legacy created from nothing");
}

/// #635 removed the Power Port / Label / Multisheet Style settings. A
/// prefs file written before that still carries their keys: it must load,
/// and migration must leave it byte-for-byte alone (no more F3 rewrite).
#[test]
fn removed_style_keys_in_an_old_prefs_file_are_ignored_and_kept() {
    let tmp = TempDir::new().unwrap();
    let canonical = tmp.path().join("signex").join("prefs.json");
    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    let original = serde_json::to_string_pretty(&serde_json::json!({
        "theme": "signex",
        "label_style": "stale-legacy-token",
        "power_port_style": "standard",
        "multisheet_style": "altium",
        "grid_style": "lines",
    }))
    .unwrap();
    fs::write(&canonical, &original).unwrap();

    signex_app::fonts::migrate_legacy_prefs(&canonical, &canonical);

    assert_eq!(fs::read_to_string(&canonical).unwrap(), original);
    assert_eq!(
        signex_app::fonts::read_grid_style_pref_at(&canonical),
        GridStyle::Lines
    );
}

#[test]
fn migration_leaves_garbage_json_untouched() {
    let tmp = TempDir::new().unwrap();
    let canonical = tmp.path().join("signex").join("prefs.json");
    let legacy = canonical.clone();
    fs::create_dir_all(canonical.parent().unwrap()).unwrap();

    let original = b"this is not valid json {{{";
    fs::write(&canonical, original).unwrap();

    signex_app::fonts::migrate_legacy_prefs(&canonical, &legacy);

    // Migration is best-effort; broken JSON returns early and leaves
    // the file alone (vs. e.g. emptying it). Only proves the F1 copy is
    // skipped for unparseable input; nothing else is migrated any more.
    let after = fs::read(&canonical).unwrap();
    assert_eq!(
        after, original,
        "garbage JSON file must be left untouched (no panic, no truncation)"
    );
}

// ─────────────────────────────────────────────────────────────────
// §4.4 — Preferences persistence sweep
//
// For each user-toggleable knob the checklist asks: "toggle, restart
// the app, confirm the value is restored". We can't restart from a
// single test process, but we can exercise the same write→read pair
// through the same `prefs.json` JSON encoding the production code
// uses. Tests inject a tempdir prefs file via the `_at(path)`
// variants on each pref function so the user's real prefs.json is
// never touched.
// ─────────────────────────────────────────────────────────────────

fn temp_prefs_path() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("signex").join("prefs.json");
    (tmp, path)
}

#[test]
fn prefs_theme_round_trip_through_json() {
    let (_tmp, path) = temp_prefs_path();
    // Default when missing.
    assert_eq!(
        signex_app::fonts::read_theme_pref_at(&path),
        ThemeId::Signex
    );

    // Each builtin theme survives a write→read cycle.
    for &theme in ThemeId::BUILTINS {
        signex_app::fonts::write_theme_pref_at(&path, theme);
        assert_eq!(
            signex_app::fonts::read_theme_pref_at(&path),
            theme,
            "theme {theme:?} must round-trip"
        );
    }
}

#[test]
fn prefs_unit_round_trip_through_json() {
    let (_tmp, path) = temp_prefs_path();
    // Default when missing.
    assert_eq!(signex_app::fonts::read_unit_pref_at(&path), Unit::Mm);

    for unit in [Unit::Mm, Unit::Mil, Unit::Inch] {
        signex_app::fonts::write_unit_pref_at(&path, unit);
        assert_eq!(
            signex_app::fonts::read_unit_pref_at(&path),
            unit,
            "unit {unit:?} must round-trip"
        );
    }
}

#[test]
fn prefs_grid_visible_round_trip_through_json() {
    let (_tmp, path) = temp_prefs_path();
    // Default when missing.
    assert!(signex_app::fonts::read_grid_visible_pref_at(&path));

    signex_app::fonts::write_grid_visible_pref_at(&path, false);
    assert!(!signex_app::fonts::read_grid_visible_pref_at(&path));

    signex_app::fonts::write_grid_visible_pref_at(&path, true);
    assert!(signex_app::fonts::read_grid_visible_pref_at(&path));
}

#[test]
fn prefs_snap_enabled_round_trip_through_json() {
    let (_tmp, path) = temp_prefs_path();
    // Default when missing.
    assert!(signex_app::fonts::read_snap_enabled_pref_at(&path));

    signex_app::fonts::write_snap_enabled_pref_at(&path, false);
    assert!(!signex_app::fonts::read_snap_enabled_pref_at(&path));
}

#[test]
fn prefs_grid_size_round_trip_through_json() {
    let (_tmp, path) = temp_prefs_path();
    // Default when missing — `None` so the caller can fall back to
    // the engine's preferred default.
    assert_eq!(signex_app::fonts::read_grid_size_mm_pref_at(&path), None);

    signex_app::fonts::write_grid_size_mm_pref_at(&path, 1.27);
    let v = signex_app::fonts::read_grid_size_mm_pref_at(&path).unwrap();
    assert!((v - 1.27).abs() < 1e-5, "grid size round-trips, got {v}");

    signex_app::fonts::write_grid_size_mm_pref_at(&path, 0.635);
    let v = signex_app::fonts::read_grid_size_mm_pref_at(&path).unwrap();
    assert!((v - 0.635).abs() < 1e-5);
}

#[test]
fn prefs_writes_dont_clobber_neighboring_keys() {
    let (_tmp, path) = temp_prefs_path();

    // Seed multiple keys.
    signex_app::fonts::write_theme_pref_at(&path, ThemeId::Signex);
    signex_app::fonts::write_unit_pref_at(&path, Unit::Mil);
    signex_app::fonts::write_grid_visible_pref_at(&path, false);

    // Write a different key — neighbouring values must survive.
    signex_app::fonts::write_snap_enabled_pref_at(&path, false);

    assert_eq!(
        signex_app::fonts::read_theme_pref_at(&path),
        ThemeId::Signex
    );
    assert_eq!(signex_app::fonts::read_unit_pref_at(&path), Unit::Mil);
    assert!(!signex_app::fonts::read_grid_visible_pref_at(&path));
    assert!(!signex_app::fonts::read_snap_enabled_pref_at(&path));
}

#[test]
fn prefs_garbage_json_falls_back_to_defaults() {
    let (_tmp, path) = temp_prefs_path();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"{ broken json content").unwrap();

    // Each read returns its default rather than panicking on parse error.
    assert_eq!(
        signex_app::fonts::read_theme_pref_at(&path),
        ThemeId::Signex
    );
    assert_eq!(signex_app::fonts::read_unit_pref_at(&path), Unit::Mm);
    assert!(signex_app::fonts::read_grid_visible_pref_at(&path));
    assert!(signex_app::fonts::read_snap_enabled_pref_at(&path));
    assert_eq!(signex_app::fonts::read_grid_size_mm_pref_at(&path), None);
}

#[test]
fn prefs_ui_font_round_trip_through_json() {
    let (_tmp, path) = temp_prefs_path();
    // Default when missing.
    assert_eq!(signex_app::fonts::read_ui_font_pref_at(&path), "Roboto");

    for font in ["Iosevka", "Helvetica Neue", "Inter", "Source Code Pro"] {
        signex_app::fonts::write_ui_font_pref_at(&path, font);
        assert_eq!(
            signex_app::fonts::read_ui_font_pref_at(&path),
            font,
            "ui_font {font} must round-trip"
        );
    }
}

#[test]
fn prefs_grid_style_round_trip_through_json() {
    let (_tmp, path) = temp_prefs_path();
    // Default when missing.
    assert_eq!(
        signex_app::fonts::read_grid_style_pref_at(&path),
        GridStyle::Dots
    );

    for &style in &[GridStyle::Dots, GridStyle::Lines, GridStyle::SmallCrosses] {
        signex_app::fonts::write_grid_style_pref_at(&path, style);
        assert_eq!(
            signex_app::fonts::read_grid_style_pref_at(&path),
            style,
            "grid_style {style:?} must round-trip"
        );
    }
}

#[test]
fn prefs_zoom_step_percent_round_trip_and_defaults() {
    let (_tmp, path) = temp_prefs_path();
    // Old prefs file without the key: default 10 (a x1.10 notch).
    assert_eq!(signex_app::fonts::read_zoom_step_percent_pref_at(&path), 10);

    signex_app::fonts::write_zoom_step_percent_pref_at(&path, 25);
    assert_eq!(signex_app::fonts::read_zoom_step_percent_pref_at(&path), 25);

    // Out-of-range / non-numeric values clamp or fall back to the default.
    for (raw, expected) in [
        ("200", 50),
        ("0", 5),
        ("-4", 5),
        ("12.6", 13),
        ("\"fast\"", 10),
        ("null", 10),
    ] {
        fs::write(&path, format!(r#"{{"zoom_step_percent": {raw}}}"#)).unwrap();
        assert_eq!(
            signex_app::fonts::read_zoom_step_percent_pref_at(&path),
            expected,
            "zoom_step_percent {raw}"
        );
    }
}

#[test]
fn prefs_move_keeps_wires_round_trips_and_missing_key_is_on() {
    let (_tmp, path) = temp_prefs_path();
    assert!(
        signex_app::fonts::read_move_keeps_wires_pref_at(&path),
        "no prefs file: on"
    );
    signex_app::fonts::write_zoom_step_percent_pref_at(&path, 25);
    assert!(
        signex_app::fonts::read_move_keeps_wires_pref_at(&path),
        "file without the key: on"
    );

    signex_app::fonts::write_move_keeps_wires_pref_at(&path, false);
    assert!(!signex_app::fonts::read_move_keeps_wires_pref_at(&path));
    signex_app::fonts::write_move_keeps_wires_pref_at(&path, true);
    assert!(signex_app::fonts::read_move_keeps_wires_pref_at(&path));

    fs::write(&path, r#"{"move_keeps_wires": "no"}"#).unwrap();
    assert!(
        signex_app::fonts::read_move_keeps_wires_pref_at(&path),
        "a non-boolean value falls back to on"
    );
}

#[test]
fn prefs_enum_case_insensitive_decode() {
    // The legacy match arms accepted both lowercase and TitleCase tokens
    // (e.g. "altium" | "Altium"). The refactor uses
    // `eq_ignore_ascii_case` to match either form. Verify a hand-written
    // mixed-case prefs.json decodes to the correct variant.
    let (_tmp, path) = temp_prefs_path();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let raw = serde_json::json!({
        "grid_style": "Lines",            // TitleCase
    });
    fs::write(&path, serde_json::to_string_pretty(&raw).unwrap()).unwrap();

    assert_eq!(
        signex_app::fonts::read_grid_style_pref_at(&path),
        GridStyle::Lines
    );
}

#[test]
fn prefs_cross_pref_independence() {
    let (_tmp, path) = temp_prefs_path();

    // Write each pref in a different "session" (sequential writes,
    // each through update_prefs_json which does read-modify-write).
    signex_app::fonts::write_theme_pref_at(&path, ThemeId::Signex);
    signex_app::fonts::write_grid_size_mm_pref_at(&path, 2.54);
    signex_app::fonts::write_unit_pref_at(&path, Unit::Mil);
    signex_app::fonts::write_grid_visible_pref_at(&path, false);
    signex_app::fonts::write_snap_enabled_pref_at(&path, false);

    // Read everything back — none should have been clobbered.
    assert_eq!(
        signex_app::fonts::read_theme_pref_at(&path),
        ThemeId::Signex
    );
    assert!((signex_app::fonts::read_grid_size_mm_pref_at(&path).unwrap() - 2.54).abs() < 1e-5);
    assert_eq!(signex_app::fonts::read_unit_pref_at(&path), Unit::Mil);
    assert!(!signex_app::fonts::read_grid_visible_pref_at(&path));
    assert!(!signex_app::fonts::read_snap_enabled_pref_at(&path));

    // Keys these tests never wrote (ui_font, etc.) stay unset — absent,
    // not written as a default.
    let raw: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert!(raw.get("ui_font").is_none());
}

#[test]
fn prefs_annotate_undo_round_trips_and_missing_key_is_one_step() {
    use signex_app::annotate_undo::AnnotateUndo;
    let (_tmp, path) = temp_prefs_path();
    assert_eq!(
        signex_app::fonts::read_annotate_undo_pref_at(&path),
        AnnotateUndo::Project,
        "no file gives the default"
    );

    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, br#"{"grid_style":"lines"}"#).unwrap();
    assert_eq!(
        signex_app::fonts::read_annotate_undo_pref_at(&path),
        AnnotateUndo::Project,
        "an old prefs file without the key gives the default"
    );

    for &scope in AnnotateUndo::ALL {
        signex_app::fonts::write_annotate_undo_pref_at(&path, scope);
        assert_eq!(signex_app::fonts::read_annotate_undo_pref_at(&path), scope);
    }
    assert_eq!(
        signex_app::fonts::read_grid_style_pref_at(&path),
        GridStyle::Lines,
        "writing the key keeps the other preferences"
    );
}

#[test]
fn prefs_window_controls_placement_round_trips_and_unknown_is_auto() {
    use signex_app::chrome::controls::WindowControlsPlacement;
    let (_tmp, path) = temp_prefs_path();
    assert_eq!(
        signex_app::fonts::read_window_controls_placement_pref_at(&path),
        WindowControlsPlacement::Auto,
        "no file gives the default"
    );

    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, br#"{"window_controls_placement":"sideways"}"#).unwrap();
    assert_eq!(
        signex_app::fonts::read_window_controls_placement_pref_at(&path),
        WindowControlsPlacement::Auto,
        "an unknown token gives the default"
    );

    for &placement in WindowControlsPlacement::ALL {
        signex_app::fonts::write_window_controls_placement_pref_at(&path, placement);
        assert_eq!(
            signex_app::fonts::read_window_controls_placement_pref_at(&path),
            placement
        );
    }
    let raw: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(raw["window_controls_placement"], "trailing");
}

// ─────────────────────────────────────────────────────────────────
// #1469 — toggles persist off the UI thread, newest value wins
// ─────────────────────────────────────────────────────────────────

#[test]
fn ordered_pref_write_skips_a_stale_write_that_arrives_late() {
    use signex_app::fonts::OrderedPrefWrite;

    let (_tmp, path) = temp_prefs_path();
    // Two rapid toggles: the user turned the grid off, then on again.
    let older = OrderedPrefWrite::new("regression_1469_grid");
    let newer = OrderedPrefWrite::new("regression_1469_grid");

    // The workers reach the prefs file in the opposite order.
    assert!(newer.run(|| signex_app::fonts::write_grid_visible_pref_at(&path, true)));
    assert!(!older.run(|| signex_app::fonts::write_grid_visible_pref_at(&path, false)));

    assert!(
        signex_app::fonts::read_grid_visible_pref_at(&path),
        "the late, older write must not overwrite the newest value"
    );
}

#[test]
fn ordered_pref_writes_of_different_keys_do_not_suppress_each_other() {
    use signex_app::fonts::OrderedPrefWrite;

    let (_tmp, path) = temp_prefs_path();
    let snap = OrderedPrefWrite::new("regression_1469_snap");
    let grid = OrderedPrefWrite::new("regression_1469_other_grid");

    assert!(grid.run(|| signex_app::fonts::write_grid_visible_pref_at(&path, false)));
    assert!(snap.run(|| signex_app::fonts::write_snap_enabled_pref_at(&path, false)));

    assert!(!signex_app::fonts::read_grid_visible_pref_at(&path));
    assert!(!signex_app::fonts::read_snap_enabled_pref_at(&path));
}
