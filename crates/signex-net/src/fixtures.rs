//! Sheet fixtures shared by the build and project test modules.
use signex_types::schematic::{Junction, Label, LabelType, Point, SchematicSheet, Wire};
use uuid::Uuid;

pub(crate) fn pt(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

pub(crate) fn empty_sheet() -> SchematicSheet {
    SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    }
}

pub(crate) fn wire(a: Point, b: Point) -> Wire {
    Wire {
        start: a,
        end: b,
        ..Default::default()
    }
}

pub(crate) fn junction(pos: Point) -> Junction {
    Junction {
        position: pos,
        ..Default::default()
    }
}

pub(crate) fn label(text: &str, pos: Point, ty: LabelType) -> Label {
    Label {
        uuid: Uuid::nil(),
        ..Label::new(text, pos, ty)
    }
}
