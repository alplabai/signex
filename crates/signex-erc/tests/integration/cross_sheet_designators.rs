//! #915: `DuplicateRefDesignator` follows the whole sheet hierarchy, not just
//! the top sheet's direct children.

use std::collections::HashMap;

use signex_erc::{RuleKind, Violation, run_with_project};
use signex_net::SheetKey;
use signex_types::schematic::{LibSymbol, SchematicSheet, Symbol};
use uuid::Uuid;

type Resolutions = HashMap<SheetKey, HashMap<String, SheetKey>>;

fn key(name: &str) -> SheetKey {
    SheetKey::new(name)
}

fn lib_symbol(id: &str) -> LibSymbol {
    LibSymbol {
        id: id.to_string(),
        reference: "R".to_string(),
        value: id.to_string(),
        footprint: String::new(),
        datasheet: String::new(),
        description: String::new(),
        keywords: String::new(),
        fp_filters: String::new(),
        in_bom: true,
        on_board: true,
        in_pos_files: true,
        duplicate_pin_numbers_are_jumpers: false,
        graphics: Vec::new(),
        pins: Vec::new(),
        show_pin_numbers: true,
        show_pin_names: true,
        pin_name_offset: 0.0,
    }
}

fn sheet(refs: &[&str]) -> SchematicSheet {
    let mut sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        symbols: refs
            .iter()
            .map(|r| Symbol {
                uuid: Uuid::now_v7(),
                lib_id: "Device:R".to_string(),
                reference: (*r).to_string(),
                unit: 1,
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    sheet
        .lib_symbols
        .insert("Device:R".to_string(), lib_symbol("Device:R"));
    sheet
}

/// `(sheet name, designators, [(child filename, child sheet name)])`; the
/// first entry is the top sheet.
type Spec<'a> = [(&'a str, &'a [&'a str], &'a [(&'a str, &'a str)])];

fn run_top(spec: &Spec) -> Vec<Violation> {
    let sheets: HashMap<SheetKey, SchematicSheet> = spec
        .iter()
        .map(|(n, refs, _)| (key(n), sheet(refs)))
        .collect();
    let resolutions: Resolutions = spec
        .iter()
        .map(|(n, _, kids)| {
            (
                key(n),
                kids.iter()
                    .map(|(f, k)| ((*f).to_string(), key(k)))
                    .collect(),
            )
        })
        .collect();
    let top = key(spec[0].0);
    run_with_project(&sheets[&top], Some(&top), &resolutions, &sheets)
        .into_iter()
        .filter(|v| v.rule == RuleKind::DuplicateRefDesignator)
        .collect()
}

#[test]
fn top_symbol_clashes_with_a_grandchild() {
    let found = run_top(&[
        ("top", &["R1"], &[("a.snxsch", "a")]),
        ("a", &[], &[("b.snxsch", "b")]),
        ("b", &["R1"], &[]),
    ]);
    assert_eq!(found.len(), 2, "{found:?}");
    let child = found
        .iter()
        .find(|v| v.primary.is_none())
        .expect("grandchild finding");
    assert!(child.message.contains("b.snxsch"), "{}", child.message);
}

#[test]
fn sibling_subtrees_clash_through_the_top() {
    let found = run_top(&[
        ("top", &[], &[("a.snxsch", "a"), ("c.snxsch", "c")]),
        ("a", &[], &[("b.snxsch", "b")]),
        ("b", &["R7"], &[]),
        ("c", &[], &[("d.snxsch", "d")]),
        ("d", &["R7"], &[]),
    ]);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found.iter().all(|v| v.primary.is_none()));
    assert!(found.iter().any(|v| v.message.contains("b.snxsch")));
    assert!(found.iter().any(|v| v.message.contains("d.snxsch")));
}

#[test]
fn a_reference_cycle_terminates_without_self_duplicates() {
    let found = run_top(&[
        ("top", &["R1"], &[("a.snxsch", "a")]),
        ("a", &["R2"], &[("top.snxsch", "top")]),
    ]);
    assert!(found.is_empty(), "{found:?}");
}

/// Every sheet's own run, as the app does; `DuplicateRefDesignator` only.
fn run_every_sheet(spec: &Spec) -> usize {
    let sheets: HashMap<SheetKey, SchematicSheet> = spec
        .iter()
        .map(|(n, refs, _)| (key(n), sheet(refs)))
        .collect();
    let resolutions: Resolutions = spec
        .iter()
        .map(|(n, _, kids)| {
            (
                key(n),
                kids.iter()
                    .map(|(f, k)| ((*f).to_string(), key(k)))
                    .collect(),
            )
        })
        .collect();
    sheets
        .iter()
        .flat_map(|(k, s)| run_with_project(s, Some(k), &resolutions, &sheets))
        .filter(|v| v.rule == RuleKind::DuplicateRefDesignator)
        .count()
}

#[test]
fn a_clash_between_two_descendants_is_reported_by_one_ancestor_only() {
    // top -> mid -> (a, b); a and b clash. Only `mid`, the lowest sheet that
    // sees both, reports it: one finding per clashing symbol, not per ancestor.
    let total = run_every_sheet(&[
        ("top", &[], &[("mid.snxsch", "mid")]),
        ("mid", &[], &[("a.snxsch", "a"), ("b.snxsch", "b")]),
        ("a", &["R9"], &[]),
        ("b", &["R9"], &[]),
    ]);
    assert_eq!(total, 2);
}

#[test]
fn a_child_instantiated_twice_is_not_a_duplicate_of_itself() {
    let found = run_top(&[
        (
            "top",
            &[],
            &[("a.snxsch", "a"), ("b.snxsch", "b"), ("c2.snxsch", "c")],
        ),
        ("a", &[], &[("c.snxsch", "c")]),
        ("b", &[], &[("c.snxsch", "c")]),
        ("c", &["R1"], &[]),
    ]);
    assert!(found.is_empty(), "{found:?}");
}
