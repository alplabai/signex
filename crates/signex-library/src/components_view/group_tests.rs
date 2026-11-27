use std::cmp::Ordering;

use super::test_rows::{interface_ic, interface_rows, m, row, text, text_order};
use super::*;
use crate::component::ComponentRow;
use crate::templates::{ColumnKey, SortKey};

fn col(name: &str) -> ColumnKey {
    ColumnKey::param(name)
}

fn sorted(rows: &[ComponentRow], keys: &[SortKey]) -> Vec<String> {
    let template = interface_ic();
    let cell =
        |i: usize, c: &ColumnKey| column_value(&rows[i], &RowFacts::default(), Some(&template), c);
    sorted_rows(rows.len(), keys, cell, text_order)
        .into_iter()
        .map(|i| rows[i].internal_pn.to_string())
        .collect()
}

#[test]
fn not_set_stays_last_in_both_directions() {
    let missing = CellValue::Missing;
    let one = CellValue::Number(1.0);
    for descending in [false, true] {
        assert_eq!(
            compare_cells(&missing, &one, descending, &text_order),
            Ordering::Greater
        );
        assert_eq!(
            compare_cells(&one, &missing, descending, &text_order),
            Ordering::Less
        );
        assert_eq!(
            compare_cells(&missing, &missing, descending, &text_order),
            Ordering::Equal
        );
    }
}

#[test]
fn measurements_sort_by_base_value_across_prefixes() {
    let rows = interface_rows();
    // 20 kbps < 2 Mbps < 5 Mbps < 20 Mbps.
    assert_eq!(
        sorted(&rows, &[SortKey::ascending(col("rate"))]),
        ["LIN7", "can200", "CAN100", "RS485A"]
    );
    assert_eq!(
        sorted(&rows, &[SortKey::descending(col("rate"))]),
        ["RS485A", "CAN100", "can200", "LIN7"]
    );
}

/// Supply voltage sorts by its minimum, Temperature by its maximum
/// (`sort_by = "max"`); a row without the pair is last.
#[test]
fn ranges_sort_by_their_slot_end() {
    let rows = interface_rows();
    assert_eq!(
        sorted(&rows, &[SortKey::ascending(col("supply"))]),
        ["RS485A", "CAN100", "LIN7", "can200"]
    );
    assert_eq!(
        sorted(&rows, &[SortKey::descending(col("temperature"))]),
        ["CAN100", "LIN7", "RS485A", "can200"]
    );
}

/// mockup row 52: Shift+click adds a nested sort; the second key orders
/// rows the first one ties.
#[test]
fn a_nested_sort_breaks_ties_with_the_next_key() {
    let rows = interface_rows();
    let by_package_then_rate = [
        SortKey::ascending(col("package")),
        SortKey::descending(col("rate")),
    ];
    assert_eq!(
        sorted(&rows, &by_package_then_rate),
        ["can200", "RS485A", "CAN100", "LIN7"]
    );
    // Without a deciding key, the part number decides.
    assert_eq!(
        sorted(&rows, &[SortKey::ascending(col("package"))]),
        ["can200", "RS485A", "CAN100", "LIN7"]
    );
    assert_eq!(sorted(&rows, &[]), ["CAN100", "can200", "LIN7", "RS485A"]);
}

#[test]
fn text_sorts_through_the_given_collation() {
    let a = CellValue::Text("b".into());
    let b = CellValue::Text("A".into());
    assert_eq!(compare_cells(&a, &b, false, &text_order), Ordering::Greater);
    assert_eq!(
        compare_cells(&a, &b, false, &|x: &str, y: &str| x.cmp(y)),
        Ordering::Greater
    );
    assert_eq!(compare_cells(&a, &b, true, &text_order), Ordering::Less);
}

#[test]
fn mixed_kinds_in_one_column_order_measurements_first() {
    let rows = [
        row("A", &[("v", text("n/a"))]),
        row("B", &[("v", m(1.0, "V"))]),
        row("C", &[("v", crate::param::ParamValue::Number(3.0))]),
        row("D", &[]),
    ];
    let cell = |i: usize, c: &ColumnKey| column_value(&rows[i], &RowFacts::default(), None, c);
    let order = sorted_rows(
        rows.len(),
        &[SortKey::ascending(col("v"))],
        cell,
        text_order,
    );
    assert_eq!(order, [1, 2, 0, 3]);
}

fn arranged(rows: &[ComponentRow], sort: &[SortKey], group: &[SortKey]) -> Vec<ViewNode> {
    let template = interface_ic();
    let cell =
        |i: usize, c: &ColumnKey| column_value(&rows[i], &RowFacts::default(), Some(&template), c);
    arrange_rows(rows.len(), sort, group, cell, text_order)
}

fn group(node: &ViewNode) -> &GroupNode {
    match node {
        ViewNode::Group(g) => g,
        ViewNode::Row(i) => panic!("row {i} where a group was expected"),
    }
}

/// The group bar nests: Protocol, then Package. Groups follow their
/// level's direction, text groups ignore case, and rows sort inside the
/// innermost group.
#[test]
fn nested_groups_hold_sorted_rows() {
    let rows = interface_rows();
    let nodes = arranged(
        &rows,
        &[SortKey::descending(col("rate"))],
        &[
            SortKey::ascending(col("protocol")),
            SortKey::ascending(col("package")),
        ],
    );
    let names: Vec<_> = nodes
        .iter()
        .map(|n| (group(n).value.clone(), group(n).count))
        .collect();
    assert_eq!(
        names,
        [
            (CellValue::Text("CAN FD".into()), 2),
            (CellValue::Text("LIN".into()), 1),
            (CellValue::Text("RS-485".into()), 1),
        ]
    );
    let can = group(&nodes[0]);
    let packages: Vec<_> = can
        .children
        .iter()
        .map(|n| group(n).value.clone())
        .collect();
    assert_eq!(
        packages,
        [
            CellValue::Text("DFN-8".into()),
            CellValue::Text("SOIC-8".into())
        ]
    );
    let soic = group(&can.children[1]);
    assert_eq!(soic.children, [ViewNode::Row(0)]);
    assert_eq!(soic.path.len(), 2);
    assert_eq!(soic.path[0].0, col("protocol"));
}

#[test]
fn a_descending_level_reverses_its_groups_and_not_set_stays_last() {
    let rows = interface_rows();
    let nodes = arranged(&rows, &[], &[SortKey::descending(col("supply"))]);
    let firsts: Vec<_> = nodes.iter().map(|n| group(n).count).collect();
    assert_eq!(firsts, [1, 1, 1, 1]);
    assert!(group(nodes.last().unwrap()).value.is_missing());
    let lows: Vec<f64> = nodes[..3]
        .iter()
        .map(|n| match &group(n).value {
            CellValue::Range { range, .. } => range.low().base(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(lows, [5.5, 4.5, 3.0]);
}

#[test]
fn without_group_levels_the_rows_come_sorted() {
    let rows = interface_rows();
    let nodes = arranged(&rows, &[SortKey::ascending(col("rate"))], &[]);
    assert_eq!(
        nodes,
        [
            ViewNode::Row(2),
            ViewNode::Row(3),
            ViewNode::Row(0),
            ViewNode::Row(1)
        ]
    );
}

#[test]
fn visible_lines_open_only_the_groups_asked_for() {
    let rows = interface_rows();
    let nodes = arranged(
        &rows,
        &[],
        &[
            SortKey::ascending(col("protocol")),
            SortKey::ascending(col("package")),
        ],
    );
    let closed = visible_lines(&nodes, &|_| false);
    assert_eq!(closed.len(), 3);
    let can_path = group(&nodes[0]).path.clone();
    let lines = visible_lines(&nodes, &|p| *p == can_path);
    // CAN FD open: its two package groups, closed, then LIN and RS-485.
    assert_eq!(lines.len(), 5);
    assert!(matches!(
        lines[0],
        ViewLine::Group {
            depth: 0,
            open: true,
            ..
        }
    ));
    assert!(matches!(
        lines[1],
        ViewLine::Group {
            depth: 1,
            open: false,
            ..
        }
    ));
    let all = group_paths(&nodes);
    assert_eq!(all.len(), 3 + 4);
    let every = visible_lines(&nodes, &|p| all.contains(p));
    let rows_shown = every
        .iter()
        .filter(|l| matches!(l, ViewLine::Row { depth: 2, .. }))
        .count();
    assert_eq!(rows_shown, 4);
}
