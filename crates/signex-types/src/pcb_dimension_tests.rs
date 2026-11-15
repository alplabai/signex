use super::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

fn dimension(start: Point, end: Point, offset: f64) -> LinearDimension {
    LinearDimension::new(Uuid::from_u128(1), start, end, offset)
}

#[test]
fn a_new_dimension_is_on_the_dimensions_layer_at_the_pcb_defaults() {
    let d = dimension(p(0.0, 0.0), p(3.0, 4.0), 1.0);
    assert_eq!(d.layer, "Dimensions");
    assert_eq!(
        d.pcb_layer(),
        Some(PcbLayer::Technical(TechnicalLayer::Dimensions))
    );
    assert_eq!(d.line_width, PCB_DEFAULT_GRAPHIC_WIDTH_MM);
    assert_eq!(d.text_height, PCB_DEFAULT_TEXT_SIZE_MM);
    assert_eq!(d.precision, DIMENSION_DEFAULT_PRECISION);
    assert!((d.value_mm() - 5.0).abs() < 1e-12);
}

#[test]
fn a_horizontal_dimension_stands_off_its_points_with_arrows_and_its_value() {
    let mut d = dimension(p(0.0, 0.0), p(10.0, 0.0), 2.0);
    d.text_height = 1.0;
    let drawing = d.drawing().expect("two distinct points");
    assert_eq!(drawing.value_mm, 10.0);
    assert_eq!(drawing.label_rotation_deg, 0.0);
    assert!(
        close(drawing.label_at, p(5.0, 3.0)),
        "{:?}",
        drawing.label_at
    );
    let s = &drawing.strokes;
    assert_eq!(s.len(), 7);
    assert!(close(s[0].0, p(0.0, 0.0)) && close(s[0].1, p(0.0, 2.5)));
    assert!(close(s[1].0, p(10.0, 0.0)) && close(s[1].1, p(10.0, 2.5)));
    assert!(close(s[2].0, p(0.0, 2.0)) && close(s[2].1, p(10.0, 2.0)));
    assert!(close(s[3].1, p(1.0, 2.3)) && close(s[4].1, p(1.0, 1.7)));
    assert!(close(s[5].1, p(9.0, 2.3)) && close(s[6].1, p(9.0, 1.7)));
}

#[test]
fn a_negative_offset_stands_off_on_the_other_side() {
    let mut d = dimension(p(0.0, 0.0), p(10.0, 0.0), -2.0);
    d.text_height = 1.0;
    let drawing = d.drawing().unwrap();
    assert!(close(drawing.strokes[2].0, p(0.0, -2.0)));
    assert!(close(drawing.strokes[0].1, p(0.0, -2.5)));
    assert!(close(drawing.label_at, p(5.0, -3.0)));
}

#[test]
fn the_value_never_reads_upside_down() {
    let leftward = dimension(p(10.0, 0.0), p(0.0, 0.0), 1.0).drawing().unwrap();
    assert_eq!(leftward.label_rotation_deg, 0.0);
    let up = dimension(p(0.0, 0.0), p(0.0, 5.0), 1.0).drawing().unwrap();
    assert!((up.label_rotation_deg - 90.0).abs() < 1e-9);
    let down = dimension(p(0.0, 5.0), p(0.0, 0.0), 1.0).drawing().unwrap();
    assert!((down.label_rotation_deg - 90.0).abs() < 1e-9);
    let slanted = dimension(p(4.0, 4.0), p(0.0, 0.0), 1.0).drawing().unwrap();
    assert!((slanted.label_rotation_deg - 45.0).abs() < 1e-9);
}

#[test]
fn a_dimension_of_one_point_or_a_number_that_is_not_finite_draws_nothing() {
    assert!(dimension(p(1.0, 1.0), p(1.0, 1.0), 1.0).drawing().is_none());
    assert!(
        dimension(p(0.0, 0.0), p(1.0, 0.0), f64::NAN)
            .drawing()
            .is_none()
    );
    let far = dimension(p(0.0, 0.0), p(f64::INFINITY, 0.0), 1.0);
    assert!(far.drawing().is_none());
    for height in [0.0, -1.0] {
        let mut flat = dimension(p(0.0, 0.0), p(1.0, 0.0), 1.0);
        flat.text_height = height;
        assert!(flat.drawing().is_none(), "text height {height}");
    }
}

#[test]
fn a_dimension_round_trips_through_json() {
    let d = dimension(p(0.0, 0.0), p(3.0, 4.0), -1.5);
    let json = serde_json::to_string(&d).unwrap();
    let back: LinearDimension = serde_json::from_str(&json).unwrap();
    assert_eq!(back, d);
}
