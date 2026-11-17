use serde_json::json;
use signex_types::pcb::PcbBoard;

use super::*;

/// A pad on net `net` (0: none).
fn pad(number: &str, net: u32, name: &str) -> serde_json::Value {
    let net = if net == 0 {
        serde_json::Value::Null
    } else {
        json!({ "number": net, "name": name })
    };
    json!({
        "uuid": uuid::Uuid::now_v7(),
        "number": number,
        "pad_type": "smd",
        "shape": "rect",
        "position": { "x": 0.0, "y": 0.0 },
        "size": { "x": 1.0, "y": 1.0 },
        "drill": null,
        "net": net,
    })
}

fn footprint(
    reference: &str,
    value: &str,
    footprint: &str,
    at: (f64, f64),
    layer: &str,
    pads: Vec<serde_json::Value>,
) -> serde_json::Value {
    json!({
        "uuid": uuid::Uuid::now_v7(),
        "reference": reference,
        "value": value,
        "footprint_id": footprint,
        "position": { "x": at.0, "y": at.1 },
        "rotation": 90.0,
        "layer": layer,
        "pads": pads,
    })
}

/// R1, R2 and R10 (10k, one footprint), C1 on the bottom, a mechanical
/// standoff MH1 left out of the BOM, and a jumper JP1 kept in it. Two nets:
/// VCC with two tracks (3 mm and 4 mm) and a via, GND with none.
pub(crate) fn fixture() -> PcbBoard {
    let mut standoff = footprint("MH1", "M3", "Hole:M3", (5.0, 5.0), "TopCopper", vec![]);
    standoff["component"] = json!({ "type": "mechanical" });
    let mut jumper = footprint("JP1", "0R", "Jumper:SMD", (6.0, 6.0), "TopCopper", vec![]);
    jumper["component"] = json!({ "type": "jumper" });
    let track = |x0: f64, y0: f64, x1: f64, y1: f64| {
        json!({
            "uuid": uuid::Uuid::now_v7(),
            "start": { "x": x0, "y": y0 },
            "end": { "x": x1, "y": y1 },
            "width": 0.25,
            "layer": "TopCopper",
            "net": 1,
        })
    };
    serde_json::from_value(json!({
        "uuid": uuid::Uuid::nil(),
        "outline": [
            { "x": 0.0, "y": 0.0 },
            { "x": 50.0, "y": 0.0 },
            { "x": 50.0, "y": 30.0 },
            { "x": 0.0, "y": 30.0 },
        ],
        "nets": [
            { "number": 1, "name": "VCC" },
            { "number": 2, "name": "GND" },
        ],
        "footprints": [
            footprint("R10", "10k", "R_0603", (30.0, 10.0), "TopCopper",
                vec![pad("1", 1, "VCC"), pad("2", 2, "GND")]),
            footprint("R2", "10k", "R_0603", (20.0, 10.0), "TopCopper",
                vec![pad("1", 1, "VCC"), pad("2", 2, "GND")]),
            footprint("R1", "10k", "R_0603", (10.0, 10.0), "TopCopper",
                vec![pad("1", 1, "VCC"), pad("2", 0, "")]),
            footprint("C1", "100n", "C_0402", (12.5, 20.0), "BottomCopper",
                vec![pad("1", 1, "VCC"), pad("2", 2, "GND")]),
            standoff,
            jumper,
        ],
        "segments": [track(0.0, 0.0, 3.0, 0.0), track(3.0, 0.0, 3.0, 4.0)],
        "vias": [{
            "uuid": uuid::Uuid::now_v7(),
            "position": { "x": 3.0, "y": 4.0 },
            "diameter": 0.6,
            "drill": 0.3,
            "net": 1,
        }],
        "options": { "origin": { "at": { "x": 0.0, "y": 30.0 } } },
    }))
    .expect("the fixture board")
}

fn texts(row: &[ReportCell]) -> Vec<String> {
    row.iter().map(ReportCell::csv_text).collect()
}

#[test]
fn the_bom_groups_by_value_and_footprint_and_leaves_out_no_bom_types() {
    let report = board_report(&fixture(), BoardReportKind::Bom);
    assert_eq!(report.columns, BoardReportKind::Bom.columns());
    let rows: Vec<Vec<String>> = report.rows.iter().map(|r| texts(r)).collect();
    assert_eq!(
        rows,
        [
            ["C1", "1", "100n", "C_0402", "", "standard"],
            ["JP1", "1", "0R", "Jumper:SMD", "", "jumper"],
            ["R1, R2, R10", "3", "10k", "R_0603", "", "standard"],
        ],
        "MH1 is mechanical, so it is not listed"
    );
}

#[test]
fn the_netlist_lists_each_net_and_its_pads() {
    let report = board_report(&fixture(), BoardReportKind::Netlist);
    let rows: Vec<Vec<String>> = report.rows.iter().map(|r| texts(r)).collect();
    assert_eq!(
        rows,
        [
            vec!["GND", "3", "C1.2 R2.2 R10.2"],
            vec!["VCC", "4", "C1.1 R1.1 R2.1 R10.1"],
        ]
    );
}

#[test]
fn the_net_length_report_sums_the_tracks_and_counts_the_vias() {
    let report = board_report(&fixture(), BoardReportKind::NetLength);
    assert_eq!(
        report.rows,
        [
            vec![
                ReportCell::Text("GND".into()),
                ReportCell::Millimetres(0.0),
                ReportCell::Count(0),
                ReportCell::Count(0),
            ],
            vec![
                ReportCell::Text("VCC".into()),
                ReportCell::Millimetres(7.0),
                ReportCell::Count(2),
                ReportCell::Count(1),
            ],
        ]
    );
}

#[test]
fn the_component_report_places_each_part_from_the_origin_y_up() {
    let report = board_report(&fixture(), BoardReportKind::Components);
    let rows: Vec<Vec<String>> = report.rows.iter().map(|r| texts(r)).collect();
    assert_eq!(rows.len(), 6);
    assert_eq!(
        rows[0],
        ["C1", "C_0402", "100n", "12.5", "10", "90", "bottom"]
    );
    let r10 = rows.iter().find(|r| r[0] == "R10").expect("R10");
    assert_eq!(r10, &["R10", "R_0603", "10k", "30", "20", "90", "top"]);
    assert_eq!(
        rows.iter().map(|r| r[0].as_str()).collect::<Vec<_>>(),
        ["C1", "JP1", "MH1", "R1", "R2", "R10"]
    );
}

#[test]
fn csv_has_a_header_of_column_ids_and_one_line_a_row() {
    let report = board_report(&fixture(), BoardReportKind::Bom);
    let csv = String::from_utf8(report.to_csv()).expect("utf-8");
    let csv = csv.trim_start_matches('\u{feff}');
    let lines: Vec<&str> = csv.split("\r\n").filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines[0],
        "designators,quantity,value,footprint,description,type"
    );
    assert_eq!(lines[3], "\"R1, R2, R10\",3,10k,R_0603,,standard");
    assert_eq!(lines.len(), 1 + report.rows.len());
}

#[test]
fn json_carries_numbers_as_numbers_and_ids_as_text() {
    let report = board_report(&fixture(), BoardReportKind::NetLength);
    let value = serde_json::to_value(&report).expect("json");
    assert_eq!(value["kind"], "net_length");
    assert_eq!(value["columns"][1], "length_mm");
    assert_eq!(value["rows"][1][1], 7.0);
    assert_eq!(value["rows"][1][3], 1);
    for column in BoardReportKind::ALL.iter().flat_map(|k| k.columns()) {
        let json = serde_json::to_value(column).expect("json");
        assert_eq!(json, column.id(), "{column:?}");
    }
    assert_eq!(
        serde_json::to_value(ReportCell::Missing).expect("json"),
        serde_json::Value::Null
    );
}

#[test]
fn every_kind_round_trips_through_its_id() {
    for kind in BoardReportKind::ALL {
        assert_eq!(BoardReportKind::from_id(kind.id()), Some(kind));
    }
    assert_eq!(BoardReportKind::from_id("gerber"), None);
}
