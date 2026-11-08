use unic_langid::LanguageIdentifier;

use crate::bundle::{build_from, render};
use crate::catalog::{EN_US, EN_XA};
use crate::locale::negotiate;
use crate::{Arg, I18n, LanguagePref, LocalNumber, MessageId, UserText, ids};

const FSI: char = '\u{2068}';
const PDI: char = '\u{2069}';

fn langs(tags: &[&str]) -> Vec<LanguageIdentifier> {
    tags.iter().map(|t| t.parse().unwrap()).collect()
}

fn i18n(tag: &str) -> I18n {
    I18n::new(&LanguagePref::Locale(tag.to_owned()))
}

#[test]
fn negotiation_falls_back_to_en_us() {
    assert_eq!(negotiate(&langs(&["tr-TR"])), EN_US);
    assert_eq!(negotiate(&langs(&["tr-TR", "fr"])), EN_US);
    assert_eq!(negotiate(&langs(&[])), EN_US);
}

#[test]
fn negotiation_prefers_an_exact_match_then_the_language() {
    assert_eq!(negotiate(&langs(&["en-XA"])), EN_XA);
    assert_eq!(negotiate(&langs(&["tr-TR", "en-XA"])), EN_XA);
    assert_eq!(negotiate(&langs(&["en"])), EN_US);
}

#[test]
fn an_unknown_saved_locale_keeps_its_pref_and_renders_in_en_us() {
    let lost = i18n("tr-TR");
    assert_eq!(lost.dropped_locale(), Some("tr-TR"));
    assert_eq!(lost.pref(), &LanguagePref::Locale("tr-TR".into()));
    assert_eq!(lost.locale(), EN_US);
    assert_eq!(t!(lost, ids::MENU_FILE).to_string(), "File");
    assert_eq!(i18n("en-US").dropped_locale(), None);
}

#[test]
fn a_case_or_script_variant_of_an_available_language_is_not_dropped() {
    for tag in ["en-xa", "EN-xa", "en-Latn-XA"] {
        let variant = i18n(tag);
        assert_eq!(variant.dropped_locale(), None, "{tag}");
        assert_eq!(variant.locale(), EN_XA, "{tag}");
        assert_eq!(variant.pref(), &LanguagePref::Locale(tag.into()));
    }
}

#[test]
fn another_english_renders_en_us_and_is_not_reported_as_lost() {
    let british = i18n("en-GB");
    assert_eq!(british.locale(), EN_US);
    assert_eq!(british.dropped_locale(), None);
}

#[test]
fn an_unparseable_saved_tag_is_dropped() {
    assert_eq!(i18n("not a tag").dropped_locale(), Some("not a tag"));
}

#[test]
fn the_picker_shows_a_saved_tag_the_list_does_not_have() {
    let lost = i18n("tr-TR");
    let row = lost.option_for(&lost.pref().clone());
    assert_eq!(row.pref, LanguagePref::Locale("tr-TR".into()));
    assert_eq!(row.label, "tr-TR");
    let system = lost.option_for(&LanguagePref::System);
    assert_eq!(system.label, "System default");
}

#[test]
fn a_problem_is_logged_once_however_often_the_message_is_rendered() {
    let en = i18n("en-US");
    let missing = MessageId::new("no-such-message");
    for _ in 0..3 {
        let _ = t!(en, missing);
    }
    assert_eq!(en.emitted_for_test(), 1);
    let xa = i18n("en-XA");
    for _ in 0..3 {
        let _ = t!(xa, missing);
    }
    // One fall-back line and one missing line, not six.
    assert_eq!(xa.emitted_for_test(), 2);
}

#[test]
fn a_message_missing_everywhere_shows_its_id() {
    let en = i18n("en-US");
    assert_eq!(
        t!(en, MessageId::new("no-such-message")).to_string(),
        "no-such-message"
    );
}

#[test]
fn the_pseudolocale_transforms_text_and_en_us_is_untouched() {
    let xa = t!(i18n("en-XA"), ids::MENU_FILE).to_string();
    assert_ne!(xa, "File");
    assert!(xa.contains('Ƒ'), "accented capital F in {xa}");
    assert_eq!(t!(i18n("en-US"), ids::MENU_FILE).to_string(), "File");
}

#[test]
fn numbers_follow_the_locale_through_icu() {
    let source = "count = { $n } items\n";
    let arg = [("n", Arg::from(LocalNumber(1_234_567.5)))];
    let us = render(&build_from("en-US", &[source]), "count", &arg)
        .unwrap()
        .text;
    let de = render(&build_from("de", &[source]), "count", &arg)
        .unwrap()
        .text;
    assert!(us.contains("1,234,567.5"), "{us}");
    assert!(de.contains("1.234.567,5"), "{de}");
}

#[test]
fn isolation_marks_wrap_only_placeables() {
    let sources = ["plain = Plain text\nhello = Hello { $name }\n"];
    let bundle = build_from("en-US", &sources);
    let plain = render(&bundle, "plain", &[]).unwrap().text;
    let hello = render(&bundle, "hello", &[("name", Arg::from("R1"))])
        .unwrap()
        .text;
    assert!(!plain.contains(FSI) && !plain.contains(PDI), "{plain:?}");
    assert_eq!(hello, format!("Hello {FSI}R1{PDI}"));
}

#[test]
fn the_catalog_title_has_no_isolation_marks() {
    let title = t!(i18n("en-US"), ids::MENU_FILE).to_string();
    assert!(!title.contains(FSI) && !title.contains(PDI));
}

#[test]
fn the_language_list_starts_with_system_and_is_ordered_by_code() {
    let en = i18n("en-US");
    let options = en.language_options();
    assert_eq!(options[0].pref, LanguagePref::System);
    assert_eq!(options[0].label, "System default");
    let tags: Vec<_> = options[1..]
        .iter()
        .map(|o| o.pref.saved_tag().unwrap())
        .collect();
    let mut sorted = tags.clone();
    sorted.sort_unstable();
    assert_eq!(tags, sorted);
    assert_eq!(options[1].label, "English (United States)");
    assert_eq!(
        en.option_for(&LanguagePref::Locale(EN_US.into())),
        options[1].clone()
    );
}

#[test]
fn the_pseudolocale_is_listed_only_in_debug_builds() {
    let listed = i18n("en-US")
        .language_options()
        .iter()
        .any(|o| o.pref == LanguagePref::Locale(EN_XA.into()));
    assert_eq!(listed, cfg!(debug_assertions));
}

#[test]
fn an_absent_saved_language_reads_as_system() {
    assert_eq!(LanguagePref::from_saved(None), LanguagePref::System);
    assert_eq!(LanguagePref::System.saved_tag(), None);
    let xa = LanguagePref::from_saved(Some("en-XA"));
    assert_eq!(LanguagePref::from_saved(xa.saved_tag()), xa);
}

#[test]
fn user_text_is_kept_verbatim() {
    assert_eq!(UserText::new("NET_1").as_str(), "NET_1");
}
