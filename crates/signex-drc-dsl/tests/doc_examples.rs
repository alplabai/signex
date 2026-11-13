//! The example block of `docs/DRC_RULES.md`, read from the file, compiles
//! and behaves as the page says (#2307), so the page and the code cannot
//! drift apart.

use signex_drc::{DrcRules, ViolationKind, run};
use signex_types::net_class::NetClass;
use signex_types::pcb::{
    Footprint, NetDef, Pad, PadNet, PadShape, PadType, PcbBoard, Point, Segment,
};
use uuid::Uuid;

const PAGE: &str = include_str!("../../../docs/DRC_RULES.md");

/// The first `text` block after the "Examples" heading.
fn examples() -> &'static str {
    let after = PAGE.split("### Examples").nth(1).expect("examples heading");
    let block = after.split("```text\n").nth(1).expect("text block");
    block.split("```").next().expect("block end")
}

fn track(n: u128, y: f64, net: u32) -> Segment {
    Segment {
        uuid: Uuid::from_u128(n),
        start: Point::new(-5.0, y),
        end: Point::new(5.0, y),
        width: 0.25,
        layer: "TopCopper".into(),
        net,
        unknown_columns: Default::default(),
    }
}

/// A 1 mm square `PWR_AUX` pad at the origin; a `PWR_MAIN` track 0.7 mm
/// above it; a `SIG_A` track 0.7 mm above that.
fn board() -> PcbBoard {
    let pad = Pad {
        uuid: Uuid::from_u128(10),
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(0.0, 0.0),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: Some(PadNet {
            number: 2,
            name: "PWR_AUX".into(),
        }),
        roundrect_ratio: 0.0,
        rotation: 0.0,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    let footprint = Footprint {
        uuid: Uuid::from_u128(11),
        reference: "J1".into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(0.0, 0.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        schematic_link: None,
        library_ref: None,
        pads: vec![pad],
        graphics: Vec::new(),
        properties: Vec::new(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    PcbBoard {
        sketch: None,
        uuid: Uuid::nil(),
        version: 1,
        generator: String::new(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: [(1, "PWR_MAIN"), (2, "PWR_AUX"), (3, "SIG_A"), (4, "HV_IN")]
            .into_iter()
            .map(|(number, name)| NetDef {
                number,
                name: name.into(),
            })
            .collect(),
        footprints: vec![footprint],
        // Pad edge at 0.5 mm; tracks 0.25 mm wide, 0.7 mm gaps.
        segments: vec![track(1, 1.325, 1), track(2, 2.275, 3)],
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

fn classes() -> Vec<NetClass> {
    let class = |name: &str, glob: &str, mm: f64| {
        let mut c = NetClass::named(name);
        c.patterns = vec![glob.into()];
        c.clearance = mm;
        c
    };
    vec![class("Power", "PWR_*", 1.0), class("HV", "HV_*", 0.5)]
}

#[test]
fn the_documented_examples_compile_and_behave_as_documented() {
    let compiled = signex_drc_dsl::compile(examples());
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let names: Vec<_> = compiled.rules.iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        names,
        ["pwr_main", "hv_spacing", "ddr_width", "vias", "track_pad"]
    );

    let b = board();
    let rules = DrcRules::for_board(&b, &classes())
        .unwrap()
        .with_user_rules(&b, &classes(), compiled.rules);
    // `pwr_main` lowers its own class for PWR_MAIN, as documented; nothing
    // else lowers a class (`track_pad` is for SIG_* nets only).
    assert_eq!(
        rules.notes,
        ["DRC rule 'pwr_main' lowers net class 'Power' clearance 1 \u{2192} 0.6 mm"]
    );
    let report = run(&b, &rules);
    // PWR_MAIN's track and SIG_A's track, 0.7 mm apart, pass on PWR_MAIN's
    // 0.6 mm; PWR_AUX's pad keeps its class's 1.0 mm against PWR_MAIN.
    let found: Vec<_> = report
        .violations
        .iter()
        .map(|v| (v.kind, v.nets, v.required_nm, v.rule.clone()))
        .collect();
    assert_eq!(
        found,
        [(ViolationKind::Clearance, (2, 1), 1_000_000, None)],
        "{report:?}"
    );
}
