//! #2410: a 50 x 40 mm board arrayed two by two.

use signex_types::pcb::{
    Footprint, NetDef, Pad, PadNet, PadShape, PadType, PcbBoard, Point, Segment,
};
use signex_types::pcb_panel::{BoardPanel, PanelError, PanelSeparation};
use uuid::Uuid;

use super::{PANEL_ROUTE_LAYER, PanelSegment, layout, panel_board};
use crate::units::{NM_PER_MM, PointNm};

fn outline(w: f64, h: f64) -> Vec<Point> {
    vec![
        Point::new(0.0, 0.0),
        Point::new(w, 0.0),
        Point::new(w, h),
        Point::new(0.0, h),
    ]
}

/// A point in whole and half mm, as nm.
fn at(x: f64, y: f64) -> PointNm {
    PointNm::new(
        (x * NM_PER_MM as f64).round() as i64,
        (y * NM_PER_MM as f64).round() as i64,
    )
}

fn line(a: (f64, f64), b: (f64, f64)) -> PanelSegment {
    PanelSegment {
        from: at(a.0, a.1),
        to: at(b.0, b.1),
    }
}

#[test]
fn the_default_panel_lays_out_copies_rails_marks_and_tabs() {
    let panel = layout(&outline(50.0, 40.0), &BoardPanel::default()).expect("lays out");
    assert_eq!((panel.width_mm(), panel.height_mm()), (102.0, 96.0));
    assert_eq!(panel.frame.min, at(0.0, -7.0));
    assert_eq!(
        panel.copies,
        [at(0.0, 0.0), at(52.0, 0.0), at(0.0, 42.0), at(52.0, 42.0)]
    );
    assert_eq!(panel.rails.len(), 2, "top and bottom");
    assert_eq!(
        panel.tooling_holes,
        [at(2.5, -4.5), at(99.5, -4.5), at(2.5, 86.5), at(99.5, 86.5)]
    );
    assert_eq!(
        panel.fiducials,
        [at(7.5, -4.5), at(7.5, 86.5), at(94.5, 86.5)]
    );
    // Two tabs to each rail, one across each of the four inner gaps.
    assert_eq!(panel.tabs, 8);
    // Twelve tabbed copy edges, four holes 0.8 mm apart on each 3 mm tab.
    assert_eq!(panel.bite_holes.len(), 48);
    assert!(panel.v_scores.is_empty());
    assert!(panel.rectangular);
}

#[test]
fn routed_edges_stay_open_where_a_tab_crosses() {
    let panel = layout(&outline(50.0, 40.0), &BoardPanel::default()).expect("lays out");
    // The top rail's inner edge, 2 mm above the first row.
    for piece in [
        line((0.0, -2.0), (23.5, -2.0)),
        line((26.5, -2.0), (75.5, -2.0)),
        line((78.5, -2.0), (102.0, -2.0)),
    ] {
        assert!(panel.routes.contains(&piece), "{piece:?}");
    }
    // The first copy's top edge, open over its tab; its left edge is the
    // frame's and is not routed again.
    assert!(panel.routes.contains(&line((0.0, 0.0), (23.5, 0.0))));
    assert!(panel.routes.contains(&line((26.5, 0.0), (50.0, 0.0))));
    assert!(!panel.routes.contains(&line((0.0, 40.0), (0.0, 0.0))));
    // The tab's sides close the slot across the gap to the rail.
    assert!(panel.routes.contains(&line((23.5, 0.0), (23.5, -2.0))));
}

#[test]
fn v_scores_run_across_the_frame_and_touching_copies_share_one() {
    let panel = BoardPanel {
        separation: PanelSeparation::VScore,
        spacing_x: 0.0,
        spacing_y: 0.0,
        ..BoardPanel::default()
    };
    let panel = layout(&outline(50.0, 40.0), &panel).expect("lays out");
    assert_eq!(panel.frame.min, at(0.0, -5.0));
    assert_eq!(
        panel.v_scores,
        [
            line((50.0, -5.0), (50.0, 85.0)),
            line((0.0, 0.0), (100.0, 0.0)),
            line((0.0, 40.0), (100.0, 40.0)),
            line((0.0, 80.0), (100.0, 80.0)),
        ]
    );
    assert!(panel.routes.is_empty() && panel.bite_holes.is_empty());
}

#[test]
fn a_board_too_small_for_its_tabs_or_without_an_outline_is_refused() {
    assert!(matches!(
        layout(&outline(2.0, 2.0), &BoardPanel::default()),
        Err(PanelError::TabsDoNotFit { tabs: 1, .. })
    ));
    assert_eq!(
        layout(&outline(50.0, 40.0)[..2], &BoardPanel::default()),
        Err(PanelError::NoOutline)
    );
    // A lone copy with no rail needs no tab, however small.
    let alone = BoardPanel {
        columns: 1,
        rows: 1,
        rails: signex_types::pcb_panel::PanelRails {
            top: 0.0,
            bottom: 0.0,
            left: 0.0,
            right: 0.0,
        },
        ..BoardPanel::default()
    };
    assert!(layout(&outline(2.0, 2.0), &alone).is_ok());
}

fn board() -> PcbBoard {
    let gnd = || PadNet {
        number: 1,
        name: "GND".into(),
    };
    let pad = Pad {
        uuid: Uuid::from_u128(0x11),
        number: "1".into(),
        pad_type: PadType::Smd,
        shape: PadShape::Rect,
        position: Point::new(0.0, 0.0),
        size: Point::new(1.0, 1.0),
        drill: None,
        layers: vec!["TopCopper".into()],
        net: Some(gnd()),
        roundrect_ratio: 0.0,
        trapezoid_delta: Default::default(),
        custom_outline: Vec::new(),
        rotation: 0.0,
        net_locked: false,
        layer_sizes: None,
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    let footprint = Footprint {
        uuid: Uuid::from_u128(0x10),
        reference: "U1".into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(10.0, 10.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads: vec![pad],
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: None,
        library_ref: None,
        component: Default::default(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    let segment = Segment {
        locked: false,
        attributes: Default::default(),
        uuid: Uuid::from_u128(0x20),
        start: Point::new(10.0, 10.0),
        end: Point::new(20.0, 10.0),
        width: 0.25,
        layer: "TopCopper".into(),
        net: 1,
        unknown_columns: Default::default(),
    };
    PcbBoard {
        sketch: None,
        stackup: None,
        free: Default::default(),
        options: Default::default(),
        uuid: Uuid::from_u128(1),
        version: 1,
        generator: String::new(),
        thickness: 1.6,
        outline: outline(50.0, 40.0),
        layers: Vec::new(),
        setup: None,
        nets: vec![NetDef {
            number: 1,
            name: "GND".into(),
        }],
        footprints: vec![footprint],
        segments: vec![segment],
        vias: Vec::new(),
        zones: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        unknown: Default::default(),
    }
}

#[test]
fn the_panel_board_holds_every_copy_with_its_own_nets_and_the_marks() {
    let source = board();
    let mut panel = BoardPanel::default();
    let mut source_with_panel = source.clone();
    source_with_panel.options.panel = Some(panel);
    let out = panel_board(&source_with_panel, &panel).expect("panelised");
    assert_eq!(out.options.panel, None);
    assert_eq!(out.outline.len(), 4);
    assert_eq!(out.outline[0], Point::new(0.0, -7.0));
    // Four copies of U1 and three fiducials.
    assert_eq!(out.footprints.len(), 7);
    let names: Vec<&str> = out.nets.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, ["GND/1", "GND/2", "GND/3", "GND/4"]);
    let numbers: Vec<u32> = out.nets.iter().map(|n| n.number).collect();
    assert_eq!(numbers, [1, 3, 5, 7]);
    let second = &out.footprints[1];
    assert_eq!(second.position, Point::new(62.0, 10.0));
    assert_eq!(second.pads[0].net.as_ref().map(|n| n.number), Some(3));
    assert_ne!(second.uuid, source.footprints[0].uuid);
    assert_eq!(out.segments[3].start, Point::new(62.0, 52.0));
    assert_eq!(out.segments[3].net, 7);
    let sketch = out.sketch.as_ref().expect("the panel's holes");
    assert_eq!(sketch.holes.len(), 4 + 48, "tooling holes and bites");
    assert!(sketch.holes.iter().all(|h| !h.plated));
    assert!(!out.graphics.is_empty());
    assert!(out.graphics.iter().all(|g| g.layer == PANEL_ROUTE_LAYER));
    // V-scores go to the sketch's score lines instead of routed edges.
    panel.separation = PanelSeparation::VScore;
    let scored = panel_board(&source, &panel).expect("panelised");
    assert!(scored.graphics.is_empty());
    let lines = scored.sketch.as_ref().map_or(0, |s| s.v_scores.len());
    assert_eq!(lines, 6, "x 50, 52 and y 0, 40, 42, 82");
}

/// #2373: a free pad is arrayed with the rest of the board, each copy with
/// its own net and id, so the panel's outputs hold one per copy.
#[test]
fn a_free_pad_is_copied_into_every_copy() {
    let mut source = board();
    let mut free = source.footprints[0].pads[0].clone();
    free.uuid = Uuid::from_u128(0x30);
    free.position = Point::new(5.0, 5.0);
    source.free.pads.push(free);
    let out = panel_board(&source, &BoardPanel::default()).expect("panelised");
    assert_eq!(out.free.pads.len(), 4);
    let nets: Vec<u32> = out
        .free
        .pads
        .iter()
        .filter_map(|p| p.net.as_ref().map(|n| n.number))
        .collect();
    assert_eq!(nets, [1, 3, 5, 7]);
    assert_eq!(out.free.pads[1].position, Point::new(57.0, 5.0));
    assert_ne!(out.free.pads[1].uuid, out.free.pads[0].uuid);
}
