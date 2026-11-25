//! Golden files and geometry checks of the fabrication outputs (#2388).
//! A golden diff is a change to what a board house receives, not a
//! snapshot to refresh: read it first. Regenerate with
//! `UPDATE_GOLDEN=1 cargo test -p signex-output fab`.

mod copper_drawings;
pub(crate) mod fixture;
mod ipc2581;
mod layer_sizes;
mod odb;
mod odb_eda;
mod origin;
mod panel;
mod reference_sign;
mod same_net;

use std::path::PathBuf;

use pcb_geom::{PointNm, pad_world_center};
use signex_types::pcb::{PadShape, PadType, Point, Via, ViaType, Zone};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use uuid::Uuid;

use super::{
    FabError, FabExtras, FabFileKind, FabNote, FabOptions, FabSet, FilledRegion, generate,
};
use fixture::{drilled, fixture_board, footprint, p, pad};

fn generated(board: &signex_types::pcb::PcbBoard) -> FabSet {
    generate(board, &FabExtras::default(), &FabOptions::all("fab")).expect("generates")
}

fn file<'a>(set: &'a FabSet, name: &str) -> Option<&'a str> {
    set.files
        .iter()
        .find(|f| f.name == name)
        .map(|f| f.contents.as_str())
}

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/fab/testdata")
        .join(name)
}

#[test]
fn the_fixture_board_matches_its_golden_files() {
    let set = generated(&fixture_board());
    let goldens = [
        "fab-copper-l1-top.gbr",
        "fab-soldermask-top.gbr",
        "fab-profile.gbr",
        "fab-pth.drl",
        "fab-centroid.csv",
    ];
    for name in goldens {
        let actual = file(&set, name).unwrap_or_else(|| panic!("{name} is generated"));
        let path = golden_path(name);
        if std::env::var_os("UPDATE_GOLDEN").is_some() {
            std::fs::write(&path, actual).expect("golden written");
            continue;
        }
        let expected = std::fs::read_to_string(&path).expect("golden file");
        assert_eq!(actual, expected, "{name} differs from its golden file");
    }
}

#[test]
fn the_same_board_gives_the_same_bytes() {
    assert_eq!(generated(&fixture_board()), generated(&fixture_board()));
}

#[test]
fn every_written_file_is_named_once_and_listed_in_the_job() {
    let set = generated(&fixture_board());
    let names: Vec<&str> = set.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "fab-copper-l1-top.gbr",
            "fab-copper-l2-bottom.gbr",
            "fab-soldermask-top.gbr",
            "fab-soldermask-bottom.gbr",
            "fab-paste-top.gbr",
            "fab-profile.gbr",
            "fab-pth.drl",
            "fab-drill-table.csv",
            "fab-centroid.csv",
            "fab-netlist.ipc",
            "fab-ipc2581.xml",
            "fab-odb.tar",
            "fab-job.gbrjob",
        ]
    );
    let job = file(&set, "fab-job.gbrjob").expect("job file");
    for name in &names[..names.len() - 1] {
        assert!(job.contains(&format!("\"{name}\"")), "job lists {name}");
    }
    assert!(job.contains("\"Copper,L2,Bot\""));
    assert!(set.notes.is_empty(), "{:?}", set.notes);
}

#[test]
fn a_turned_bottom_side_part_flashes_where_its_centroid_says() {
    let mut board = fixture_board();
    let layers = ["BottomCopper", "BottomSolderMask", "BottomPaste"];
    let smd = pad(
        "1",
        PadType::Smd,
        PadShape::Rect,
        p(1.0, 0.0),
        p(2.0, 1.0),
        &layers,
    );
    let mut u1 = footprint(50, "U1", p(12.0, 3.0), 30.0, "BottomCopper", vec![smd]);
    u1.value = "IC".into();
    u1.footprint_id = "SOT".into();
    let model_center = pad_world_center(&u1, &u1.pads[0]).expect("centre");
    board.footprints.push(u1);
    let set = generated(&board);

    let centroid = file(&set, "fab-centroid.csv").expect("centroid");
    assert!(
        centroid.contains("U1,IC,SOT,12.000,-3.000,330.000,Bottom\n"),
        "{centroid}"
    );
    // The pad is mirrored and turned 30° in the model's axes, which the
    // frame's y flip shows as 330°, the centroid's rotation.
    assert_eq!(model_center, PointNm::new(11_133_975, 2_500_000));
    let bottom = file(&set, "fab-copper-l2-bottom.gbr").expect("bottom copper");
    assert!(bottom.contains("21,1,2,1,0,0,330*\n"), "{bottom}");
    assert!(bottom.contains("X11133975Y-2500000D03*\n"), "{bottom}");
    let top = file(&set, "fab-copper-l1-top.gbr").expect("top copper");
    assert!(!top.contains("X11133975Y-2500000D03*"), "not on the top");
    let paste = file(&set, "fab-paste-bottom.gbr").expect("bottom paste");
    assert!(paste.contains("X11133975Y-2500000D03*\n"), "{paste}");
}

#[test]
fn an_unplated_slot_lands_in_the_non_plated_file() {
    let mut board = fixture_board();
    let hole = pad(
        "1",
        PadType::NpThru,
        PadShape::Oval,
        p(0.0, 0.0),
        p(3.0, 1.5),
        &[],
    );
    board.footprints.push(footprint(
        60,
        "H1",
        p(2.0, 2.0),
        0.0,
        "TopCopper",
        vec![drilled(hole, 1.0, "oval")],
    ));
    let set = generated(&board);
    let slot = "X1.250Y-2.000G85X2.750Y-2.000\n";
    let npth = file(&set, "fab-npth.drl").expect("non-plated file");
    assert!(npth.contains("; #@! TF.FileFunction,NonPlated,1,2,NPTH\n"));
    assert!(npth.contains("T1C1.000\n"));
    assert!(npth.contains(slot), "{npth}");
    let pth = file(&set, "fab-pth.drl").expect("plated file");
    assert!(!pth.contains("G85"), "{pth}");
    let table = file(&set, "fab-drill-table.csv").expect("drill table");
    assert!(table.contains("NPTH,T1,1.000,0,1\n"), "{table}");
    assert!(table.contains("PTH,T2,0.800,1,0\n"), "{table}");
    // A non-plated pad that lists no copper has none.
    let top = file(&set, "fab-copper-l1-top.gbr").expect("top copper");
    assert!(!top.contains("X2000000Y-2000000D03*"), "{top}");
}

#[test]
fn a_plated_slot_lands_in_the_plated_file() {
    let mut board = fixture_board();
    let slot = pad(
        "1",
        PadType::Thru,
        PadShape::Oval,
        p(0.0, 0.0),
        p(1.5, 3.0),
        &[],
    );
    board.footprints.push(footprint(
        70,
        "J2",
        p(2.0, 2.0),
        0.0,
        "TopCopper",
        vec![drilled(slot, 1.0, "oval")],
    ));
    let set = generated(&board);
    let pth = file(&set, "fab-pth.drl").expect("plated file");
    assert!(pth.contains("X2.000Y-1.250G85X2.000Y-2.750\n"), "{pth}");
    assert!(file(&set, "fab-npth.drl").is_none());
}

#[test]
fn a_board_without_an_outline_or_a_selection_is_refused() {
    let mut board = fixture_board();
    let none = FabOptions {
        gerber: false,
        drill: false,
        centroid: false,
        test_netlist: false,
        ipc2581: false,
        odb: None,
        ..FabOptions::all("fab")
    };
    assert_eq!(
        generate(&board, &FabExtras::default(), &none),
        Err(FabError::NothingSelected)
    );
    for bad in ["", "  ", "a/b", "a:b"] {
        assert_eq!(
            generate(&board, &FabExtras::default(), &FabOptions::all(bad)),
            Err(FabError::BadName),
            "{bad:?}"
        );
    }
    board.outline.clear();
    assert_eq!(
        generate(&board, &FabExtras::default(), &FabOptions::all("fab")),
        Err(FabError::NoOutline)
    );
}

fn zone(layer: &str) -> Zone {
    Zone {
        uuid: Uuid::from_u128(80),
        net: 1,
        net_name: "GND".into(),
        layer: layer.into(),
        outline: vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0)],
        holes: Vec::new(),
        priority: 0,
        fill_type: String::new(),
        thermal_relief: false,
        thermal_gap: 0.0,
        thermal_width: 0.0,
        clearance: 0.0,
        min_thickness: 0.0,
        min_island_area: 0.0,
        remove_dead_islands: false,
        shelved: false,
        unfilled: false,
        unknown: Default::default(),
    }
}

#[test]
fn an_unfilled_zone_is_left_out_and_noted_never_drawn_as_its_outline() {
    let mut board = fixture_board();
    board.zones.push(zone("BottomCopper"));
    let set = generated(&board);
    assert_eq!(set.notes, vec![FabNote::ZonesNotFilled { count: 1 }]);
    let bottom = file(&set, "fab-copper-l2-bottom.gbr").expect("bottom copper");
    assert!(!bottom.contains("G36*"), "{bottom}");
}

#[test]
fn a_supplied_zone_fill_is_a_region_with_its_holes_cleared() {
    let mut board = fixture_board();
    board.zones.push(zone("BottomCopper"));
    let ring =
        |pts: &[(f64, f64)]| -> Vec<Point> { pts.iter().map(|&(x, y)| Point::new(x, y)).collect() };
    let extras = FabExtras {
        zone_fills: vec![FilledRegion {
            layer: PcbLayer::Copper(CopperLayer::Bottom),
            net_name: "GND".into(),
            outline: ring(&[(0.0, 0.0), (8.0, 0.0), (8.0, 8.0), (0.0, 8.0)]),
            holes: vec![ring(&[(2.0, 2.0), (3.0, 2.0), (3.0, 3.0)])],
        }],
        strokes: Vec::new(),
    };
    let set = generate(&board, &extras, &FabOptions::all("fab")).expect("generates");
    assert!(set.notes.is_empty(), "{:?}", set.notes);
    let bottom = file(&set, "fab-copper-l2-bottom.gbr").expect("bottom copper");
    let region = "%TO.N,GND*%\nG36*\nG01*\nX0Y0D02*\nX8000000Y0D01*\n\
                  X8000000Y-8000000D01*\nX0Y-8000000D01*\nX0Y0D01*\nG37*\n\
                  %LPC*%\nG36*\nX2000000Y-2000000D02*\nX3000000Y-2000000D01*\n\
                  X3000000Y-3000000D01*\nX2000000Y-2000000D01*\nG37*\n%LPD*%\n%TD*%\n";
    assert!(bottom.contains(region), "{bottom}");
    // The fill comes before every pad, so the clear never cuts one.
    let fill_at = bottom.find("G36*").expect("region");
    let pad_at = bottom.find("D03*").expect("a flash");
    assert!(fill_at < pad_at);
}

#[test]
fn the_test_netlist_has_a_record_per_pad_and_via() {
    let mut board = fixture_board();
    board.nets.push(signex_types::pcb::NetDef {
        number: 3,
        name: "A_VERY_LONG_NET_NAME".into(),
    });
    board.segments[0].net = 3;
    board.vias[0].net = 3;
    let set = generated(&board);
    let ipc = file(&set, "fab-netlist.ipc").expect("test netlist");
    let lines: Vec<&str> = ipc.lines().collect();
    assert_eq!(lines[2], "P  UNITS CUST 1");
    assert_eq!(lines[3], "P  NNAME1 A_VERY_LONG_NET_NAME");
    assert_eq!(lines.last(), Some(&"999"));
    let r1_1 = lines
        .iter()
        .find(|l| l.starts_with("327GND") && l[20..].starts_with("R1    -1"))
        .expect("R1 pin 1");
    assert_eq!(&r1_1[38..57], "A01X+004000Y-005000");
    assert_eq!(&r1_1[72..74], "S2");
    let j1 = lines
        .iter()
        .find(|l| l.starts_with("317GND"))
        .expect("J1 pin 1");
    assert_eq!(&j1[32..41], "D0800PA00");
    assert_eq!(&j1[72..74], "S0");
    let via = lines
        .iter()
        .find(|l| l.starts_with("317NNAME1"))
        .expect("the via, by its alias");
    assert_eq!(&via[20..23], "VIA");
    assert_eq!(&via[32..38], "D0300P");
    assert_eq!(&via[72..74], "S3");
}

/// The strokes (`D01` moves) an image draws.
fn strokes_in(set: &FabSet, name: &str) -> usize {
    file(set, name).map_or(0, |f| f.matches("D01*").count())
}

#[test]
fn board_text_is_written_on_its_legend_in_the_stroke_font() {
    let mut board = fixture_board();
    board.texts.push(signex_types::pcb::BoardText {
        uuid: Uuid::from_u128(90),
        text: "REV A".into(),
        position: p(1.0, 1.0),
        layer: "TopSilkscreen".into(),
        font_size: 1.0,
        rotation: 0.0,
    });
    let set = generated(&board);
    assert!(set.notes.is_empty(), "{:?}", set.notes);
    assert!(strokes_in(&set, "fab-legend-top.gbr") > 0);
    assert_eq!(strokes_in(&set, "fab-legend-bottom.gbr"), 0);
}

fn with_designator(layer: &str, hidden: bool) -> signex_types::pcb::PcbBoard {
    let mut board = fixture_board();
    let part = &mut board.footprints[0];
    part.layer = layer.into();
    part.properties.push(signex_types::property::PcbProperty {
        key: "Reference".into(),
        value: "R1".into(),
        hidden,
        ..Default::default()
    });
    board
}

#[test]
fn a_designator_is_written_on_its_sides_legend_unless_hidden() {
    let set = generated(&with_designator("TopCopper", false));
    assert!(strokes_in(&set, "fab-legend-top.gbr") > 0);
    let set = generated(&with_designator("BottomCopper", false));
    assert!(strokes_in(&set, "fab-legend-bottom.gbr") > 0);
    assert_eq!(strokes_in(&set, "fab-legend-top.gbr"), 0);
    let set = generated(&with_designator("TopCopper", true));
    assert_eq!(strokes_in(&set, "fab-legend-top.gbr"), 0);
}

/// A board drawing of `kind` on the top overlay, from `(0, 0)` to `(20, 5)`.
fn board_drawing(kind: &str, text: &str) -> signex_types::pcb::BoardGraphic {
    serde_json::from_value(serde_json::json!({
        "uuid": Uuid::from_u128(91), "graphic_type": kind, "layer": "TopSilkscreen",
        "width": 0.1, "start": {"x": 0.0, "y": 0.0}, "end": {"x": 20.0, "y": 5.0},
        "center": null, "text": text, "font_size": 1.0
    }))
    .expect("a board drawing")
}

#[test]
fn a_full_width_text_frame_line_stays_inside_its_box() {
    // 1 mm text in a 20 mm box wraps at 32 characters; the stroke font is
    // wider than the wrap assumes, so the line is condensed to the box.
    let mut board = fixture_board();
    board
        .graphics
        .push(board_drawing("text_frame", &"W".repeat(32)));
    let frame = super::frame::FabFrame::of(&board);
    let mut images = super::collect::Images::new();
    let mut tally = super::notes::Tally::default();
    super::text::texts(&mut images, &board, frame, &mut tally);
    assert_eq!(tally.skipped, 0);
    let left = frame.of_mm(p(0.0, 0.0), "left").unwrap().x;
    let right = frame.of_mm(p(20.0, 0.0), "right").unwrap().x;
    let mut written = 0;
    for mark in images.values().flatten() {
        let super::marks::Mark::Stroke {
            width,
            start,
            steps,
            ..
        } = mark
        else {
            continue;
        };
        let ends = steps.iter().filter_map(|step| match step {
            super::marks::Step::Line(point) => Some(*point),
            _ => None,
        });
        for point in std::iter::once(*start).chain(ends) {
            written += 1;
            assert!(
                point.x - width / 2 >= left && point.x + width / 2 <= right,
                "{point:?} leaves the box {left}..{right}"
            );
        }
    }
    assert!(written > 0, "the line is written");
}

#[test]
fn a_designator_the_geometry_refuses_is_counted_as_skipped() {
    let mut board = with_designator("TopCopper", false);
    let designator = board.footprints[0].properties.last_mut().unwrap();
    designator.position = Some(p(1.0e9, 0.0));
    let set = generated(&board);
    assert_eq!(set.notes, vec![FabNote::ItemsSkipped { count: 1 }]);
    assert_eq!(strokes_in(&set, "fab-legend-top.gbr"), 0);
}

#[test]
fn a_board_drawing_of_kind_text_is_counted_as_skipped() {
    let mut board = fixture_board();
    board.graphics.push(board_drawing("text", "REV A"));
    let set = generated(&board);
    assert_eq!(set.notes, vec![FabNote::ItemsSkipped { count: 1 }]);
}

#[test]
fn only_the_selected_outputs_are_written() {
    let options = FabOptions {
        gerber: false,
        test_netlist: false,
        ..FabOptions::all("fab")
    };
    let set = generate(&fixture_board(), &FabExtras::default(), &options).expect("generates");
    let kinds: Vec<&FabFileKind> = set.files.iter().map(|f| &f.kind).collect();
    assert!(matches!(kinds[0], FabFileKind::Drill { .. }));
    assert!(
        kinds
            .iter()
            .all(|k| !matches!(k, FabFileKind::Gerber { .. } | FabFileKind::TestNetlist))
    );
    assert!(matches!(kinds.last(), Some(FabFileKind::Job)));
}

fn via_on(id: u128, at: Point, via_type: ViaType, layers: &[&str]) -> Via {
    Via {
        uuid: Uuid::from_u128(id),
        position: at,
        diameter: 0.45,
        drill: 0.2,
        layers: layers.iter().map(|l| l.to_string()).collect(),
        net: 1,
        via_type,
        unknown_columns: Default::default(),
        attributes: Default::default(),
        layer_sizes: None,
        locked: false,
    }
}

#[test]
fn a_blind_or_buried_via_is_drilled_only_through_its_span() {
    let mut board = fixture_board();
    board.vias.push(via_on(
        41,
        p(3.0, 3.0),
        ViaType::Blind,
        &["TopCopper", "Inner1Copper"],
    ));
    board.vias.push(via_on(
        42,
        p(4.0, 4.0),
        ViaType::Micro,
        &["Inner1Copper", "Inner2Copper"],
    ));
    let set = generated(&board);
    let blind = file(&set, "fab-blind-l1-l2.drl").expect("the blind span's file");
    assert!(blind.contains("; #@! TF.FileFunction,Plated,1,2,Blind\n"));
    assert!(blind.contains("T1C0.200\n"), "{blind}");
    assert!(blind.contains("T1\nX3.000Y-3.000\n"), "{blind}");
    let buried = file(&set, "fab-buried-l2-l3.drl").expect("the buried span's file");
    assert!(buried.contains("; #@! TF.FileFunction,Plated,2,3,Buried\n"));
    assert!(buried.contains("X4.000Y-4.000\n"), "{buried}");
    let pth = file(&set, "fab-pth.drl").expect("plated through file");
    assert!(pth.contains("; #@! TF.FileFunction,Plated,1,4,PTH\n"));
    assert!(!pth.contains("X3.000Y-3.000"), "{pth}");
    assert!(!pth.contains("X4.000Y-4.000"), "{pth}");
    let table = file(&set, "fab-drill-table.csv").expect("drill table");
    assert!(table.contains("Blind L1-L2,T1,0.200,1,0\n"), "{table}");
    assert!(table.contains("Buried L2-L3,T1,0.200,1,0\n"), "{table}");
    let job = file(&set, "fab-job.gbrjob").expect("job file");
    assert!(job.contains("\"Plated,1,2,Blind\""), "{job}");
    assert!(job.contains("\"Plated,2,3,Buried\""), "{job}");
    let bottom = file(&set, "fab-copper-l4-bottom.gbr").expect("bottom copper");
    assert!(!bottom.contains("X3000000Y-3000000D03*"), "{bottom}");
    assert!(set.notes.is_empty(), "{:?}", set.notes);
}

#[test]
fn a_blind_via_naming_one_layer_is_not_drilled_through_the_board() {
    let mut board = fixture_board();
    board
        .vias
        .push(via_on(43, p(3.0, 3.0), ViaType::Blind, &["TopCopper"]));
    let set = generated(&board);
    let pth = file(&set, "fab-pth.drl").expect("plated through file");
    assert!(!pth.contains("X3.000Y-3.000"), "{pth}");
    assert_eq!(set.notes, vec![FabNote::ItemsSkipped { count: 1 }]);
}

#[test]
fn a_keepout_is_not_counted_as_an_unfilled_zone() {
    let mut board = fixture_board();
    let keepout = Zone {
        fill_type: "keepout".into(),
        ..zone("TopCopper")
    };
    board.zones.push(keepout);
    assert!(generated(&board).notes.is_empty());
    board.zones.push(zone("BottomCopper"));
    assert_eq!(
        generated(&board).notes,
        vec![FabNote::ZonesNotFilled { count: 1 }]
    );
}

/// #2506: an unfilled pour is counted as not filled even when the fills of
/// the other pours are supplied; a shelved pour is not counted.
#[test]
fn an_unfilled_pour_is_counted_as_not_filled_beside_supplied_fills() {
    let mut board = fixture_board();
    let pour = |n: u128| Zone {
        uuid: Uuid::from_u128(n),
        fill_type: signex_types::zone_pour::POUR_FILL.into(),
        ..zone("BottomCopper")
    };
    board.zones.push(pour(81));
    board.zones.push(Zone {
        unfilled: true,
        ..pour(82)
    });
    board.zones.push(Zone {
        shelved: true,
        ..pour(83)
    });
    let ring =
        |pts: &[(f64, f64)]| -> Vec<Point> { pts.iter().map(|&(x, y)| Point::new(x, y)).collect() };
    let extras = FabExtras {
        zone_fills: vec![FilledRegion {
            layer: PcbLayer::Copper(CopperLayer::Bottom),
            net_name: "GND".into(),
            outline: ring(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0)]),
            holes: Vec::new(),
        }],
        strokes: Vec::new(),
    };
    let set = generate(&board, &extras, &FabOptions::all("fab")).expect("generates");
    assert_eq!(set.notes, vec![FabNote::ZonesNotFilled { count: 1 }]);
}

#[test]
fn a_part_without_a_designator_is_noted_not_dropped_silently() {
    let mut board = fixture_board();
    board
        .footprints
        .push(footprint(56, "", p(1.0, 1.0), 0.0, "TopCopper", Vec::new()));
    let set = generated(&board);
    assert_eq!(
        set.notes,
        vec![FabNote::PartsWithoutDesignator { count: 1 }]
    );
    let centroid = file(&set, "fab-centroid.csv").expect("centroid");
    assert_eq!(centroid.lines().count(), 3, "{centroid}");
}

#[test]
fn mask_and_paste_openings_carry_no_copper_pad_function() {
    let set = generated(&fixture_board());
    for name in ["fab-soldermask-top.gbr", "fab-paste-top.gbr"] {
        let image = file(&set, name).expect("an opening image");
        assert!(
            image.contains("%TA.AperFunction,Other,PadOpening*%"),
            "{image}"
        );
        for copper_only in ["SMDPad", "ComponentPad", "ConnectorPad", "OtherPad"] {
            assert!(!image.contains(copper_only), "{name}: {image}");
        }
    }
    let top = file(&set, "fab-copper-l1-top.gbr").expect("top copper");
    assert!(top.contains("%TA.AperFunction,SMDPad,CuDef*%"), "{top}");
}
