//! Measurements (mockup row 199): readings read from `#measurement`,
//! their verdict, the block rows, the timeline and saved readings.

use std::path::Path;

use crate::attachment::DesignValues;
use crate::bench::record;
use crate::measurement::{
    Limit, MeasurementProblemKind, Tolerance, Verdict, changed_saved_readings, check, error_text,
    quantities_of, quantity_text, readings, timeline,
};
use crate::report::measurements_report;
use crate::{BlockId, NotebookSources, Reference};

/// The block of mockup row 199: four readings of the output rail.
pub(crate) const MEAS_009: &str = r#"#measurement(
  id: "MEAS-009",
  title: [Output rail at 2 A],
  caption: [Bring-up results, rev B],
  target: (net("+3V3"), component("C15")),
  requirements: ("REQ-001", "REQ-002", "REQ-004"),
  conditions: "VIN 12.0 V, 2.0 A, 25 °C",
  operator: "j.berg",
  timestamp: "2026-11-24T15:12",
  rev: "B",
  board: "SN 0007",
)[
  #measurement(quantity: "+3V3 DC", value: 3.310, unit: "V", nominal: 3.3, tolerance: 2%, instrument: "LAB-0117")
  #measurement(
    target: net("+3V3"),
    quantity: "ripple",
    value: 18.0, unit: "mV",
    limit: (max: 30),
    instrument: "LAB-0042",
    attach: "scope/ripple-2A-revB.csv",
  )
  #measurement(quantity: "Efficiency", value: 91.2, unit: "%", limit: (min: 90))
  #measurement(quantity: "Load step 0.5→2 A", value: 64, unit: "mV", limit: (max: 100))
]
"#;

/// The two earlier ripple readings of the timeline in mockup row 199.
pub(crate) const EARLIER: &str = r#"#measurement(id: "MEAS-002", target: net("+3V3"), quantity: "ripple",
  value: 41.0, unit: "mV", limit: (max: 30), timestamp: "2026-10-02", rev: "A")
#measurement(id: "MEAS-005", target: net("+3V3"), quantity: "ripple",
  value: 27.0, unit: "mV", limit: (max: 30), timestamp: "2026-10-09", rev: "A rework",
  taken-with: ("L1": "3.3 µH"))
"#;

fn net(name: &str) -> Reference {
    Reference::Net(name.into())
}

#[test]
fn a_block_gives_one_reading_per_call_inside_it() {
    let (found, problems) = readings(MEAS_009);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(found.len(), 4, "the block itself holds no value");
    let id = BlockId::parse("MEAS-009");
    for (row, reading) in found.iter().enumerate() {
        assert_eq!(reading.id, id, "the block's id");
        assert_eq!(reading.row as usize, row + 1);
        assert_eq!(reading.operator, "j.berg", "inherited");
        assert_eq!(reading.board, "SN 0007");
        assert_eq!(reading.requirements, ["REQ-001", "REQ-002", "REQ-004"]);
        let stamp = reading.timestamp.map(|t| t.to_string());
        assert_eq!(stamp.as_deref(), Some("2026-11-24 15:12"));
    }
    let block = [net("+3V3"), Reference::Component("C15".into())];
    assert_eq!(found[0].targets, block, "the block's");
    assert_eq!(found[1].targets, [net("+3V3")], "its own");
    assert_eq!(found[0].instrument, "LAB-0117");
    assert_eq!(found[1].attach, "scope/ripple-2A-revB.csv");
    assert_eq!(found[0].value_text, "3.310", "kept as written");
    assert_eq!((found[0].line, found[1].line), (13, 14));
}

#[test]
fn each_row_of_the_mockup_block_passes_with_its_error() {
    let (found, _) = readings(MEAS_009);
    let shown: Vec<(Verdict, String)> = found
        .iter()
        .map(|r| {
            let check = r.check().expect("value and limit");
            (check.verdict, error_text(check.error.expect("an error")))
        })
        .collect();
    let t = '\u{2009}';
    assert_eq!(
        shown,
        [
            (Verdict::Pass, format!("+0.30{t}%")),
            (Verdict::Pass, format!("\u{2212}40{t}%")),
            (Verdict::Pass, format!("+1.3{t}%")),
            (Verdict::Pass, format!("\u{2212}36{t}%")),
        ]
    );
}

#[test]
fn inside_the_tolerance_but_near_the_limit_warns() {
    // VBUS 4.78 V against 5 V ±5 %: 0.6 % above the low limit (mockup row 60).
    let limit = Limit::Nominal {
        nominal: 5.0,
        tolerance: Tolerance::Percent(5.0),
    };
    assert_eq!(check(4.78, limit).verdict, Verdict::Warn);
    assert_eq!(check(4.9, limit).verdict, Verdict::Pass);
    assert_eq!(check(4.7, limit).verdict, Verdict::Fail);
    let over = check(310.0, Limit::Max { max: 250.0 });
    assert_eq!(over.verdict, Verdict::Fail);
    assert_eq!(error_text(over.error.unwrap()), "+24\u{2009}%");
}

#[test]
fn a_range_and_an_absolute_tolerance_are_checked_against_their_middle() {
    let range = Limit::Range { min: 2.9, max: 3.5 };
    let inside = check(3.2, range);
    assert_eq!(inside.verdict, Verdict::Pass);
    assert!(inside.error.unwrap().abs() < 1e-12, "the middle is 3.2");
    assert_eq!(check(3.6, range).verdict, Verdict::Fail);
    let absolute = Limit::Nominal {
        nominal: 3.3,
        tolerance: Tolerance::Absolute(0.1),
    };
    assert_eq!(check(3.35, absolute).verdict, Verdict::Pass);
    assert_eq!(check(3.45, absolute).verdict, Verdict::Fail);
    let zero = check(0.01, Limit::Max { max: 0.0 });
    let zero = (zero.error, zero.verdict);
    assert_eq!(zero, (None, Verdict::Fail), "no error against 0");
}

#[test]
fn the_target_and_tolerance_columns_read_as_drawn() {
    let t = '\u{2009}';
    let nominal = Limit::Nominal {
        nominal: 3.3,
        tolerance: Tolerance::Percent(2.0),
    };
    assert_eq!(nominal.target_text("V"), format!("3.3{t}V"));
    assert_eq!(nominal.tolerance_text("V"), format!("±2{t}%"));
    let (max, min) = (Limit::Max { max: 30.0 }, Limit::Min { min: 90.0 });
    assert_eq!(max.target_text("mV"), format!("< 30{t}mV"));
    assert_eq!(max.tolerance_text("mV"), "max");
    assert_eq!(min.target_text("%"), format!("≥ 90{t}%"));
    assert_eq!(min.tolerance_text("%"), "min");
    let range = Limit::Range { min: 2.9, max: 3.4 };
    assert_eq!(range.target_text("V"), format!("2.9–3.4{t}V"));
    assert_eq!(range.tolerance_text("V"), "range");
    let band = Limit::Nominal {
        nominal: 3.3,
        tolerance: Tolerance::Absolute(0.05),
    };
    assert_eq!(band.tolerance_text("V"), format!("±0.05{t}V"));
    assert_eq!(quantity_text("18.0", "mV"), format!("18.0{t}mV"));
    assert_eq!(quantity_text("7", ""), "7");
    assert_eq!(error_text(0.0), format!("0{t}%"));
}

#[test]
fn a_field_that_cannot_be_read_is_a_problem_on_its_line() {
    let text = "\n#measurement(id: \"DEC-001\", value: x * 2, limit: (max: \"a\"), timestamp: \"24/11\", supersedes: \"nope\")\n#measurement(nominal: 3.3)\n";
    let (found, problems) = readings(text);
    assert_eq!(found.len(), 2, "the readings are still listed");
    let kinds: Vec<&MeasurementProblemKind> = problems.iter().map(|p| &p.kind).collect();
    let lines: Vec<u32> = problems.iter().map(|p| p.line).collect();
    assert_eq!(lines, [2, 2, 2, 2, 2, 3]);
    let written = |text: &str| text.to_string();
    let expected = [
        MeasurementProblemKind::BadId {
            written: written("\"DEC-001\""),
        },
        MeasurementProblemKind::BadValue {
            written: written("x * 2"),
        },
        MeasurementProblemKind::BadTimestamp {
            written: written("\"24/11\""),
        },
        MeasurementProblemKind::BadSupersedes {
            written: written("\"nope\""),
        },
    ];
    for kind in &expected {
        assert!(kinds.contains(&kind), "{kind:?}: {kinds:?}");
    }
    let no_tolerance = matches!(kinds[5], MeasurementProblemKind::BadLimit { .. });
    assert!(no_tolerance, "no tolerance");
    assert_eq!(found[0].value, None);
    assert_eq!(found[0].limit, None);
}

#[test]
fn a_reading_in_code_and_a_negative_value_are_read() {
    let text = "#let r = measurement(value: -1.5, unit: \"V\", limit: (min: -2, max: -1))\n";
    let (found, problems) = readings(text);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(found[0].value, Some(-1.5));
    assert_eq!(found[0].value_text, "-1.5");
    let range = Limit::Range {
        min: -2.0,
        max: -1.0,
    };
    assert_eq!(found[0].limit, Some(range));
}

#[test]
fn a_part_timeline_holds_the_readings_of_its_pins() {
    let text = "#measurement(target: pin(\"U3\", \"3\"), value: 1, limit: (max: 2))\n\
                #measurement(target: pin(\"U3.4\"), value: 1, limit: (max: 2))\n\
                #measurement(target: component(\"U4\"), value: 1, limit: (max: 2))\n";
    let (found, _) = readings(text);
    let u3 = Reference::Component("U3".into());
    assert_eq!(found.iter().filter(|r| r.is_of(&u3)).count(), 2);
    let pin = Reference::Pin {
        component: "U3".into(),
        pin: "3".into(),
    };
    assert_eq!(found.iter().filter(|r| r.is_of(&pin)).count(), 1);
}

#[test]
fn the_timeline_of_a_net_lists_its_readings_newest_first_with_fail_and_stale() {
    let (mut found, _) = readings(EARLIER);
    found.extend(readings(MEAS_009).0);
    let values: DesignValues = [("L1", "4.7 µH")].into_iter().collect();
    let line = timeline(&found, &net("+3V3"), Some("ripple"), &[], &values);
    let ids: Vec<String> = line
        .entries
        .iter()
        .map(|e| e.id.clone().unwrap().to_string())
        .collect();
    assert_eq!(ids, ["MEAS-009", "MEAS-005", "MEAS-002"]);
    let counts = (line.readings, line.fails, line.stale);
    assert_eq!(counts, (3, 1, 1), "3 readings · 1 fail · 1 stale");
    assert_eq!(line.limit, Some(Limit::Max { max: 30.0 }));
    assert_eq!(line.unit, "mV");
    assert_eq!(line.entries[1].changes[0].was, "3.3 µH");
    assert_eq!(line.entries[2].verdict, Some(Verdict::Fail));
    assert_eq!(
        quantities_of(&found, &net("+3V3")),
        ["ripple", "+3V3 DC", "Efficiency", "Load step 0.5→2 A"]
    );
    let all = timeline(&found, &net("+3V3"), None, &[], &values);
    assert_eq!(all.readings, 6, "every quantity");
}

#[test]
fn a_correction_supersedes_and_the_csv_lists_the_timeline() {
    let text = "#measurement(id: \"MEAS-001\", target: net(\"A\"), value: 1, unit: \"V\", limit: (max: 2), timestamp: \"2026-01-01\")\n\
                #measurement(id: \"MEAS-002\", supersedes: \"MEAS-001\", target: net(\"A\"), value: 1.1, unit: \"V\", limit: (max: 2), timestamp: \"2026-01-02\", conditions: \"a, b\")\n\
                #measurement(target: net(\"A\"), value: 3, unit: \"V\")\n";
    let (found, _) = readings(text);
    let line = timeline(&found, &net("A"), None, &[], &DesignValues::new());
    assert_eq!(line.entries[1].superseded_by, BlockId::parse("MEAS-002"));
    assert_eq!(line.entries[2].timestamp, None, "undated last");
    assert_eq!(line.stale, 0, "the design is not read");
    let csv = line.to_csv();
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines[0], crate::measurement::CSV_HEADER);
    let newest = lines[1];
    let start = "MEAS-002,1,2026-01-02,,1.1,V,pass,";
    assert!(newest.starts_with(start), "{newest}");
    assert!(lines[1].contains(",\"a, b\","), "quoted: {}", lines[1]);
    assert_eq!(lines.len(), 4);
}

#[test]
fn a_saved_reading_may_not_change_but_may_move() {
    let saved = readings(EARLIER).0;
    let moved = readings(&format!("\n\n{EARLIER}")).0;
    let moved = changed_saved_readings(&saved, &moved);
    assert!(moved.is_empty(), "moving is no change");
    let edited = readings(&EARLIER.replace("27.0", "26.0")).0;
    let changed = changed_saved_readings(&saved, &edited);
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].id.to_string(), "MEAS-005");
    assert_eq!(changed[0].line, 3);
    let removed = readings("").0;
    assert_eq!(changed_saved_readings(&saved, &removed).len(), 2);
}

#[test]
fn the_record_reads_every_notebook_file_and_names_it() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../signex-types/tests/corpus/notebook/with-chapters");
    let (sources, _) =
        NotebookSources::load("design.snxnbk", |path| std::fs::read(root.join(path)));
    let found = record(&sources);
    assert_eq!(found.readings.len(), 1);
    let reading = &found.readings[0];
    assert_eq!(reading.file, "chapters/power.snxnbk");
    assert_eq!(reading.id, BlockId::parse("MEAS-010"));
    assert_eq!(reading.check().map(|c| c.verdict), Some(Verdict::Fail));
}

#[test]
fn the_measurements_answer_counts_what_the_status_strip_shows() {
    let mut sources = NotebookSources::new("design.snxnbk", format!("{EARLIER}{MEAS_009}"));
    sources.set_design_values([("L1", "4.7 µH")].into_iter().collect());
    let report = measurements_report(&sources);
    let s = report.summary;
    let counts = (s.readings, s.pass, s.warn, s.fail, s.stale_readings);
    assert_eq!(counts, (6, 5, 0, 1, 1));
    assert_eq!(s.missing_attachments, 0, "no #attachment names the CSV");
    let row = &report.readings[3];
    assert_eq!(row.quantity_label, "ripple");
    assert_eq!(row.measured_text, "18.0\u{2009}mV");
    assert_eq!(row.target_text, "< 30\u{2009}mV");
    let json = serde_json::to_value(&report).expect("json");
    assert_eq!(json["readings"][0]["id"], "MEAS-002");
    assert_eq!(json["readings"][0]["verdict"], "fail");
    assert_eq!(json["readings"][0]["limit"]["kind"], "max");
    assert_eq!(json["summary"]["fail"], 1);
}
