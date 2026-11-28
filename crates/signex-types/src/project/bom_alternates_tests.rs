//! #2938: a BOM line's alternates are kept in order, refused edits change
//! nothing, and the list round-trips in the project file, skipped when empty.

use super::*;

const LINE: &str = "019df45d-af5e-7a00-8c3b-dd3bf89b40ae/019df45d-af5e-7a00-8c3b-000000000001";
const ALT_A: &str = "019df45d-af5e-7a00-8c3b-dd3bf89b40ae/019df45d-af5e-7a00-8c3b-00000000000a";
const ALT_B: &str = "019df45d-af5e-7a00-8c3b-dd3bf89b40ae/019df45d-af5e-7a00-8c3b-00000000000b";

fn project() -> ProjectData {
    serde_json::from_str(r#"{"name":"P","dir":"","schematic_root":null,"pcb_file":null}"#)
        .expect("a project")
}

#[test]
fn alternates_are_added_in_order_moved_and_removed() {
    let mut data = project();
    data.add_bom_alternate(LINE, ALT_A).unwrap();
    data.add_bom_alternate(LINE, ALT_B).unwrap();
    assert_eq!(data.bom_alternates_of(LINE), [ALT_A, ALT_B]);
    data.move_bom_alternate(LINE, ALT_B, 1).unwrap();
    assert_eq!(data.bom_alternates_of(LINE), [ALT_B, ALT_A]);
    data.remove_bom_alternate(LINE, ALT_B).unwrap();
    assert_eq!(data.bom_alternates_of(LINE), [ALT_A]);
    data.remove_bom_alternate(LINE, ALT_A).unwrap();
    assert!(data.bom_alternates.is_empty(), "an emptied line is dropped");
}

#[test]
fn an_mpn_line_is_keyed_upper_cased() {
    let mut data = project();
    data.add_bom_alternate(" mpn: rc0603fr-0710kl ", ALT_A)
        .unwrap();
    assert_eq!(data.bom_alternates[0].line, "mpn:RC0603FR-0710KL");
    assert_eq!(data.bom_alternates_of("mpn:RC0603FR-0710KL"), [ALT_A]);
}

#[test]
fn a_refused_edit_changes_nothing_and_says_why() {
    let mut data = project();
    data.add_bom_alternate(LINE, ALT_A).unwrap();
    let before = data.bom_alternates.clone();
    let cases = [
        data.clone().add_bom_alternate(LINE, ALT_A),
        data.clone().add_bom_alternate(LINE, LINE),
        data.clone().add_bom_alternate("R1", ALT_B),
        data.clone().add_bom_alternate(LINE, "not-a-row"),
        data.clone().remove_bom_alternate(LINE, ALT_B),
        data.clone().move_bom_alternate(LINE, ALT_A, 2),
        data.clone().move_bom_alternate(LINE, ALT_A, 0),
    ];
    assert!(matches!(
        cases[0],
        Err(BomAlternateProblem::AlreadyListed { .. })
    ));
    assert!(matches!(cases[1], Err(BomAlternateProblem::OwnPart { .. })));
    assert!(matches!(cases[2], Err(BomAlternateProblem::BadLine(_))));
    assert!(matches!(cases[3], Err(BomAlternateProblem::BadRow(_))));
    assert!(matches!(
        cases[4],
        Err(BomAlternateProblem::NotListed { .. })
    ));
    assert!(matches!(
        cases[5],
        Err(BomAlternateProblem::NoPosition { count: 1, .. })
    ));
    assert!(matches!(
        cases[6],
        Err(BomAlternateProblem::NoPosition { .. })
    ));
    assert_eq!(data.bom_alternates, before);
}

#[test]
fn alternates_round_trip_in_the_project_file_and_are_skipped_when_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("P.snxprj");
    let mut data = project();
    write_project(&path, &data).unwrap();
    let raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(raw.get("bom_alternates").is_none(), "skipped when empty");

    data.add_bom_alternate(LINE, ALT_A).unwrap();
    data.add_bom_alternate(LINE, ALT_B).unwrap();
    write_project(&path, &data).unwrap();
    let raw: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        raw["bom_alternates"],
        serde_json::json!([{"line": LINE, "rows": [ALT_A, ALT_B]}])
    );
    assert_eq!(raw["format"], SNXPRJ_FORMAT_V1, "no format token change");
    let back = parse_project(&path).unwrap();
    assert_eq!(back.bom_alternates, data.bom_alternates);
}

/// A key a newer build wrote on a line is kept through a load and a save.
#[test]
fn an_unknown_key_on_a_line_survives_a_round_trip() {
    let text = format!(
        r#"{{"format":"{SNXPRJ_FORMAT_V1}","name":"P","dir":"","schematic_root":null,"pcb_file":null,
        "bom_alternates":[{{"line":"{LINE}","rows":["{ALT_A}"],"note":"from the buyer"}}]}}"#
    );
    let path = std::path::Path::new("P.snxprj");
    let data = parse_project_bytes(path, text.as_bytes(), |_| false).expect("the project opens");
    assert_eq!(data.bom_alternates_of(LINE), [ALT_A]);
    let written: serde_json::Value =
        serde_json::from_slice(&encode_project(&data).unwrap()).unwrap();
    assert_eq!(written["bom_alternates"][0]["note"], "from the buyer");
    assert_eq!(written["bom_alternates"][0]["rows"][0], ALT_A);
}

/// A `bom_alternates` value this build cannot read does not refuse the
/// project: it is kept as written for the next save, and edits of the
/// alternates are refused rather than write a second one over it.
#[test]
fn a_malformed_alternates_value_still_opens_the_project_and_is_kept() {
    let text = format!(
        r#"{{"format":"{SNXPRJ_FORMAT_V1}","name":"P","dir":"","schematic_root":null,"pcb_file":null,
        "bom_alternates":{{"not":"a list"}}}}"#
    );
    let path = std::path::Path::new("P.snxprj");
    let mut data =
        parse_project_bytes(path, text.as_bytes(), |_| false).expect("the project opens");
    assert!(data.bom_alternates.is_empty());
    assert!(data.has_unreadable_bom_alternates());
    assert_eq!(
        data.add_bom_alternate(LINE, ALT_A),
        Err(BomAlternateProblem::Unreadable)
    );
    let written: serde_json::Value =
        serde_json::from_slice(&encode_project(&data).unwrap()).unwrap();
    assert_eq!(
        written["bom_alternates"],
        serde_json::json!({"not": "a list"}),
        "kept as written"
    );
}
