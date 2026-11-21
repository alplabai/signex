//! #2460: the board counters' texts, tones and anchors, and the popovers.

use std::time::{Duration, SystemTime};

use signex_i18n::{I18n, Localized};
use signex_types::theme::{ThemeId, theme_tokens};

use super::kit::{Tone, badge_tone, cycles, tone_colours};
use super::*;
use crate::styles::tokens::resolve;

fn plain(text: Localized) -> String {
    text.into_string().replace(['\u{2068}', '\u{2069}'], "")
}

fn at(secs: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
}

#[test]
fn the_drc_age_counts_whole_minutes() {
    assert_eq!(DrcAge::between(at(100), at(100)), DrcAge::JustNow);
    assert_eq!(DrcAge::between(at(100), at(159)), DrcAge::JustNow);
    assert_eq!(DrcAge::between(at(100), at(160)), DrcAge::Minutes(1));
    assert_eq!(DrcAge::between(at(100), at(100 + 125)), DrcAge::Minutes(2));
    // The clock went back: never a negative age.
    assert_eq!(DrcAge::between(at(100), at(40)), DrcAge::JustNow);
}

#[test]
fn the_drc_caption_says_when_it_ran_or_that_it_did_not() {
    let i18n = I18n::en_us();
    let caption = |drc| plain(counters::drc_caption(drc, &i18n));
    assert_eq!(caption(DrcTally::NotRun), "· not run yet");
    let ran = |age| DrcTally::Ran { violations: 3, age };
    assert_eq!(caption(ran(DrcAge::JustNow)), "· just now");
    assert_eq!(caption(ran(DrcAge::Minutes(2))), "· 2 min ago");
}

/// The mock's 38 of 46 reads 83%; a last unrouted connection never rounds
/// up to 100%.
#[test]
fn the_routed_percent_rounds_but_never_claims_done_early() {
    let tally = |routed, total| ConnectionTally {
        routed,
        total,
        unrouted: total - routed,
    };
    assert_eq!(tally(38, 46).percent(), 83);
    assert_eq!(tally(0, 46).percent(), 0);
    assert_eq!(tally(199, 200).percent(), 99);
    assert_eq!(tally(46, 46).percent(), 100);
    assert_eq!(tally(0, 0).percent(), 100);
}

/// A zero count is drawn neutral (sunken, muted), never hidden or coloured.
#[test]
fn a_zero_badge_is_neutral() {
    for tone in [Tone::Error, Tone::Warning, Tone::Accent] {
        assert_eq!(badge_tone(0, tone), Tone::Neutral);
        assert_eq!(badge_tone(3, tone), tone);
    }
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let t = theme_tokens(id);
        let r = resolve(&t);
        assert_eq!(
            tone_colours(Tone::Neutral, &t),
            (r.sunken, r.ink3),
            "{id:?}"
        );
        assert_eq!(tone_colours(Tone::Warning, &t).0, r.warn_soft, "{id:?}");
        assert_eq!(tone_colours(Tone::Error, &t).0, r.err_soft, "{id:?}");
        assert_eq!(tone_colours(Tone::Accent, &t).0, r.accent_soft, "{id:?}");
    }
}

/// The four counters show on every board, Routed and Unrouted once the
/// connections are counted; each opens its own popover.
#[test]
fn a_board_shows_four_counters_once_counted() {
    let mut b = BoardReadout::default();
    assert_eq!(
        counters::counter_list(&b),
        [BoardCounter::Drc, BoardCounter::Selected]
    );
    b.connections = Some(ConnectionTally::default());
    assert_eq!(
        counters::counter_list(&b),
        [
            BoardCounter::Routed,
            BoardCounter::Unrouted,
            BoardCounter::Drc,
            BoardCounter::Selected
        ]
    );
    assert_eq!(BoardCounter::Routed.popover(), BoardPopoverKind::Unrouted);
    assert_eq!(BoardCounter::Unrouted.popover(), BoardPopoverKind::Unrouted);
    assert_eq!(BoardCounter::Drc.popover(), BoardPopoverKind::Drc);
    assert_eq!(
        BoardCounter::Selected.popover(),
        BoardPopoverKind::Selection
    );
    assert_eq!(BoardCounter::Layer.popover(), BoardPopoverKind::Layer);
    assert_eq!(BoardCounter::Grid.popover(), BoardPopoverKind::Grid);
}

/// Custom… takes a pitch above 0 mm, nothing else.
#[test]
fn a_custom_pitch_must_be_a_number_above_zero() {
    assert_eq!(grid_popover::custom_pitch("0.15"), Some(0.15));
    assert_eq!(grid_popover::custom_pitch(" 2 "), Some(2.0));
    for refused in ["", "0", "-0.1", "abc", "NaN", "inf"] {
        assert_eq!(grid_popover::custom_pitch(refused), None, "{refused:?}");
    }
}

/// The presets are the mock's; only 1.270 mm is a whole number of mil.
#[test]
fn the_grid_presets_are_the_mocks_and_name_their_mil() {
    assert_eq!(BOARD_GRID_PRESETS_MM, [0.1, 0.25, 0.5, 1.27]);
    assert_eq!(grid_popover::whole_mil(1.27), Some(50));
    for mm in [0.1, 0.25, 0.5] {
        assert_eq!(grid_popover::whole_mil(mm), None, "{mm}");
    }
}

#[test]
fn the_cycle_caption_names_the_keys_bound() {
    let i18n = I18n::en_us();
    let shown = |next, prev| cycles(next, prev, &i18n).map(plain);
    assert_eq!(shown("+", "-").as_deref(), Some("+ / - cycles"));
    assert_eq!(shown("G", "").as_deref(), Some("G cycles"));
    assert_eq!(shown("", "").as_deref(), None);
}

fn name(designator: &str, pad: &str) -> pcb_geom::PadName {
    pcb_geom::PadName {
        designator: designator.into(),
        pad: pad.into(),
    }
}

/// A net's detail line lists its pad pairs, then the pour note.
#[test]
fn a_net_row_lists_its_pads_and_the_pour_note() {
    let i18n = I18n::en_us();
    let mut net = NetRow {
        net: 3,
        name: "GND".into(),
        pairs: vec![
            (name("C5", "2"), name("C6", "2")),
            (name("R2", "2"), name("U1", "4")),
        ],
        pour_will_connect: false,
    };
    let line = |net: &NetRow| {
        unrouted_popover::net_detail(&i18n, net).replace(['\u{2068}', '\u{2069}'], "")
    };
    assert_eq!(line(&net), "C5-2 → C6-2, R2-2 → U1-4");
    net.pour_will_connect = true;
    assert_eq!(line(&net), "C5-2 → C6-2, R2-2 → U1-4 · pour will connect");
    assert_eq!(net.pair_count(), 2);
}

#[test]
fn the_drc_header_says_when_it_ran_and_the_edits_since() {
    let i18n = I18n::en_us();
    let mut list = DrcList {
        last_run: None,
        running: false,
        rows: Vec::new(),
        more: 0,
    };
    assert_eq!(
        plain(drc_popover::last_run_text(&list, &i18n)),
        "not run yet"
    );
    list.last_run = Some(("14:12:58".into(), 2));
    assert_eq!(
        plain(drc_popover::last_run_text(&list, &i18n)),
        "last run 14:12:58 · 2 edits since"
    );
    list.last_run = Some(("14:12:58".into(), 1));
    assert_eq!(
        plain(drc_popover::last_run_text(&list, &i18n)),
        "last run 14:12:58 · 1 edit since"
    );
}

#[test]
fn every_pick_kind_has_its_own_name() {
    let names: std::collections::HashSet<String> = pcb_geom::PickKind::ALL
        .into_iter()
        .map(|kind| selection_popover::kind_name(kind).as_str().to_string())
        .collect();
    assert_eq!(names.len(), pcb_geom::PickKind::ALL.len());
}

/// Every popover draws from its model in both Graphite themes.
#[test]
fn every_popover_draws() {
    let i18n = I18n::en_us();
    let models = [
        BoardPopoverModel::Layer(LayerList {
            rows: vec![LayerChoice {
                layer: signex_types::pcb_layer::PcbLayer::Copper(
                    signex_types::pcb_layer::CopperLayer::Top,
                ),
                name: signex_i18n::t!(i18n, signex_i18n::ids::PCB_LAYER_TOP_COPPER),
                colour: iced::Color::WHITE,
                active: true,
            }],
            next_key: "+".into(),
            prev_key: "-".into(),
        }),
        BoardPopoverModel::Grid(GridChoice {
            current_mm: 0.25,
            custom: "abc".into(),
            refused: true,
            snap: true,
            cycle_key: "G".into(),
        }),
        BoardPopoverModel::Unrouted(UnroutedList {
            unrouted: 1,
            nets: vec![NetRow {
                net: 1,
                name: "VBUS".into(),
                pairs: vec![(name("J2", "1"), name("R5", "1"))],
                pour_will_connect: false,
            }],
            route_key: String::new(),
        }),
        BoardPopoverModel::Drc(DrcList {
            last_run: None,
            running: true,
            rows: Vec::new(),
            more: 0,
        }),
        BoardPopoverModel::Selection(SelectionList {
            count: 2,
            rows: vec![SelectedRow {
                item: pcb_geom::ItemRef::Footprint(uuid::Uuid::nil()),
                what: SelectedWhat::Part {
                    designator: "U3".into(),
                    value: "AP2112K-3.3".into(),
                    footprint: "SOT-23-5".into(),
                },
            }],
            linked: false,
        }),
    ];
    for id in [ThemeId::GraphiteLight, ThemeId::GraphiteDark] {
        let t = theme_tokens(id);
        for model in &models {
            let _card = popover_view(model, &t, &i18n);
        }
    }
}

fn anchor_bar(board: &BoardReadout) -> AnchorBar<'_> {
    AnchorBar {
        grid_value: board_grid_value(
            &I18n::en_us(),
            true,
            0.25,
            signex_types::coord::Unit::Mm,
            false,
        ),
        snap_enabled: true,
        zoom: 100.0,
        view_angle: None,
        board,
        window_width: 1400.0,
    }
}

/// The Grid card opens over the Grid read-out; the layer card further
/// right, over the pill after it; the counters' cards over their counters,
/// left to right, and the Selection card right-aligned inside the window.
#[test]
fn each_card_opens_over_its_segment() {
    let i18n = I18n::en_us();
    let board = BoardReadout {
        connections: Some(ConnectionTally {
            routed: 38,
            total: 46,
            unrouted: 8,
        }),
        drc: DrcTally::Ran {
            violations: 3,
            age: DrcAge::Minutes(2),
        },
        selected: 2,
        open: None,
        grid_extra: false,
    };
    let bar = anchor_bar(&board);
    let left = |kind| popover_left(kind, &bar, &i18n);
    assert_eq!(left(BoardPopoverKind::Grid), super::super::grid_readout_x());
    assert!(left(BoardPopoverKind::Layer) > left(BoardPopoverKind::Grid));
    assert!(left(BoardPopoverKind::Unrouted) < left(BoardPopoverKind::Drc));
    let selection = left(BoardPopoverKind::Selection);
    let width = kit::card_width(BoardPopoverKind::Selection);
    assert!(selection + width <= bar.window_width);
    assert!(selection > left(BoardPopoverKind::Unrouted));
    // A narrow window keeps every card inside it.
    let narrow = AnchorBar {
        window_width: 400.0,
        ..anchor_bar(&board)
    };
    for kind in [
        BoardPopoverKind::Layer,
        BoardPopoverKind::Grid,
        BoardPopoverKind::Unrouted,
        BoardPopoverKind::Drc,
        BoardPopoverKind::Selection,
    ] {
        let x = popover_left(kind, &narrow, &i18n);
        assert!(x >= 0.0, "{kind:?}");
        assert!(x + kit::card_width(kind) <= 400.0, "{kind:?}");
    }
}

#[test]
fn the_cards_are_the_mocks_widths() {
    let px = kit::PX;
    assert_eq!(kit::card_width(BoardPopoverKind::Layer), 230.0 * px);
    assert_eq!(kit::card_width(BoardPopoverKind::Grid), 300.0 * px);
    assert_eq!(kit::card_width(BoardPopoverKind::Unrouted), 320.0 * px);
    assert_eq!(kit::card_width(BoardPopoverKind::Drc), 330.0 * px);
    assert_eq!(kit::card_width(BoardPopoverKind::Selection), 300.0 * px);
}
