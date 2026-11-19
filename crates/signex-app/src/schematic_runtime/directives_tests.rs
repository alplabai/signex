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
        track_width: None,
        pour: signex_types::schematic::BlanketPour::Normal,
        include_edge_nets: false,
    }
}

fn diff_pair() -> SchDirective {
    SchDirective::DiffPair {
        uuid: uuid::Uuid::from_u128(2),
        position: Point::new(70.0, 70.0),
        class: None,
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

fn net_parameters(show: bool) -> SchDirective {
    SchDirective::NetParameters {
        uuid: uuid::Uuid::from_u128(3),
        position: Point::new(100.0, 100.0),
        net_class: Some("POWER_IN".to_string()),
        diff_pair_class: None,
        rules: signex_types::schematic::NetRules {
            width_mm: Some(0.8),
            clearance_mm: Some(0.3),
            ..Default::default()
        },
        show_on_flag: show,
    }
}

/// #1281: the flag stands above its wire on a stem, holds its text and
/// lists the rules only when told to.
#[test]
fn a_net_parameters_flag_sits_above_its_wire_and_holds_its_text() {
    use super::directives::{flag, flag_lines};
    assert_eq!(flag_lines(&net_parameters(false)), ["Net Class = POWER_IN"]);
    assert_eq!(
        flag_lines(&net_parameters(true)),
        ["Net Class = POWER_IN", "Width 0.8 mm · Clr 0.3 mm"]
    );
    let f = flag(&net_parameters(true)).expect("a flag");
    assert!(f.rect.max_y < 100.0, "above the wire: {f:?}");
    assert_eq!(f.stem.map(|(a, _)| a), Some(Point::new(100.0, 100.0)));
    // Every character fits: at least 0.6 of the text height each.
    let longest = f.lines.iter().map(|l| l.chars().count()).max().unwrap();
    assert!(f.rect.width() >= longest as f64 * 1.27 * 0.6, "{f:?}");
}

#[test]
fn a_blanket_class_flag_sits_below_the_outline_clear_of_its_edge() {
    use super::directives::flag;
    let SchDirective::Blanket {
        uuid,
        start,
        end,
        erc_exclusions,
        track_width,
        pour,
        include_edge_nets,
        ..
    } = blanket()
    else {
        unreachable!()
    };
    let classed = SchDirective::Blanket {
        uuid,
        start,
        end,
        net_class: Some("SW_NODE".to_string()),
        erc_exclusions,
        track_width,
        pour,
        include_edge_nets,
    };
    let f = flag(&classed).expect("a class flag");
    assert!(f.rect.min_y > 40.0, "below the outline: {f:?}");
    assert!(f.stem.is_none());
    assert!(flag(&blanket()).is_none(), "no class, no flag");
    // The outline's four edges, then the flag box.
    assert_eq!(strokes(&classed, 3.0).len(), 8);
}

#[test]
fn a_net_parameters_directive_is_picked_on_its_flag() {
    let snap = SchematicSheet {
        directives: vec![net_parameters(false)],
        ..Default::default()
    };
    let id = net_parameters(false).uuid();
    let f = super::directives::flag(&net_parameters(false)).unwrap();
    let inside = Point::new(f.rect.min_x + 0.5, (f.rect.min_y + f.rect.max_y) / 2.0);
    assert!(hit(&snap, id, inside));
    assert!(hit(&snap, id, Point::new(100.2, 100.2)));
    assert!(!hit(&snap, id, Point::new(130.0, 130.0)));
}
