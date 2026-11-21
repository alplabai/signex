//! The small declarative drawing vocabulary of a property illustration.
//!
//! Coordinates are in the illustration's own view units (y down), which the
//! widget fits uniformly into its bounds. Text sizes, stroke widths, arrow
//! heads and dashes are not in view units: they come from the text size.

use iced::{Point, Radians, Rectangle};

/// Label text: a catalog message, user text (a net name, a designator) or,
/// for text not yet in the catalog, `raw`, as every house widget takes it.
pub type Text = crate::Label<'static>;

/// How the ends of a [`Primitive::Band`] are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Cap {
    /// A half circle past the end point.
    #[default]
    Round,
    /// Extended past the end point by half the width.
    Square,
    /// Cut at the end point.
    Flat,
}

/// Horizontal anchor of a label against its point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Anchor {
    /// The label starts at the point.
    Start,
    /// The label is centred on the point.
    #[default]
    Middle,
    /// The label ends at the point.
    End,
}

/// The type-scale step of a label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TextRole {
    /// A callout or object label, in the caption size.
    #[default]
    Caption,
    /// A small note ("Top Layer · round ends"), one step down and faint.
    Note,
    /// A designator or value in the monospace face, bold.
    Mono,
}

/// A free label.
#[derive(Debug, Clone)]
pub struct TextLabel {
    pub at: Point,
    pub text: Text,
    pub anchor: Anchor,
    pub role: TextRole,
}

/// An engineering dimension callout: a line with an arrow head at each
/// end, optional extension lines, and a label.
///
/// A span too short to hold both heads is drawn with the heads outside,
/// pointing in, on a line that runs past both ends.
#[derive(Debug, Clone)]
pub struct Callout {
    pub from: Point,
    pub to: Point,
    pub label: Option<Text>,
    /// Where the label goes; `None` puts it beside the middle of the span.
    pub label_at: Option<(Point, Anchor)>,
    /// Extension lines from the object to the dimension line, drawn dashed.
    pub extensions: Vec<(Point, Point)>,
}

impl Callout {
    pub fn new(from: Point, to: Point) -> Self {
        Self {
            from,
            to,
            label: None,
            label_at: None,
            extensions: Vec::new(),
        }
    }

    pub fn label(mut self, label: impl Into<Text>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn label_at(mut self, at: Point, anchor: Anchor) -> Self {
        self.label_at = Some((at, anchor));
        self
    }

    pub fn extension(mut self, from: Point, to: Point) -> Self {
        self.extensions.push((from, to));
        self
    }
}

/// A leader: a dot on the object, a line out to a label.
#[derive(Debug, Clone)]
pub struct Leader {
    pub at: Point,
    pub label_at: Point,
    pub label: Text,
    pub anchor: Anchor,
}

/// One drawing primitive.
#[derive(Debug, Clone)]
pub enum Primitive {
    Line {
        from: Point,
        to: Point,
    },
    Rect {
        rect: Rectangle,
    },
    RoundedRect {
        rect: Rectangle,
        radius: f32,
    },
    Circle {
        center: Point,
        radius: f32,
    },
    /// An arc from `start` to `end`, clockwise on screen (y down).
    Arc {
        center: Point,
        radius: f32,
        start: Radians,
        end: Radians,
    },
    Polyline {
        points: Vec<Point>,
        is_closed: bool,
    },
    /// A wide stroked path drawn as an outline (a track, a wire): an edge,
    /// a body knocked out to the panel ground, and the fill tint on top.
    /// `width` is in view units, so it scales with the drawing.
    Band {
        points: Vec<Point>,
        width: f32,
        cap: Cap,
    },
    Callout(Callout),
    Leader(Leader),
    Label(TextLabel),
}

impl Primitive {
    /// True for primitives drawn in the callout ink (callouts, leaders,
    /// labels), which take the accent as a whole when highlighted.
    pub fn is_annotation(&self) -> bool {
        matches!(
            self,
            Primitive::Callout(_) | Primitive::Leader(_) | Primitive::Label(_)
        )
    }
}
