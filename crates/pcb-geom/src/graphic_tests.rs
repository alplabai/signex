use super::*;
use uuid::Uuid;

fn footprint(rotation: f64, layer: &str) -> Footprint {
    Footprint {
        uuid: Uuid::nil(),
        reference: "R1".into(),
        value: String::new(),
        footprint_id: String::new(),
        position: Point::new(10.0, 20.0),
        rotation,
        layer: layer.into(),
        locked: false,
        pads: Vec::new(),
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: None,
        library_ref: None,
        component: Default::default(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    }
}

fn graphic(kind: &str) -> FpGraphic {
    FpGraphic {
        graphic_type: kind.into(),
        layer: "TopSilkscreen".into(),
        width: 0.12,
        start: None,
        end: None,
        center: None,
        mid: None,
        radius: 0.0,
        points: Vec::new(),
        text: String::new(),
        font_size: 0.0,
        position: None,
        rotation: 0.0,
        fill: String::new(),
    }
}

fn line(from: (f64, f64), to: (f64, f64)) -> FpGraphic {
    FpGraphic {
        start: Some(Point::new(from.0, from.1)),
        end: Some(Point::new(to.0, to.1)),
        ..graphic("line")
    }
}

fn mm(p: PointNm) -> (f64, f64) {
    let p = p.to_mm();
    (p.x, p.y)
}

fn placed(fp: &Footprint, g: &FpGraphic) -> PlacedGraphic {
    footprint_graphic_placement(fp, g)
        .expect("finite")
        .expect("a known kind")
}

#[test]
fn a_line_is_turned_and_moved_with_its_footprint() {
    let g = line((1.0, 0.0), (2.0, 0.0));
    let at = placed(&footprint(90.0, "TopCopper"), &g);
    let GraphicShape::Line { a, b } = at.shape else {
        panic!("{at:?}");
    };
    assert_eq!((mm(a), mm(b)), ((10.0, 21.0), (10.0, 22.0)));
    assert_eq!(at.width, 120_000);
    assert!(!at.filled);
}

#[test]
fn a_bottom_footprint_mirrors_its_line() {
    let g = line((1.0, 0.5), (2.0, 0.5));
    let at = placed(&footprint(0.0, "BottomCopper"), &g);
    let GraphicShape::Line { a, b } = at.shape else {
        panic!("{at:?}");
    };
    assert_eq!((mm(a), mm(b)), ((9.0, 20.5), (8.0, 20.5)));
}

#[test]
fn a_rectangle_is_four_turned_corners() {
    let g = FpGraphic {
        start: Some(Point::new(-1.0, -0.5)),
        end: Some(Point::new(1.0, 0.5)),
        fill: "solid".into(),
        ..graphic("rect")
    };
    let at = placed(&footprint(90.0, "TopCopper"), &g);
    let GraphicShape::Polygon { points } = &at.shape else {
        panic!("{at:?}");
    };
    let corners: Vec<_> = points.iter().map(|&p| mm(p)).collect();
    assert_eq!(
        corners,
        [(10.5, 19.0), (10.5, 21.0), (9.5, 21.0), (9.5, 19.0)]
    );
    assert!(at.filled);
}

#[test]
fn an_arc_keeps_its_way_round_and_the_mirror_reverses_it() {
    // A quarter from (1, 0) through the middle to (0, 1): increasing angle.
    let half = std::f64::consts::FRAC_1_SQRT_2;
    let g = FpGraphic {
        center: Some(Point::new(0.0, 0.0)),
        start: Some(Point::new(1.0, 0.0)),
        mid: Some(Point::new(half, half)),
        end: Some(Point::new(0.0, 1.0)),
        radius: 1.0,
        ..graphic("arc")
    };
    let top = placed(&footprint(0.0, "TopCopper"), &g);
    let GraphicShape::Arc {
        center,
        radius,
        start_deg,
        sweep_deg,
    } = top.shape
    else {
        panic!("{top:?}");
    };
    assert_eq!(mm(center), (10.0, 20.0));
    assert_eq!(radius, 1_000_000);
    assert!(start_deg.abs() < 1e-9 && (sweep_deg - 90.0).abs() < 1e-9);

    // The same arc through the other middle runs the other way.
    let other = FpGraphic {
        mid: Some(Point::new(-half, -half)),
        ..g.clone()
    };
    let GraphicShape::Arc { sweep_deg, .. } = placed(&footprint(0.0, "TopCopper"), &other).shape
    else {
        panic!("arc");
    };
    assert!((sweep_deg + 270.0).abs() < 1e-9, "{sweep_deg}");

    // Mirrored: the start lands at (-1, 0), and the sweep runs back.
    let bottom = placed(&footprint(0.0, "BottomCopper"), &g);
    let GraphicShape::Arc {
        start_deg,
        sweep_deg,
        ..
    } = bottom.shape
    else {
        panic!("{bottom:?}");
    };
    assert!((start_deg.abs() - 180.0).abs() < 1e-9, "{start_deg}");
    assert!((sweep_deg + 90.0).abs() < 1e-9, "{sweep_deg}");
}

#[test]
fn a_circle_takes_its_radius_and_a_text_its_placement() {
    let circle = FpGraphic {
        center: Some(Point::new(1.0, 0.0)),
        radius: 0.5,
        ..graphic("circle")
    };
    let at = placed(&footprint(90.0, "TopCopper"), &circle);
    assert_eq!(
        at.shape,
        GraphicShape::Circle {
            center: PointNm::new(10_000_000, 21_000_000),
            radius: 500_000,
        }
    );
    let text = FpGraphic {
        position: Some(Point::new(1.0, 0.0)),
        rotation: 30.0,
        text: "TP".into(),
        ..graphic("text")
    };
    let GraphicShape::Text(placement) = placed(&footprint(90.0, "BottomCopper"), &text).shape
    else {
        panic!("text");
    };
    assert_eq!(mm(placement.anchor), (10.0, 19.0));
    assert_eq!(placement.rotation_deg, 60.0);
    assert!(placement.mirrored);
}

#[test]
fn an_unknown_or_incomplete_drawing_is_skipped_and_a_non_finite_one_refused() {
    let fp = footprint(0.0, "TopCopper");
    assert_eq!(
        footprint_graphic_placement(&fp, &graphic("spline")),
        Ok(None)
    );
    assert_eq!(footprint_graphic_placement(&fp, &graphic("line")), Ok(None));
    let bad = line((f64::NAN, 0.0), (1.0, 0.0));
    assert!(footprint_graphic_placement(&fp, &bad).is_err());
}

#[test]
fn a_board_drawing_stays_where_it_is_saved() {
    let edge = BoardGraphic {
        graphic_type: "arc".into(),
        layer: "EdgeCuts".into(),
        width: 0.05,
        start: Some(Point::new(40.0, 0.0)),
        end: Some(Point::new(50.0, 10.0)),
        center: Some(Point::new(40.0, 10.0)),
        radius: 10.0,
        points: Vec::new(),
    };
    let at = board_graphic_placement(&edge).unwrap().unwrap();
    let GraphicShape::Arc {
        center,
        radius,
        start_deg,
        sweep_deg,
    } = at.shape
    else {
        panic!("{at:?}");
    };
    assert_eq!(mm(center), (40.0, 10.0));
    assert_eq!(radius, 10_000_000);
    assert!((start_deg + 90.0).abs() < 1e-9 && (sweep_deg - 90.0).abs() < 1e-9);
}

#[test]
fn a_board_text_on_a_bottom_layer_is_mirrored() {
    let text = |layer: &str| BoardText {
        uuid: Uuid::nil(),
        text: "REV A".into(),
        position: Point::new(5.0, 6.0),
        layer: layer.into(),
        font_size: 1.5,
        rotation: 90.0,
    };
    let top = board_text_placement(&text("TopSilkscreen")).unwrap();
    assert_eq!(mm(top.anchor), (5.0, 6.0));
    assert_eq!(top.rotation_deg, 90.0);
    assert!(!top.mirrored);
    assert!(
        board_text_placement(&text("BottomSilkscreen"))
            .unwrap()
            .mirrored
    );
}
