use std::cmp::Ordering;

use chrono::{DateTime, FixedOffset, Local, NaiveDateTime, TimeZone, Utc};

use crate::format::wall_clock;
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

fn wall(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> NaiveDateTime {
    utc(y, mo, d, h, mi, s).naive_utc()
}

fn east(hours: i32) -> FixedOffset {
    FixedOffset::east_opt(hours * 3600).unwrap()
}

#[test]
fn a_wall_clock_formats_in_the_locale() {
    let at = wall(2025, 1, 15, 16, 9, 35);
    let en = i18n("en-US");
    let tr = i18n("tr-TR");
    let medium = en.show(at, DateLength::Medium, true);
    assert!(medium.starts_with("Jan 15, 2025"), "{medium}");
    assert!(medium.contains("4:09"), "{medium}");
    assert_eq!(tr.show(at, DateLength::Medium, true), "15 Oca 2025 16:09");
    assert_eq!(tr.show(at, DateLength::Long, false), "15 Ocak 2025");
    assert_eq!(en.show(at, DateLength::Short, false), "1/15/25");
}

#[test]
fn a_fixed_offset_moves_the_wall_clock_across_midnight() {
    let at = utc(2025, 1, 15, 23, 30, 0);
    let tr = i18n("tr-TR");
    let same = tr.show(wall_clock(at, &Utc), DateLength::Medium, false);
    let istanbul = tr.show(wall_clock(at, &east(3)), DateLength::Medium, false);
    assert_eq!(same, "15 Oca 2025");
    assert_eq!(istanbul, "16 Oca 2025");
}

#[test]
fn offsets_either_side_of_a_dst_change_give_the_wall_clock_either_side() {
    let en = i18n("en-US");
    // New York springs forward 2025-03-09 at 07:00 UTC: 01:59 EST, then 03:00 EDT.
    let before = wall_clock(utc(2025, 3, 9, 6, 59, 59), &east(-5));
    let after = wall_clock(utc(2025, 3, 9, 7, 0, 0), &east(-4));
    assert!(en.show(before, DateLength::Medium, true).contains("1:59"));
    assert!(en.show(after, DateLength::Medium, true).contains("3:00"));
}

#[test]
fn the_os_variants_show_the_local_wall_clock() {
    let en = i18n("en-US");
    let at = utc(2025, 1, 15, 13, 9, 35);
    let local = wall_clock(at, &Local);
    assert_eq!(
        en.date_time(at, DateLength::Short),
        en.show(local, DateLength::Short, true)
    );
    assert_eq!(
        en.date(at, DateLength::Long),
        en.show(local, DateLength::Long, false)
    );
}

#[test]
fn a_date_outside_icu_range_falls_back_to_iso_and_warns_once() {
    let en = i18n("en-US");
    let far = NaiveDateTime::MAX;
    let shown = en.show(far, DateLength::Short, false);
    assert!(shown.contains('T'), "{shown}");
    en.show(far, DateLength::Short, false);
    assert_eq!(en.format_warned.lock().unwrap().len(), 1);
}

fn locale_of(tag: &str) -> String {
    i18n(tag).format_locale().to_string()
}

#[test]
fn the_formatting_locale_keeps_unicode_extensions() {
    assert_eq!(locale_of("th-TH-u-ca-buddhist"), "th-TH-u-ca-buddhist");
    assert_eq!(locale_of("en-US-u-nu-arab"), "en-US-u-nu-arab");
    assert_eq!(locale_of("en-US-u-hc-h23"), "en-US-u-hc-h23");
}

#[test]
fn extensions_change_the_calendar_digits_and_clock() {
    let at = wall(2025, 1, 15, 16, 9, 35);
    let thai = i18n("th-TH-u-ca-buddhist").show(at, DateLength::Long, false);
    assert!(thai.contains("2568"), "{thai}");
    let greg = i18n("th-TH-u-ca-gregory").show(at, DateLength::Long, false);
    assert!(greg.contains("2025"), "{greg}");
    let japanese = i18n("ja-JP-u-ca-japanese").show(at, DateLength::Long, false);
    assert!(japanese.contains("令和7"), "{japanese}");
    assert_eq!(
        i18n("en-US-u-nu-arab").number(LocalNumber(1234.5)),
        "١,٢٣٤.٥"
    );
    let h23 = i18n("en-US-u-hc-h23").show(at, DateLength::Medium, true);
    assert!(h23.contains("16:09"), "{h23}");
    assert!(!h23.contains("PM"), "{h23}");
}

#[test]
fn a_language_without_icu_data_formats_as_the_message_locale() {
    for tag in ["und", "xx-YY", "tlh"] {
        let unknown = i18n(tag);
        assert_eq!(unknown.format_locale().to_string(), "en-US", "{tag}");
        assert_eq!(unknown.number(LocalNumber(1234.5)), "1,234.5", "{tag}");
        assert_eq!(
            unknown.list(&["a", "b", "c"], ListKind::And),
            "a, b, and c",
            "{tag}"
        );
        assert_eq!(
            unknown.show(wall(2025, 1, 15, 16, 9, 35), DateLength::Short, false),
            "1/15/25",
            "{tag}"
        );
    }
}

#[test]
fn counts_are_exact_for_every_i64() {
    let en = i18n("en-US");
    assert_eq!(en.count(i64::MAX), "9,223,372,036,854,775,807");
    assert_eq!(en.count(i64::MIN), "-9,223,372,036,854,775,808");
    assert_eq!(i18n("de").count(i64::MAX), "9.223.372.036.854.775.807");
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
fn a_full_fold_changes_length_and_en_does_not_match_dotted_capital_i() {
    let en = i18n("en-US");
    assert_eq!(en.fold_for_search("ß").chars().count(), 2);
    assert!(
        !en.fold_for_search("İstanbul")
            .contains(&en.fold_for_search("istanbul"))
    );
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
