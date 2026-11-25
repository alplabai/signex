//! Rail budgets (#2943): the rails, their sources and loads from the parts'
//! parameters, and what keeps a budget from being told.

use signex_bom::{BomRow, BomTable};

use super::*;

fn part(designator: &str, fitted: u32, parameters: &[(&str, &str)]) -> BomRow {
    BomRow {
        references: vec![designator.to_string()],
        qty: 1,
        fitted_qty: fitted,
        custom: parameters
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
        ..BomRow::default()
    }
}

fn table(rows: Vec<BomRow>) -> BomTable {
    BomTable {
        rows,
        ..BomTable::default()
    }
}

fn spec(voltage: f64, tolerance_percent: Option<f64>) -> RailSpec {
    RailSpec {
        voltage,
        tolerance_percent,
    }
}

#[test]
fn a_current_reads_in_amps_with_or_without_a_unit() {
    assert_eq!(parse_current_a("0.25"), Some(0.25));
    assert_eq!(parse_current_a(" 1.5 A "), Some(1.5));
    assert_eq!(parse_current_a("250mA"), Some(0.25));
    assert_eq!(parse_current_a("500 uA"), Some(0.0005));
    assert_eq!(parse_current_a("500µA"), Some(0.0005));
    for bad in ["", "a lot", "-1", "NaN", "inf", "1 kA"] {
        assert_eq!(parse_current_a(bad), None, "{bad}");
    }
}

#[test]
fn every_rail_listed_or_named_gets_its_source_and_fitted_loads() {
    let rails = BTreeMap::from([("+3V3".to_string(), spec(3.3, Some(5.0)))]);
    let parts = table(vec![
        part(
            "U1",
            1,
            &[("Rail Supply +3V3", "1A"), ("Rail Load +5V", "1.2")],
        ),
        part("U2", 1, &[("Rail Load +3V3", "250mA")]),
        part("U3", 1, &[("Rail Load +3V3", "0.1"), ("MPN", "X")]),
        part("U4", 0, &[("Rail Load +3V3", "5")]),
        part("U5", 1, &[("Rail Load +5V", "lots")]),
    ]);
    let budgets = rail_budgets(&rails, &parts);
    let names: Vec<&str> = budgets.iter().map(|b| b.net.as_str()).collect();
    assert_eq!(names, ["+3V3", "+5V"]);
    let rail = &budgets[0];
    assert_eq!(rail.supply_a(), Some(1.0));
    assert!(
        (rail.total_a() - 0.35).abs() < 1e-12,
        "a part not fitted draws nothing"
    );
    assert!(rail.missing_data().is_empty(), "{:?}", rail.missing_data());
    let answer = rail.to_json();
    assert_eq!(
        answer["source"],
        json!({"designator": "U1", "current_a": 1.0})
    );
    assert!((answer["headroom_a"].as_f64().unwrap() - 0.65).abs() < 1e-12);

    let five = &budgets[1];
    let missing = five.missing_data();
    assert_eq!(missing.len(), 3, "{missing:?}");
    assert!(missing[0].contains("no voltage"), "{missing:?}");
    assert!(missing[1].contains("no source"), "{missing:?}");
    assert!(missing[2].contains("`lots`"), "{missing:?}");
    assert_eq!(five.to_json()["source"], Value::Null);
}

#[test]
fn two_sources_or_none_leave_the_supply_untold() {
    let parts = table(vec![
        part("U1", 1, &[("Rail Supply VBAT", "1")]),
        part("U2", 1, &[("Rail Supply VBAT", "2")]),
        part("U3", 1, &[("Rail Load VBAT", "0.5")]),
    ]);
    let rails = BTreeMap::from([("VBAT".to_string(), spec(3.7, None))]);
    let budget = &rail_budgets(&rails, &parts)[0];
    assert_eq!(budget.supply_a(), None);
    assert!(budget.missing_data()[0].contains("2 sources"));
}

fn wanted(net: &str) -> RailWanted<'_> {
    RailWanted {
        net,
        voltage: None,
        tolerance_percent: None,
        max_current_a: None,
        margin_percent: None,
    }
}

#[test]
fn a_rail_passes_within_its_supply_less_the_margin_and_its_voltage() {
    let rails = Ok(BTreeMap::from([("+3V3".to_string(), spec(3.3, Some(2.0)))]));
    let parts = Ok(table(vec![
        part("U1", 1, &[("Rail Supply +3V3", "1")]),
        part("U2", 1, &[("Rail Load +3V3", "0.85")]),
    ]));
    let pass = RailWanted {
        voltage: Some(3.3),
        tolerance_percent: Some(5.0),
        margin_percent: Some(10.0),
        ..wanted("+3V*")
    };
    assert!(rail_budget(&rails, Some(&parts), &pass).is_empty());

    let tight = RailWanted {
        margin_percent: Some(20.0),
        ..wanted("+3V3")
    };
    let misses = rail_budget(&rails, Some(&parts), &tight);
    assert_eq!(misses.len(), 1);
    let evidence = &misses[0].evidence;
    assert!((evidence["limit_a"].as_f64().unwrap() - 0.8).abs() < 1e-12);
    assert_eq!(evidence["rail"]["loads"][0]["designator"], "U2");
    assert_eq!(evidence["rail"]["source"]["designator"], "U1");

    let capped = RailWanted {
        max_current_a: Some(0.5),
        ..wanted("+3V3")
    };
    assert_eq!(rail_budget(&rails, Some(&parts), &capped).len(), 1);

    let other_voltage = RailWanted {
        voltage: Some(5.0),
        ..wanted("+3V3")
    };
    let misses = rail_budget(&rails, Some(&parts), &other_voltage);
    assert!(
        misses[0].message.contains("not 5 V"),
        "{}",
        misses[0].message
    );
    let too_loose = RailWanted {
        voltage: Some(3.3),
        tolerance_percent: Some(1.0),
        ..wanted("+3V3")
    };
    assert_eq!(rail_budget(&rails, Some(&parts), &too_loose).len(), 1);
}

#[test]
fn missing_data_and_an_unreadable_ask_are_findings_never_a_pass() {
    let parts = Ok(table(vec![part("U2", 1, &[("Rail Load +3V3", "0.1")])]));
    let none = Ok(BTreeMap::new());
    let misses = rail_budget(&none, Some(&parts), &wanted("+3V3"));
    assert_eq!(misses.len(), 2, "no voltage and no source");
    assert_eq!(rail_budget(&none, Some(&parts), &wanted("VBUS")).len(), 1);
    let unread = Err("bad".to_string());
    assert_eq!(rail_budget(&unread, Some(&parts), &wanted("*")).len(), 1);
    assert_eq!(rail_budget(&none, None, &wanted("*")).len(), 1);
    let ask = RailWanted {
        tolerance_percent: Some(5.0),
        margin_percent: Some(100.0),
        ..wanted(" ")
    };
    assert_eq!(rail_budget(&none, Some(&parts), &ask).len(), 3);
}
