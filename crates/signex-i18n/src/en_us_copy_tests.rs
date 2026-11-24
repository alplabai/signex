//! The en-US catalog's spelling, and chrome copy that mockup rows settle.

use fluent_syntax::ast::{Entry, Expression, Pattern, PatternElement};
use fluent_syntax::parser;

use crate::catalog::{EN_US, EN_US_SOURCES};
use crate::{I18n, LanguagePref, command_ids, ids, menu_ids};

/// Every literal text run of `pattern`, the variants of a select included.
/// Ids, variable names and comments are not text, so they stay out.
fn text_runs(pattern: &Pattern<String>, out: &mut Vec<String>) {
    for element in &pattern.elements {
        match element {
            PatternElement::TextElement { value } => out.push(value.clone()),
            PatternElement::Placeable {
                expression: Expression::Select { variants, .. },
            } => {
                for variant in variants {
                    text_runs(&variant.value, out);
                }
            }
            PatternElement::Placeable { .. } => {}
        }
    }
}

/// `(entry id, text run)` for every message, term and attribute value of
/// the embedded en-US catalog.
fn en_us_text() -> Vec<(String, String)> {
    let mut all = Vec::new();
    for source in EN_US_SOURCES {
        let resource = parser::parse(source.to_owned())
            .unwrap_or_else(|(_, errors)| panic!("en-US does not parse: {errors:?}"));
        for entry in resource.body {
            let (id, value, attributes) = match entry {
                Entry::Message(m) => (m.id.name, m.value, m.attributes),
                Entry::Term(t) => (t.id.name, Some(t.value), t.attributes),
                _ => continue,
            };
            let mut runs = Vec::new();
            if let Some(pattern) = &value {
                text_runs(pattern, &mut runs);
            }
            for attribute in &attributes {
                text_runs(&attribute.value, &mut runs);
            }
            all.extend(runs.into_iter().map(|run| (id.clone(), run)));
        }
    }
    all
}

/// Mockup row 72: en-US spells it "color" in every value; ids keep their
/// names.
#[test]
fn no_en_us_value_says_colour() {
    let british: Vec<(String, String)> = en_us_text()
        .into_iter()
        .filter(|(_, run)| run.to_lowercase().contains("colour"))
        .collect();
    assert!(
        british.is_empty(),
        "en-US values that say colour: {british:?}"
    );
}

/// The values mockup rows 11, 60, 72, 83, 87 and 14 settle.
#[test]
fn the_chrome_copy_of_the_settled_mockup_rows() {
    let i18n = I18n::new(&LanguagePref::Locale(EN_US.into()));
    for (id, english) in [
        (
            ids::NOTICE_SAVE_FAILED_EDITS_KEPT,
            "Your edits are still open and unsaved.",
        ),
        (ids::PANELS_STUB_FAVORITES, "Favorite components"),
        (ids::DIALOG_TITLE_NET_COLOR_CUSTOM, "Custom Net Color"),
        (ids::DIALOG_NET_COLOR_CLEAR_ALL, "Clear all colors"),
        (ids::PCB_REANNOTATE_TITLE, "Re-Annotate Board"),
        (command_ids::CMD_MENU_OPEN_DRC_PANEL, "DRC"),
        (menu_ids::MENU_HELP_WELCOME_TIPS, "Show Welcome Tips"),
    ] {
        assert_eq!(i18n.message_plain(id, &[]), english, "{id:?}");
    }
    let subtitle = i18n.message_plain(
        ids::PCB_REANNOTATE_SUBTITLE,
        &[("board", "b.snxpcb".into())],
    );
    assert_eq!(subtitle, "Positional \u{00B7} b.snxpcb");
}
