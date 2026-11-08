use std::cmp::Ordering;

use chrono::{DateTime, TimeZone, Utc};

use crate::{DateLength, I18n, LanguagePref, ListKind, LocalNumber};

fn i18n(tag: &str) -> I18n {
    I18n::new(&LanguagePref::Locale(tag.to_owned()))
}

fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, mo, d, h, mi, s).unwrap()
}

#[test]
fn numbers_follow_the_locale() {
    let value = LocalNumber(1234.5);
    assert_eq!(i18n("en-US").number(value), "1,234.5");
    assert_eq!(i18n("de").number(value), "1.234,5");
    assert_eq!(i18n("tr-TR").number(value), "1.234,5");
    assert_eq!(i18n("en-US").number(LocalNumber(0.25)), "0.25");
    assert_eq!(i18n("tr-TR").number(LocalNumber(0.25)), "0,25");
}

#[test]
fn counts_group_in_the_locale() {
    assert_eq!(i18n("en-US").count(1_234_567), "1,234,567");
    assert_eq!(i18n("de").count(1_234_567), "1.234.567");
    assert_eq!(i18n("tr-TR").count(-42), "-42");
}

#[test]
fn a_non_finite_number_shows_as_rust_prints_it() {
    assert_eq!(i18n("de").number(LocalNumber(f64::NAN)), "NaN");
}

#[test]
fn number_takes_local_number_not_a_bare_float() {
    // The signature is the guard: `number(1.27_f64)` does not compile.
    let _: fn(&I18n, LocalNumber) -> String = I18n::number;
}

#[test]
fn formatting_follows_the_requested_locale_when_messages_fall_back() {
    let tr = i18n("tr-TR");
    assert_eq!(tr.locale(), "en-US", "no tr-TR catalog yet");
    assert_eq!(tr.format_locale().to_string(), "tr-TR");
    assert_eq!(i18n("en-US").format_locale().to_string(), "en-US");
}

#[test]
fn an_unparseable_locale_formats_as_en_us() {
    let bad = i18n("not a tag!");
    assert_eq!(bad.format_locale().to_string(), "en-US");
    assert_eq!(bad.number(LocalNumber(1234.5)), "1,234.5");
}

#[test]
fn lists_join_with_the_locale_conjunction() {
    let en = i18n("en-US");
    let none: [&str; 0] = [];
    assert_eq!(en.list(&none, ListKind::And), "");
    assert_eq!(en.list(&["a"], ListKind::And), "a");
    assert_eq!(en.list(&["a", "b"], ListKind::And), "a and b");
    assert_eq!(en.list(&["a", "b", "c"], ListKind::And), "a, b, and c");
    assert_eq!(en.list(&["a", "b", "c"], ListKind::Or), "a, b, or c");
    let tr = i18n("tr-TR");
    assert_eq!(tr.list(&none, ListKind::Or), "");
    assert_eq!(tr.list(&["a"], ListKind::Or), "a");
    assert_eq!(tr.list(&["a", "b"], ListKind::And), "a ve b");
    assert_eq!(tr.list(&["a", "b", "c"], ListKind::And), "a, b ve c");
    assert_eq!(tr.list(&["a", "b", "c"], ListKind::Or), "a, b veya c");
    assert_eq!(
        tr.list(&[String::from("x"), String::from("y")], ListKind::And),
        "x ve y"
    );
}

#[test]
fn date_time_shows_the_zone_wall_clock() {
    let at = utc(2025, 1, 15, 13, 9, 35);
    let en = i18n("en-US");
    let tr = i18n("tr-TR");
    let istanbul = en.date_time_in(at, "Europe/Istanbul", DateLength::Medium);
    assert!(istanbul.starts_with("Jan 15, 2025"), "{istanbul}");
    assert!(istanbul.contains("4:09"), "{istanbul}");
    let new_york = en.date_time_in(at, "America/New_York", DateLength::Medium);
    assert!(new_york.contains("8:09"), "{new_york}");
    assert_eq!(
        tr.date_time_in(at, "Europe/Istanbul", DateLength::Medium),
        "15 Oca 2025 16:09"
    );
    assert_eq!(
        tr.date_in(at, "Europe/Istanbul", DateLength::Long),
        "15 Ocak 2025"
    );
    assert_eq!(
        en.date_in(at, "Europe/Istanbul", DateLength::Short),
        "1/15/25"
    );
}

#[test]
fn a_date_crosses_midnight_with_the_zone() {
    let at = utc(2025, 1, 15, 23, 30, 0);
    let tr = i18n("tr-TR");
    assert_eq!(tr.date_in(at, "UTC", DateLength::Medium), "15 Oca 2025");
    assert_eq!(
        tr.date_in(at, "Europe/Istanbul", DateLength::Medium),
        "16 Oca 2025"
    );
}

#[test]
fn an_unknown_zone_shows_utc_and_logs_once() {
    let en = i18n("en-US");
    let at = utc(2025, 1, 15, 13, 9, 35);
    let utc_text = en.date_time_in(at, "UTC", DateLength::Medium);
    assert_eq!(
        en.date_time_in(at, "Not/AZone", DateLength::Medium),
        utc_text
    );
    assert_eq!(en.date_time_in(at, "", DateLength::Medium), utc_text);
    en.date_time_in(at, "Not/AZone", DateLength::Medium);
    assert_eq!(
        en.bad_zones.lock().unwrap().len(),
        2,
        "each bad id is kept once"
    );
}

#[test]
fn date_time_crosses_a_dst_boundary() {
    let en = i18n("en-US");
    // New York springs forward 2025-03-09 at 07:00 UTC: 01:59 EST, then 03:00 EDT.
    let before = en.date_time_in(
        utc(2025, 3, 9, 6, 59, 59),
        "America/New_York",
        DateLength::Medium,
    );
    let after = en.date_time_in(
        utc(2025, 3, 9, 7, 0, 0),
        "America/New_York",
        DateLength::Medium,
    );
    assert!(before.contains("1:59"), "{before}");
    assert!(after.contains("3:00"), "{after}");
}

#[test]
fn the_os_zone_variants_do_not_panic() {
    let en = i18n("en-US");
    let at = utc(2025, 1, 15, 13, 9, 35);
    assert!(!en.date_time(at, DateLength::Short).is_empty());
    assert!(!en.date(at, DateLength::Long).is_empty());
}

#[test]
fn fold_matches_turkish_dotted_and_dotless_i() {
    let tr = i18n("tr-TR");
    assert_eq!(tr.fold_for_search("İstanbul"), "istanbul");
    assert_eq!(tr.fold_for_search("ISPARTA"), "ısparta");
    assert_eq!(tr.fold_for_search("ılık"), "ılık");
    let query = tr.fold_for_search("istanbul");
    assert!(tr.fold_for_search("İSTANBUL").contains(&query));
}

#[test]
fn fold_is_plain_under_english() {
    let en = i18n("en-US");
    assert_eq!(en.fold_for_search("ISPARTA"), "isparta");
    assert_eq!(en.fold_for_search("Straße"), "strasse");
}

#[test]
fn upper_for_display_follows_the_language() {
    assert_eq!(
        i18n("tr-TR").upper_for_display("istanbul ılık"),
        "İSTANBUL ILIK"
    );
    assert_eq!(i18n("en-US").upper_for_display("istanbul"), "ISTANBUL");
}

#[test]
fn collation_orders_by_the_alphabet() {
    let mut tr = vec!["z", "ş", "s", "i", "ı", "ç", "c"];
    i18n("tr-TR").sort_for_display(&mut tr, |s| s);
    assert_eq!(tr, ["c", "ç", "ı", "i", "s", "ş", "z"]);
    let mut en = vec!["z", "d", "ç", "c"];
    i18n("en-US").sort_for_display(&mut en, |s| s);
    assert_eq!(en, ["c", "ç", "d", "z"]);
}

#[test]
fn collation_differs_between_languages() {
    let a = ("ı", "i");
    assert_eq!(i18n("tr-TR").compare_for_display(a.0, a.1), Ordering::Less);
    assert_eq!(
        i18n("en-US").compare_for_display(a.0, a.1),
        Ordering::Greater
    );
}

#[test]
fn collation_has_no_numeric_ordering() {
    let mut rows = vec!["R10", "R9", "R2"];
    i18n("en-US").sort_for_display(&mut rows, |s| s);
    assert_eq!(rows, ["R10", "R2", "R9"]);
}
