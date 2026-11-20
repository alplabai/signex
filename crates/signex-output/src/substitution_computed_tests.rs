//! Maintainer decision 2026-11-18: every row name of Document Options ▸
//! Parameters is a token of the one resolver, the computed ones (the clock,
//! the file's modified date and full path) included.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use super::*;
use crate::DocumentFacts;

const PATH: &str = "/work/power.snxsch";

fn sheet(title_block: &[(&str, &str)]) -> ProjectMetadata {
    let title_block: HashMap<String, String> = title_block
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let mut m = ProjectMetadata::default().for_sheet(&title_block);
    m.document_facts = DocumentFacts {
        current_date: "2026-11-18".into(),
        current_time: "09:05".into(),
        modified_dates: BTreeMap::from([(PathBuf::from(PATH), "2026-11-17".into())]),
    };
    m
}

fn resolve_on(m: &ProjectMetadata, text: &str) -> String {
    resolve(
        text,
        &SubstitutionContext {
            metadata: m,
            document_path: PathBuf::from(PATH),
            filename: "power.snxsch".into(),
            sheet_name: "power".into(),
            sheet_number: 1,
            sheet_count: 1,
            signex_version: "1.0.0",
            variant: None,
            physical_structure: false,
            physical_sheet_number: true,
            physical_document_number: true,
        },
    )
}

#[test]
fn each_computed_token_resolves() {
    let m = sheet(&[]);
    for (token, value) in [
        ("${CurrentDate}", "2026-11-18"),
        ("${CURRENTDATE}", "2026-11-18"),
        ("${CurrentTime}", "09:05"),
        ("${Time}", "09:05"),
        ("${ModifiedDate}", "2026-11-17"),
        ("${DocumentFullPathAndName}", PATH),
        ("${DocumentName}", "power.snxsch"),
    ] {
        assert_eq!(resolve_on(&m, token), value, "{token}");
    }
}

/// An empty Date shows today; a typed one wins.
#[test]
fn an_empty_date_shows_today_and_a_typed_one_wins() {
    let empty = resolve_on(&sheet(&[]), "${Date}|${DATE}");
    assert_eq!(empty, "2026-11-18|2026-11-18");
    let typed = resolve_on(&sheet(&[("date", "2026-01-02")]), "${Date}|${CurrentDate}");
    assert_eq!(typed, "2026-01-02|2026-11-18");
}

/// The title-block fields read by their Parameters names, the new ones
/// included; an unset one is empty.
#[test]
fn each_title_block_field_reads_by_its_parameter_name() {
    let m = sheet(&[
        ("company", "Alp Lab"),
        ("author", "A"),
        ("engineer", "E"),
        ("organization", "O"),
        ("address1", "L1"),
        ("address2", "L2"),
        ("address3", "L3"),
        ("address4", "L4"),
    ]);
    assert_eq!(
        resolve_on(
            &m,
            "${CompanyName}|${Author}|${Engineer}|${Organization}|${Address1}|${Address2}|${Address3}|${Address4}"
        ),
        "Alp Lab|A|E|O|L1|L2|L3|L4"
    );
    assert_eq!(resolve_on(&sheet(&[]), "[${Author}]"), "[]");
}

/// A context with no document, as the BOM and the netlist use, shows the
/// computed tokens empty rather than another file's values.
#[test]
fn no_document_shows_no_path_or_modified_date() {
    let m = sheet(&[]);
    let context = crate::sheet_context(&m);
    assert_eq!(resolve("[${ModifiedDate}]", &context), "[]");
}
