//! Block diagram export (#2796, mockup row 171): one builder turns a
//! diagram into marks in diagram mm (y down): group frames, blocks with
//! their outline, name, function, note and ports, and connections with an
//! arrow and their interface label. `svg::block_diagram` writes the marks
//! as an SVG file and `pdf::block_diagram` as a PDF page, so both formats
//! draw the same picture. Rules: `docs/BLOCK_DIAGRAM_EXPORT.md`.

mod geometry;

use signex_types::block_diagram::{
    Block, BlockDiagram, BlockGroup, BlockPort, Connection, DEFAULT_BLOCK_HEIGHT_MM,
    DEFAULT_BLOCK_WIDTH_MM, Interface,
};
use signex_types::schematic::Point;
use signex_types::theme::Color;

use geometry::{arrow_head, block_outline, connection_path, end_point, label_point, port_anchor};

/// A 0..1 RGB colour.
pub type Rgb = (f32, f32, f32);

/// What a block diagram file is written as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockDiagramFormat {
    Svg,
    Pdf,
}

impl BlockDiagramFormat {
    pub const ALL: [Self; 2] = [Self::Svg, Self::Pdf];

    /// The token the commands use, which is also the file extension.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Svg => "svg",
            Self::Pdf => "pdf",
        }
    }

    /// The format `id` names, ignoring case and surrounding spaces.
    pub fn from_id(id: &str) -> Option<Self> {
        let id = id.trim();
        Self::ALL
            .into_iter()
            .find(|format| format.id().eq_ignore_ascii_case(id))
    }

    /// The format a file name's extension names.
    pub fn from_path(path: &std::path::Path) -> Option<Self> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(Self::from_id)
    }
}

/// How the marks are coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlockDiagramLook {
    /// Every mark in black on white: blocks unfilled, power and buses in
    /// the line ink.
    pub monochrome: bool,
}

/// One mark of a drawing, in diagram mm (y down).
#[derive(Debug, Clone, PartialEq)]
pub enum BlockMark {
    /// A line through `points`, closed back to the first if `closed`.
    Stroke {
        points: Vec<Point>,
        closed: bool,
        width_mm: f64,
        rgb: Rgb,
    },
    /// A filled ring.
    Fill { ring: Vec<Point>, rgb: Rgb },
    /// One line of text centred on `at`, `size_mm` high.
    Text {
        at: Point,
        size_mm: f64,
        text: String,
        rgb: Rgb,
    },
}

/// A block diagram as marks, with the extent they cover.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockDiagramDrawing {
    pub min: Point,
    pub max: Point,
    /// In drawing order: group frames, connections, blocks, labels.
    pub marks: Vec<BlockMark>,
}

const PAPER: Rgb = (1.0, 1.0, 1.0);
const BLACK: Rgb = (0.0, 0.0, 0.0);
/// The line and text ink of a block or a plain connection.
const INK: Rgb = (0.20, 0.22, 0.26);
/// A block with no colour of its own.
const BLOCK_FILL: Rgb = (0.95, 0.96, 0.97);
const GROUP_INK: Rgb = (0.45, 0.47, 0.52);
const POWER_INK: Rgb = (0.80, 0.20, 0.17);
const BUS_INK: Rgb = (0.18, 0.44, 0.86);
/// How much of a block's own colour its fill takes; the rest is paper.
const FILL_TINT: f32 = 0.18;

const BLOCK_EDGE_MM: f64 = 0.35;
const LINK_MM: f64 = 0.35;
const POWER_LINK_MM: f64 = 0.5;
const BUS_LINK_MM: f64 = 0.8;
const GROUP_EDGE_MM: f64 = 0.25;
/// Paper between a group's blocks and its frame.
const GROUP_PAD_MM: f64 = 5.0;
const PORT_MM: f64 = 1.6;
const NAME_MM: f64 = 3.5;
const FUNCTION_MM: f64 = 2.4;
const NOTE_MM: f64 = 2.2;
const PORT_TEXT_MM: f64 = 2.0;
const LINK_TEXT_MM: f64 = 2.2;
const GROUP_TEXT_MM: f64 = 2.6;
/// A text's width per mm of height, for the extent and for placing a port
/// name beside its port. Roboto averages a little under this.
const TEXT_ADVANCE: f64 = 0.6;

/// The drawing of `diagram`. An empty diagram draws nothing over the area
/// of one default block.
pub fn block_diagram_drawing(
    diagram: &BlockDiagram,
    look: &BlockDiagramLook,
) -> BlockDiagramDrawing {
    let mut marks = Vec::new();
    for group in &diagram.groups {
        group_marks(&mut marks, diagram, group, look);
    }
    for link in &diagram.connections {
        connection_marks(&mut marks, diagram, link, look);
    }
    for block in &diagram.blocks {
        block_marks(&mut marks, block, look);
    }
    let (min, max) = extent(&marks);
    BlockDiagramDrawing { min, max, marks }
}

fn rgb_of(color: Color) -> Rgb {
    let c = |v: u8| f32::from(v) / 255.0;
    (c(color.r), c(color.g), c(color.b))
}

fn tint(rgb: Rgb) -> Rgb {
    let mix = |c: f32, p: f32| p + (c - p) * FILL_TINT;
    (
        mix(rgb.0, PAPER.0),
        mix(rgb.1, PAPER.1),
        mix(rgb.2, PAPER.2),
    )
}

fn group_marks(
    marks: &mut Vec<BlockMark>,
    diagram: &BlockDiagram,
    group: &BlockGroup,
    look: &BlockDiagramLook,
) {
    let members: Vec<&Block> = diagram
        .blocks
        .iter()
        .filter(|b| b.group == Some(group.id))
        .collect();
    let Some(first) = members.first() else {
        return;
    };
    let (mut min, mut max) = (first.position, first.position);
    for b in &members {
        min = Point::new(min.x.min(b.position.x), min.y.min(b.position.y));
        max = Point::new(
            max.x.max(b.position.x + b.width_mm),
            max.y.max(b.position.y + b.height_mm),
        );
    }
    let (min, max) = (
        Point::new(min.x - GROUP_PAD_MM, min.y - GROUP_PAD_MM),
        Point::new(max.x + GROUP_PAD_MM, max.y + GROUP_PAD_MM),
    );
    let rgb = if look.monochrome {
        BLACK
    } else {
        group.color.map_or(GROUP_INK, rgb_of)
    };
    marks.push(BlockMark::Stroke {
        points: vec![min, Point::new(max.x, min.y), max, Point::new(min.x, max.y)],
        closed: true,
        width_mm: GROUP_EDGE_MM,
        rgb,
    });
    let width = text_width(&group.name, GROUP_TEXT_MM);
    marks.push(BlockMark::Text {
        at: Point::new(min.x + width / 2.0, min.y - GROUP_TEXT_MM),
        size_mm: GROUP_TEXT_MM,
        text: one_line(&group.name),
        rgb,
    });
}

/// The ink and width of a connection carrying `interface`.
fn link_style(interface: &Interface, look: &BlockDiagramLook) -> (Rgb, f64) {
    let (rgb, width) = match interface {
        Interface::Power => (POWER_INK, POWER_LINK_MM),
        Interface::Bus => (BUS_INK, BUS_LINK_MM),
        _ => (INK, LINK_MM),
    };
    (if look.monochrome { BLACK } else { rgb }, width)
}

/// The text a connection carries: the protocol's name, then its label. A
/// plain signal, power, analog or bus link shows its label alone.
pub fn connection_text(link: &Connection) -> String {
    let prefix = match &link.interface {
        Interface::Signal | Interface::Bus | Interface::Power | Interface::Analog => String::new(),
        Interface::Other(name) => name.clone(),
        known => known.token().to_ascii_uppercase(),
    };
    one_line(&format!("{prefix} {}", link.label))
}

fn connection_marks(
    marks: &mut Vec<BlockMark>,
    diagram: &BlockDiagram,
    link: &Connection,
    look: &BlockDiagramLook,
) {
    let (Some(from), Some(to)) = (diagram.block(link.from.block), diagram.block(link.to.block))
    else {
        return;
    };
    let start = end_point(from, link.from.port, to);
    let end = end_point(to, link.to.port, from);
    let path = connection_path(start, end);
    let (rgb, width_mm) = link_style(&link.interface, look);
    let head = arrow_head(&path, width_mm);
    let mid = label_point(&path);
    marks.push(BlockMark::Stroke {
        points: path,
        closed: false,
        width_mm,
        rgb,
    });
    if let Some(ring) = head {
        marks.push(BlockMark::Fill { ring, rgb });
    }
    let text = connection_text(link);
    if !text.is_empty() {
        marks.push(BlockMark::Text {
            at: Point::new(mid.x, mid.y - LINK_TEXT_MM),
            size_mm: LINK_TEXT_MM,
            text,
            rgb,
        });
    }
}

fn block_marks(marks: &mut Vec<BlockMark>, block: &Block, look: &BlockDiagramLook) {
    let outline = block_outline(block);
    let own = block.color.map(rgb_of);
    let (fill, edge, ink) = if look.monochrome {
        (PAPER, BLACK, BLACK)
    } else {
        (own.map_or(BLOCK_FILL, tint), own.unwrap_or(INK), INK)
    };
    marks.push(BlockMark::Fill {
        ring: outline.clone(),
        rgb: fill,
    });
    marks.push(BlockMark::Stroke {
        points: outline,
        closed: true,
        width_mm: BLOCK_EDGE_MM,
        rgb: edge,
    });
    block_texts(marks, block, ink);
    for port in &block.ports {
        port_marks(marks, block, port, edge, ink);
    }
}

/// The name and function in the block, the note under it, one mark per
/// line of the note.
fn block_texts(marks: &mut Vec<BlockMark>, block: &Block, ink: Rgb) {
    let centre = Point::new(
        block.position.x + block.width_mm / 2.0,
        block.position.y + block.height_mm / 2.0,
    );
    let has_function = !block.function.trim().is_empty();
    let name_y = if has_function {
        centre.y - FUNCTION_MM
    } else {
        centre.y
    };
    marks.push(text(
        Point::new(centre.x, name_y),
        NAME_MM,
        &block.name,
        ink,
    ));
    if has_function {
        let at = Point::new(centre.x, centre.y + FUNCTION_MM);
        marks.push(text(at, FUNCTION_MM, &block.function, ink));
    }
    let below = block.position.y + block.height_mm + NOTE_MM * 1.5;
    let lines = block.note.lines().filter(|line| !line.trim().is_empty());
    for (i, line) in lines.enumerate() {
        let at = Point::new(centre.x, below + i as f64 * NOTE_MM * 1.4);
        marks.push(text(at, NOTE_MM, line, ink));
    }
}

/// A port's square on the edge and its name inside the block, the near
/// end of the name clear of the square.
fn port_marks(marks: &mut Vec<BlockMark>, block: &Block, port: &BlockPort, edge: Rgb, ink: Rgb) {
    let (anchor, inward) = port_anchor(block, port);
    let half = PORT_MM / 2.0;
    marks.push(BlockMark::Fill {
        ring: vec![
            Point::new(anchor.x - half, anchor.y - half),
            Point::new(anchor.x + half, anchor.y - half),
            Point::new(anchor.x + half, anchor.y + half),
            Point::new(anchor.x - half, anchor.y + half),
        ],
        rgb: edge,
    });
    let along = if inward.y == 0.0 {
        PORT_MM + text_width(&port.name, PORT_TEXT_MM) / 2.0
    } else {
        PORT_TEXT_MM * 1.5
    };
    let at = Point::new(anchor.x + inward.x * along, anchor.y + inward.y * along);
    marks.push(text(at, PORT_TEXT_MM, &port.name, ink));
}

/// A one-line text mark: runs of white space, line breaks included, read
/// as one space.
fn text(at: Point, size_mm: f64, text: &str, rgb: Rgb) -> BlockMark {
    BlockMark::Text {
        at,
        size_mm,
        text: one_line(text),
        rgb,
    }
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn text_width(text: &str, size_mm: f64) -> f64 {
    one_line(text).chars().count() as f64 * size_mm * TEXT_ADVANCE
}

/// The box every mark lies in, a text by its estimated width.
fn extent(marks: &[BlockMark]) -> (Point, Point) {
    let mut min = Point::new(f64::INFINITY, f64::INFINITY);
    let mut max = Point::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
    let mut take = |p: Point| {
        if p.x.is_finite() && p.y.is_finite() {
            min = Point::new(min.x.min(p.x), min.y.min(p.y));
            max = Point::new(max.x.max(p.x), max.y.max(p.y));
        }
    };
    for mark in marks {
        match mark {
            BlockMark::Stroke { points, .. } => points.iter().copied().for_each(&mut take),
            BlockMark::Fill { ring, .. } => ring.iter().copied().for_each(&mut take),
            BlockMark::Text {
                at, size_mm, text, ..
            } => {
                let half = Point::new(text_width(text, *size_mm) / 2.0, *size_mm / 2.0);
                take(Point::new(at.x - half.x, at.y - half.y));
                take(Point::new(at.x + half.x, at.y + half.y));
            }
        }
    }
    if min.x.is_finite() && max.x > min.x && max.y > min.y {
        (min, max)
    } else {
        (
            Point::new(0.0, 0.0),
            Point::new(DEFAULT_BLOCK_WIDTH_MM, DEFAULT_BLOCK_HEIGHT_MM),
        )
    }
}

#[cfg(test)]
mod tests;
