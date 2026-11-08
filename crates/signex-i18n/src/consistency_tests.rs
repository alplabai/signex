//! The catalog and the id registry agree, and every locale on disk is sound.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement, Resource};
use fluent_syntax::parser;

use crate::catalog::{AVAILABLE, EN_US};
use crate::ids;

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
