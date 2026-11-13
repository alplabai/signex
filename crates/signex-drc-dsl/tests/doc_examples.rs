//! The example block of `docs/DRC_RULES.md`, read from the file, compiles
//! and behaves as the page says (#2307), so the page and the code cannot
//! drift apart.

use signex_drc::{DrcRules, ViolationKind, run};
use signex_types::net_class::NetClass;
use signex_types::pcb::{
    Footprint, NetDef, Pad, PadNet, PadShape, PadType, PcbBoard, Point, Segment, Via, ViaType,
};
use uuid::Uuid;

const PAGE: &str = include_str!("../../../docs/DRC_RULES.md");

/// The first `text` block after the "Examples" heading.
fn examples() -> &'static str {
    let after = PAGE.split("### Examples").nth(1).expect("examples heading");
    let block = after.split("```text\n").nth(1).expect("text block");
    block.split("```").next().expect("block end")
}

const NETS: [(u32, &str); 8] = [
    (1, "PWR_MAIN"),
    (2, "PWR_AUX"),
    (3, "SIG_A"),
    (4, "HV_IN"),
    (5, "SIG_B"),
    (6, "DDR_0"),
    (7, "GND"),
    (8, "SIG_C"),
];

fn track(n: u128, x: f64, y: f64, net: u32, width: f64) -> Segment {
    Segment {
        uuid: Uuid::from_u128(n),
        start: Point::new(x - 5.0, y),
        end: Point::new(x + 5.0, y),
        width,
        layer: "TopCopper".into(),
        net,
        unknown_columns: Default::default(),
    }
}

/// A footprint holding one 1 mm square pad of `net` at `(x, 0)`.
fn pad_at(n: u128, x: f64, net: u32) -> Footprint {
    let pad = Pad {
        uuid: Uuid::from_u128(n),
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(0.0, 0.0),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: Some(PadNet {
            number: net,
            name: NETS[net as usize - 1].1.into(),
        }),
        roundrect_ratio: 0.0,
        rotation: 0.0,
        net_locked: false,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    Footprint {
        uuid: Uuid::from_u128(n + 1),
        reference: format!("J{n}"),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(x, 0.0),
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
    }
}

/// One scene per example, 20 mm apart (tracks 0.25 mm wide unless said):
/// - x 0: a `PWR_AUX` pad, a `PWR_MAIN` track 0.7 mm above it, a `SIG_A`
///   track 0.7 mm above that;
/// - x 20: `HV_IN` and `SIG_B` tracks 0.5 mm apart;
/// - x 40: a 0.05 mm `DDR_0` track on top;
/// - x 60: a `GND` via 0.4 mm across with a 0.2 mm drill;
/// - x 80: a `GND` pad and a `SIG_C` track 0.22 mm above it.
fn board() -> PcbBoard {
    PcbBoard {
        sketch: None,
        uuid: Uuid::nil(),
        version: 1,
        generator: String::new(),
        thickness: 1.6,
        outline: Vec::new(),
        layers: Vec::new(),
        setup: None,
        nets: NETS
            .iter()
            .map(|&(number, name)| NetDef {
                number,
                name: name.into(),
            })
            .collect(),
        footprints: vec![pad_at(10, 0.0, 2), pad_at(20, 80.0, 7)],
        segments: vec![
            track(1, 0.0, 1.325, 1, 0.25),
            track(2, 0.0, 2.275, 3, 0.25),
            track(3, 20.0, 0.0, 4, 0.25),
            track(4, 20.0, 0.75, 5, 0.25),
            track(5, 40.0, 0.0, 6, 0.05),
            track(6, 80.0, 0.845, 8, 0.25),
        ],
        vias: vec![Via {
            uuid: Uuid::from_u128(30),
            position: Point::new(60.0, 0.0),
            diameter: 0.4,
            drill: 0.2,
            layers: vec!["TopCopper".into(), "BottomCopper".into()],
            net: 7,
            via_type: ViaType::Through,
            unknown_columns: Default::default(),
        }],
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

fn rules_of(text: &str) -> DrcRules {
    let compiled = signex_drc_dsl::compile(text);
    assert!(
        compiled.diagnostics.is_empty(),
        "{:?}",
        compiled.diagnostics
    );
    let b = board();
    DrcRules::for_board(&b, &classes())
        .unwrap()
        .with_user_rules(&b, &classes(), compiled.rules)
}

#[test]
fn the_documented_examples_compile_and_behave_as_documented() {
    let names: Vec<_> = signex_drc_dsl::compile(examples())
        .rules
        .into_iter()
        .map(|r| r.name)
        .collect();
    assert_eq!(
        names,
        ["pwr_main", "hv_spacing", "ddr_width", "vias", "track_pad"]
    );
    let rules = rules_of(examples());
    // `pwr_main` lowers its own class for PWR_MAIN, as documented; nothing
    // else lowers a class (`track_pad` is for SIG_* nets only), and every
    // rule's scope matches a net.
    assert_eq!(
        rules.notes,
        ["DRC rule 'pwr_main' lowers net class 'Power' clearance 1 \u{2192} 0.6 mm"]
    );
    assert!(rules.remarks.is_empty(), "{:?}", rules.remarks);

    let report = run(&board(), &rules);
    let id = |name: &str| Some(format!("user::{name}"));
    let mut found: Vec<_> = report
        .violations
        .iter()
        .map(|v| (v.kind, v.required_nm, v.rule.clone()))
        .collect();
    found.sort();
    let mut want = vec![
        // PWR_AUX's pad keeps its class's 1.0 mm against PWR_MAIN's track;
        // PWR_MAIN's and SIG_A's tracks, 0.7 mm apart, pass on PWR_MAIN's
        // 0.6 mm.
        (ViolationKind::Clearance, 1_000_000, None),
        // A SIG_* track keeps 0.25 mm from a pad.
        (ViolationKind::Clearance, 250_000, id("track_pad")),
        // An HV net keeps 0.8 mm.
        (ViolationKind::Clearance, 800_000, id("hv_spacing")),
        // A DDR_* track on an outer layer is at least 0.1 mm wide.
        (ViolationKind::TrackWidth, 100_000, id("ddr_width")),
        // Every via is at least 450 µm across (its 0.2 mm drill passes).
        (ViolationKind::ViaDiameter, 450_000, id("vias")),
    ];
    want.sort();
    assert_eq!(found, want, "{report:?}");
}

/// The page's aside: without its `SIG_*` condition, `track_pad` would also
/// cover Power's track-to-pad pairs and lower that class, and Run DRC says
/// so.
#[test]
fn an_unscoped_track_pad_rule_would_lower_power_as_the_page_says() {
    let unscoped = examples().replace(" when net matches \"SIG_*\"", "");
    assert_ne!(unscoped, examples(), "the condition is in the example");
    let rules = rules_of(&unscoped);
    assert!(
        rules.notes.contains(
            &"DRC rule 'track_pad' lowers net class 'Power' clearance 1 \u{2192} 0.25 mm"
                .to_string()
        ),
        "{:?}",
        rules.notes
    );
}
