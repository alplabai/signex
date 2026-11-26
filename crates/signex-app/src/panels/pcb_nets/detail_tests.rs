//! The net detail's pad descriptors and the page's fit at the dock's usual
//! height (#3001, mockup row 106).

use signex_types::net_class::NetClass;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::*;

/// The PCB dock panel at its default size in the audit captures.
const DOCK: iced::Size = iced::Size::new(240.0, 770.0);

/// A part with through-hole pads, each `(number, net, net name)`.
fn part(id: u128, reference: &str, value: &str, pads: &[(&str, u32, &str)]) -> serde_json::Value {
    let pads: Vec<serde_json::Value> = pads
        .iter()
        .enumerate()
        .map(|(i, (number, net, name))| {
            serde_json::json!({
                "uuid": Uuid::from_u128(id * 100 + i as u128), "number": number,
                "pad_type": "thru", "shape": "circle",
                "position": {"x": i as f64 * 2.54, "y": 0.0}, "size": {"x": 1.6, "y": 1.6},
                "drill": {"diameter": 0.8},
                "layers": ["TopCopper", "BottomCopper"],
                "net": {"number": net, "name": name}
            })
        })
        .collect();
    serde_json::json!({
        "uuid": Uuid::from_u128(id), "reference": reference, "value": value,
        "position": {"x": 0.0, "y": id as f64}, "layer": "TopCopper", "pads": pads
    })
}

/// Five nets like the audit's supply board: GND on nine pads of eight
/// parts, with tracks on two layers and a via.
fn board() -> PcbBoard {
    const GND: (u32, &str) = (2, "GND");
    let g = |number: &'static str| (number, GND.0, GND.1);
    let track = |id: u128, layer: &str, length: f64| {
        serde_json::json!({
            "uuid": Uuid::from_u128(id), "start": {"x": 0.0, "y": 0.0},
            "end": {"x": length, "y": 0.0}, "width": 0.25, "layer": layer, "net": GND.0
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "nets": [
            {"number": 1, "name": "VIN"}, {"number": 2, "name": "GND"},
            {"number": 3, "name": "+12V"}, {"number": 4, "name": "+5V"},
            {"number": 5, "name": "LED_A"},
        ],
        "footprints": [
            part(10, "J1", "12V IN", &[("1", 3, "+12V"), g("2"), g("3")]),
            part(11, "C1", "10uF", &[("1", 1, "VIN"), g("2")]),
            part(12, "U1", "7805", &[("1", 1, "VIN"), g("2"), ("3", 4, "+5V")]),
            part(13, "C2", "22uF", &[("1", 4, "+5V"), g("2")]),
            part(14, "C3", "", &[g("1"), ("2", 4, "+5V")]),
            part(15, "C4", "100nF", &[g("1"), ("2", 1, "VIN")]),
            part(16, "J2", "5V OUT", &[("1", 4, "+5V"), g("2")]),
            part(17, "D2", "LED", &[("1", 5, "LED_A"), g("2")]),
        ],
        "segments": [
            track(2, "TopCopper", 2.0),
            track(3, "TopCopper", 1.5),
            track(4, "BottomCopper", 0.37),
        ],
        "vias": [{"uuid": Uuid::from_u128(5), "position": {"x": 0.0, "y": 0.0}, "net": GND.0}],
    }))
    .expect("a board")
}

fn ctx(mode: PcbPanelMode) -> PanelContext {
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_state.panel_ctx.clone();
    ctx.pcb_nets = Some(PcbNetsSummary::of(&board()));
    ctx.pcb_layers.board = Some("board.snxpcb".into());
    ctx.pcb_panel.mode = mode;
    ctx
}

fn texts(ctx: &PanelContext) -> Vec<crate::layout_probe::LaidOutText> {
    crate::layout_probe::lay_out_texts(view(ctx), DOCK)
        .into_iter()
        .map(|mut t| {
            t.text = t.text.replace(['\u{2068}', '\u{2069}'], "");
            t
        })
        .collect()
}

/// The bottom edge of the text `want`.
fn bottom(texts: &[crate::layout_probe::LaidOutText], want: &str) -> f32 {
    let t = crate::layout_probe::find(texts, want);
    t.bounds.y + t.bounds.height
}

/// Headless: each pad carries its part's value, trimmed; a part with no
/// value leaves it empty.
#[test]
fn a_pad_carries_the_value_of_its_part() {
    let summary = PcbNetsSummary::of(&board());
    let gnd = summary.nets.iter().find(|n| n.name == "GND").unwrap();
    assert_eq!(gnd.pads.len(), 9);
    let value = |part: &str| {
        gnd.pads
            .iter()
            .find(|p| p.part == part)
            .map(|p| p.part_value.as_str())
    };
    assert_eq!(value("U1"), Some("7805"));
    assert_eq!(value("J1"), Some("12V IN"));
    assert_eq!(value("C3"), Some(""));
}

/// Mockup row 106: a pad row reads `U1-2`, its descriptor and its layer,
/// and at the dock's usual height the whole detail, down to Vias, shows
/// above the foot.
#[test]
fn the_net_detail_describes_each_pad_and_fits_the_dock() {
    let mut ctx = ctx(PcbPanelMode::Nets);
    ctx.pcb_panel.selected = Some("GND".to_owned());
    let texts = texts(&ctx);
    let shows = |want: &str| texts.iter().any(|t| t.text == want);
    for want in [
        "U1-2",
        "7805",
        "C1-2",
        "10uF",
        "Multi",
        "Tracks and vias",
        "Vias",
    ] {
        assert!(shows(want), "{want}: {texts:?}");
    }
    let pad = crate::layout_probe::find(&texts, "U1-2");
    let descriptor = crate::layout_probe::find(&texts, "7805");
    assert!(
        (pad.bounds.center_y() - descriptor.bounds.center_y()).abs() < 1.0
            && descriptor.bounds.x > pad.bounds.x,
        "the descriptor sits right of its pad: {pad:?} {descriptor:?}"
    );
    let foot = texts.last().expect("the foot");
    assert!(
        bottom(&texts, "Vias") <= foot.bounds.y,
        "Vias above the foot: {texts:?}"
    );
}

/// Mockup row 106: a picked class shows its rules and member nets under
/// the class list, at the dock's usual height.
#[test]
fn a_picked_class_shows_its_rules_and_members_in_the_dock() {
    let mut ctx = ctx(PcbPanelMode::NetClasses);
    let power = NetClass {
        nets: vec!["GND".into(), "VIN".into()],
        trace_width: 0.5,
        clearance: 0.2,
        ..NetClass::named("Power")
    };
    ctx.net_classes.board = Some(crate::panels::BoardNetClassView {
        in_project: true,
        classes: vec![power],
        nets: vec!["GND".into(), "VIN".into()],
        ..Default::default()
    });
    ctx.pcb_panel.class = Some("Power".to_owned());
    let texts = texts(&ctx);
    for want in [
        "Rules · Power",
        "Width / Clear.",
        "Member nets",
        "GND",
        "VIN",
    ] {
        assert!(texts.iter().any(|t| t.text == want), "{want}: {texts:?}");
    }
    let foot = texts.last().expect("the foot");
    assert!(
        bottom(&texts, "Member nets") <= foot.bounds.y,
        "the members show above the foot: {texts:?}"
    );
}
