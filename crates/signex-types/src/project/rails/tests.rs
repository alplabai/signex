//! `rails` in the `.snxprj` (#2943): absent by default, kept in the form it
//! was written, edited one rail at a time, and a value this build cannot
//! read kept as written and not edited over.

use serde_json::json;

use super::super::{encode_project, parse_project_bytes};
use super::*;

fn project(rails: Option<Value>) -> ProjectData {
    let mut file = json!({"name": "P", "dir": "", "schematic_root": null, "pcb_file": null});
    if let Some(rails) = rails {
        file["rails"] = rails;
    }
    let bytes = serde_json::to_vec(&file).unwrap();
    parse_project_bytes(std::path::Path::new("P.snxprj"), &bytes, |_| false).unwrap()
}

fn saved(data: &ProjectData) -> Value {
    serde_json::from_slice(&encode_project(data).unwrap()).unwrap()
}

fn edit(voltage: Option<&str>, tolerance: Option<&str>) -> RailEdit {
    RailEdit {
        voltage: voltage.map(str::to_string),
        tolerance_percent: tolerance.map(str::to_string),
        remove: false,
    }
}

#[test]
fn a_project_without_rails_saves_no_key_and_reads_none() {
    let data = project(None);
    assert_eq!(data.rails(), Ok(BTreeMap::new()));
    assert!(saved(&data).get(RAILS_KEY).is_none());
}

#[test]
fn a_rail_is_set_changed_and_removed_and_the_others_keep_their_form() {
    let mut data = project(Some(json!({"+5V": {"voltage": 5}})));
    data.set_rail(" +3V3 ", &edit(Some("3.3"), Some("5")))
        .unwrap();
    assert_eq!(
        saved(&data)[RAILS_KEY],
        json!({"+3V3": {"voltage": 3.3, "tolerance_percent": 5}, "+5V": {"voltage": 5}})
    );
    data.set_rail("+3V3", &edit(None, Some(""))).unwrap();
    assert_eq!(data.rails().unwrap()["+3V3"].tolerance_percent, None);
    let remove = RailEdit {
        remove: true,
        ..RailEdit::default()
    };
    data.set_rail("+3V3", &remove).unwrap();
    data.set_rail("+5V", &remove).unwrap();
    assert!(
        saved(&data).get(RAILS_KEY).is_none(),
        "the last rail removes the key"
    );
}

#[test]
fn a_refused_rail_edit_changes_nothing_and_says_why() {
    let mut data = project(Some(json!({"+5V": {"voltage": 5}})));
    let before = data.rails_value();
    let remove = RailEdit {
        remove: true,
        ..RailEdit::default()
    };
    for (net, asked, problem) in [
        ("", edit(Some("1"), None), RailProblem::NetEmpty),
        (
            "+5V",
            edit(Some("0"), None),
            RailProblem::NotAVoltage("0".into()),
        ),
        (
            "+5V",
            edit(Some("five"), None),
            RailProblem::NotAVoltage("five".into()),
        ),
        (
            "+5V",
            edit(None, Some("100")),
            RailProblem::NotAPercent("100".into()),
        ),
        (
            "+5V",
            edit(None, Some("-1")),
            RailProblem::NotAPercent("-1".into()),
        ),
        (
            "+3V3",
            edit(None, Some("5")),
            RailProblem::NoVoltage("+3V3".into()),
        ),
        ("+3V3", remove.clone(), RailProblem::NoRail("+3V3".into())),
        ("+5V", edit(None, None), RailProblem::NothingToSet),
    ] {
        assert_eq!(data.set_rail(net, &asked), Err(problem), "{net} {asked:?}");
    }
    let both = RailEdit {
        voltage: Some("5".into()),
        ..remove
    };
    assert_eq!(
        data.set_rail("+5V", &both),
        Err(RailProblem::RemoveWithValues)
    );
    assert_eq!(data.rails_value(), before);
}

#[test]
fn a_negative_rail_reads_its_band_low_first() {
    let data = project(Some(
        json!({"-12V": {"voltage": -12, "tolerance_percent": 10}}),
    ));
    let rail = data.rails().unwrap()["-12V"];
    let (low, high) = rail.band();
    assert!(
        (low + 13.2).abs() < 1e-9 && (high + 10.8).abs() < 1e-9,
        "{low} {high}"
    );
}

#[test]
fn rails_this_build_cannot_read_are_kept_as_written_and_not_edited() {
    for raw in [
        json!(["+5V"]),
        json!({"+5V": {"voltage": 0}}),
        json!({"+5V": {"voltage": 5, "ripple_mv": 50}}),
        json!({"+5V": {"voltage": 5, "tolerance_percent": 150}}),
    ] {
        let mut data = project(Some(raw.clone()));
        assert!(data.rails().is_err(), "{raw}");
        assert!(matches!(
            data.set_rail("+5V", &edit(Some("5"), None)),
            Err(RailProblem::Unreadable(_))
        ));
        assert_eq!(saved(&data)[RAILS_KEY], raw);
    }
}
