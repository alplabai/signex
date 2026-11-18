//! The ODB++ job's nets, components and layer names (#2485) on boards
//! built to break them: pads that share a number, a panel's copies, a
//! bottom part at an angle, net and layer names that collide, and a cut
//! drawn on the outline layer.

use signex_types::pcb::{BoardGraphic, LayerDef, NetDef, PadNet, PadShape, PadType, Segment};
use signex_types::pcb_panel::BoardPanel;
use uuid::Uuid;

use super::FabExtras;
use super::fixture::{fixture_board, footprint, p, pad};
use super::odb::{Resolved, job, matrix, resolve};

const SMD_TOP: [&str; 3] = ["TopCopper", "TopSolderMask", "TopPaste"];

fn smd(number: &str, x: f64, y: f64, net: Option<(u32, &str)>) -> signex_types::pcb::Pad {
    let mut pad = pad(
        number,
        PadType::Smd,
        PadShape::Rect,
        p(x, y),
        p(0.6, 0.8),
        &SMD_TOP,
    );
    pad.net = net.map(|(number, name)| PadNet {
        number,
        name: name.into(),
    });
    pad
}

fn toeprints_of<'a>(all: &'a [Resolved], part: &str) -> Vec<&'a Resolved> {
    all.iter().filter(|t| t.part == part).collect()
}

#[test]
fn two_pads_numbered_alike_each_keep_their_own_copper_and_net() {
    let mut board = fixture_board();
    // A tab split in two: both pads numbered 2, both on GND.
    board.footprints.push(footprint(
        50,
        "U1",
        p(12.0, 3.0),
        0.0,
        "TopCopper",
        vec![
            smd("1", -1.5, 0.0, Some((2, "VCC"))),
            smd("2", 0.0, 0.0, Some((1, "GND"))),
            smd("2", 1.5, 0.0, Some((1, "GND"))),
        ],
    ));
    let job = job(&board, &FabExtras::default(), "fab");
    let all = resolve(&job);
    let u1 = toeprints_of(&all, "U1");
    assert_eq!(u1.len(), 3, "{all:?}");
    for toeprint in &u1 {
        assert_ne!(toeprint.net, "$NONE$", "{toeprint:?}");
        assert_eq!(toeprint.fids, 1, "one copper flash each: {toeprint:?}");
    }
    let tabs: Vec<&str> = u1
        .iter()
        .filter(|t| t.pin == "2")
        .map(|t| t.net.as_str())
        .collect();
    assert_eq!(tabs, ["GND", "GND"]);
}

#[test]
fn a_panels_copies_each_list_their_pins_under_their_own_net() {
    let board = fixture_board();
    let whole = pcb_geom::panel::panel_board(&board, &BoardPanel::default()).expect("panel");
    let job = job(&whole, &FabExtras::default(), "fab");
    let all = resolve(&job);
    // Each copy has its own designator: R1, R1_2, R1_3, R1_4.
    let copies = ["R1", "R1_2", "R1_3", "R1_4"];
    let r1: Vec<&Resolved> = all
        .iter()
        .filter(|t| copies.contains(&t.part.as_str()))
        .collect();
    assert_eq!(r1.len(), 8, "four copies of two pins: {all:?}");
    for copy in copies {
        assert_eq!(toeprints_of(&all, copy).len(), 2, "{copy}: {all:?}");
    }
    for toeprint in &r1 {
        assert_ne!(toeprint.net, "$NONE$", "{toeprint:?}");
        assert_eq!(toeprint.fids, 1, "{toeprint:?}");
    }
    let mut pin1_nets: Vec<&str> = r1
        .iter()
        .filter(|t| t.pin == "1")
        .map(|t| t.net.as_str())
        .collect();
    pin1_nets.sort_unstable();
    pin1_nets.dedup();
    assert_eq!(pin1_nets.len(), 4, "one GND per copy: {pin1_nets:?}");
}

/// A record's numeric field.
fn number(fields: &[&str], i: usize) -> f64 {
    fields[i].parse().expect("a number")
}

#[test]
fn each_toeprint_is_its_package_pin_mirrored_then_turned_clockwise() {
    let mut board = fixture_board();
    board.footprints.push(footprint(
        60,
        "U2",
        p(8.0, 4.0),
        30.0,
        "BottomCopper",
        vec![
            smd("1", -1.2, 0.4, None),
            smd("2", 1.0, -0.6, None),
            smd("3", 0.3, 0.9, None),
        ],
    ));
    let job = job(&board, &FabExtras::default(), "fab");
    // Each package's pin centres, in package order.
    let mut packages: Vec<Vec<(f64, f64)>> = Vec::new();
    for line in job["steps/pcb/eda/data"].lines() {
        let fields: Vec<&str> = line.split(' ').collect();
        match fields[0] {
            "PKG" => packages.push(Vec::new()),
            "PIN" => packages
                .last_mut()
                .expect("a package")
                .push((number(&fields, 3), number(&fields, 4))),
            _ => {}
        }
    }
    let mut checked = [0, 0];
    for (side, file) in [(0, "comp_+_top"), (1, "comp_+_bot")] {
        let components = &job[&format!("steps/pcb/layers/{file}/components")];
        let mut placed = (0, 0.0, 0.0, 0.0, false);
        for line in components.lines() {
            let fields: Vec<&str> = line.split(' ').collect();
            match fields[0] {
                "CMP" => {
                    let package: usize = fields[1].parse().expect("package");
                    let mirrored = fields[5] == "M";
                    assert_eq!(mirrored, side == 1, "{line}");
                    placed = (
                        package,
                        number(&fields, 2),
                        number(&fields, 3),
                        number(&fields, 4),
                        mirrored,
                    );
                }
                "TOP" => {
                    let (package, cx, cy, rot, mirrored) = placed;
                    let pin: usize = fields[1].parse().expect("pin");
                    let (px, py) = packages[package][pin];
                    let px = if mirrored { -px } else { px };
                    let (sin, cos) = rot.to_radians().sin_cos();
                    let x = cx + px * cos + py * sin;
                    let y = cy - px * sin + py * cos;
                    let (tx, ty) = (number(&fields, 2), number(&fields, 3));
                    assert!(
                        (x - tx).abs() < 1e-5 && (y - ty).abs() < 1e-5,
                        "{line}: expected ({x}, {y})"
                    );
                    checked[side] += 1;
                }
                _ => {}
            }
        }
    }
    assert_eq!(checked[1], 3, "U2's three pins on the bottom");
    assert!(checked[0] > 0);
    assert!(
        job["steps/pcb/layers/comp_+_bot/components"].contains(" 30 M U2 "),
        "{}",
        job["steps/pcb/layers/comp_+_bot/components"]
    );
}

#[test]
fn net_names_that_become_one_word_stay_apart() {
    let mut board = fixture_board();
    board.nets.push(NetDef {
        number: 3,
        name: "VCC 3V3".into(),
    });
    board.nets.push(NetDef {
        number: 4,
        name: "VCC_3V3".into(),
    });
    for (i, net) in [3, 4].into_iter().enumerate() {
        board.segments.push(Segment {
            uuid: Uuid::from_u128(80 + i as u128),
            start: p(2.0, 2.0 + i as f64),
            end: p(4.0, 2.0 + i as f64),
            width: 0.25,
            layer: "TopCopper".into(),
            net,
            unknown_columns: Default::default(),
            attributes: Default::default(),
            locked: false,
        });
    }
    let job = job(&board, &FabExtras::default(), "fab");
    let names: Vec<&str> = job["steps/pcb/eda/data"]
        .lines()
        .filter_map(|l| l.strip_prefix("NET "))
        .collect();
    assert!(names.contains(&"VCC_3V3"), "{names:?}");
    assert!(names.contains(&"VCC_3V3_2"), "{names:?}");
    let mut unique = names.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), names.len(), "{names:?}");
}

#[test]
fn layer_names_that_collide_get_a_free_suffix() {
    let mut board = fixture_board();
    let mechanical = |id: u8, n: u8, display: &str| LayerDef {
        id,
        name: format!("Mechanical{n}"),
        layer_type: "user".into(),
        display_name: display.into(),
    };
    // Mechanical2 takes "mechanical3", so Mechanical3 finds both its name
    // and its token taken.
    board.layers.push(mechanical(71, 2, "Mechanical3"));
    board.layers.push(mechanical(72, 3, ""));
    board.layers.push(mechanical(73, 4, "Fold"));
    board.layers.push(mechanical(74, 5, "Fold"));
    board.layers.push(mechanical(75, 6, "comp_+_top"));
    let job = job(&board, &FabExtras::default(), "fab");
    let names: Vec<String> = matrix(&job).into_iter().map(|l| l[0].clone()).collect();
    let mut unique = names.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), names.len(), "{names:?}");
    let expected = [
        "mechanical3",
        "mechanical3_2",
        "fold",
        "mechanical5",
        "mechanical6",
    ];
    for name in expected {
        assert!(names.iter().any(|n| n == name), "{name} in {names:?}");
    }
    assert!(job.contains_key("steps/pcb/layers/comp_+_top/components"));
}

#[test]
fn a_cut_drawn_on_the_outline_layer_is_on_its_rout_layer() {
    let lines = |board: &signex_types::pcb::PcbBoard| {
        let job = job(board, &FabExtras::default(), "fab");
        let layers = matrix(&job);
        let edge = layers
            .iter()
            .find(|l| l[0] == "edgecuts")
            .unwrap_or_else(|| panic!("the outline layer in {layers:?}"));
        assert_eq!(edge[1..3], ["BOARD", "ROUT"]);
        job["steps/pcb/layers/edgecuts/features"]
            .lines()
            .filter(|l| l.starts_with("L "))
            .count()
    };
    let mut board = fixture_board();
    let outline_only = lines(&board);
    let slot: BoardGraphic = serde_json::from_value(serde_json::json!({
        "graphic_type": "line", "layer": "EdgeCuts", "width": 0.1,
        "start": {"x": 5.0, "y": 2.0}, "end": {"x": 8.0, "y": 2.0}
    }))
    .expect("a line");
    board.graphics.push(slot);
    assert_eq!(lines(&board), outline_only + 1);
}
