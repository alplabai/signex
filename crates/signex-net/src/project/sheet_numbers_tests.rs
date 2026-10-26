//! Sheet numbering on the spec's example tree (#1081): the root places Power
//! twice (A, B) and IO once (C); Power places LDO (L).

use std::collections::HashMap;

use signex_types::schematic::{
    ChildSheet, FillType, InstancePath, Point, SchematicSheet, SheetInstance,
};
use uuid::Uuid;

use super::*;
use crate::project::{ProjectGraph, ProjectRoot, SheetKey};

const A: u128 = 0xA;
const B: u128 = 0xB;
const C: u128 = 0xC;
const L: u128 = 0x1;

fn child(name: &str, filename: &str, uuid: u128) -> ChildSheet {
    ChildSheet {
        uuid: Uuid::from_u128(uuid),
        name: name.to_string(),
        filename: filename.to_string(),
        position: Point::new(0.0, 0.0),
        size: (10.0, 10.0),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    }
}

fn sheet(children: Vec<ChildSheet>) -> SchematicSheet {
    SchematicSheet {
        child_sheets: children,
        ..Default::default()
    }
}

fn path(chain: &[u128]) -> InstancePath {
    InstancePath::from_chain(chain.iter().map(|n| Uuid::from_u128(*n)).collect())
}

fn entry(path: &str, page: &str) -> SheetInstance {
    SheetInstance {
        project: String::new(),
        path: path.to_string(),
        page: page.to_string(),
    }
}

fn example() -> HashMap<String, SchematicSheet> {
    HashMap::from([
        (
            "root.snxsch".to_string(),
            sheet(vec![
                child("A", "power.snxsch", A),
                child("B", "power.snxsch", B),
                child("C", "io.snxsch", C),
            ]),
        ),
        (
            "power.snxsch".to_string(),
            sheet(vec![child("L", "ldo.snxsch", L)]),
        ),
        ("ldo.snxsch".to_string(), sheet(Vec::new())),
        ("io.snxsch".to_string(), sheet(Vec::new())),
    ])
}

/// Put `entry` on sheet symbol `symbol` of `file`.
fn with_entry(
    files: &mut HashMap<String, SchematicSheet>,
    file: &str,
    symbol: usize,
    entry: SheetInstance,
) {
    files.get_mut(file).expect("fixture file").child_sheets[symbol]
        .instances
        .push(entry);
}

/// Number `files` walked from `roots` (the first is the project root); each
/// sheet resolves its children by bare filename.
fn number(
    files: &HashMap<String, SchematicSheet>,
    roots: &[&str],
) -> (SheetNumbering, Vec<NumberingIssue>) {
    let sheets: HashMap<SheetKey, SchematicSheet> = files
        .iter()
        .map(|(name, sheet)| (SheetKey::new(name.clone()), sheet.clone()))
        .collect();
    let resolved: HashMap<SheetKey, HashMap<String, SheetKey>> = sheets
        .iter()
        .map(|(key, sheet)| {
            let submap = sheet
                .child_sheets
                .iter()
                .map(|cs| (cs.filename.clone(), SheetKey::new(cs.filename.clone())))
                .collect();
            (key.clone(), submap)
        })
        .collect();
    let roots: Vec<ProjectRoot> = roots
        .iter()
        .map(|root| ProjectRoot {
            key: SheetKey::new(*root),
        })
        .collect();
    let graph = ProjectGraph {
        sheets: &sheets,
        resolved: &resolved,
        roots: &roots,
    };
    sheet_numbering(&graph, "p")
}

fn numbers(numbering: &SheetNumbering) -> Vec<usize> {
    numbering.pages().iter().map(|page| page.number).collect()
}

#[test]
fn pages_are_numbered_depth_first_from_the_root() {
    let (numbering, issues) = number(&example(), &["root.snxsch"]);
    let order: Vec<(String, InstancePath)> = numbering
        .pages()
        .iter()
        .map(|page| (page.key.to_string(), page.instance_path.clone()))
        .collect();
    assert_eq!(
        order,
        vec![
            ("root.snxsch".to_string(), path(&[])),
            ("power.snxsch".to_string(), path(&[A])),
            ("ldo.snxsch".to_string(), path(&[A, L])),
            ("power.snxsch".to_string(), path(&[B])),
            ("ldo.snxsch".to_string(), path(&[B, L])),
            ("io.snxsch".to_string(), path(&[C])),
        ]
    );
    assert_eq!(numbers(&numbering), [1, 2, 3, 4, 5, 6]);
    assert_eq!(numbering.count(), 6);
    assert_eq!(
        numbering.number_of(&SheetKey::new("ldo.snxsch"), &path(&[B, L])),
        Some(5)
    );
    assert!(issues.is_empty(), "{issues:?}");
}

#[test]
fn a_manual_number_changes_only_its_own_page() {
    let mut files = example();
    // On L inside Power, for the Power placement B: numbers /B/L.
    with_entry(
        &mut files,
        "power.snxsch",
        0,
        entry(&path(&[B]).to_string(), "9"),
    );
    let (numbering, issues) = number(&files, &["root.snxsch"]);
    assert_eq!(numbers(&numbering), [1, 2, 3, 4, 9, 6]);
    assert!(numbering.pages()[4].manual);
    assert!(
        !numbering.pages()[2].manual,
        "/A/L keeps its automatic number"
    );
    assert_eq!(
        issues,
        vec![
            NumberingIssue::OutOfRange { page: 4, number: 9 },
            NumberingIssue::Gap { missing: vec![5] },
        ]
    );
}

#[test]
fn two_pages_with_one_number_clash() {
    let mut files = example();
    with_entry(&mut files, "root.snxsch", 2, entry("/", "2"));
    let (numbering, issues) = number(&files, &["root.snxsch"]);
    assert_eq!(numbers(&numbering), [1, 2, 3, 4, 5, 2]);
    assert_eq!(
        issues,
        vec![
            NumberingIssue::Clash {
                number: 2,
                pages: vec![1, 5],
            },
            NumberingIssue::Gap { missing: vec![6] },
        ]
    );
}

#[test]
fn an_unusable_manual_number_is_reported_and_ignored() {
    let mut files = example();
    with_entry(&mut files, "root.snxsch", 0, entry("/", "abc"));
    with_entry(&mut files, "root.snxsch", 1, entry("/", ""));
    with_entry(&mut files, "root.snxsch", 2, entry("/", "0"));
    let (numbering, issues) = number(&files, &["root.snxsch"]);
    assert_eq!(numbers(&numbering), [1, 2, 3, 4, 5, 6]);
    assert_eq!(
        issues,
        vec![
            NumberingIssue::InvalidOverride {
                page: 1,
                text: "abc".to_string(),
            },
            NumberingIssue::InvalidOverride {
                page: 3,
                text: String::new(),
            },
            NumberingIssue::InvalidOverride {
                page: 5,
                text: "0".to_string(),
            },
        ]
    );
}

#[test]
fn an_entry_for_another_project_is_ignored() {
    let mut files = example();
    let mut other = entry("/", "2");
    other.project = "other".to_string();
    with_entry(&mut files, "root.snxsch", 2, other);
    let (numbering, issues) = number(&files, &["root.snxsch"]);
    assert_eq!(numbers(&numbering), [1, 2, 3, 4, 5, 6]);
    assert!(issues.is_empty(), "{issues:?}");

    let mut files = example();
    let mut ours = entry("/", "7");
    ours.project = "p".to_string();
    with_entry(&mut files, "root.snxsch", 2, ours);
    let (numbering, _) = number(&files, &["root.snxsch"]);
    assert_eq!(numbering.pages()[5].number, 7);
}

#[test]
fn only_a_slash_path_names_the_root_placement() {
    let mut files = example();
    with_entry(&mut files, "root.snxsch", 2, entry("", "7"));
    let (numbering, issues) = number(&files, &["root.snxsch"]);
    assert_eq!(
        numbers(&numbering),
        [1, 2, 3, 4, 5, 6],
        "\"\" is an unset path"
    );
    assert!(issues.is_empty(), "{issues:?}");
}

#[test]
fn a_page_the_root_does_not_reach_follows_the_hierarchy() {
    let mut files = example();
    files.insert("notes.snxsch".to_string(), sheet(Vec::new()));
    let (numbering, issues) = number(&files, &["root.snxsch", "notes.snxsch"]);
    assert_eq!(numbering.count(), 7);
    // Both sit at the empty path; the key tells them apart.
    assert_eq!(
        numbering.number_of(&SheetKey::new("notes.snxsch"), &path(&[])),
        Some(7)
    );
    assert_eq!(
        numbering.number_of(&SheetKey::new("root.snxsch"), &path(&[])),
        Some(1)
    );
    assert!(issues.is_empty(), "{issues:?}");
}

#[test]
fn a_missing_or_cyclic_child_is_not_a_page() {
    let mut files = example();
    files
        .get_mut("ldo.snxsch")
        .expect("fixture file")
        .child_sheets
        .push(child("Back", "power.snxsch", 0xD));
    files
        .get_mut("io.snxsch")
        .expect("fixture file")
        .child_sheets
        .push(child("Gone", "gone.snxsch", 0xE));
    let (numbering, issues) = number(&files, &["root.snxsch"]);
    assert_eq!(numbers(&numbering), [1, 2, 3, 4, 5, 6]);
    assert!(issues.is_empty(), "{issues:?}");
}

#[test]
fn index_of_finds_a_placement_in_walk_order() {
    let mut files = example();
    // A manual number moves the printed number, not the walk position.
    with_entry(&mut files, "root.snxsch", 2, entry("/", "2"));
    let (numbering, _) = number(&files, &["root.snxsch"]);
    let io = SheetKey::new("io.snxsch");
    assert_eq!(numbering.index_of(&io, &path(&[C])), Some(5));
    assert_eq!(numbering.number_of(&io, &path(&[C])), Some(2));
    assert_eq!(numbering.index_of(&io, &path(&[A])), None);
}

#[test]
fn a_page_label_is_its_name_chain_or_its_sheet() {
    let page = |name_chain: Vec<String>| SheetPage {
        key: SheetKey::new("power.snxsch"),
        instance_path: path(&[]),
        name_chain,
        number: 1,
        manual: false,
    };
    assert_eq!(page(Vec::new()).label(), "power.snxsch");
    assert_eq!(page(vec!["A".to_string(), "L".to_string()]).label(), "A/L");
}
