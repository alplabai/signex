use super::*;
use crate::{Command, Engine};
use signex_types::schematic::{
    InstancePath, Label, LabelType, Point, SchematicSheet, SelectedKind, Symbol, SymbolInstance,
};
use uuid::Uuid;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

/// `needle` as the case-insensitive substring Find matched before #2525.
fn q(needle: &str) -> FindQuery {
    FindQuery::new(needle, MatchOptions::SUBSTRING).unwrap()
}

/// `needle` under `options`; panics on a bad pattern.
fn with(needle: &str, options: MatchOptions) -> FindQuery {
    FindQuery::new(needle, options).unwrap()
}

fn placement(n: u128) -> InstancePath {
    InstancePath::from_chain(vec![id(100), id(n)])
}

fn sheet() -> SchematicSheet {
    let entry = |p: &InstancePath, r: &str| SymbolInstance {
        project: "board".into(),
        path: p.to_string(),
        reference: r.into(),
        unit: 1,
    };
    SchematicSheet {
        labels: vec![Label {
            uuid: id(1),
            shape: "input".to_string(),
            ..Label::new("GND_RAIL", Point::new(0.0, 0.0), LabelType::Net)
        }],
        symbols: vec![Symbol {
            uuid: id(2),
            reference: "R1".into(),
            value: "10k".into(),
            unit: 1,
            instances: vec![entry(&placement(1), "R1"), entry(&placement(2), "R101")],
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn apply(sheet: SchematicSheet, commands: Vec<Command>) -> SchematicSheet {
    let mut engine = Engine::new(sheet).unwrap();
    engine.execute_batch(commands).unwrap();
    engine.document().clone()
}

/// Mockup row 67: a Value result names its part, at each placement read.
#[test]
fn a_value_hit_names_its_part_at_the_scope_placements() {
    let mut sheet = sheet();
    let unplaced = DesignatorScope::default();
    let hits = scan_sheet(&sheet, &q("10k"), &unplaced);
    assert_eq!(
        value_part_name(&sheet, &hits[0], &unplaced).as_deref(),
        Some("R1")
    );
    let twice = DesignatorScope {
        placements: vec![placement(1), placement(2)],
        single: false,
    };
    assert_eq!(
        value_part_name(&sheet, &hits[0], &twice).as_deref(),
        Some("R1, R101")
    );
    let designator = scan_sheet(&sheet, &q("R1"), &unplaced);
    assert_eq!(designator[0].kind, FindKind::Designator);
    assert_eq!(value_part_name(&sheet, &designator[0], &unplaced), None);
    sheet.symbols[0].is_power = true;
    assert_eq!(value_part_name(&sheet, &hits[0], &unplaced), None);
}

#[test]
fn an_empty_needle_finds_nothing() {
    assert!(scan_sheet(&sheet(), &q("  "), &DesignatorScope::default()).is_empty());
}

#[test]
fn scan_finds_label_designator_and_value_case_insensitively() {
    let hits = scan_sheet(&sheet(), &q("r"), &DesignatorScope::default());
    let kinds: Vec<_> = hits.iter().map(|h| h.kind).collect();
    assert_eq!(kinds, [FindKind::NetLabel, FindKind::Designator]);
}

#[test]
fn a_twice_placed_sheet_yields_a_designator_hit_per_placement() {
    let scope = DesignatorScope {
        placements: vec![placement(1), placement(2)],
        single: false,
    };
    let texts: Vec<_> = scan_sheet(&sheet(), &q("r1"), &scope)
        .into_iter()
        .map(|h| (h.placement.unwrap(), h.text))
        .collect();
    assert_eq!(
        texts,
        [
            (placement(1), "R1".to_string()),
            (placement(2), "R101".to_string())
        ]
    );
}

#[test]
fn replace_recomputes_from_the_document_not_the_hit_text() {
    let mut hits = scan_sheet(&sheet(), &q("rail"), &DesignatorScope::default());
    hits[0].text = "STALE_RAIL".into();
    let commands = replace_commands(&sheet(), &hits[0], &q("rail"), "BUS");
    let out = apply(sheet(), commands);
    assert_eq!(out.labels[0].text, "GND_BUS");
}

#[test]
fn replace_is_empty_once_the_needle_is_gone() {
    let hits = scan_sheet(&sheet(), &q("rail"), &DesignatorScope::default());
    let mut edited = sheet();
    edited.labels[0].text = "VCC".into();
    assert!(replace_commands(&edited, &hits[0], &q("rail"), "BUS").is_empty());
}

#[test]
fn a_placement_designator_edit_changes_only_that_placement() {
    let scope = DesignatorScope {
        placements: vec![placement(1), placement(2)],
        single: false,
    };
    let hit = scan_sheet(&sheet(), &q("r101"), &scope).remove(0);
    let out = apply(sheet(), replace_commands(&sheet(), &hit, &q("r101"), "R7"));
    let symbol = &out.symbols[0];
    assert_eq!(symbol.designator_in(&placement(2)), "R7");
    assert_eq!(symbol.designator_in(&placement(1)), "R1");
    assert_eq!(symbol.reference, "R1");
}

#[test]
fn a_single_placement_edit_writes_reference_and_drops_the_entry() {
    let scope = DesignatorScope::at(placement(2), true);
    let hit = scan_sheet(&sheet(), &q("r101"), &scope).remove(0);
    let out = apply(sheet(), replace_commands(&sheet(), &hit, &q("r101"), "R7"));
    assert_eq!(out.symbols[0].reference, "R7");
    assert!(!out.symbols[0].has_instance_entry_in(&placement(2)));
}

#[test]
fn replace_all_ci_matches_the_old_app_behaviour() {
    assert_eq!(replace_all_ci("GND_RAIL", "r", "X"), "GND_XAIL");
    assert_eq!(replace_all_ci("R1_r2_R3", "r", "net"), "net1_net2_net3");
    assert_eq!(replace_all_ci("VCC", "", "X"), "VCC");
    assert_eq!(replace_all_ci("A_B", "_", "$1"), "A$1B");
}

#[test]
fn non_ascii_case_folds_the_same_way_in_scan_and_replace() {
    let mut s = sheet();
    s.labels[0].text = "\u{DC}NIT_\u{FC}nit".into();
    let hits = scan_sheet(&s, &q("\u{FC}nit"), &DesignatorScope::default());
    assert_eq!(hits.len(), 1, "the scan finds it");
    let out = apply(
        s.clone(),
        replace_commands(&s, &hits[0], &q("\u{FC}nit"), "X"),
    );
    assert_eq!(out.labels[0].text, "X_X", "and the replace rewrites both");
    assert_eq!(replace_all_ci("STRASSE", "\u{DF}", "x"), "STRASSE");
}

#[test]
fn a_power_ports_hidden_designator_is_not_found_but_its_net_name_is() {
    let mut sheet = sheet();
    sheet.symbols[0].reference = "#PWR01".into();
    sheet.symbols[0].value = "+5V".into();
    sheet.symbols[0].is_power = true;
    let scope = DesignatorScope::default();
    assert!(scan_sheet(&sheet, &q("PWR"), &scope).is_empty());
    let hits = scan_sheet(&sheet, &q("+5V"), &scope);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, FindKind::Value);
    // The text is fixed to the port, so the hit selects the port
    // (docs/DECISIONS.md, 2026-11-07); the replace still edits the value.
    assert_eq!(hits[0].item.kind, SelectedKind::Symbol);
    assert_eq!(
        hits[0].target,
        TextTarget::SymbolValue(sheet.symbols[0].uuid)
    );
}

// ---- #2525: Match case, Whole word, Regex ----

/// The sheet of the #2525 sample: three net labels that share a stem, a
/// note naming the net, and a part value built on it.
fn vout_sheet() -> SchematicSheet {
    let label = |n: u128, text: &str| Label {
        uuid: id(n),
        ..Label::new(text, Point::new(n as f64, 0.0), LabelType::Net)
    };
    SchematicSheet {
        labels: vec![
            label(10, "VOUT"),
            label(11, "VOUT_3V3"),
            label(12, "VOUT_SENSE"),
        ],
        text_notes: vec![signex_types::schematic::TextNote {
            uuid: id(13),
            text: "Keep VOUT trace short".into(),
            position: Point::new(0.0, 10.0),
            rotation: 0.0,
            font_size: 1.27,
            font_family: None,
            justify_h: Default::default(),
            justify_v: Default::default(),
            locked: false,
        }],
        symbols: vec![Symbol {
            uuid: id(14),
            reference: "R5".into(),
            value: "VOUT_ADJ".into(),
            unit: 1,
            ..Default::default()
        }],
        ..Default::default()
    }
}

fn hit_texts(hits: &[FindHit]) -> Vec<&str> {
    hits.iter().map(|h| h.text.as_str()).collect()
}

#[test]
fn whole_word_does_not_match_inside_a_longer_net_name() {
    let query = with("VOUT", MatchOptions::WHOLE_WORD);
    let hits = scan_sheet(&vout_sheet(), &query, &DesignatorScope::default());
    assert_eq!(hit_texts(&hits), ["VOUT", "Keep VOUT trace short"]);
    // The substring search the dialog had before matches all five.
    let all = scan_sheet(&vout_sheet(), &q("VOUT"), &DesignatorScope::default());
    assert_eq!(all.len(), 5);
}

#[test]
fn whole_word_is_the_default_and_a_symbol_next_to_the_word_is_a_boundary() {
    assert_eq!(MatchOptions::default(), MatchOptions::WHOLE_WORD);
    let query = with("+5V", MatchOptions::WHOLE_WORD);
    assert!(query.is_match("+5V"));
    assert!(query.is_match("rail +5V, filtered"));
    assert!(!query.is_match("+5V0"));
    // The one match, bytes 2..6 ("VOUT").
    let vout = 2..6;
    assert_eq!(
        with("vout", MatchOptions::WHOLE_WORD).ranges("a VOUT b"),
        [vout]
    );
}

#[test]
fn match_case_tells_upper_from_lower() {
    let options = MatchOptions {
        match_case: true,
        ..MatchOptions::SUBSTRING
    };
    assert!(with("VOUT", options).is_match("VOUT_3V3"));
    assert!(!with("vout", options).is_match("VOUT_3V3"));
    assert!(q("vout").is_match("VOUT_3V3"), "off, case folds");
    assert_eq!(
        with("vout", options).replace_all("vout VOUT", "x"),
        "x VOUT"
    );
}

#[test]
fn regex_matches_and_expands_its_groups() {
    let options = MatchOptions {
        regex: true,
        ..MatchOptions::SUBSTRING
    };
    let query = with(r"VOUT_(\d)V(\d)", options);
    let hits = scan_sheet(&vout_sheet(), &query, &DesignatorScope::default());
    assert_eq!(hit_texts(&hits), ["VOUT_3V3"]);
    assert_eq!(query.replace_all("VOUT_3V3", "V${1}P$2"), "V3P3");
    // Case-insensitive unless Match case is on.
    assert!(with("vout_s", options).is_match("VOUT_SENSE"));
    let cased = MatchOptions {
        match_case: true,
        ..options
    };
    assert!(!with("vout_s", cased).is_match("VOUT_SENSE"));
    // Whole word filters the pattern's hits too.
    let whole = MatchOptions {
        whole_word: true,
        ..options
    };
    let hits = scan_sheet(
        &vout_sheet(),
        &with("VOUT", whole),
        &DesignatorScope::default(),
    );
    assert_eq!(hit_texts(&hits), ["VOUT", "Keep VOUT trace short"]);
}

#[test]
fn an_invalid_regex_is_an_error_not_a_panic() {
    let options = MatchOptions {
        regex: true,
        ..MatchOptions::WHOLE_WORD
    };
    for bad in ["VOUT(", "[", "*", "a{2,1}"] {
        assert!(FindQuery::new(bad, options).is_err(), "{bad}");
    }
    // The same text is a plain needle when Regex is off.
    let literal = with("VOUT(", MatchOptions::SUBSTRING);
    assert!(literal.is_match("VOUT(1)"));
}

#[test]
fn a_regex_that_can_match_nothing_never_inserts_the_replacement() {
    let options = MatchOptions {
        regex: true,
        ..MatchOptions::SUBSTRING
    };
    let query = with("x*", options);
    assert_eq!(query.replace_all("VOUT", "Y"), "VOUT");
    assert_eq!(query.replace_all("AxxB", "Y"), "AYB");
}

#[test]
fn replace_with_whole_word_rewrites_only_whole_word_hits() {
    let sheet = vout_sheet();
    let query = with("VOUT", MatchOptions::WHOLE_WORD);
    let commands: Vec<Command> = scan_sheet(&sheet, &query, &DesignatorScope::default())
        .iter()
        .flat_map(|hit| replace_commands(&sheet, hit, &query, "VOUT_3V3"))
        .collect();
    let out = apply(sheet, commands);
    let labels: Vec<_> = out.labels.iter().map(|l| l.text.as_str()).collect();
    assert_eq!(labels, ["VOUT_3V3", "VOUT_3V3", "VOUT_SENSE"]);
    assert_eq!(out.text_notes[0].text, "Keep VOUT_3V3 trace short");
    assert_eq!(out.symbols[0].value, "VOUT_ADJ");
}

#[test]
fn a_hit_on_text_with_a_longer_word_too_rewrites_only_the_whole_word() {
    let mut sheet = vout_sheet();
    sheet.labels[0].text = "VOUT_SENSE VOUT".into();
    let query = with("vout", MatchOptions::WHOLE_WORD);
    let hit = scan_sheet(&sheet, &query, &DesignatorScope::default()).remove(0);
    let out = apply(sheet.clone(), replace_commands(&sheet, &hit, &query, "V"));
    assert_eq!(out.labels[0].text, "VOUT_SENSE V");
}

#[test]
fn a_literal_needle_is_trimmed_and_a_regex_is_kept_as_typed() {
    let literal = with(" VOUT ", MatchOptions::SUBSTRING);
    assert!(
        literal.is_match("VOUT"),
        "the spaces around a word are dropped"
    );
    let options = MatchOptions {
        regex: true,
        ..MatchOptions::SUBSTRING
    };
    let pattern = with(" VOUT", options);
    assert!(
        !pattern.is_match("VOUT"),
        "the space is part of the pattern"
    );
    assert_eq!(
        pattern.ranges("Keep VOUT short"),
        std::slice::from_ref(&(4..9))
    );
}
