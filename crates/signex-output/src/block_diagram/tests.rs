//! The block diagram drawing and its SVG file (#2796, mockup row 171).

use signex_types::block_diagram::{
    Block, BlockDiagram, BlockPort, BlockShape, Connection, Endpoint, Interface, PortDirection,
    PortSide, parse_block_diagram,
};
use signex_types::schematic::Point;
use uuid::Uuid;

use super::*;
use crate::svg::block_diagram::block_diagram_svg;

const DCDC: &str =
    include_str!("../../../signex-types/tests/corpus/block_diagram/dcdc-buck.snxblk");

fn dcdc() -> BlockDiagram {
    parse_block_diagram(DCDC)
        .expect("the corpus diagram reads")
        .diagram
}

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn block(n: u128, x: f64, ports: Vec<BlockPort>) -> Block {
    Block {
        id: id(n),
        name: format!("B{n}"),
        function: String::new(),
        note: String::new(),
        shape: BlockShape::Rectangle,
        position: Point::new(x, 0.0),
        width_mm: 40.0,
        height_mm: 20.0,
        color: None,
        group: None,
        rails: Vec::new(),
        specs: Default::default(),
        ports,
        sheet: None,
    }
}

fn port(n: u128, side: PortSide, offset_mm: f64) -> BlockPort {
    BlockPort {
        id: id(n),
        name: format!("P{n}"),
        side,
        offset_mm,
        direction: PortDirection::Output,
    }
}

/// B1 at x 0 with port P11 on its right edge 5 mm down, B2 at x 100, and a
/// connection from P11 to B2's edge carrying `interface`.
fn pair(interface: Interface, label: &str) -> BlockDiagram {
    let mut diagram = BlockDiagram::new();
    diagram.blocks = vec![
        block(1, 0.0, vec![port(11, PortSide::Right, 5.0)]),
        block(2, 100.0, Vec::new()),
    ];
    diagram.connections = vec![Connection {
        id: id(3),
        from: Endpoint {
            block: id(1),
            port: Some(id(11)),
        },
        to: Endpoint {
            block: id(2),
            port: None,
        },
        interface,
        label: label.to_string(),
        derived_from: None,
    }];
    assert_eq!(diagram.fault(), None);
    diagram
}

fn strokes(drawing: &BlockDiagramDrawing) -> Vec<(&Vec<Point>, f64, Rgb)> {
    drawing
        .marks
        .iter()
        .filter_map(|mark| match mark {
            BlockMark::Stroke {
                points,
                closed: false,
                width_mm,
                rgb,
            } => Some((points, *width_mm, *rgb)),
            _ => None,
        })
        .collect()
}

fn texts(drawing: &BlockDiagramDrawing) -> Vec<&str> {
    drawing
        .marks
        .iter()
        .filter_map(|mark| match mark {
            BlockMark::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn ink(mark: &BlockMark) -> Rgb {
    match mark {
        BlockMark::Stroke { rgb, .. }
        | BlockMark::Fill { rgb, .. }
        | BlockMark::Text { rgb, .. } => *rgb,
    }
}

#[test]
fn a_format_is_named_by_its_token_or_the_file_extension() {
    assert_eq!(
        BlockDiagramFormat::from_id(" SVG "),
        Some(BlockDiagramFormat::Svg)
    );
    assert_eq!(
        BlockDiagramFormat::from_id("pdf"),
        Some(BlockDiagramFormat::Pdf)
    );
    assert_eq!(BlockDiagramFormat::from_id("png"), None);
    let from = |p: &str| BlockDiagramFormat::from_path(std::path::Path::new(p));
    assert_eq!(from("out/blocks.PDF"), Some(BlockDiagramFormat::Pdf));
    assert_eq!(from("blocks.svg"), Some(BlockDiagramFormat::Svg));
    assert_eq!(from("blocks"), None);
    assert_eq!(from("blocks.txt"), None);
}

#[test]
fn a_connection_reads_as_its_protocol_then_its_label() {
    let text = |interface, label| connection_text(&pair(interface, label).connections[0]);
    assert_eq!(text(Interface::I2c, "SDA, SCL"), "I2C SDA, SCL");
    assert_eq!(text(Interface::Usb, ""), "USB");
    assert_eq!(text(Interface::Power, "VIN_12V 12.0 V"), "VIN_12V 12.0 V");
    assert_eq!(text(Interface::Signal, ""), "");
    assert_eq!(
        text(Interface::Other("MIPI CSI".into()), "cam"),
        "MIPI CSI cam"
    );
    assert_eq!(text(Interface::Spi, "  two\nlines "), "SPI two lines");
}

#[test]
fn a_connection_leaves_its_port_and_arrives_at_the_facing_edge_with_an_arrow() {
    let drawing = block_diagram_drawing(&pair(Interface::Spi, ""), &BlockDiagramLook::default());
    let links = strokes(&drawing);
    assert_eq!(links.len(), 1, "one connection line");
    let (points, width, rgb) = links[0];
    assert_eq!(points.first(), Some(&Point::new(40.0, 5.0)), "B1's port");
    assert_eq!(
        points.last(),
        Some(&Point::new(100.0, 10.0)),
        "B2's left edge"
    );
    assert!(
        points
            .windows(2)
            .all(|w| w[0].x == w[1].x || w[0].y == w[1].y),
        "right angles only: {points:?}"
    );
    assert_eq!((width, rgb), (LINK_MM, INK));
    let tip = drawing.marks.iter().any(|mark| {
        matches!(mark, BlockMark::Fill { ring, .. } if ring.len() == 3 && ring[0] == Point::new(100.0, 10.0))
    });
    assert!(tip, "the arrow's tip is on the arriving end");
}

#[test]
fn power_is_red_a_bus_is_thick_and_monochrome_is_black_on_white() {
    let look = BlockDiagramLook::default();
    let style = |diagram: &BlockDiagram| {
        let drawing = block_diagram_drawing(diagram, &look);
        let (_, width, rgb) = strokes(&drawing)[0];
        (width, rgb)
    };
    assert_eq!(
        style(&pair(Interface::Power, "VIN")),
        (POWER_LINK_MM, POWER_INK)
    );
    assert_eq!(style(&pair(Interface::Bus, "")), (BUS_LINK_MM, BUS_INK));

    let mono = BlockDiagramLook { monochrome: true };
    let drawing = block_diagram_drawing(&dcdc(), &mono);
    for mark in &drawing.marks {
        assert!(ink(mark) == BLACK || ink(mark) == PAPER, "{mark:?}");
    }
}

#[test]
fn the_corpus_diagram_draws_every_block_group_and_label_inside_its_extent() {
    let diagram = dcdc();
    let drawing = block_diagram_drawing(&diagram, &BlockDiagramLook::default());
    let shown = texts(&drawing);
    for block in &diagram.blocks {
        assert!(
            shown.contains(&block.name.as_str()),
            "{} is drawn",
            block.name
        );
        let far = Point::new(
            block.position.x + block.width_mm,
            block.position.y + block.height_mm,
        );
        for corner in [block.position, far] {
            assert!(
                drawing.min.x <= corner.x && corner.x <= drawing.max.x,
                "{} lies inside the extent",
                block.name
            );
            assert!(drawing.min.y <= corner.y && corner.y <= drawing.max.y);
        }
    }
    for group in &diagram.groups {
        assert!(shown.contains(&group.name.as_str()), "group {}", group.name);
    }
    assert!(shown.contains(&"I2C SDA, SCL"), "{shown:?}");
    // A note of two lines is two texts, never one line with a break in it.
    assert!(shown.contains(&"Not fitted.") && shown.contains(&"Pads only, no header."));
    assert!(shown.iter().all(|t| !t.contains('\n')));
    assert_eq!(
        strokes(&drawing).len(),
        diagram.connections.len(),
        "one line per connection"
    );
}

#[test]
fn every_shape_stays_inside_its_box() {
    for shape in [
        BlockShape::Rectangle,
        BlockShape::RoundedRectangle,
        BlockShape::Ellipse,
        BlockShape::Hexagon,
    ] {
        let mut b = block(1, 10.0, Vec::new());
        b.shape = shape;
        let ring = geometry::block_outline(&b);
        assert!(ring.len() >= 4, "{shape:?}");
        for p in ring {
            assert!(
                (10.0 - 1e-9..=50.0 + 1e-9).contains(&p.x),
                "{shape:?} {p:?}"
            );
            assert!((-1e-9..=20.0 + 1e-9).contains(&p.y), "{shape:?} {p:?}");
        }
    }
}

#[test]
fn an_empty_diagram_draws_nothing_on_a_default_block_area() {
    let drawing = block_diagram_drawing(&BlockDiagram::new(), &BlockDiagramLook::default());
    assert!(drawing.marks.is_empty());
    assert_eq!(drawing.min, Point::new(0.0, 0.0));
    assert_eq!(
        drawing.max,
        Point::new(DEFAULT_BLOCK_WIDTH_MM, DEFAULT_BLOCK_HEIGHT_MM)
    );
    let svg = block_diagram_svg(&drawing);
    assert!(svg.starts_with("<?xml") && svg.ends_with("</svg>\n"));
}

#[test]
fn the_svg_is_the_same_bytes_every_time_and_escapes_its_text() {
    let mut diagram = dcdc();
    diagram.blocks[0].name = "A<B & \"C\"".to_string();
    let look = BlockDiagramLook::default();
    let svg = block_diagram_svg(&block_diagram_drawing(&diagram, &look));
    assert_eq!(
        svg,
        block_diagram_svg(&block_diagram_drawing(&diagram, &look))
    );
    assert!(svg.contains("A&lt;B &amp; "), "{svg}");
    assert!(!svg.contains("A<B"));
    assert!(svg.contains(">DC/DC Buck</text>"));
    assert!(svg.contains(">I2C SDA, SCL</text>"));
    assert!(
        svg.contains("stroke=\"#cc332b\""),
        "a power connection in red"
    );
    // One view-box unit is one mm: the width is the extent plus the margins.
    let drawing = block_diagram_drawing(&diagram, &look);
    let width = crate::svg::board::num(drawing.max.x - drawing.min.x + 10.0);
    assert!(svg.contains(&format!("width=\"{width}mm\"")), "{svg}");
}
