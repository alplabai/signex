//! `prefs.json` migration plus the read/write round-trip sweep.

use signex_app::render_config::{GridStyle, LabelStyle, MultisheetStyle, PowerPortStyle};
use signex_types::coord::Unit;
use signex_types::theme::ThemeId;

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// ─────────────────────────────────────────────────────────────────
// F1 / F3 — Prefs migration (Windows path bug + stale label_style)
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

#[test]
fn f3_stale_label_style_rewritten_to_standard() {
    let tmp = TempDir::new().unwrap();
    let canonical = tmp.path().join("signex").join("prefs.json");
    let legacy = canonical.clone(); // legacy unused — canonical exists already.

    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    // Pre-v0.10 stale token. Use a non-canonical placeholder so this
    // test source itself stays License-Guard-clean (no historic-EDA-
    // tool substring under crates/).
    let stale = serde_json::json!({
        "ui_font": "Roboto",
        "label_style": "stale-legacy-token",
    });
    fs::write(&canonical, serde_json::to_string_pretty(&stale).unwrap()).unwrap();

    signex_app::fonts::migrate_legacy_prefs(&canonical, &legacy);

    let rewritten: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&canonical).unwrap()).unwrap();
    assert_eq!(
        rewritten["label_style"], "standard",
        "F3: non-canonical label_style normalised to default"
    );
    assert_eq!(
        rewritten["ui_font"], "Roboto",
        "other prefs preserved during F3 normalisation"
    );
}

#[test]
fn f3_canonical_label_style_left_alone() {
    let tmp = TempDir::new().unwrap();
    let canonical = tmp.path().join("signex").join("prefs.json");
    let legacy = canonical.clone();

    fs::create_dir_all(canonical.parent().unwrap()).unwrap();
    let canonical_pref = serde_json::json!({
        "ui_font": "Iosevka",
        "label_style": "altium",
    });
    let original = serde_json::to_string_pretty(&canonical_pref).unwrap();
    fs::write(&canonical, &original).unwrap();

    signex_app::fonts::migrate_legacy_prefs(&canonical, &legacy);

    // Idempotent — file content unchanged.
    let after = fs::read_to_string(&canonical).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&after).unwrap();
    assert_eq!(parsed["label_style"], "altium");
    assert_eq!(parsed["ui_font"], "Iosevka");
}

#[test]
fn f3_label_style_case_variants_all_normalise() {
    for stale_token in ["STANDARD", "Altium", "ALTIUM"] {
        // These are case variants of CANONICAL tokens — they should
        // round-trip unchanged (eq_ignore_ascii_case match).
        let tmp = TempDir::new().unwrap();
        let canonical = tmp.path().join("signex").join("prefs.json");
        let legacy = canonical.clone();
        fs::create_dir_all(canonical.parent().unwrap()).unwrap();
        fs::write(
            &canonical,
            serde_json::to_string(&serde_json::json!({"label_style": stale_token})).unwrap(),
        )
        .unwrap();

        signex_app::fonts::migrate_legacy_prefs(&canonical, &legacy);

        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&canonical).unwrap()).unwrap();
        assert_eq!(
            parsed["label_style"], stale_token,
            "case-variant of canonical token left unchanged: {stale_token}"
        );
    }
}

#[test]
fn f3_garbage_json_doesnt_corrupt_file() {
    let tmp = TempDir::new().unwrap();
    let canonical = tmp.path().join("signex").join("prefs.json");
    let legacy = canonical.clone();
    fs::create_dir_all(canonical.parent().unwrap()).unwrap();

    let original = b"this is not valid json {{{";
    fs::write(&canonical, original).unwrap();

    signex_app::fonts::migrate_legacy_prefs(&canonical, &legacy);

    // Migration is best-effort; broken JSON returns early and leaves
    // the file alone (vs. e.g. emptying it).
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
fn prefs_label_style_round_trip_through_json() {
    let (_tmp, path) = temp_prefs_path();
    // Default when missing.
    assert_eq!(
        signex_app::fonts::read_label_style_pref_at(&path),
        LabelStyle::Standard
    );

    for &style in &[LabelStyle::Standard, LabelStyle::Altium] {
        signex_app::fonts::write_label_style_pref_at(&path, style);
        assert_eq!(
            signex_app::fonts::read_label_style_pref_at(&path),
            style,
            "label_style {style:?} must round-trip"
        );
    }
}

#[test]
fn prefs_power_port_style_round_trip_through_json() {
    let (_tmp, path) = temp_prefs_path();
    // Default when missing.
    assert_eq!(
        signex_app::fonts::read_power_port_style_pref_at(&path),
        PowerPortStyle::Altium
    );

    for &style in &[PowerPortStyle::Standard, PowerPortStyle::Altium] {
        signex_app::fonts::write_power_port_style_pref_at(&path, style);
        assert_eq!(
            signex_app::fonts::read_power_port_style_pref_at(&path),
            style,
            "power_port_style {style:?} must round-trip"
        );
    }
}

#[test]
fn prefs_multisheet_style_round_trip_through_json() {
    let (_tmp, path) = temp_prefs_path();
    // Default when missing.
    assert_eq!(
        signex_app::fonts::read_multisheet_style_pref_at(&path),
        MultisheetStyle::Standard
    );

    for &style in &[MultisheetStyle::Standard, MultisheetStyle::Altium] {
        signex_app::fonts::write_multisheet_style_pref_at(&path, style);
        assert_eq!(
            signex_app::fonts::read_multisheet_style_pref_at(&path),
            style
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
fn prefs_enum_case_insensitive_decode() {
    // The legacy match arms accepted both lowercase and TitleCase tokens
    // (e.g. "altium" | "Altium"). The refactor uses
    // `eq_ignore_ascii_case` to match either form. Verify a hand-written
    // mixed-case prefs.json decodes to the correct variant.
    let (_tmp, path) = temp_prefs_path();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let raw = serde_json::json!({
        "label_style": "Altium",         // TitleCase
        "power_port_style": "STANDARD",   // UPPERCASE
        "multisheet_style": "altium",     // lowercase
        "grid_style": "Lines",            // TitleCase
    });
    fs::write(&path, serde_json::to_string_pretty(&raw).unwrap()).unwrap();

    assert_eq!(
        signex_app::fonts::read_label_style_pref_at(&path),
        LabelStyle::Altium
    );
    assert_eq!(
        signex_app::fonts::read_power_port_style_pref_at(&path),
        PowerPortStyle::Standard
    );
    assert_eq!(
        signex_app::fonts::read_multisheet_style_pref_at(&path),
        MultisheetStyle::Altium
    );
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

    // Pre-existing keys (label_style, ui_font, etc.) should remain
    // unset — we never wrote them — but absent ≠ default-failure.
    let raw: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert!(
        raw.get("label_style").is_none(),
        "label_style not written by these tests"
    );
    assert!(raw.get("ui_font").is_none());
}
