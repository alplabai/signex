//! Project setup defaults (#2232): profiles, the draft, rule files, the
//! `.snxprj` round trip and what a new board or sheet inherits.

use super::*;

fn blank_project() -> ProjectData {
    ProjectData {
        document_id: None,
        description: None,
        name: "P".into(),
        dir: String::new(),
        schematic_root: None,
        pcb_file: None,
        sheets: Vec::new(),
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        net_classes: Vec::new(),
        rules: None,
        annotation: None,
        parameters: Default::default(),
        schematic_defaults: Default::default(),
        project_defaults: Default::default(),
        agent_journal: Default::default(),
        schematic_design: Default::default(),
        fab_outputs: None,
        router_tool_diameter: None,
        requirements: Default::default(),
        unknown: Default::default(),
    }
}

#[test]
fn the_profiles_carry_the_approved_values() {
    let row = |p: SetupProfile| {
        let d = p.pcb(8);
        (
            d.copper_layers,
            d.thickness_mm,
            d.clearance_mm,
            d.track_width_mm,
            d.via_diameter_mm,
            d.via_drill_mm,
        )
    };
    // The canonical tier table (#2537, XF-06) on each tier's default layers.
    assert_eq!(row(SetupProfile::Standard), (4, 1.6, 0.15, 0.2, 0.5, 0.25));
    assert_eq!(row(SetupProfile::Advanced), (6, 1.6, 0.1, 0.125, 0.4, 0.2));
    assert_eq!(row(SetupProfile::Hdi), (8, 1.6, 0.075, 0.075, 0.25, 0.1));
    assert_eq!(SetupProfile::Custom.pcb(11).copper_layers, 11);
    assert_eq!(
        SetupProfile::Custom.rules_on(4),
        SetupProfile::Standard.rules_on(4),
        "Custom starts from Standard"
    );
    for tier in SetupProfile::TIERS {
        let fab = tier.fab_template().expect("a tier is a fab class");
        assert_eq!(SetupProfile::from_fab(fab), tier);
        assert!(tier.pcb(4).problems().is_empty(), "{tier:?}");
    }
    assert_eq!(SetupProfile::Custom.fab_template(), None);
    assert_eq!(SetupProfile::from_token("hobby"), None, "no legacy token");
    assert_eq!(ProjectSetup::default().profile, SetupProfile::Standard);
}

/// #2537: a tier card's rules pass that tier's own fab floor, so a new
/// board's default vias and tracks do not fail its `fab_*` rules.
#[test]
fn every_tier_card_passes_its_own_fab_limits() {
    const EPS: f64 = 1e-9;
    for tier in SetupProfile::TIERS {
        let fab = tier.fab_template().expect("a tier is a fab class");
        let c: FabCapabilities = fab.capabilities();
        let d = tier.rules_on(tier.default_layers());
        let annular = (d.via_diameter_mm - d.via_drill_mm) / 2.0;
        assert!(d.clearance_mm + EPS >= c.clearance, "{tier:?} clearance");
        assert!(d.track_width_mm + EPS >= c.track_width, "{tier:?} track");
        assert!(d.via_diameter_mm + EPS >= c.via_diameter, "{tier:?} via");
        assert!(d.via_drill_mm + EPS >= c.via_drill, "{tier:?} drill");
        assert!(d.via_drill_mm + EPS >= c.hole, "{tier:?} hole");
        assert!(annular + EPS >= c.annular_ring, "{tier:?} annular ring");
    }
}

#[test]
fn the_layer_count_is_its_own_choice_and_keeps_the_tier() {
    let mut setup = ProjectSetup::default();
    setup.set_layers(40);
    assert_eq!(
        (setup.profile, setup.pcb.copper_layers),
        (SetupProfile::Standard, 32),
        "a layer count does not switch to Custom (#2537)"
    );
    setup.set_layers(0);
    assert_eq!(setup.pcb.copper_layers, 1);
    setup.pick_profile(SetupProfile::Hdi, &[]);
    assert_eq!(
        (setup.profile, setup.pcb.copper_layers),
        (SetupProfile::Hdi, 8)
    );
    assert_eq!(setup.pcb.clearance_mm, 0.075);
}

/// #2537: each card has its own stepper, 2 to 32 in steps of 2; stepping a
/// card picks it on the new count and keeps the tier's rules.
#[test]
fn a_card_stepper_moves_by_two_and_picks_its_card() {
    assert_eq!(stepped_layers(4, 2), 6);
    assert_eq!(stepped_layers(4, -2), 2);
    assert_eq!(stepped_layers(2, -2), 2);
    assert_eq!(stepped_layers(32, 2), 32);
    assert_eq!(
        stepped_layers(5, 1),
        6,
        "an odd count steps to the even one"
    );
    assert_eq!(stepped_layers(3, -1), 2);
    let mut setup = ProjectSetup::default();
    setup.step_card_layers(SetupProfile::Advanced, 2, &[]);
    assert_eq!(
        (
            setup.profile,
            setup.pcb.copper_layers,
            setup.pcb.track_width_mm
        ),
        (SetupProfile::Advanced, 8, 0.125)
    );
    assert_eq!(setup.card_layers(SetupProfile::Advanced, &[]), 8);
    assert_eq!(setup.card_layers(SetupProfile::Hdi, &[]), 8);
    assert_eq!(setup.card_layers(SetupProfile::Standard, &[]), 4);
    // Another card and back: each card keeps its own count.
    setup.pick_profile(SetupProfile::Standard, &[]);
    assert_eq!(setup.pcb.copper_layers, 4);
    setup.pick_profile(SetupProfile::Advanced, &[]);
    assert_eq!(setup.pcb.copper_layers, 8);
    for _ in 0..20 {
        setup.step_card_layers(SetupProfile::Custom, -2, &[]);
    }
    assert_eq!(
        (setup.profile, setup.pcb.copper_layers),
        (SetupProfile::Custom, MIN_STEPPED_LAYERS)
    );
}

#[test]
fn a_fab_class_picks_its_tier_on_the_layers_shown_and_a_card_pick_clears_it() {
    let mut setup = ProjectSetup::default();
    setup.apply_fab(FabTemplate::Advanced);
    assert_eq!(setup.fab, Some(FabTemplate::Advanced));
    assert_eq!(
        setup.profile,
        SetupProfile::Advanced,
        "the field mirrors the card"
    );
    assert_eq!(
        (setup.pcb.clearance_mm, setup.pcb.track_width_mm),
        (0.1, 0.125),
        "the canonical tier table"
    );
    assert_eq!(setup.pcb.copper_layers, 4, "layers are not part of a class");
    setup.pick_profile(SetupProfile::Standard, &[]);
    assert_eq!((setup.fab, setup.pcb.clearance_mm), (None, 0.15));
}

#[test]
fn rule_values_are_validated_with_a_reason() {
    assert_eq!(PcbField::Clearance.parse("0.2 mm"), Ok(0.2));
    assert_eq!(
        PcbField::Clearance.parse("abc"),
        Err(PcbValueError::NotANumber)
    );
    assert!(matches!(
        PcbField::Clearance.parse("9"),
        Err(PcbValueError::OutOfRange { .. })
    ));
    let mut bad = SetupProfile::Standard.pcb(4);
    bad.via_drill_mm = bad.via_diameter_mm;
    assert_eq!(
        bad.problems(),
        vec![(PcbField::ViaDrill, PcbValueError::DrillNotSmallerThanVia)]
    );
}

#[test]
fn overrides_apply_on_the_profile_and_refuse_bad_input_by_key() {
    let mut setup = ProjectSetup::from_profile(SetupProfile::Advanced);
    setup
        .apply_overrides("layers=10, clearance=0.12,sheet=A4,designators=din40719_2")
        .unwrap();
    assert_eq!(setup.pcb.copper_layers, 10);
    assert_eq!(
        setup.profile,
        SetupProfile::Advanced,
        "layers keep the tier"
    );
    assert_eq!(setup.pcb.clearance_mm, 0.12);
    assert_eq!(setup.sheet.paper_size, "A4");
    assert_eq!(
        setup.designators,
        crate::designator::DesignatorStandard::Din40719_2
    );
    for (text, want) in [
        ("clearance=x", "clearance"),
        ("layers=33", "layers"),
        ("sheet=Z9", "sheet"),
        ("colour=red", "unknown override"),
        ("drill=0.9", "drill"),
        ("nonsense", "key=value"),
    ] {
        let err = ProjectSetup::default().apply_overrides(text).unwrap_err();
        assert!(err.contains(want), "{text}: {err}");
    }
}

const GOOD_RULES: &str = r#"
name = "Team rules"
[pcb]
clearance_mm = 0.127
track_width_mm = 0.127
via_diameter_mm = 0.45
via_drill_mm = 0.2
copper_layers = 6
"#;

#[test]
fn a_rule_file_fills_the_rules_and_the_layers() {
    let template = RuleTemplate::parse(GOOD_RULES).unwrap();
    assert_eq!(template.name.as_deref(), Some("Team rules"));
    let mut setup = ProjectSetup::default();
    setup.apply_template(&template);
    assert!(setup.imported);
    assert_eq!(
        (
            setup.pcb.clearance_mm,
            setup.pcb.via_diameter_mm,
            setup.pcb.copper_layers
        ),
        (0.127, 0.45, 6)
    );
    assert_eq!(setup.profile, SetupProfile::Custom);
}

#[test]
fn a_bad_rule_file_says_which_key_is_wrong() {
    for (text, want) in [
        ("[pcb\nx=1", "not valid TOML"),
        ("name = \"x\"", "no `[pcb]` table"),
        ("[pcb]\nclearance_mm = 0.1", "`track_width_mm` is missing"),
        (
            "[pcb]\nclearance_mm = 99\ntrack_width_mm=0.1\nvia_diameter_mm=0.4\nvia_drill_mm=0.2",
            "`clearance_mm` must be",
        ),
        (
            "[pcb]\nclearance_mm = 0.1\ntrack_width_mm=0.1\nvia_diameter_mm=0.4\nvia_drill_mm=0.2\ncolour=1",
            "unknown field",
        ),
        (
            "[pcb]\nclearance_mm = 0.1\ntrack_width_mm=0.1\nvia_diameter_mm=0.4\nvia_drill_mm=0.2\ncopper_layers=40",
            "`copper_layers` must be 1 to 32",
        ),
    ] {
        let err = RuleTemplate::parse(text).unwrap_err().to_string();
        assert!(err.contains(want), "{text}: {err}");
    }
}

#[test]
fn project_defaults_are_written_only_when_set_and_round_trip() {
    let plain = blank_project();
    let bytes = encode_project(&plain).unwrap();
    assert!(
        !String::from_utf8(bytes)
            .unwrap()
            .contains("project_defaults")
    );

    let mut data = blank_project();
    data.project_defaults = ProjectSetup::from_profile(SetupProfile::Hdi).defaults();
    let bytes = encode_project(&data).unwrap();
    let back = parse_project_bytes(std::path::Path::new("P.snxprj"), &bytes).unwrap();
    assert_eq!(back.project_defaults, data.project_defaults);
    assert_eq!(back.project_defaults.pcb.unwrap().copper_layers, 8);
}

#[test]
fn a_new_board_inherits_the_rules_and_the_layer_stack() {
    let mut data = blank_project();
    assert!(data.new_board().is_none(), "no defaults, no override");
    let mut setup = ProjectSetup::from_profile(SetupProfile::Advanced);
    setup.set_layers(4);
    data.project_defaults = setup.defaults();
    let board = data.new_board().unwrap();
    let rules = board.setup.as_ref().unwrap();
    assert_eq!(
        (rules.clearance, rules.trace_width, rules.via_drill),
        (0.1, 0.125, 0.2)
    );
    let copper: Vec<_> = board
        .layers
        .iter()
        .filter(|l| l.layer_type == "copper")
        .map(|l| l.name.as_str())
        .collect();
    assert_eq!(
        copper,
        ["TopCopper", "Inner1Copper", "Inner2Copper", "BottomCopper"]
    );
    assert_eq!(board.thickness, 1.6);
}

#[test]
fn a_new_sheet_inherits_the_size_and_the_title_block() {
    let mut data = blank_project();
    let mut sheet = crate::schematic::SchematicSheet {
        paper_size: "A4".into(),
        ..Default::default()
    };
    data.apply_sheet_defaults(&mut sheet);
    assert_eq!(sheet.paper_size, "A4", "no defaults, sheet unchanged");

    let mut setup = ProjectSetup::default();
    setup.sheet.paper_size = "A2".into();
    setup.sheet.show_title_block = false;
    data.project_defaults = setup.defaults();
    data.apply_sheet_defaults(&mut sheet);
    assert_eq!(sheet.paper_size, "A2");
    assert!(!sheet.frame.show_title_block);
}

#[test]
fn the_sheet_line_reads_like_the_preview() {
    assert_eq!(ProjectSetup::default().sheet_line(), "A3 landscape");
}

/// New projects start on A3; the A4 in mockup row 21 is a sample value.
#[test]
fn a_new_setup_defaults_to_a3() {
    assert_eq!(ProjectSetup::default().sheet.paper_size, "A3");
}

#[test]
fn a_stored_setup_reads_back_as_the_profile_it_matches() {
    let hdi = ProjectSetup::from_profile(SetupProfile::Hdi);
    let back = ProjectSetup::from_stored(&hdi.defaults(), None);
    assert_eq!(back.profile, SetupProfile::Hdi);
    assert_eq!(back.pcb, hdi.pcb);
    // A tier on another layer count is still that tier (#2537).
    let mut ten = ProjectSetup::default();
    ten.set_layers(10);
    let back = ProjectSetup::from_stored(&ten.defaults(), None);
    assert_eq!(
        (back.profile, back.card_layers(SetupProfile::Standard, &[])),
        (SetupProfile::Standard, 10)
    );
    let mut custom = ten;
    custom.pcb.clearance_mm = 0.13;
    let back = ProjectSetup::from_stored(
        &custom.defaults(),
        Some(crate::designator::DesignatorStandard::Iec81346_2),
    );
    assert_eq!(
        (
            back.profile,
            back.pcb.copper_layers,
            back.card_layers(SetupProfile::Custom, &[])
        ),
        (SetupProfile::Custom, 10, 10)
    );
    assert_eq!(
        back.designators,
        crate::designator::DesignatorStandard::Iec81346_2
    );
    assert_eq!(
        ProjectSetup::from_stored(&ProjectDefaults::default(), None),
        ProjectSetup::default()
    );
}

fn opened(json: &str) -> ProjectData {
    parse_project_bytes(std::path::Path::new("P.snxprj"), json.as_bytes()).expect("opens")
}

const BASE: &str = r#""name":"P","dir":"","schematic_root":null,"pcb_file":null"#;

#[test]
fn an_unusable_project_defaults_section_is_kept_and_not_applied() {
    let good_pcb = r#""copper_layers":4,"thickness_mm":1.6,"clearance_mm":0.15,"track_width_mm":0.2,"via_diameter_mm":0.5,"via_drill_mm":0.25"#;
    for pcb in [
        // u8 overflow
        good_pcb.replace("\"copper_layers\":4", "\"copper_layers\":300"),
        // a missing field
        good_pcb.replace(",\"via_drill_mm\":0.25", ""),
        // a negative clearance
        good_pcb.replace("0.15", "-0.15"),
    ] {
        let json = format!(r#"{{{BASE},"project_defaults":{{"pcb":{{{pcb}}}}}}}"#);
        let data = opened(&json);
        assert!(data.project_defaults.is_empty(), "{pcb}");
        assert!(data.has_unreadable_project_defaults(), "{pcb}");
        // Kept verbatim for the next save.
        let bytes = encode_project(&data).unwrap();
        let back: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let wanted: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            back["project_defaults"], wanted["project_defaults"],
            "{pcb}"
        );
    }
    let json = format!(r#"{{{BASE},"project_defaults":{{"pcb":{{{good_pcb}}}}}}}"#);
    let data = opened(&json);
    assert_eq!(data.project_defaults.pcb.map(|p| p.copper_layers), Some(4));
}

#[test]
fn a_new_board_has_a_real_grid() {
    let board = SetupProfile::Standard.pcb(4).new_board();
    let grid = board.setup.unwrap().grid_size;
    assert_eq!(grid, NEW_BOARD_GRID_MM);
    assert!(grid > 0.0);
}

/// #2384: a new board's origin is the bottom-left corner of its board area,
/// whatever rules it starts from.
#[test]
fn a_new_board_counts_from_the_bottom_left_of_its_area() {
    let board = SetupProfile::Standard.pcb(2).new_board();
    let corner = crate::pcb_origin::board_area_bottom_left(&board.outline);
    assert_eq!(board.options.origin.at, corner);
    assert_eq!(corner, crate::pcb::Point::new(0.0, 80.0));
}

/// #2311: `fab=<tier>` in the overrides picks a tier as the dialog's chip
/// does, in written order with the other keys.
#[test]
fn a_fab_override_picks_the_tier() {
    let mut setup = ProjectSetup::default();
    setup.apply_overrides("fab=hdi, clearance=0.2").unwrap();
    assert_eq!(setup.fab, Some(FabTemplate::Hdi));
    assert_eq!(setup.pcb.track_width_mm, 0.075);
    assert_eq!(setup.pcb.clearance_mm, 0.2, "a later key wins");
    let err = ProjectSetup::default()
        .apply_overrides("fab=fancy")
        .unwrap_err();
    assert_eq!(
        err,
        "override `fab`: fancy is not standard, advanced or hdi"
    );
    for tier in FabTemplate::ALL {
        assert_eq!(FabTemplate::from_token(tier.token()), Some(tier));
    }
}

/// #2868: a project stores its own dimension defaults and reads them back;
/// out-of-range ones are kept verbatim and not applied.
#[test]
fn a_project_stores_its_dimension_defaults() {
    use crate::pcb_dimension_style::{ArrowStyle, DimensionDefaults};
    let mut data = blank_project();
    let mut dimension = DimensionDefaults::default();
    dimension.style.arrow_style = ArrowStyle::Filled;
    dimension.style.suffix = " ref".into();
    data.project_defaults.dimension = Some(dimension.clone());
    let bytes = encode_project(&data).unwrap();
    let back = parse_project_bytes(std::path::Path::new("P.snxprj"), &bytes).unwrap();
    assert_eq!(back.project_defaults.dimension, Some(dimension));

    let mut wild = data.clone();
    if let Some(d) = wild.project_defaults.dimension.as_mut() {
        d.line_width = -1.0;
    }
    let bytes = encode_project(&wild).unwrap();
    let back = parse_project_bytes(std::path::Path::new("P.snxprj"), &bytes).unwrap();
    assert!(back.project_defaults.dimension.is_none());
    assert!(back.has_unreadable_project_defaults());
}
