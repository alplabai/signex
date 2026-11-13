//! #2307 step 2b - the DRC rule language: syntax, validation, compiled
//! rules and the layer messages.

use signex_drc::{CopperObject, Limit, NetScope};
use signex_types::project::DrcRuleSet;
use signex_types::violation::Severity;

use super::*;

const SAMPLE: &str = r#"
rule hv_spacing on clearance when class == "HV" require clearance >= 0.8mm
  then error "HV nets need 0.8 mm"
rule ddr_neck on track when net matches "DDR_*" and layer == outer
  require width >= 0.1mm and width <= 12mil then warning
rule vias on via require diameter >= 450um and drill >= 200000nm then info
rule pad_track on clearance between track and pad require clearance >= 0.25mm then error
"#;

fn one(src: &str) -> Compiled {
    let got = compile(src);
    assert!(got.rules.len() <= 1, "{got:?}");
    got
}

fn messages(got: &Compiled) -> Vec<&str> {
    got.diagnostics.iter().map(|d| d.message.as_str()).collect()
}

#[test]
fn the_sample_compiles_to_four_rules() {
    let got = compile(SAMPLE);
    assert!(got.diagnostics.is_empty(), "{:?}", got.diagnostics);
    let names: Vec<_> = got.rules.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["hv_spacing", "ddr_neck", "vias", "pad_track"]);

    let hv = &got.rules[0];
    assert_eq!(hv.nets, NetScope::Class("HV".into()));
    assert_eq!(hv.limits, [Limit::MinClearance(800_000)]);
    assert_eq!(hv.severity, Severity::Error);
    assert_eq!(hv.message.as_deref(), Some("HV nets need 0.8 mm"));
    assert_eq!(hv.id(), "user::hv_spacing");

    let ddr = &got.rules[1];
    assert_eq!(ddr.nets, NetScope::Pattern("DDR_*".into()));
    assert_eq!(ddr.layers, Some(1 | 1 << 31));
    assert_eq!(
        ddr.limits,
        [Limit::MinTrackWidth(100_000), Limit::MaxTrackWidth(304_800)]
    );
    assert_eq!(ddr.severity, Severity::Warning);
    assert_eq!(ddr.message, None);

    let vias = &got.rules[2];
    assert_eq!(vias.nets, NetScope::Board);
    assert_eq!(
        vias.limits,
        [Limit::MinViaDiameter(450_000), Limit::MinViaDrill(200_000)]
    );
    assert_eq!(vias.severity, Severity::Info);

    let pair = &got.rules[3];
    assert_eq!(pair.pair, Some([CopperObject::Track, CopperObject::Pad]));
}

#[test]
fn every_unit_converts_to_integer_nm() {
    for (text, nm) in [
        ("1mm", 1_000_000),
        ("4mil", 101_600),
        ("150um", 150_000),
        ("150\u{b5}m", 150_000),
        ("125nm", 125),
        ("0.127mm", 127_000),
    ] {
        let got = one(&format!(
            "rule r on clearance require clearance >= {text} then error"
        ));
        assert_eq!(got.rules[0].limits, [Limit::MinClearance(nm)], "{text}");
    }
}

#[test]
fn net_and_layer_scopes_compile() {
    let src = r#"rule a on clearance when net == "GND" and layer == inner require clearance >= 1mm then error"#;
    let rule = &one(src).rules[0];
    assert_eq!(rule.nets, NetScope::Net("GND".into()));
    assert_eq!(rule.layers, Some(!(1 | 1 << 31)));
    let named = one("rule a on track when layer == Inner2Copper require width >= 1mm then error");
    assert_eq!(named.rules[0].layers, Some(1 << 2));
}

#[test]
fn a_syntax_error_has_a_span_and_applies_no_rule() {
    let src = "rule ok on via require drill >= 0.2mm then error\nrule bad on via require drill 0.2mm then error";
    let got = compile(src);
    assert!(got.rules.is_empty());
    let d = &got.diagnostics[0];
    assert_eq!((d.level, d.rule.as_deref()), (Level::Error, None));
    // The parser stops at `0.2mm`, where `>=` or `<=` belongs.
    assert_eq!(d.line_col(src), (2, 31), "{d:?}");
    assert!(d.render(src).starts_with("line 2, column 31: "), "{d:?}");
}

#[test]
fn a_space_before_a_known_unit_is_accepted_and_and_is_not_a_unit() {
    let got = compile(
        "rule r on track require width >= 0.2 mm and width <= 12 mil then error\n\
         rule v on via require drill >= 150 \u{b5}m then error",
    );
    assert!(got.diagnostics.is_empty(), "{got:?}");
    assert_eq!(
        got.rules[0].limits,
        [Limit::MinTrackWidth(200_000), Limit::MaxTrackWidth(304_800)]
    );
    assert_eq!(got.rules[1].limits, [Limit::MinViaDrill(150_000)]);
    // A spaced word that is not a unit is not taken as one.
    let missing = compile("rule r on via require drill >= 0.2 and drill >= 1mm then error");
    assert_eq!(
        messages(&missing),
        ["'0.2' needs a unit (mm, mil, um or nm)"]
    );
}

#[test]
fn an_implausible_or_zero_limit_is_said() {
    let got = compile(
        "rule big on clearance require clearance >= 20mm then error\n\
         rule huge on clearance require clearance >= 200mm then error\n\
         rule none on track require width >= 0mm then error\n\
         rule all on track require width <= 0mm then error",
    );
    let names: Vec<_> = got.rules.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["big", "none", "all"]);
    let said: Vec<_> = got
        .diagnostics
        .iter()
        .map(|d| (d.level, d.message.as_str()))
        .collect();
    assert_eq!(
        said,
        [
            (
                Level::Warning,
                "a clearance of 20 mm is over 10 mm; check the unit"
            ),
            (
                Level::Error,
                "a clearance of 200 mm is over 100 mm; check the unit"
            ),
            (
                Level::Warning,
                "a limit of 0 checks nothing for this rule's scope"
            ),
            (
                Level::Warning,
                "a maximum of 0 reports every track in this rule's scope"
            ),
        ]
    );
}

#[test]
fn an_invalid_rule_does_not_take_its_name_from_a_valid_one() {
    let got = compile(
        "rule w on track require width >= 1in then error\n\
         rule w on track require width >= 1mm then error",
    );
    assert_eq!(got.rules.len(), 1, "{got:?}");
    assert_eq!(got.rules[0].limits, [Limit::MinTrackWidth(1_000_000)]);
    assert_eq!(got.not_applied, 1);
}

#[test]
fn what_was_not_applied_is_counted() {
    let skipped = compile(
        "rule a on via require drill >= 2in then error\n\
         rule e on clearance between track and edge require clearance >= 1mm then error\n\
         rule ok on via require drill >= 1mm then error",
    );
    assert_eq!((skipped.not_applied, skipped.unparsed), (2, false));
    let broken = compile("rule a on via require drill");
    assert_eq!((broken.not_applied, broken.unparsed), (0, true));
    let layered = compile_layers(
        Some(&layer("rule a on")),
        Some(&layer("rule a on via require drill >= 2in then error")),
    );
    assert_eq!((layered.not_applied, layered.unparsed), (1, 1));
}

#[test]
fn an_unknown_target_field_or_condition_drops_only_that_rule() {
    let src = r#"
rule a on silk require width >= 1mm then error
rule b on track require gap >= 1mm then error
rule c on track when colour == "red" require width >= 1mm then error
rule d on via require drill <= 1mm then error
rule ok on via require drill >= 0.2mm then error
"#;
    let got = compile(src);
    let names: Vec<_> = got.rules.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["ok"]);
    assert_eq!(
        messages(&got),
        [
            "unknown target 'silk' (clearance, track or via)",
            "unknown field 'gap' for this target (width)",
            "unknown condition 'colour' (net, class or layer)",
            "'drill' takes only '>=' (a minimum)",
        ]
    );
    // Each error points at the word it is about.
    let at = |i: usize| &src[got.diagnostics[i].span.clone()];
    assert_eq!(
        [at(0), at(1), at(2), at(3)],
        ["silk", "gap", "colour", "drill"]
    );
}

#[test]
fn a_missing_or_unknown_unit_is_an_error_at_the_number() {
    let src = "rule a on via require drill >= 0.2 then error\nrule b on via require drill >= 2in then error";
    let got = compile(src);
    assert!(got.rules.is_empty());
    assert_eq!(
        messages(&got),
        [
            "'0.2' needs a unit (mm, mil, um or nm)",
            "unknown unit 'in' (mm, mil, um or nm)",
        ]
    );
    assert_eq!(&src[got.diagnostics[1].span.clone()], "in");
}

#[test]
fn an_unmeasured_pair_parses_but_is_a_not_checked_warning() {
    let got =
        compile("rule e on clearance between track and edge require clearance >= 0.3mm then error");
    assert!(got.rules.is_empty());
    assert_eq!(got.diagnostics[0].level, Level::Warning);
    assert_eq!(
        messages(&got),
        ["clearance to 'edge' is not checked yet; the rule is not applied"]
    );
    let unknown =
        compile("rule m on clearance between track and mask require clearance >= 0.3mm then error");
    assert_eq!(unknown.diagnostics[0].level, Level::Error);
}

#[test]
fn a_region_scope_is_reported_as_not_supported_yet() {
    let got = compile(r#"rule r on track when region == "BGA1" require width >= 0.1mm then error"#);
    assert!(got.rules.is_empty());
    assert_eq!(messages(&got), ["a region scope is not supported yet"]);
}

#[test]
fn two_net_scopes_or_a_repeated_limit_or_name_are_errors() {
    let src = r#"
rule a on track when net == "A" and class == "B" require width >= 1mm then error
rule b on track require width >= 1mm and width >= 2mm then error
rule c on track require width >= 1mm then error
rule c on track require width >= 2mm then error
rule d on track between track and pad require width >= 1mm then error
"#;
    let got = compile(src);
    let names: Vec<_> = got.rules.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["c"]);
    assert_eq!(got.rules[0].limits, [Limit::MinTrackWidth(1_000_000)]);
    assert_eq!(got.diagnostics.len(), 4, "{:?}", got.diagnostics);
}

#[test]
fn empty_text_has_no_rules_and_says_nothing() {
    assert_eq!(compile(""), Compiled::default());
    assert_eq!(compile("  \n\t "), Compiled::default());
}

fn layer(src: &str) -> DrcRuleSet {
    DrcRuleSet {
        dsl: Some(src.into()),
        ..DrcRuleSet::default()
    }
}

#[test]
fn layers_compile_profile_then_project_and_the_project_replaces_a_name() {
    let profile = layer(
        "rule w on track require width >= 1mm then error\nrule v on via require drill >= 1mm then error",
    );
    let project = layer("rule w on track require width >= 2mm then warning");
    let got = compile_layers(Some(&profile), Some(&project));
    assert!(got.messages.is_empty(), "{:?}", got.messages);
    let names: Vec<_> = got.rules.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, ["v", "w"]);
    assert_eq!(got.rules[1].limits, [Limit::MinTrackWidth(2_000_000)]);
    assert_eq!(compile_layers(None, None), LayeredRules::default());
    assert_eq!(
        compile_layers(Some(&DrcRuleSet::default()), None),
        LayeredRules::default()
    );
}

#[test]
fn layer_messages_name_the_layer_and_say_what_was_dropped() {
    let project = layer("rule a on via require drill >= 2in then error\nrule b on");
    let got = compile_layers(None, Some(&project));
    assert!(got.rules.is_empty());
    assert_eq!(got.messages.len(), 1, "{:?}", got.messages);
    assert!(
        got.messages[0].starts_with("DRC DSL (project): line 2, column ")
            && got.messages[0].ends_with("; no rule of this text is applied"),
        "{:?}",
        got.messages
    );
    let skipped = compile_layers(
        Some(&layer("rule a on via require drill >= 2in then error")),
        None,
    );
    assert_eq!(
        skipped.messages,
        [
            "DRC DSL (profile): line 1, column 33: rule 'a': unknown unit 'in' (mm, mil, um or nm); the rule is skipped"
        ]
    );
}
