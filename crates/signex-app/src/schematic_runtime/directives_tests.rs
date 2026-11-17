//! #2420: how directives are drawn and picked.

use super::directives::{hit, push_lines, strokes};
use super::hit_index::HitIndex;
use super::*;
use signex_types::schematic::SchDirective;

fn blanket() -> SchDirective {
    SchDirective::Blanket {
        uuid: uuid::Uuid::from_u128(1),
        start: Point::new(10.0, 10.0),
        end: Point::new(50.0, 40.0),
        net_class: None,
        erc_exclusions: Vec::new(),
    }
}

fn diff_pair() -> SchDirective {
    SchDirective::DiffPair {
        uuid: uuid::Uuid::from_u128(2),
        position: Point::new(70.0, 70.0),
    }
}

fn sheet() -> SchematicRenderSnapshot {
    SchematicSheet {
        directives: vec![blanket(), diff_pair()],
        ..Default::default()
    }
}

#[test]
fn a_region_is_its_four_edges_and_a_pair_two_strokes() {
    let edges = strokes(&blanket(), 3.0);
    assert_eq!(edges.len(), 4);
    assert!(edges.contains(&(Point::new(10.0, 10.0), Point::new(50.0, 10.0))));
    assert!(edges.contains(&(Point::new(10.0, 40.0), Point::new(10.0, 10.0))));
    let marks = strokes(&diff_pair(), 3.0);
    assert_eq!(marks.len(), 2);
    assert!(marks.iter().all(|(a, b)| a.y == b.y && a.x < b.x));
}

#[test]
fn a_region_is_picked_at_its_edge_only() {
    let snap = sheet();
    let id = blanket().uuid();
    assert!(hit(&snap, id, Point::new(30.0, 10.2)));
    assert!(hit(&snap, id, Point::new(49.8, 25.0)));
    assert!(!hit(&snap, id, Point::new(30.0, 25.0)));
    assert!(!hit(&snap, id, Point::new(30.0, 5.0)));
    assert!(hit(&snap, diff_pair().uuid(), Point::new(70.5, 70.5)));
}

#[test]
fn the_hover_index_picks_what_a_click_picks() {
    let snap = sheet();
    let designators = Designators::new();
    let index = HitIndex::build(&snap, &designators, WireWidths::default());
    for (x, y) in [
        (30.0, 10.2),
        (30.0, 25.0),
        (49.8, 25.0),
        (70.5, 70.5),
        (0.0, 0.0),
    ] {
        assert_eq!(
            index.hit(x, y),
            super::hit_test::hit_test(&snap, &designators, x, y),
            "at {x},{y}"
        );
    }
}

#[test]
fn every_directive_is_drawn_in_the_directive_colour_with_its_dash() {
    let snap = sheet();
    let colors = signex_types::theme::canvas_colors(signex_types::theme::ThemeId::GraphiteDark);
    let transform = ScreenTransform {
        offset_x: 0.0,
        offset_y: 0.0,
        scale: 3.0,
        device_px_per_logical_px: signex_types::schematic::UNSCALED_DEVICE_PX_PER_LOGICAL_PX,
    };
    let bounds = Rectangle::new(iced::Point::ORIGIN, iced::Size::new(1000.0, 1000.0));
    let mut wires = Vec::new();
    let mut styles = HashMap::new();
    push_lines(
        &snap,
        &transform,
        &colors,
        bounds,
        None,
        &mut wires,
        &mut styles,
    );
    assert_eq!(wires.len(), 6);
    let want = to_rgba(to_iced(&colors.no_connect));
    assert!(wires.iter().all(|w| w.explicit_color == Some(want)));
    assert_eq!(
        styles
            .values()
            .filter(|s| **s == NetLineStyle::Dashed)
            .count(),
        4
    );
}
