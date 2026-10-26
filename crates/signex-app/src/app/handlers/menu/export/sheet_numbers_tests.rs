//! The numbering applied to printed pages, and the length of its Messages
//! lines (#1081).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use signex_net::{NumberingIssue, ProjectGraph, ProjectRoot, SheetKey};
use signex_output::SheetSnapshot;
use signex_types::schematic::{InstancePath, SchematicSheet};
use uuid::Uuid;

use super::{MAX_LINE_BYTES, ScopeNumbering, numbering_messages};
use crate::app::handlers::menu::export::tests::sheet_with_net;
use crate::app::state::scope::CaseFold;

const DIR: &str = "/w/units";

fn key(name: &str) -> SheetKey {
    crate::app::project_sheets::sheet_key_with(
        &Path::new(DIR).join(name),
        Some(Path::new(DIR)),
        CaseFold::default(),
    )
}

/// `top.snxsch` placing `child.snxsch` once per name in `places`.
fn scope(places: &[&str]) -> ScopeNumbering {
    let mut top = sheet_with_net("R_TOP", "TOP_NET", &vec!["child.snxsch"; places.len()]);
    for (n, (cs, name)) in top.child_sheets.iter_mut().zip(places).enumerate() {
        cs.name = (*name).to_string();
        cs.uuid = Uuid::from_u128(n as u128 + 1);
    }
    let sheets: HashMap<SheetKey, SchematicSheet> = HashMap::from([
        (key("top.snxsch"), top),
        (key("child.snxsch"), SchematicSheet::default()),
    ]);
    let resolved = HashMap::from([(
        key("top.snxsch"),
        HashMap::from([("child.snxsch".to_string(), key("child.snxsch"))]),
    )]);
    let roots = [ProjectRoot {
        key: key("top.snxsch"),
    }];
    let graph = ProjectGraph {
        sheets: &sheets,
        resolved: &resolved,
        roots: &roots,
    };
    let (numbering, issues) = signex_net::sheet_numbering(&graph, "p");
    ScopeNumbering::new(
        numbering,
        issues,
        Some(PathBuf::from(DIR)),
        CaseFold::default(),
    )
}

fn page(name: &str, placement: InstancePath, number: usize) -> SheetSnapshot {
    SheetSnapshot {
        path: Path::new(DIR).join(name),
        schematic: SchematicSheet::default(),
        sheet_name: name.to_string(),
        sheet_number: number,
        sheet_count: 9,
        placement,
    }
}

#[test]
fn pages_follow_the_walk_and_an_unknown_page_goes_last_with_the_count() {
    let numbering = scope(&["A", "B"]);
    let second = InstancePath::from_chain(vec![Uuid::from_u128(2)]);
    let pages = vec![
        page("stray.snxsch", InstancePath::default(), 5),
        page("child.snxsch", second, 1),
        page("top.snxsch", InstancePath::default(), 2),
    ];
    let numbered: Vec<(String, usize, usize)> = numbering
        .numbered_pages(pages)
        .into_iter()
        .map(|page| (page.sheet_name, page.sheet_number, page.sheet_count))
        .collect();
    assert_eq!(
        numbered,
        [
            ("top.snxsch".to_string(), 1, 3),
            ("child.snxsch".to_string(), 3, 3),
            ("stray.snxsch".to_string(), 5, 3),
        ]
    );
}

#[test]
fn a_clash_among_long_names_fits_the_messages_line() {
    let names: Vec<String> = (0..6)
        .map(|n| format!("A_rather_long_sheet_symbol_name_number_{n}"))
        .collect();
    let places: Vec<&str> = names.iter().map(String::as_str).collect();
    let numbering = scope(&places);
    let clash = NumberingIssue::Clash {
        number: 2,
        pages: (1..=6).collect(),
    };
    let lines = numbering_messages(&numbering.numbering, &[clash]);
    assert_eq!(lines.len(), 1);
    let line = &lines[0];
    assert!(line.len() <= MAX_LINE_BYTES, "{} bytes: {line}", line.len());
    assert!(
        line.starts_with("Sheet numbering: number 2 is printed by pages A_rather_long"),
        "{line}"
    );
    assert!(line.ends_with(" more"), "{line}");
    assert!(line.contains(" and "), "{line}");
}

#[test]
fn a_long_gap_fits_the_messages_line() {
    let numbering = scope(&["A"]);
    let gap = NumberingIssue::Gap {
        missing: (100..140).collect(),
    };
    let lines = numbering_messages(&numbering.numbering, &[gap]);
    let line = &lines[0];
    assert!(line.len() <= MAX_LINE_BYTES, "{} bytes: {line}", line.len());
    assert!(
        line.starts_with("Sheet numbering: no page prints numbers 100, 101"),
        "{line}"
    );
    let shown = line.matches(", ").count() + 1;
    assert!(
        line.ends_with(&format!(" and {} more", 40 - shown)),
        "{line}"
    );
}

#[test]
fn a_single_name_too_long_for_the_line_is_cut_and_the_rest_counted() {
    let long = "N".repeat(200);
    let numbering = scope(&[long.as_str(), "B"]);
    let clash = NumberingIssue::Clash {
        number: 2,
        pages: vec![1, 2],
    };
    let line = &numbering_messages(&numbering.numbering, &[clash])[0];
    assert!(line.len() <= MAX_LINE_BYTES, "{} bytes: {line}", line.len());
    assert!(line.ends_with("NNN and 1 more"), "{line}");
}
