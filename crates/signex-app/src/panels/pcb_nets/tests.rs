//! The PCB panel's model and its Nets view (mockup row 106).

use signex_i18n::I18n;
use signex_types::net_class::NetClass;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

use super::detail::{layer_text, pad_layer};
use super::model::{class_of, net_chips};
use super::*;

fn row(number: u32, name: &str, unrouted: usize) -> PcbNetRow {
    PcbNetRow {
        number,
        name: name.to_owned(),
        nodes: 2,
        length_mm: 1.0,
        unrouted,
        pads: Vec::new(),
        layers: Vec::new(),
        vias: 0,
    }
}

fn summary() -> PcbNetsSummary {
    PcbNetsSummary {
        nets: vec![
            row(1, "GND", 0),
            row(2, "SPI_CLK", 1),
            row(3, "SPI_MOSI", 0),
        ],
        pads: 6,
        unrouted_known: true,
        ..PcbNetsSummary::default()
    }
}

fn spi_class() -> NetClass {
    let mut class = NetClass::named("HighSpeed");
    class.patterns.push("SPI_*".to_owned());
    class
}

fn names(ui: &PcbPanelUi, classes: &[NetClass]) -> Vec<String> {
    let summary = summary();
    ui.shown(&summary, classes, I18n::en_us_ref())
        .into_iter()
        .map(|net| net.name.clone())
        .collect()
}

#[test]
fn the_filter_ignores_case_and_combines_with_the_chip() {
    let classes = [spi_class()];
    let mut ui = PcbPanelUi::default();
    ui.apply(PcbPanelMsg::FilterChanged(" spi ".to_owned()));
    assert_eq!(names(&ui, &classes), ["SPI_CLK", "SPI_MOSI"]);
    ui.apply(PcbPanelMsg::ChipPicked(PcbNetChip::Unrouted));
    assert_eq!(names(&ui, &classes), ["SPI_CLK"]);
}

#[test]
fn a_class_chip_keeps_the_nets_its_class_matches_first() {
    let classes = [spi_class()];
    assert_eq!(class_of(&classes, "SPI_CLK"), Some("HighSpeed"));
    assert_eq!(class_of(&classes, "GND"), None);
    let mut ui = PcbPanelUi::default();
    ui.apply(PcbPanelMsg::ChipPicked(PcbNetChip::Class(
        "HighSpeed".to_owned(),
    )));
    assert_eq!(names(&ui, &classes), ["SPI_CLK", "SPI_MOSI"]);
}

#[test]
fn a_chip_that_stands_for_nothing_falls_back_to_all() {
    let mut ui = PcbPanelUi {
        chip: PcbNetChip::Class("Gone".to_owned()),
        ..PcbPanelUi::default()
    };
    assert_eq!(names(&ui, &[]).len(), 3);
    ui.chip = PcbNetChip::Unrouted;
    let unread = PcbNetsSummary {
        unrouted_known: false,
        ..summary()
    };
    assert_eq!(ui.chip_in_force(&unread, &[]), PcbNetChip::All);
}

#[test]
fn the_chips_count_all_unrouted_and_each_class_with_nets() {
    let classes = [spi_class(), NetClass::named("Empty")];
    assert_eq!(
        net_chips(&summary(), &classes),
        [
            (PcbNetChip::All, 3),
            (PcbNetChip::Unrouted, 1),
            (PcbNetChip::Class("HighSpeed".to_owned()), 2),
        ]
    );
    let unread = PcbNetsSummary {
        unrouted_known: false,
        ..summary()
    };
    assert!(
        !net_chips(&unread, &[])
            .iter()
            .any(|(chip, _)| *chip == PcbNetChip::Unrouted)
    );
}

#[test]
fn the_highlight_follows_the_selection() {
    assert_eq!(
        highlight_toggle(None, None, Some("A")).as_deref(),
        Some("A")
    );
    assert_eq!(highlight_toggle(Some("A"), None, Some("A")), None);
    assert_eq!(
        highlight_toggle(Some("A"), Some("A"), Some("B")).as_deref(),
        Some("B")
    );
    assert_eq!(
        highlight_toggle(Some("A"), Some("A"), None).as_deref(),
        Some("A")
    );
    assert_eq!(highlight_toggle(Some("B"), Some("A"), None), None);
}

fn board_with_tracks() -> PcbBoard {
    let track = |id: u128, layer: &str, length: f64| {
        serde_json::json!({
            "uuid": Uuid::from_u128(id), "start": {"x": 0.0, "y": 0.0},
            "end": {"x": length, "y": 0.0}, "width": 0.25, "layer": layer, "net": 1
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "nets": [{"number": 1, "name": "CLK"}],
        "segments": [
            track(2, "BottomCopper", 2.0),
            track(3, "TopCopper", 3.0),
            track(4, "TopCopper", 4.0),
        ],
        "vias": [{"uuid": Uuid::from_u128(5), "position": {"x": 0.0, "y": 0.0}, "net": 1}],
    }))
    .expect("a board")
}

#[test]
fn the_detail_sums_tracks_per_layer_top_first_and_counts_vias() {
    let summary = PcbNetsSummary::of(&board_with_tracks());
    let net = &summary.nets[0];
    let layers: Vec<(&str, usize, f64)> = net
        .layers
        .iter()
        .map(|l| (l.layer.as_str(), l.tracks, l.length_mm))
        .collect();
    assert_eq!(layers, [("TopCopper", 2, 7.0), ("BottomCopper", 1, 2.0)]);
    assert_eq!(net.length_mm, 9.0);
    assert_eq!(net.vias, 1);
}

#[test]
fn a_drilled_or_multi_layer_pad_reads_multi() {
    let i18n = I18n::en_us_ref();
    let pad = |layers: &[&str], is_through| PcbNetPad {
        part: "U1".to_owned(),
        pad: "1".to_owned(),
        layers: layers.iter().map(|l| (*l).to_owned()).collect(),
        is_through,
    };
    let top = layer_text(i18n, "TopCopper");
    assert_eq!(
        pad_layer(i18n, &pad(&["TopCopper"], false)).as_str(),
        top.as_str()
    );
    assert_eq!(
        pad_layer(i18n, &pad(&["TopCopper"], true)).as_str(),
        "Multi"
    );
    let both = pad(&["TopCopper", "BottomCopper"], false);
    assert_eq!(pad_layer(i18n, &both).as_str(), "Multi");
}

#[test]
fn a_click_on_the_selected_net_deselects_it() {
    let mut ui = PcbPanelUi::default();
    ui.apply(PcbPanelMsg::NetClicked("GND".to_owned()));
    assert_eq!(ui.selected.as_deref(), Some("GND"));
    ui.apply(PcbPanelMsg::NetClicked("GND".to_owned()));
    assert_eq!(ui.selected, None);
}

// pcb-nets-modes: Differential Pairs, xSignals, Polygons and the Net
// Classes list (mockup row 106).

fn board_with_pairs_and_pours() -> PcbBoard {
    let track = |id: u128, net: u32, length: f64| {
        serde_json::json!({
            "uuid": Uuid::from_u128(id), "start": {"x": 0.0, "y": 0.0},
            "end": {"x": length, "y": 0.0}, "width": 0.25, "layer": "TopCopper", "net": net
        })
    };
    let pour = |id: u128, shelved: bool| {
        serde_json::json!({
            "uuid": Uuid::from_u128(id), "net": 3, "net_name": "GND", "layer": "BottomCopper",
            "fill_type": "pour", "priority": 1, "shelved": shelved,
            "outline": [{"x": 0.0, "y": 0.0}, {"x": 5.0, "y": 0.0}, {"x": 5.0, "y": 5.0}]
        })
    };
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "nets": [
            {"number": 1, "name": "USB_P"},
            {"number": 2, "name": "USB_N"},
            {"number": 3, "name": "GND"},
        ],
        "segments": [track(2, 1, 3.0), track(3, 2, 2.5)],
        "zones": [
            pour(4, false),
            pour(5, true),
            {"uuid": Uuid::from_u128(6), "net": 3, "net_name": "GND", "layer": "TopCopper",
             "fill_type": "solid", "outline": []},
        ],
    }))
    .expect("a board")
}

fn modes_summary() -> PcbNetsSummary {
    let board = board_with_pairs_and_pours();
    PcbNetsSummary::of(&board).with_pairs(&board, &[])
}

#[test]
fn a_pair_is_found_once_with_its_shared_name_lengths_and_skew() {
    let summary = modes_summary();
    assert_eq!(summary.pairs.len(), 1, "{:?}", summary.pairs);
    let pair = &summary.pairs[0];
    assert_eq!(
        (
            pair.name.as_str(),
            pair.positive.as_str(),
            pair.negative.as_str()
        ),
        ("USB", "USB_P", "USB_N")
    );
    assert_eq!((pair.positive_mm, pair.negative_mm), (3.0, 2.5));
    assert_eq!(pair.skew_mm(), 0.5);
}

#[test]
fn a_class_with_a_pair_gap_and_two_nets_makes_a_pair() {
    let board: PcbBoard = serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(1),
        "nets": [{"number": 1, "name": "USB_DP"}, {"number": 2, "name": "USB_DM"}],
    }))
    .expect("a board");
    let mut usb = NetClass::named("USB");
    usb.patterns.push("USB_D*".to_owned());
    usb.diff_pair_gap = 0.175;
    let summary = PcbNetsSummary::of(&board).with_pairs(&board, &[usb]);
    let names: Vec<&str> = summary.pairs.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["USB_D"]);
    let none = PcbNetsSummary::of(&board).with_pairs(&board, &[]);
    assert!(none.pairs.is_empty(), "no suffix pair without the class");
}

#[test]
fn the_pours_are_the_boards_polygon_pours_and_the_chip_keeps_the_shelved() {
    let summary = modes_summary();
    let pours: Vec<(u128, bool)> = summary
        .pours
        .iter()
        .map(|p| (p.zone.as_u128(), p.shelved))
        .collect();
    assert_eq!(pours, [(4, false), (5, true)], "a solid region is no pour");
    let mut ui = PcbPanelUi::default();
    assert_eq!(ui.shown_pours(&summary).len(), 2);
    ui.apply(PcbPanelMsg::PourChipPicked(PcbPourChip::NeedsAttention));
    let shown: Vec<u128> = ui
        .shown_pours(&summary)
        .iter()
        .map(|p| p.zone.as_u128())
        .collect();
    assert_eq!(shown, [5]);
}

#[test]
fn each_mode_counts_what_it_lists() {
    let summary = modes_summary();
    let classes = [spi_class(), NetClass::named("Power")];
    let counts: Vec<usize> = PcbPanelMode::ALL
        .iter()
        .map(|mode| mode.count(&summary, &classes))
        .collect();
    assert_eq!(counts, [3, 2, 1, 0, 2]);
    let i18n = I18n::en_us_ref();
    let label = super::mode_label(i18n, PcbPanelMode::DiffPairs, 1);
    assert_eq!(
        label.replace(['\u{2068}', '\u{2069}'], ""),
        "Differential Pairs · 1"
    );
}

#[test]
fn a_click_on_a_selected_class_pair_or_pour_deselects_it() {
    let mut ui = PcbPanelUi::default();
    let pour = Uuid::from_u128(4);
    for msg in [
        PcbPanelMsg::ClassClicked("Power".to_owned()),
        PcbPanelMsg::PairClicked("USB".to_owned()),
        PcbPanelMsg::PourClicked(pour),
    ] {
        ui.apply(msg);
    }
    assert_eq!(
        (ui.class.as_deref(), ui.pair.as_deref(), ui.pour),
        (Some("Power"), Some("USB"), Some(pour))
    );
    let summary = modes_summary();
    assert_eq!(
        ui.selected_pair(&summary).map(|p| p.negative.as_str()),
        Some("USB_N")
    );
    ui.apply(PcbPanelMsg::PairClicked("USB".to_owned()));
    ui.apply(PcbPanelMsg::PourClicked(pour));
    ui.apply(PcbPanelMsg::ClassClicked("Power".to_owned()));
    assert_eq!((ui.class, ui.pair, ui.pour), (None, None, None));
}

const PANEL: iced::Size = iced::Size::new(640.0, 900.0);

fn panel_ctx(mode: PcbPanelMode) -> PanelContext {
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_state.panel_ctx.clone();
    ctx.pcb_nets = Some(modes_summary());
    ctx.pcb_layers.board = Some("board.snxpcb".into());
    ctx.pcb_panel.mode = mode;
    ctx
}

fn panel_texts(ctx: &PanelContext) -> Vec<String> {
    crate::layout_probe::lay_out_texts(view(ctx), PANEL)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

fn shows(texts: &[String], want: &str) -> bool {
    texts.iter().any(|t| t == want)
}

#[test]
fn the_pairs_mode_lists_the_pair_over_its_detail() {
    let mut ctx = panel_ctx(PcbPanelMode::DiffPairs);
    let texts = panel_texts(&ctx);
    for want in ["USB", "skew 0.50", "P USB_P 3.00", "N USB_N 2.50", "1 pair"] {
        assert!(shows(&texts, want), "{want}: {texts:?}");
    }
    assert!(!shows(&texts, "Positive"), "no detail before a click");
    let sent = crate::layout_probe::messages_on_click(view(&ctx), PANEL, "USB");
    assert!(
        matches!(sent.as_slice(), [PanelMsg::PcbPanel(PcbPanelMsg::PairClicked(p))] if p == "USB"),
        "{sent:?}"
    );
    ctx.pcb_panel.pair = Some("USB".to_owned());
    let texts = panel_texts(&ctx);
    for want in [
        "Positive",
        "USB_P",
        "Routed",
        "P 3.00 · N 2.50",
        "Skew",
        "0.50 mm",
    ] {
        assert!(shows(&texts, want), "{want}: {texts:?}");
    }
}

#[test]
fn the_xsignals_mode_says_the_board_has_none() {
    let texts = panel_texts(&panel_ctx(PcbPanelMode::XSignals));
    assert!(
        shows(&texts, "This board has no xSignals yet."),
        "{texts:?}"
    );
    assert!(shows(&texts, "0 xSignals"), "{texts:?}");
}

#[test]
fn the_polygons_mode_lists_the_pours_and_runs_the_copper_card_actions() {
    use crate::panels::PcbPrimitiveMsg;
    use crate::panels::pcb_primitive_properties::PourMsg;
    let ctx = panel_ctx(PcbPanelMode::Polygons);
    let texts = panel_texts(&ctx);
    for want in [
        "Repour all",
        "Open Polygon Manager",
        "Needs attention",
        "Poured",
        "Shelved",
        "Unshelve",
        "2 pours",
    ] {
        assert!(shows(&texts, want), "{want}: {texts:?}");
    }
    let sent = crate::layout_probe::messages_on_click(view(&ctx), PANEL, "Repour all");
    assert!(
        matches!(
            sent.as_slice(),
            [PanelMsg::PcbPrimitive(PcbPrimitiveMsg::Pour(PourMsg::RepourAllPressed { board }))]
                if board.as_path() == std::path::Path::new("board.snxpcb")
        ),
        "{sent:?}"
    );
    let sent = crate::layout_probe::messages_on_click(view(&ctx), PANEL, "Unshelve");
    assert!(
        matches!(
            sent.as_slice(),
            [PanelMsg::PcbPrimitive(PcbPrimitiveMsg::Pour(PourMsg::ShelveToggled {
                target,
                shelved: false,
            }))] if target.items.len() == 1
        ),
        "{sent:?}"
    );
    let sent = crate::layout_probe::messages_on_click(view(&ctx), PANEL, "Open Polygon Manager");
    assert!(sent.is_empty(), "no Polygon Manager tab yet: {sent:?}");
}

fn class_table() -> super::super::BoardNetClassView {
    let power = NetClass {
        nets: vec!["GND".into()],
        patterns: vec!["USB_*".into()],
        trace_width: 0.5,
        clearance: 0.3,
        ..NetClass::named("Power")
    };
    super::super::BoardNetClassView {
        in_project: true,
        classes: vec![power],
        nets: vec!["GND".into(), "USB_N".into(), "USB_P".into()],
        ..Default::default()
    }
}

#[test]
fn the_net_classes_mode_is_a_class_list_over_the_selected_class() {
    let mut ctx = panel_ctx(PcbPanelMode::NetClasses);
    ctx.net_classes.board = Some(class_table());
    let texts = panel_texts(&ctx);
    for want in [
        "Add Class",
        "The first matching class wins.",
        "Power",
        "0.500 / 0.300",
        "1 + 1 pattern",
    ] {
        assert!(shows(&texts, want), "{want}: {texts:?}");
    }
    assert!(!shows(&texts, "Member nets"), "no detail before a click");
    let sent = crate::layout_probe::messages_on_click(view(&ctx), PANEL, "Power");
    assert!(
        matches!(sent.as_slice(), [PanelMsg::PcbPanel(PcbPanelMsg::ClassClicked(c))] if c == "Power"),
        "{sent:?}"
    );
    ctx.pcb_panel.class = Some("Power".to_owned());
    let texts = panel_texts(&ctx);
    for want in [
        "Rules · Power",
        "Width / Clear.",
        "Via",
        "Impedance",
        "Member nets",
        "GND",
        "USB_*",
        "2 matches",
    ] {
        assert!(shows(&texts, want), "{want}: {texts:?}");
    }
}

#[test]
fn a_class_summary_reads_unset_rules_as_a_dash_and_shows_vias_once_set() {
    let i18n = I18n::en_us_ref();
    let clean = |s: signex_i18n::Localized| s.into_string().replace(['\u{2068}', '\u{2069}'], "");
    let mut class = NetClass {
        clearance: 0.3,
        ..NetClass::named("Analog")
    };
    let summary = super::super::board_net_classes::rule_summary;
    assert_eq!(clean(summary(i18n, &class)), "— / 0.300");
    class.via_diameter = 0.8;
    class.via_drill = 0.4;
    assert_eq!(clean(summary(i18n, &class)), "— / 0.300 · via 0.800/0.400");
    let count = super::super::board_net_classes::member_count;
    class.nets = vec!["A".into(), "B".into()];
    assert_eq!(clean(count(i18n, &class)), "2 nets");
}
