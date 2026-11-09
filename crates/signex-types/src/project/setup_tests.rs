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
    assert_eq!(row(SetupProfile::Hobby), (2, 1.6, 0.2, 0.25, 0.6, 0.3));
    assert_eq!(row(SetupProfile::Standard), (4, 1.6, 0.15, 0.2, 0.5, 0.25));
    assert_eq!(row(SetupProfile::Hdi), (6, 1.2, 0.1, 0.1, 0.3, 0.15));
    assert_eq!(SetupProfile::Custom.pcb(11).copper_layers, 11);
    assert_eq!(ProjectSetup::default().profile, SetupProfile::Standard);
    assert_eq!(ProjectSetup::default().ai_mode, AiMode::Assistant);
}

#[test]
fn the_stepper_switches_to_custom_and_clamps() {
    let mut setup = ProjectSetup::default();
    setup.set_layers(40);
    assert_eq!(
        (setup.profile, setup.pcb.copper_layers),
        (SetupProfile::Custom, 32)
    );
    setup.set_layers(0);
    assert_eq!(setup.pcb.copper_layers, 1);
    setup.pick_profile(SetupProfile::Hdi);
    assert_eq!(
        (setup.profile, setup.pcb.copper_layers),
        (SetupProfile::Hdi, 6)
    );
}

#[test]
fn a_fab_class_fills_the_rules_and_a_profile_pick_clears_it() {
    let mut setup = ProjectSetup::default();
    setup.apply_fab(FabTemplate::Advanced);
    assert_eq!(setup.fab, Some(FabTemplate::Advanced));
    assert_eq!(setup.pcb.clearance_mm, 0.1);
    assert_eq!(setup.pcb.copper_layers, 4, "layers are not part of a class");
    setup.pick_profile(SetupProfile::Hobby);
    assert_eq!((setup.fab, setup.pcb.clearance_mm), (None, 0.2));
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
    let mut setup = ProjectSetup::from_profile(SetupProfile::Hobby);
    setup
        .apply_overrides("layers=6, clearance=0.12,sheet=A4,units=mil,designators=din40719_2")
        .unwrap();
    assert_eq!(setup.pcb.copper_layers, 6);
    assert_eq!(setup.profile, SetupProfile::Custom);
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
    assert_eq!(back.project_defaults.ai_mode, Some(AiMode::Assistant));
    assert_eq!(back.project_defaults.pcb.unwrap().copper_layers, 6);
}

#[test]
fn a_new_board_inherits_the_rules_and_the_layer_stack() {
    let mut data = blank_project();
    assert!(data.new_board().is_none(), "no defaults, no override");
    let mut setup = ProjectSetup::from_profile(SetupProfile::Hobby);
    setup.set_layers(4);
    data.project_defaults = setup.defaults();
    let board = data.new_board().unwrap();
    let rules = board.setup.as_ref().unwrap();
    assert_eq!(
        (rules.clearance, rules.trace_width, rules.via_drill),
        (0.2, 0.25, 0.3)
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
    assert_eq!(
        ProjectSetup::default().sheet_line(),
        ("A3 landscape".to_string(), "mm".to_string(), 2.54)
    );
}
