use uuid::Uuid;

use super::*;
use crate::object_id::SequentialIds;

fn at(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn close(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

/// A at the origin and B at x 100 mm, both 40 x 25 mm.
fn two_blocks(ids: &mut SequentialIds) -> (BlockDiagram, Uuid, Uuid) {
    let mut d = BlockDiagram::new();
    let a = d
        .add_block(ids, BlockFields::named("A", at(0.0, 0.0)))
        .unwrap();
    let b = d
        .add_block(ids, BlockFields::named("B", at(100.0, 0.0)))
        .unwrap();
    (d, a, b)
}

#[test]
fn a_port_sits_on_its_side_and_an_offset_past_the_edge_is_held_at_its_end() {
    let mut ids = SequentialIds::default();
    let (mut d, a, _) = two_blocks(&mut ids);
    let left = d
        .add_port(&mut ids, a, "IN", PortSide::Left, 5.0, PortDirection::Input)
        .unwrap();
    let bottom = d
        .add_port(
            &mut ids,
            a,
            "EN",
            PortSide::Bottom,
            99.0,
            PortDirection::Input,
        )
        .unwrap();
    let block = d.block(a).unwrap();
    assert!(close(
        block.port_point(block.port(left).unwrap()),
        at(0.0, 5.0)
    ));
    assert!(close(
        block.port_point(block.port(bottom).unwrap()),
        at(40.0, 25.0)
    ));
}

#[test]
fn a_route_between_ports_runs_in_right_angles_and_its_label_sits_midway() {
    let mut ids = SequentialIds::default();
    let (mut d, a, b) = two_blocks(&mut ids);
    let out = d
        .add_port(
            &mut ids,
            a,
            "OUT",
            PortSide::Right,
            5.0,
            PortDirection::Output,
        )
        .unwrap();
    let inp = d
        .add_port(
            &mut ids,
            b,
            "IN",
            PortSide::Left,
            15.0,
            PortDirection::Input,
        )
        .unwrap();
    let from = Endpoint {
        block: a,
        port: Some(out),
    };
    let to = Endpoint {
        block: b,
        port: Some(inp),
    };
    let c = d
        .connect(&mut ids, from, to, Interface::Spi, "SPI1")
        .unwrap();
    let link = d.connection(c).unwrap();
    let route = d.route(link).unwrap();
    let want = [
        at(40.0, 5.0),
        at(70.0, 5.0),
        at(70.0, 15.0),
        at(100.0, 15.0),
    ];
    assert_eq!(route.len(), want.len());
    for (got, want) in route.iter().zip(want) {
        assert!(close(*got, want), "{got:?} != {want:?}");
    }
    assert!(close(d.label_point(link).unwrap(), at(70.0, 10.0)));
}

#[test]
fn a_route_without_ports_leaves_the_facing_sides() {
    let mut ids = SequentialIds::default();
    let mut d = BlockDiagram::new();
    let a = d
        .add_block(&mut ids, BlockFields::named("A", at(0.0, 0.0)))
        .unwrap();
    let b = d
        .add_block(&mut ids, BlockFields::named("B", at(0.0, 100.0)))
        .unwrap();
    let ends = (
        Endpoint {
            block: a,
            port: None,
        },
        Endpoint {
            block: b,
            port: None,
        },
    );
    let c = d
        .connect(&mut ids, ends.0, ends.1, Interface::Power, "")
        .unwrap();
    let route = d.route(d.connection(c).unwrap()).unwrap();
    assert!(close(route[0], at(20.0, 25.0)), "{:?}", route[0]);
    assert!(close(route[3], at(20.0, 100.0)), "{:?}", route[3]);
}

#[test]
fn hits_find_the_topmost_block_the_nearest_port_and_a_connection_near_its_route() {
    let mut ids = SequentialIds::default();
    let (mut d, a, b) = two_blocks(&mut ids);
    let over = d
        .add_block(&mut ids, BlockFields::named("C", at(10.0, 10.0)))
        .unwrap();
    assert_eq!(d.block_at(at(15.0, 15.0)), Some(over));
    assert_eq!(d.block_at(at(1.0, 1.0)), Some(a));
    assert_eq!(d.block_at(at(70.0, 1.0)), None);
    let port = d
        .add_port(
            &mut ids,
            b,
            "IN",
            PortSide::Left,
            10.0,
            PortDirection::Input,
        )
        .unwrap();
    assert_eq!(d.port_at(at(101.0, 10.5), 2.0), Some((b, port)));
    assert_eq!(d.port_at(at(110.0, 10.0), 2.0), None);
    let ends = (
        Endpoint {
            block: a,
            port: None,
        },
        Endpoint {
            block: b,
            port: Some(port),
        },
    );
    let c = d
        .connect(&mut ids, ends.0, ends.1, Interface::I2c, "")
        .unwrap();
    let mid = d.label_point(d.connection(c).unwrap()).unwrap();
    assert_eq!(d.connection_at(at(mid.x + 0.5, mid.y), 1.0), Some(c));
    assert_eq!(d.connection_at(at(mid.x, mid.y + 50.0), 1.0), None);
}

#[test]
fn a_group_frame_holds_its_blocks_with_a_margin_and_an_empty_group_has_none() {
    let mut ids = SequentialIds::default();
    let (mut d, a, b) = two_blocks(&mut ids);
    let g = d.add_group(&mut ids, "Power", None).unwrap();
    assert_eq!(d.group_frame(g), None);
    for block in [a, b] {
        let patch = BlockPatch {
            group: Some(Some(g)),
            ..BlockPatch::default()
        };
        d.set_block(block, patch).unwrap();
    }
    let frame = d.group_frame(g).unwrap();
    let m = GROUP_MARGIN_MM;
    assert_eq!(
        frame,
        DiagramBox {
            x: -m,
            y: -m,
            width: 140.0 + 2.0 * m,
            height: 25.0 + 2.0 * m,
        }
    );
    assert_eq!(d.group_at(at(-m, 10.0), 0.5), Some(g));
    assert_eq!(d.group_at(at(50.0, 10.0), 0.5), None);
}

#[test]
fn a_new_block_without_a_position_goes_right_of_the_others() {
    let mut ids = SequentialIds::default();
    assert_eq!(BlockDiagram::new().next_free_position(), Point::ZERO);
    let (d, _, _) = two_blocks(&mut ids);
    assert!(close(
        d.next_free_position(),
        at(140.0 + PLACEMENT_GAP_MM, 0.0)
    ));
}
