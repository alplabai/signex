//! How a part is inked: stroke and fill, named by token, never by colour.

use iced::Color;

/// A chrome ink token. The palette resolves each one from the theme, so an
/// illustration reads the same in the light and the dark theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ink {
    /// Object outlines and main labels (second text step).
    Secondary,
    /// Dashed construction lines: courtyard, paste opening (third text step).
    Tertiary,
    /// Callouts and small notes (fourth, faintest text step).
    Faint,
    /// Grid lines and other rules.
    Rule,
    /// The highlight colour of a hovered part.
    Accent,
    /// The panel ground; used to knock a body out of a band.
    Panel,
    /// A fault mark, such as a not-fitted cross.
    Error,
}

/// What a fill or stroke is painted with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Paint {
    /// A chrome token.
    Ink(Ink),
    /// A document colour token the caller resolved, such as a layer colour
    /// from the board palette or the schematic wire colour. The widget only
    /// ever draws it at a low alpha (see [`Tint`]).
    Document(Color),
}

/// The low alpha a fill is drawn at. Fills stay light so the line art reads
/// on the panel ground; only [`Tint::Solid`] marks (a pin-1 dot) are opaque.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tint {
    /// A barely visible wash (a sheet area).
    Wash,
    /// A light tint (a dielectric, a part body).
    Light,
    /// The usual object fill (a pad, a footprint body).
    Body,
    /// A stronger tint (pads around a body, copper in a side view).
    Strong,
    /// Fully opaque; for small ink marks only.
    Solid,
}

impl Tint {
    pub fn alpha(self) -> f32 {
        match self {
            Tint::Wash => 0.06,
            Tint::Light => 0.12,
            Tint::Body => 0.22,
            Tint::Strong => 0.35,
            Tint::Solid => 1.0,
        }
    }
}

/// Stroke weight, as a multiple of the base stroke the metrics derive from
/// the text size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weight {
    Hairline,
    Thin,
    Regular,
    Heavy,
}

impl Weight {
    pub fn factor(self) -> f32 {
        match self {
            Weight::Hairline => 0.6,
            Weight::Thin => 0.8,
            Weight::Regular => 1.0,
            Weight::Heavy => 1.4,
        }
    }
}

/// A stroke: paint, weight and whether it is dashed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stroke {
    pub paint: Paint,
    pub weight: Weight,
    pub is_dashed: bool,
}

/// A fill: paint at a low alpha.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fill {
    pub paint: Paint,
    pub tint: Tint,
}

/// The stroke and fill of one part.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    pub stroke: Option<Stroke>,
    pub fill: Option<Fill>,
}

impl Style {
    /// A solid outline in `ink`.
    pub fn outline(ink: Ink, weight: Weight) -> Self {
        Self {
            stroke: Some(Stroke {
                paint: Paint::Ink(ink),
                weight,
                is_dashed: false,
            }),
            fill: None,
        }
    }

    /// The object edge: a regular secondary-ink outline.
    pub fn edge() -> Self {
        Self::outline(Ink::Secondary, Weight::Regular)
    }

    /// A dashed construction line (courtyard, paste opening).
    pub fn dashed() -> Self {
        Self {
            stroke: Some(Stroke {
                paint: Paint::Ink(Ink::Tertiary),
                weight: Weight::Thin,
                is_dashed: true,
            }),
            fill: None,
        }
    }

    /// The ink of callouts, leaders and their labels.
    pub fn callout() -> Self {
        Self::outline(Ink::Faint, Weight::Thin)
    }

    /// A fill only, no stroke.
    pub fn filled(paint: Paint, tint: Tint) -> Self {
        Self {
            stroke: None,
            fill: Some(Fill { paint, tint }),
        }
    }

    /// Adds a fill under the stroke.
    pub fn with_fill(mut self, paint: Paint, tint: Tint) -> Self {
        self.fill = Some(Fill { paint, tint });
        self
    }

    /// Replaces the stroke paint (an outline drawn in a layer colour).
    pub fn with_stroke_paint(mut self, paint: Paint) -> Self {
        if let Some(stroke) = self.stroke.as_mut() {
            stroke.paint = paint;
        }
        self
    }
}
