//! #2398 - the rule text as rows, and edits that rewrite one rule only.

use signex_drc::NetScope;
use signex_types::violation::Severity;

use super::*;
use crate::compile;

/// Odd spacing and line breaks on purpose: an edit of one rule must leave
/// the others exactly as typed.
const TEXT: &str = "rule hv   on clearance when class == \"HV\"\n    require clearance >= 0.8mm then error \"HV gap\"\n\nrule ddr on track when net matches \"DDR_*\" and layer == outer\n  require width >= 0.1mm and width <= 12mil then warning\nrule vias on via require diameter >= 450um and drill >= 0.2 mm then info\n";

fn rows(src: &str) -> RuleTable {
    let got = table(src);
    assert!(!got.is_unparsed(), "{:?}", got.syntax);
    got
}

#[test]
fn each_rule_is_a_row_with_its_span_and_fields() {
    let got = rows(TEXT);
    let names: Vec<&str> = got.rows.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["hv", "ddr", "vias"]);
    let hv = &got.rows[0];
    assert_eq!(
        hv.text(TEXT),
        "rule hv   on clearance when class == \"HV\"\n    require clearance >= 0.8mm then error \"HV gap\""
    );
    assert_eq!(hv.kind, Some(RuleKind::Clearance));
    assert_eq!(hv.scope, NetScope::Class("HV".into()));
    assert_eq!(hv.limit(LimitField::MinClearance).unwrap().value, "0.8mm");
    assert_eq!(hv.message.as_deref(), Some("HV gap"));
    assert!(hv.enabled && hv.applied && !hv.text_only);
    let ddr = &got.rows[1];
    assert_eq!(ddr.scope, NetScope::Pattern("DDR_*".into()));
    assert_eq!(ddr.layer.as_deref(), Some("outer"));
    assert_eq!(ddr.limit(LimitField::MaxWidth).unwrap().value, "12mil");
    assert_eq!(ddr.severity, Severity::Warning);
    let vias = &got.rows[2];
    assert_eq!(vias.limit(LimitField::MinDrill).unwrap().value, "0.2 mm");
    assert_eq!(vias.id(), "user::vias");
}

#[test]
fn editing_a_cell_rewrites_only_that_rule() {
    let edited = set_field(
        TEXT,
        "ddr",
        RuleField::Limit(LimitField::MinWidth),
        "0.15mm",
    )
    .unwrap();
    let before = rows(TEXT);
    let after = rows(&edited);
    assert_eq!(after.rows[0].text(&edited), before.rows[0].text(TEXT));
    assert_eq!(after.rows[2].text(&edited), before.rows[2].text(TEXT));
    assert_eq!(
        after.rows[1].text(&edited),
        "rule ddr on track when net matches \"DDR_*\" and layer == outer \
         require width >= 0.15mm and width <= 12mil then warning"
    );
    // Everything before and after the edited rule is byte for byte the same.
    let start = before.rows[1].span.start;
    assert_eq!(&edited[..start], &TEXT[..start]);
    assert!(edited.ends_with(&TEXT[before.rows[1].span.end..]));
    let compiled = compile(&edited);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    assert_eq!(compiled.rules.len(), 3);
}

#[test]
fn scope_layer_pair_severity_and_name_cells_are_written_back() {
    let text = set_field(TEXT, "hv", RuleField::Scope, "net == \"HV_BUS\"").unwrap();
    let text = set_field(&text, "hv", RuleField::Pair, "track-pad").unwrap();
    let text = set_field(&text, "hv", RuleField::Layer, "inner").unwrap();
    let text = set_field(&text, "hv", RuleField::Severity, "warning").unwrap();
    let text = set_field(&text, "hv", RuleField::Name, "hv_bus").unwrap();
    let row = rows(&text).row("hv_bus").cloned().unwrap();
    assert_eq!(row.scope, NetScope::Net("HV_BUS".into()));
    assert_eq!(row.pair, Some(["track".to_string(), "pad".to_string()]));
    assert_eq!(row.layer.as_deref(), Some("inner"));
    assert_eq!(row.severity, Severity::Warning);
    assert!(row.problems.is_empty(), "{:?}", row.problems);
    let back = set_field(&text, "hv_bus", RuleField::Scope, "").unwrap();
    assert_eq!(rows(&back).row("hv_bus").unwrap().scope, NetScope::Board);
}

#[test]
fn a_disabled_rule_keeps_its_name_and_is_not_applied() {
    let off = enable_rule(TEXT, "vias", false).unwrap();
    assert!(off.contains("disabled rule vias on via"));
    let row = rows(&off).row("vias").cloned().unwrap();
    assert!(!row.enabled && !row.applied);
    assert!(row.compiled.is_some(), "still checked");
    let compiled = compile(&off);
    assert_eq!(compiled.rules.len(), 2);
    assert_eq!((compiled.disabled, compiled.not_applied), (1, 0));
    // The name stays taken: a second `vias` is the repeated one.
    let twice = format!("{off}rule vias on via require drill >= 0.3mm then error\n");
    assert_eq!(compile(&twice).not_applied, 1);
    // Back on, the text is what it was.
    assert_eq!(enable_rule(&off, "vias", true).unwrap(), TEXT);
    assert_eq!(
        set_field(&off, "vias", RuleField::Enabled, "true").unwrap(),
        TEXT
    );
}

#[test]
fn a_rule_is_added_after_the_others_and_removed_with_its_line() {
    let mut draft = RuleDraft::new(RuleKind::Track, "");
    draft.set_values("min_width=0.2mm").unwrap();
    draft.set(RuleField::Scope, "class == \"Power\"").unwrap();
    let (added, name) = add_rule(TEXT, &draft).unwrap();
    assert_eq!(name, "track_1");
    assert!(added.starts_with(TEXT));
    assert!(added.ends_with(
        "rule track_1 on track when class == \"Power\" require width >= 0.2mm then error\n"
    ));
    let removed = remove_rule(&added, "track_1").unwrap();
    assert_eq!(removed, TEXT);
    let without_ddr = remove_rule(TEXT, "user::ddr").unwrap();
    assert!(!without_ddr.contains("ddr"));
    assert!(without_ddr.contains("rule vias on via"));
    assert_eq!(rows(&without_ddr).rows.len(), 2);
}

#[test]
fn a_bare_length_sets_the_first_limit_of_the_kind() {
    let mut draft = RuleDraft::new(RuleKind::Clearance, "gap");
    draft.set_values("0.25mm").unwrap();
    assert_eq!(draft.limits[0].field, LimitField::MinClearance);
    let (text, _) = add_rule("", &draft).unwrap();
    assert_eq!(
        text,
        "rule gap on clearance require clearance >= 0.25mm then error\n"
    );
}

#[test]
fn an_edit_that_would_not_compile_is_refused_and_says_why() {
    let unit = set_field(
        TEXT,
        "hv",
        RuleField::Limit(LimitField::MinClearance),
        "0.8xx",
    );
    assert!(matches!(unit, Err(EditError::Invalid(_))), "{unit:?}");
    let no_unit = set_field(
        TEXT,
        "hv",
        RuleField::Limit(LimitField::MinClearance),
        "0.8",
    );
    assert!(
        matches!(no_unit, Err(EditError::BadValue { .. })),
        "{no_unit:?}"
    );
    let layer = set_field(TEXT, "hv", RuleField::Layer, "Nowhere");
    assert!(matches!(layer, Err(EditError::Invalid(_))), "{layer:?}");
    assert_eq!(
        set_field(TEXT, "hv", RuleField::Name, "ddr"),
        Err(EditError::NameTaken("ddr".into()))
    );
    assert!(matches!(
        set_field(TEXT, "vias", RuleField::Pair, "track-pad"),
        Err(EditError::BadValue { .. })
    ));
    assert!(matches!(
        set_field(
            TEXT,
            "ddr",
            RuleField::Limit(LimitField::MinClearance),
            "1mm"
        ),
        Err(EditError::BadValue { .. })
    ));
    assert_eq!(
        set_field(TEXT, "nope", RuleField::Severity, "info"),
        Err(EditError::NoSuchRule("nope".into()))
    );
    assert_eq!(
        set_field("rule broken", "broken", RuleField::Severity, "info"),
        Err(EditError::Unparsed)
    );
}

#[test]
fn a_rule_the_table_cannot_show_is_a_text_only_row() {
    let src = "rule two on clearance when net == \"A\" and class == \"B\" require clearance >= 1mm then error\n";
    let got = rows(src);
    let row = &got.rows[0];
    assert!(row.text_only);
    assert!(row.draft().is_none());
    assert_eq!(
        set_field(src, "two", RuleField::Severity, "info"),
        Err(EditError::TextOnly("two".into()))
    );
    // On / off and removal still work on it.
    assert!(
        enable_rule(src, "two", false)
            .unwrap()
            .starts_with("disabled rule two")
    );
    assert_eq!(remove_rule(src, "two").unwrap(), "");
}

#[test]
fn the_scope_cell_reads_what_it_writes() {
    for scope in [
        NetScope::Board,
        NetScope::Net("GND".into()),
        NetScope::Class("HV".into()),
        NetScope::Pattern("USB_*".into()),
    ] {
        assert_eq!(parse_scope(&scope_text(&scope)), Ok(scope));
    }
    assert_eq!(parse_scope("net==GND"), Ok(NetScope::Net("GND".into())));
    assert!(parse_scope("layer == outer").is_err());
    assert!(parse_scope("net == \"a\"b\"").is_err());
}
