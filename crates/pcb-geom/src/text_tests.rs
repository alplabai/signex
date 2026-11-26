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

fn designator(position: Option<Point>, rotation: f64) -> PcbProperty {
    PcbProperty {
        key: BoardTextField::Designator.key().into(),
        value: "R1".into(),
        position,
        rotation,
        layer: Some("TopSilkscreen".into()),
        font_size: None,
        hidden: false,
        justify: HAlign::Center,
    }
}

fn mm(p: PointNm) -> (f64, f64) {
    let p = p.to_mm();
    (p.x, p.y)
}

/// Within a few nm: a point goes through the integer-nm frame twice.
fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-5 && (a.1 - b.1).abs() < 1e-5
}

#[test]
fn the_text_follows_the_footprint_placement() {
    let text = designator(Some(Point::new(1.0, 0.0)), 0.0);
    let placed = property_text_placement(&footprint(0.0, "TopCopper"), &text).unwrap();
    assert_eq!(mm(placed.anchor), (11.0, 20.0));
    assert_eq!(placed.rotation_deg, 0.0);
    assert!(!placed.mirrored);
    // A quarter turn carries the local x axis onto the board's y axis.
    let placed = property_text_placement(&footprint(90.0, "TopCopper"), &text).unwrap();
    assert_eq!(mm(placed.anchor), (10.0, 21.0));
    assert_eq!(placed.rotation_deg, 90.0);
}

#[test]
fn a_bottom_footprint_mirrors_its_text() {
    let text = designator(Some(Point::new(1.0, 0.5)), 30.0);
    let placed = property_text_placement(&footprint(90.0, "BottomCopper"), &text).unwrap();
    // Mirrored x (-1, 0.5), turned 90 degrees: (-0.5, -1).
    assert_eq!(mm(placed.anchor), (9.5, 19.0));
    assert_eq!(placed.rotation_deg, 60.0);
    assert!(placed.mirrored);
}

#[test]
fn a_bottom_footprint_text_is_mirrored_on_the_bottom_overlay() {
    let mut fp = footprint(0.0, "BottomCopper");
    fp.properties = vec![PcbProperty {
        layer: None,
        ..designator(Some(Point::new(0.0, 0.0)), 0.0)
    }];
    let texts = footprint_texts(&fp);
    assert_eq!(texts.len(), 1);
    let text = &texts[0];
    assert_eq!(text.field, BoardTextField::Designator);
    assert_eq!(
        text.layer,
        PcbLayer::Technical(TechnicalLayer::SilkscreenBottom)
    );
    assert!(text.placement.mirrored);
    // Mirrored, the line reads towards -x: a left-justified text runs left
    // of its anchor.
    let left = FootprintText {
        justify: HAlign::Left,
        ..text.clone()
    };
    let strokes = left.strokes().unwrap();
    let xs: Vec<i64> = strokes.iter().flatten().map(|p| p.x).collect();
    let anchor = text.placement.anchor.x;
    assert!(xs.iter().all(|&x| x <= anchor));
    assert!(xs.iter().any(|&x| x < anchor));
}

#[test]
fn a_text_with_no_position_takes_the_default_spot() {
    let placed =
        property_text_placement(&footprint(0.0, "TopCopper"), &designator(None, 0.0)).unwrap();
    assert_eq!(mm(placed.anchor), (10.0, 20.0 - PCB_FP_TEXT_OFFSET_MM));
    let comment = PcbProperty {
        key: BoardTextField::Comment.key().into(),
        ..designator(None, 0.0)
    };
    let placed = property_text_placement(&footprint(0.0, "TopCopper"), &comment).unwrap();
    assert_eq!(mm(placed.anchor), (10.0, 20.0 + PCB_FP_TEXT_OFFSET_MM));
}

#[test]
fn only_the_designator_and_comment_are_board_texts() {
    assert!(is_board_text_key("Reference") && is_board_text_key("Value"));
    assert!(!is_board_text_key("MPN") && !is_board_text_key("value"));
}

#[test]
fn hidden_and_empty_texts_are_not_shown() {
    let mut fp = footprint(0.0, "TopCopper");
    fp.properties = vec![
        PcbProperty {
            hidden: true,
            ..designator(None, 0.0)
        },
        PcbProperty {
            key: "Value".into(),
            value: "  ".into(),
            ..designator(None, 0.0)
        },
        PcbProperty {
            key: "MPN".into(),
            ..designator(None, 0.0)
        },
    ];
    assert!(footprint_texts(&fp).is_empty());
    // Hidden, it is still placed for an edit.
    assert!(footprint_text(&fp, BoardTextField::Designator).is_some_and(|t| t.is_ok()));
}

#[test]
fn the_size_falls_back_to_the_default() {
    let mut property = designator(None, 0.0);
    assert_eq!(property_text_size_mm(&property), PCB_DEFAULT_TEXT_SIZE_MM);
    property.font_size = Some(f64::NAN);
    assert_eq!(property_text_size_mm(&property), PCB_DEFAULT_TEXT_SIZE_MM);
    property.font_size = Some(1.5);
    assert_eq!(property_text_size_mm(&property), 1.5);
}

#[test]
fn justification_moves_the_box_along_the_line() {
    let at = TextPlacement {
        anchor: PointNm::new(0, 0),
        rotation_deg: 0.0,
        mirrored: false,
    };
    let centre = |justify| mm(justified_centre(&at, justify, 4.0).unwrap());
    assert!(close(centre(HAlign::Left), (2.0, 0.0)));
    assert!(close(centre(HAlign::Center), (0.0, 0.0)));
    assert!(close(centre(HAlign::Right), (-2.0, 0.0)));
    // Turned a quarter, the line runs down the board's y axis.
    let turned = TextPlacement {
        rotation_deg: 90.0,
        ..at
    };
    assert!(close(
        mm(justified_centre(&turned, HAlign::Left, 4.0).unwrap()),
        (0.0, 2.0)
    ));
    // Mirrored, it runs the other way.
    let mirrored = TextPlacement {
        mirrored: true,
        ..at
    };
    assert!(close(
        mm(justified_centre(&mirrored, HAlign::Left, 4.0).unwrap()),
        (-2.0, 0.0)
    ));
}

#[test]
fn a_board_point_and_angle_turn_back_into_the_footprint_frame() {
    for layer in ["TopCopper", "BottomCopper"] {
        let fp = footprint(90.0, layer);
        let world = Point::new(12.0, 19.0);
        let local = local_text_position(&fp, world).unwrap();
        let property = designator(Some(local), local_text_rotation(&fp, 45.0).unwrap());
        let placed = property_text_placement(&fp, &property).unwrap();
        assert!(close(mm(placed.anchor), (12.0, 19.0)), "{layer}");
        let off = (placed.rotation_deg - 45.0).rem_euclid(360.0);
        assert!(off.min(360.0 - off) < 1e-9, "{layer}: {off}");
    }
}

#[test]
fn a_non_finite_text_is_refused() {
    let text = designator(None, f64::NAN);
    assert!(property_text_placement(&footprint(0.0, "TopCopper"), &text).is_err());
    let text = designator(Some(Point::new(f64::INFINITY, 0.0)), 0.0);
    assert!(property_text_placement(&footprint(0.0, "TopCopper"), &text).is_err());
    assert!(local_text_rotation(&footprint(0.0, "TopCopper"), f64::NAN).is_err());
}
