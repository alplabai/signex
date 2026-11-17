//! #2451 review: the zone copper of one net on one layer is written as one
//! area, so a hole of one region that another same-net region covers is
//! not cleared out of the image.

use signex_types::pcb::{NetDef, PadNet, PadShape, PadType, Zone};

use super::fixture::{empty_board, footprint, p, pad};
use super::{FabExtras, FabOptions, file, generate};

fn gnd_zone(
    id: u128,
    fill_type: &str,
    outline: [(f64, f64); 2],
    holes: Vec<[(f64, f64); 2]>,
) -> Zone {
    let rect =
        |[(x0, y0), (x1, y1)]: [(f64, f64); 2]| vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)];
    Zone {
        uuid: uuid::Uuid::from_u128(id),
        net: 1,
        net_name: "GND".into(),
        layer: "TopCopper".into(),
        outline: rect(outline),
        holes: holes.into_iter().map(rect).collect(),
        priority: 0,
        fill_type: fill_type.into(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.0,
        min_thickness: 0.0,
        min_island_area: 0.0,
        remove_dead_islands: false,
        unknown: Default::default(),
    }
}

/// A GND pour, a GND solid region inside it with a 4 x 4 mm hole, and a
/// GND pad in that hole: the pour's copper fills the hole and reaches the
/// pad, so the image clears nothing.
#[test]
fn a_same_net_region_hole_inside_a_pour_does_not_erase_the_pour() {
    let mut board = empty_board();
    board.outline = vec![p(0.0, 0.0), p(20.0, 0.0), p(20.0, 10.0), p(0.0, 10.0)];
    board.nets = vec![NetDef {
        number: 1,
        name: "GND".into(),
    }];
    let mut gnd = pad(
        "1",
        PadType::Smd,
        PadShape::Rect,
        p(0.0, 0.0),
        p(1.0, 1.0),
        &["TopCopper"],
    );
    gnd.net = Some(PadNet {
        number: 1,
        name: "GND".into(),
    });
    board.footprints = vec![footprint(
        10,
        "TP1",
        p(10.0, 5.0),
        0.0,
        "TopCopper",
        vec![gnd],
    )];
    board.zones = vec![
        gnd_zone(0x80, "pour", [(1.0, 1.0), (19.0, 9.0)], Vec::new()),
        gnd_zone(
            0x81,
            "solid",
            [(5.0, 2.0), (15.0, 8.0)],
            vec![[(8.0, 3.0), (12.0, 7.0)]],
        ),
    ];

    let extras = FabExtras::of_board(&board);
    let set = generate(&board, &extras, &FabOptions::all("fab")).expect("generates");

    let top = file(&set, "fab-copper-l1-top.gbr").expect("top copper");
    assert!(top.contains("%TO.N,GND*%\nG36*"), "the GND copper: {top}");
    assert!(!top.contains("%LPC*%"), "nothing is cleared: {top}");
    assert!(top.contains("D03*"), "the pad is flashed: {top}");
}
