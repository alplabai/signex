//! The catalog and the id registry agree, and every locale on disk is sound.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement, Resource};
use fluent_syntax::parser;

use crate::catalog::{AVAILABLE, EN_US, EN_US_SOURCES};
use crate::ids;
use crate::{I18n, LanguagePref};

/// message id -> the variables its pattern (and its attributes) read.
type Messages = BTreeMap<String, BTreeSet<String>>;

fn locales_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("locales")
}

fn parse_file(path: &Path) -> Resource<String> {
    let source = std::fs::read_to_string(path).unwrap();
    parser::parse(source)
        .unwrap_or_else(|(_, errors)| panic!("{} does not parse: {errors:?}", path.display()))
}

fn variables_of_pattern(pattern: &Pattern<String>, out: &mut BTreeSet<String>) {
    for element in &pattern.elements {
        if let PatternElement::Placeable { expression } = element {
            variables_of_expression(expression, out);
        }
    }
}

fn variables_of_expression(expression: &Expression<String>, out: &mut BTreeSet<String>) {
    match expression {
        Expression::Inline(inline) => variables_of_inline(inline, out),
        Expression::Select { selector, variants } => {
            variables_of_inline(selector, out);
            for variant in variants {
                variables_of_pattern(&variant.value, out);
            }
        }
    }
}

fn variables_of_inline(inline: &InlineExpression<String>, out: &mut BTreeSet<String>) {
    match inline {
        InlineExpression::VariableReference { id } => {
            out.insert(id.name.clone());
        }
        InlineExpression::Placeable { expression } => variables_of_expression(expression, out),
        InlineExpression::FunctionReference { arguments, .. }
        | InlineExpression::TermReference {
            arguments: Some(arguments),
            ..
        } => {
            for arg in arguments.positional.iter() {
                variables_of_inline(arg, out);
            }
            for named in &arguments.named {
                variables_of_inline(&named.value, out);
            }
        }
        _ => {}
    }
}

/// Every message of every `.ftl` file in `locales/<tag>/`; panics on a
/// parse error or a message id repeated across files.
fn messages_of(tag: &str) -> Messages {
    let mut files: Vec<PathBuf> = std::fs::read_dir(locales_dir().join(tag))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "ftl"))
        .collect();
    files.sort();
    let mut messages = Messages::new();
    for path in files {
        for entry in parse_file(&path).body {
            let Entry::Message(message) = entry else {
                continue;
            };
            let mut vars = BTreeSet::new();
            if let Some(pattern) = &message.value {
                variables_of_pattern(pattern, &mut vars);
            }
            for attribute in &message.attributes {
                variables_of_pattern(&attribute.value, &mut vars);
            }
            let id = message.id.name;
            assert!(
                messages.insert(id.clone(), vars).is_none(),
                "{tag}: message {id} is defined twice"
            );
        }
    }
    messages
}

fn locale_dirs() -> Vec<String> {
    let mut tags: Vec<String> = std::fs::read_dir(locales_dir())
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    tags.sort();
    tags
}

/// The ids rendered from a built key rather than a constant: one
/// `language-name-<tag>` per locale negotiation may pick.
fn language_name_ids() -> BTreeSet<String> {
    AVAILABLE
        .iter()
        .map(|tag| format!("language-name-{}", tag.to_lowercase()))
        .collect()
}

#[test]
fn every_defined_id_exists_in_en_us() {
    let en_us = messages_of(EN_US);
    for id in ids::ALL {
        assert!(
            en_us.contains_key(id.as_str()),
            "{} is not in en-US",
            id.as_str()
        );
    }
}

#[test]
fn every_en_us_message_is_a_defined_id() {
    let defined: BTreeSet<String> = ids::ALL
        .iter()
        .map(|id| id.as_str().to_owned())
        .chain(language_name_ids())
        .collect();
    for id in messages_of(EN_US).keys() {
        assert!(
            defined.contains(id),
            "en-US message {id} has no id in `ids`"
        );
    }
}

#[test]
fn the_id_registry_has_no_duplicates() {
    let unique: BTreeSet<_> = ids::ALL.iter().map(|id| id.as_str()).collect();
    assert_eq!(unique.len(), ids::ALL.len());
}

#[test]
fn every_locale_on_disk_parses_and_its_variables_are_in_the_source() {
    let source = messages_of(EN_US);
    let tags = locale_dirs();
    assert!(tags.iter().any(|tag| tag == EN_US));
    for tag in tags.iter().filter(|tag| *tag != EN_US) {
        for (id, vars) in messages_of(tag) {
            let Some(source_vars) = source.get(&id) else {
                panic!("{tag}: message {id} is not in en-US");
            };
            assert!(
                vars.is_subset(source_vars),
                "{tag}: {id} uses {vars:?}, en-US offers {source_vars:?}"
            );
        }
    }
}

#[test]
fn every_id_renders_from_the_embedded_en_us_catalog() {
    let i18n = I18n::new(&LanguagePref::Locale(EN_US.into()));
    for id in ids::ALL {
        let text = i18n.message(*id, &[]).to_string();
        assert_ne!(
            text,
            id.as_str(),
            "{} is not in the embedded catalog",
            id.as_str()
        );
    }
    // Ids render with no arguments, so a message with a `{ $var }` logs a
    // format problem; only a fall-back or a missing id is a failure here.
    assert_eq!(
        i18n.lookup_problems_for_test(),
        0,
        "an id fell back or is missing"
    );
}

#[test]
fn a_message_with_a_variable_is_not_a_lookup_problem() {
    let i18n = I18n::en_us_from_sources_for_test(&["with-var = Zoom { $percent }\n"]);
    let id = crate::MessageId::new("with-var");
    let _ = i18n.message(id, &[]);
    assert_eq!(i18n.emitted_for_test(), 1, "the format problem is logged");
    assert_eq!(i18n.lookup_problems_for_test(), 0);
    let _ = i18n.message(crate::MessageId::new("absent"), &[]);
    assert_eq!(i18n.lookup_problems_for_test(), 1);
}

#[test]
fn every_en_us_file_on_disk_is_embedded() {
    let dir = locales_dir().join(EN_US);
    let mut on_disk = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "ftl") {
            continue;
        }
        on_disk += 1;
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            EN_US_SOURCES.contains(&text.as_str()),
            "{} is not listed in EN_US_SOURCES",
            path.display()
        );
    }
    assert_eq!(
        on_disk,
        EN_US_SOURCES.len(),
        "EN_US_SOURCES lists a file twice or one that is gone"
    );
}

/// #2225: catalog text moved out of the views keeps its exact English. These
/// three once carried an extraction note after the value (#2298 review).
#[test]
fn moved_graphite_strings_keep_their_english() {
    let i18n = I18n::new(&LanguagePref::Locale(EN_US.into()));
    let plain = |s: String| s.replace(['\u{2068}', '\u{2069}'], "");
    assert_eq!(
        plain(
            i18n.message(ids::EDITORS_COMPONENT_UNSAVED_MARKER, &[])
                .to_string()
        ),
        "\u{2022} unsaved"
    );
    assert_eq!(
        plain(
            i18n.message(
                ids::LIBRARY_RANGE_FILTER_EXCLUDED,
                &[("count", 3usize.into()), ("unit", "Ω".into())]
            )
            .to_string()
        ),
        "3 excluded: not a measurement in Ω."
    );
    assert_eq!(
        plain(
            i18n.message(ids::LIBRARY_PREVIEW_OVERRIDE_PIN, &[("pin", "1".into())])
                .to_string()
        ),
        "Override pin 1 \u{2192}"
    );
    assert_eq!(
        plain(
            i18n.message(ids::PANELS_STATUS_BAR_SNAP_ON, &[])
                .to_string()
        ),
        "Snap on"
    );
    assert_eq!(
        plain(
            i18n.message(ids::PANELS_STATUS_BAR_SNAP_OFF, &[])
                .to_string()
        ),
        "Snap off"
    );
}

/// #2225 part 1, second wave: the strings moved out of the views #2291 had
/// open keep their English, plural and spacing included.
#[test]
fn moved_graphite_strings_of_the_second_wave_keep_their_english() {
    let i18n = I18n::new(&LanguagePref::Locale(EN_US.into()));
    let shown = |id, args: &[(&str, crate::Arg)]| {
        i18n.message(id, args)
            .to_string()
            .replace(['\u{2068}', '\u{2069}'], "")
    };
    let waiver = |hidden: usize| {
        shown(
            ids::PANELS_ERC_WAIVER_ROW,
            &[
                ("rule", "E101".into()),
                ("net", "GND".into()),
                ("hidden", hidden.into()),
            ],
        )
    };
    assert_eq!(waiver(0), "E101  net GND  (matches no finding)");
    assert_eq!(waiver(1), "E101  net GND  (1 finding hidden)");
    assert_eq!(waiver(3), "E101  net GND  (3 findings hidden)");
    let breadcrumb = |count: usize| {
        shown(
            ids::PANELS_FOOTPRINT_LIBRARY_BREADCRUMB,
            &[("file", "SOIC-8".into()), ("count", count.into())],
        )
    };
    assert_eq!(breadcrumb(1), "SOIC-8  (1 footprint)");
    assert_eq!(breadcrumb(0), "SOIC-8  (0 footprints)");
    assert_eq!(breadcrumb(2), "SOIC-8  (2 footprints)");
    assert_eq!(
        shown(
            ids::PANELS_SCH_LIBRARY_BREADCRUMB_IN_LIBRARY,
            &[
                ("library", "passives".into()),
                ("file", "R.snxsym".into()),
                ("count", 2usize.into()),
            ],
        ),
        "passives  \u{203A}  R.snxsym  (2 symbols)"
    );
    assert_eq!(
        shown(
            ids::PANELS_LIBRARY_ROW_SUMMARY,
            &[
                ("table", "Resistors".into()),
                ("class", "Resistor".into()),
                ("lifecycle", "Draft".into()),
                ("row", "c0ffee".into()),
            ],
        ),
        "table: Resistors  \u{00B7}  class: Resistor  \u{00B7}  Draft  \u{00B7}  row c0ffee"
    );
    assert_eq!(
        shown(
            ids::OVERLAY_PALETTE_MORE_RESULTS,
            &[("count", 181usize.into())]
        ),
        "181 more results \u{2014} refine query"
    );
    assert_eq!(
        shown(ids::PANELS_ERC_COUNT_ERRORS, &[("count", 2usize.into())]),
        "2 errors"
    );
    assert_eq!(
        shown(
            ids::PREFERENCES_APPEARANCE_ZOOM_STEP_VALUE,
            &[("percent", 15u32.into())]
        ),
        "15% per notch"
    );
    assert_eq!(
        shown(
            ids::DIALOG_ERC_PROFILE_UPDATE_HEADING,
            &[("name", "Strict".into()), ("count", 4usize.into())]
        ),
        "Update from profile \"Strict\" changes 4 entries:"
    );
    assert_eq!(
        shown(ids::PANELS_FOOTPRINT_LIBRARY_EMPTY, &[]),
        "Open a `.snxfpt` to see its footprints here. Right-click a `.snxlib` (or a project) \
         in the project tree and pick `Add New \u{25B8} Footprint Library` to create one."
    );
    assert_eq!(shown(ids::OVERLAY_PALETTE_GROUP_COMMANDS, &[]), "COMMANDS");
}

/// #2262: the Pad Properties header of a selected pad, singular and plural.
#[test]
fn the_pad_heading_names_the_pad_and_its_place_among_the_pads() {
    let i18n = I18n::new(&LanguagePref::Locale(EN_US.into()));
    let heading = |index: usize, pads: usize| {
        i18n.message(
            ids::EDITORS_FOOTPRINT_PAD_HEADING,
            &[
                ("number", "11".into()),
                ("kind", "SMD".into()),
                ("index", index.into()),
                ("pads", pads.into()),
            ],
        )
        .to_string()
        .replace(['\u{2068}', '\u{2069}'], "")
    };
    assert_eq!(heading(1, 11), "Pad 11 \u{00B7} SMD \u{00B7} 1 of 11 pads");
    assert_eq!(heading(1, 1), "Pad 11 \u{00B7} SMD \u{00B7} 1 of 1 pad");
}
