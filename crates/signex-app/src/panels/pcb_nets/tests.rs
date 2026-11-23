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
