//! #2462 — the command-line form of the report: ids that hold from read to
//! read, the answer's sides, fixes and blocked reasons, the match summary,
//! and the Apply choice that skips every row it does not name.

use super::super::fixture::*;
use super::*;
use crate::pcb::eco::{BackChange, BackRow, EcoChange, EcoGroup, EcoRow};

const REVISION: u64 = 7;

fn eco_list(rows: Vec<EcoRow>) -> EcoChangeList {
    EcoChangeList {
        revision: REVISION,
        rows,
        notes: Vec::new(),
    }
}

fn back_list(rows: Vec<BackRow>) -> BackChangeList {
    BackChangeList {
        revision: REVISION,
        rows,
        notes: Vec::new(),
        designators: Vec::new(),
    }
}

/// R4 is 10k on the sheet and 12k on the board.
fn value_r4() -> EcoRow {
    EcoRow::new(
        EcoKey::SetValue(u(0xa5)),
        EcoGroup::Values,
        EcoChange::SetValue {
            footprint: u(0xa5),
            reference: "R4".into(),
            from: "12k".into(),
            to: "10k".into(),
        },
        true,
    )
}

fn back_value_r4() -> BackRow {
    BackRow {
        key: BackKey::Value(u(0xa5)),
        group: BackGroup::Values,
        change: BackChange::Value {
            footprint: u(0xa5),
            sheet: root(),
            units: vec![u(0x104)],
            reference: "R4".into(),
            from: "10k".into(),
            to: "12k".into(),
        },
        on: true,
        blocked: None,
    }
}

/// TP3 is on the board only.
fn remove_tp3() -> EcoRow {
    EcoRow::new(
        EcoKey::RemoveFootprint(u(0xa4)),
        EcoGroup::RemoveFootprints,
        EcoChange::RemoveFootprint {
            footprint: u(0xa4),
            reference: "TP3".into(),
        },
        true,
    )
}

fn sample() -> CompareReport {
    compare(
        &eco_list(vec![value_r4(), remove_tp3()]),
        &back_list(vec![back_value_r4()]),
    )
}

fn token_of(report: &CompareReport, key: &EcoKey) -> String {
    report
        .rows
        .iter()
        .find(|row| row.pcb.as_ref() == Some(key))
        .map(CompareRow::token)
        .unwrap_or_else(|| panic!("no row for {key:?}"))
}

#[test]
fn a_row_id_is_sixteen_hex_digits_and_the_same_in_every_read() {
    let first = sample();
    let again = sample();
    let value = token_of(&first, &EcoKey::SetValue(u(0xa5)));
    assert_eq!(value.len(), 16);
    assert!(value.chars().all(|c| c.is_ascii_hexdigit()), "{value}");
    assert_eq!(value, token_of(&again, &EcoKey::SetValue(u(0xa5))));
    assert_ne!(value, token_of(&first, &EcoKey::RemoveFootprint(u(0xa4))));
    assert_eq!(
        first.row_by_token(&value).map(|row| row.kind),
        Some(CompareKind::ValueChange)
    );
    assert!(first.row_by_token("0000000000000000").is_none());
}

#[test]
fn the_answer_lists_each_difference_with_both_sides_and_its_fixes() {
    let report = sample();
    let answer = report.answer();
    assert_eq!(answer["revision"], REVISION);
    assert_eq!(answer["matches"], false);
    assert_eq!(answer["summary"], "2 differences");
    assert_eq!(answer["count"], 2);
    assert_eq!(answer["kinds"]["value_change"], 1);
    assert_eq!(answer["kinds"]["extra_on_board"], 1);
    assert_eq!(answer["kinds"]["net_difference"], 0);
    let rows = answer["differences"].as_array().expect("differences");
    let value = rows
        .iter()
        .find(|row| row["kind"] == "value_change")
        .expect("the value row");
    assert_eq!(value["object"]["designator"], "R4");
    assert_eq!(value["schematic"]["text"], "10k");
    assert_eq!(value["board"]["text"], "12k");
    assert_eq!(value["default"], "update_pcb");
    assert_eq!(value["board_item"], u(0xa5).to_string());
    let fixes = value["fixes"].as_array().expect("fixes");
    assert_eq!(fixes.len(), 2, "{value}");
    assert_eq!(fixes[0]["direction"], "update_pcb");
    assert_eq!(fixes[0]["action"], "set_value");
    assert_eq!(fixes[1]["direction"], "update_schematic");
    assert_eq!(fixes[1]["action"], "take_board_value");
    assert_eq!(value["blocked"].as_array().map(Vec::len), Some(0));
}

#[test]
fn a_board_only_part_says_why_it_cannot_go_to_the_schematic() {
    let answer = sample().answer();
    let extra = answer["differences"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["kind"] == "extra_on_board")
        .cloned()
        .expect("the extra row");
    assert_eq!(extra["default"], "skip", "a board deletion starts skipped");
    assert_eq!(extra["removes_from_board"], true);
    assert_eq!(extra["schematic"], serde_json::Value::Null);
    assert_eq!(extra["blocked"][0]["direction"], "update_schematic");
    assert_eq!(
        extra["blocked"][0]["reason"],
        "taking this difference into the schematic is not built yet"
    );
}

#[test]
fn an_empty_report_says_the_board_matches() {
    let answer = compare(&eco_list(Vec::new()), &back_list(Vec::new())).answer();
    assert_eq!(answer["matches"], true);
    assert_eq!(answer["summary"], "Board matches schematic");
    assert_eq!(answer["differences"].as_array().map(Vec::len), Some(0));
}

#[test]
fn choose_skips_every_row_it_does_not_name() {
    let mut report = sample();
    let value = token_of(&report, &EcoKey::SetValue(u(0xa5)));
    report
        .choose(&[(value.clone(), Direction::UpdateSchematic)])
        .expect("allowed");
    let counts = report.counts();
    assert_eq!(
        (counts.to_pcb, counts.to_schematic, counts.skipped),
        (0, 1, 1)
    );
    let (eco, back) = report.ticks();
    assert!(
        eco.rows.iter().all(|row| !row.on),
        "nothing goes to the board"
    );
    assert!(back.rows.iter().all(BackRow::applies));
    // Naming the deletion too takes it to the board.
    let tp3 = token_of(&report, &EcoKey::RemoveFootprint(u(0xa4)));
    report
        .choose(&[(tp3, Direction::UpdatePcb), (value, Direction::Skip)])
        .expect("allowed");
    let counts = report.counts();
    assert_eq!(
        (counts.to_pcb, counts.to_schematic, counts.skipped),
        (1, 0, 1)
    );
    assert_eq!(report.board_deletions().len(), 1);
}

#[test]
fn choose_refuses_an_unknown_a_repeated_or_a_blocked_row_and_changes_nothing() {
    let mut report = sample();
    let before = report.clone();
    let value = token_of(&report, &EcoKey::SetValue(u(0xa5)));
    let tp3 = token_of(&report, &EcoKey::RemoveFootprint(u(0xa4)));
    assert_eq!(
        report.choose(&[("feedfacefeedface".into(), Direction::UpdatePcb)]),
        Err(ChooseRefusal::UnknownRow("feedfacefeedface".into()))
    );
    assert_eq!(
        report.choose(&[
            (value.clone(), Direction::UpdatePcb),
            (value.clone(), Direction::Skip)
        ]),
        Err(ChooseRefusal::Twice(value.clone()))
    );
    let blocked = report
        .choose(&[(tp3.clone(), Direction::UpdateSchematic)])
        .expect_err("not built yet");
    assert_eq!(
        blocked,
        ChooseRefusal::NotAllowed {
            row: tp3,
            direction: Direction::UpdateSchematic,
            why: Unavailable::SchematicLater,
        }
    );
    assert!(blocked.to_string().contains("not built yet"), "{blocked}");
    assert_eq!(report, before, "a refused choice changes no direction");
}

/// #3059 (mockup row 162): a Missing on board row for a footprint that is
/// on the board but not linked shows the missing link on the board side,
/// not the designator again, and says Update PCB links it to its part, in
/// the dialog's data, the command-line answer and the report text.
#[test]
fn a_link_row_shows_the_missing_link() {
    let link = EcoRow::new(
        EcoKey::Link(u(0xa6)),
        EcoGroup::Links,
        EcoChange::Link {
            footprint: u(0xa6),
            reference: "R3".into(),
            link: signex_types::pcb::SymbolLink::new(root(), [u(0x103)]),
        },
        true,
    );
    let report = compare(&eco_list(vec![link]), &back_list(Vec::new()));
    let row = &report.rows[0];
    assert_eq!(row.kind, CompareKind::MissingOnBoard);
    assert_eq!(row.schematic, Some(CompareValue::Text("R3".into())));
    assert_eq!(row.board, Some(CompareValue::Unlinked));
    assert_eq!(row.jump.footprint, Some(u(0xa6)), "the footprint is there");
    assert_eq!(row.direction, Direction::UpdatePcb);

    let answer = report.answer();
    let rows = answer["differences"].as_array().expect("differences");
    assert_eq!(rows[0]["kind"], "missing_on_board");
    assert_eq!(rows[0]["schematic"]["text"], "R3");
    assert_eq!(rows[0]["board"], serde_json::json!({ "unlinked": true }));
    assert_eq!(rows[0]["fixes"][0]["action"], "link_footprint");

    let text = report.report_text();
    assert!(
        text.contains("  R3: schematic R3, board (not linked) -> Update PCB: Link to its part\n"),
        "{text}"
    );
}
