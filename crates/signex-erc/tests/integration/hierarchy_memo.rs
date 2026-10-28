//! #998: a sheet reachable through several paths is projected once per ERC
//! run, not once per path.

use std::collections::HashMap;

use signex_erc::{ErcConfig, HierarchyMemo, run_with_project, run_with_project_memoised};
use signex_net::SheetKey;
use signex_types::schematic::{LibSymbol, SchematicSheet, Symbol};
use uuid::Uuid;

type Resolutions = HashMap<SheetKey, HashMap<String, SheetKey>>;
type Sheets = HashMap<SheetKey, SchematicSheet>;

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

fn sheet(reference: &str) -> SchematicSheet {
    let mut sheet = SchematicSheet {
        paper_size: "A4".to_string(),
        symbols: vec![Symbol {
            uuid: Uuid::new_v4(),
            lib_id: "Device:R".to_string(),
            reference: reference.to_string(),
            unit: 1,
            ..Default::default()
        }],
        ..Default::default()
    };
    sheet
        .lib_symbols
        .insert("Device:R".to_string(), lib_symbol("Device:R"));
    sheet
}

/// `k` stacked diamonds: `t{i} -> l{i}, r{i}`; `l{i}, r{i} -> t{i+1}`.
/// `3k + 1` sheets, `2^k` paths from the top to the last one. Every sheet
/// carries a distinct designator unless `clash` is set, in which case the
/// two arms of the first diamond share one.
fn diamond_chain(k: usize, clash: bool) -> (Resolutions, Sheets) {
    let mut sheets = Sheets::new();
    let mut resolutions = Resolutions::new();
    let mut add = |name: String, reference: String, kids: Vec<String>| {
        sheets.insert(key(&name), sheet(&reference));
        resolutions.insert(
            key(&name),
            kids.iter()
                .map(|n| (format!("{n}.snxsch"), key(n)))
                .collect(),
        );
    };
    for i in 0..k {
        let next = format!("t{}", i + 1);
        add(
            format!("t{i}"),
            format!("RT{i}"),
            vec![format!("l{i}"), format!("r{i}")],
        );
        let arm = |side: &str| {
            if clash && i == 0 {
                "RDUP".to_string()
            } else {
                format!("R{side}{i}")
            }
        };
        add(format!("l{i}"), arm("L"), vec![next.clone()]);
        add(format!("r{i}"), arm("R"), vec![next]);
    }
    add(format!("t{k}"), format!("RT{k}"), Vec::new());
    (resolutions, sheets)
}

#[test]
fn each_sheet_of_a_diamond_chain_is_projected_exactly_once() {
    let k = 12; // 2^12 paths; the old walk projected thousands of contexts.
    let (resolutions, sheets) = diamond_chain(k, false);
    let top = key("t0");
    let mut memo = HierarchyMemo::new();
    let found = run_with_project_memoised(
        &sheets[&top],
        Some(&top),
        &resolutions,
        &sheets,
        &[],
        &ErcConfig::default(),
        &mut memo,
    );
    assert_eq!(memo.projected_count(), sheets.len());
    assert!(
        !found
            .iter()
            .any(|v| v.rule == signex_erc::RuleKind::DuplicateRefDesignator),
        "{found:?}"
    );
}

#[test]
fn a_shared_memo_projects_each_child_once_across_every_sheets_run() {
    let (resolutions, sheets) = diamond_chain(6, false);
    let mut memo = HierarchyMemo::new();
    for (name, snapshot) in &sheets {
        run_with_project_memoised(
            snapshot,
            Some(name),
            &resolutions,
            &sheets,
            &[],
            &ErcConfig::default(),
            &mut memo,
        );
    }
    // One projection per sheet as a child, plus one per sheet as a run's top.
    assert!(
        memo.projected_count() <= 2 * sheets.len(),
        "{} contexts for {} sheets",
        memo.projected_count(),
        sheets.len()
    );
}

#[test]
fn memoised_runs_report_what_the_plain_runs_report() {
    let (resolutions, sheets) = diamond_chain(3, true);
    let mut memo = HierarchyMemo::new();
    let render = |v: &[signex_erc::Violation]| {
        let mut lines: Vec<String> = v
            .iter()
            .map(|v| format!("{:?}|{}|{:?}", v.rule, v.message, v.location_space))
            .collect();
        lines.sort();
        lines
    };
    let mut clashes = 0;
    for (name, snapshot) in &sheets {
        let plain = run_with_project(snapshot, Some(name), &resolutions, &sheets);
        let shared = run_with_project_memoised(
            snapshot,
            Some(name),
            &resolutions,
            &sheets,
            &[],
            &ErcConfig::default(),
            &mut memo,
        );
        clashes += plain.len();
        assert_eq!(render(&plain), render(&shared), "sheet {name:?}");
    }
    assert!(clashes > 0, "fixture must exercise a real clash");
}
