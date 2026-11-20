//! A rule drawing as data: board-coloured parts, the rule's values as
//! labels, and notes, in the view box every drawing shares. Builders stay
//! free of the locale; notes and captions are catalog ids rendered in
//! `view`.

use iced::{Color, Point, Rectangle, Vector};
use signex_drc_dsl::table::LimitField;
use signex_i18n::{I18n, MessageId, UserText, t};
use signex_types::pcb_layer::{PcbLayer, TechnicalLayer};
use signex_types::theme::{ThemeId, theme_tokens};
use signex_widgets::Label;
use signex_widgets::dimension_label::LabelAnchor;
use signex_widgets::property_illustration::{
    Anchor, Callout, Fill, Paint, Part, Primitive, Stroke, Style, TextLabel, TextRole, Tint, Weight,
};

use crate::styles::board_palette::{BoardPalette, BoardSwatch};
use crate::styles::ti;

/// Width of the view box, in view units.
pub(super) const VIEW_W: f32 = 640.0;
/// Height of the view box, in view units.
pub(super) const VIEW_H: f32 = 300.0;
/// Spacing of the board grid, in view units.
const GRID_STEP: f32 = 16.0;
/// Where a drawing's title and its note sit.
const TITLE_AT: Point = Point::new(16.0, 24.0);
const NOTE_AT: Point = Point::new(16.0, 42.0);

/// The board-ground colours of a drawing: the board palette and the
/// Graphite Dark ink, the same under both chrome themes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct BoardInk {
    pub ground: Color,
    pub grid: Color,
    pub copper: Color,
    pub plating: Color,
    pub mask: Color,
    pub paste: Color,
    pub silk: Color,
    pub courtyard: Color,
    pub edge: Color,
    /// Dimension lines, value labels and clearance zones.
    pub dimension: Color,
    /// Notes and limit outlines.
    pub note: Color,
    pub title: Color,
}

impl BoardInk {
    pub fn new() -> Self {
        let palette = BoardPalette::new();
        let dark = theme_tokens(ThemeId::GraphiteDark);
        let resolved = crate::styles::tokens::resolve(&dark);
        Self {
            ground: palette.canvas,
            grid: resolved.line_soft,
            copper: palette.swatch(BoardSwatch::TopCopper),
            plating: palette.multi_layer,
            mask: palette.swatch(BoardSwatch::SolderMask),
            paste: palette.swatch(BoardSwatch::Paste),
            silk: palette.layer(PcbLayer::Technical(TechnicalLayer::SilkscreenTop)),
            courtyard: palette.swatch(BoardSwatch::Courtyard),
            edge: palette.swatch(BoardSwatch::BoardEdge),
            dimension: ti(dark.warning),
            note: ti(dark.text_secondary),
            title: ti(dark.text),
        }
    }
}

/// The values of a rule, as the caller passed them.
pub(super) struct Values<'v>(pub &'v [(LimitField, f64)]);

impl Values<'_> {
    /// The finite value set for `field`; the last one wins.
    pub fn get(&self, field: LimitField) -> Option<f64> {
        self.0
            .iter()
            .rev()
            .find(|(f, _)| *f == field)
            .map(|(_, v)| *v)
            .filter(|v| v.is_finite())
    }

    /// The value the geometry is drawn with: the rule's own when it is
    /// positive, else `nominal`.
    pub fn size_or(&self, field: LimitField, nominal: f64) -> f64 {
        self.get(field).filter(|v| *v > 0.0).unwrap_or(nominal)
    }
}

/// A drag handle in view units; `mm_per_unit` is the value change for one
/// view unit moved along `axis`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Handle {
    pub at: Point,
    pub axis: Vector,
    pub mm_per_unit: f64,
}

/// One value of the rule on the drawing.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct ValueLabel {
    /// The grid column it writes; `None` for a pair gap.
    pub field: Option<LimitField>,
    pub caption: MessageId,
    pub value_mm: f64,
    pub at: Point,
    pub anchor: LabelAnchor,
    pub handle: Option<Handle>,
}

/// Note text: a catalog message or the user's own name (a class).
#[derive(Debug, Clone, PartialEq)]
pub(super) enum NoteText {
    Id(MessageId),
    User(String),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Note {
    pub at: Point,
    pub text: NoteText,
    pub anchor: Anchor,
    pub role: TextRole,
    pub color: Color,
}

/// What one drawing shows.
#[derive(Debug, Clone)]
pub(super) struct Scene {
    pub ink: BoardInk,
    pub parts: Vec<Part>,
    pub labels: Vec<ValueLabel>,
    pub notes: Vec<Note>,
}

fn stroke(color: Color, weight: Weight, is_dashed: bool) -> Style {
    Style {
        stroke: Some(Stroke {
            paint: Paint::Document(color),
            weight,
            is_dashed,
        }),
        fill: None,
    }
}

fn filled(color: Color, tint: Tint) -> Style {
    Style::filled(Paint::Document(color), tint)
}

pub(super) fn rect(x: f32, y: f32, width: f32, height: f32) -> Rectangle {
    Rectangle {
        x,
        y,
        width: width.max(0.0),
        height: height.max(0.0),
    }
}

/// `r` grown by `by` on every side.
pub(super) fn grown(r: Rectangle, by: f32) -> Rectangle {
    rect(r.x - by, r.y - by, r.width + 2.0 * by, r.height + 2.0 * by)
}

impl Scene {
    /// An empty drawing on the board grid.
    pub fn new(ink: BoardInk) -> Self {
        let mut scene = Self {
            ink,
            parts: Vec::new(),
            labels: Vec::new(),
            notes: Vec::new(),
        };
        let grid = stroke(ink.grid, Weight::Hairline, false);
        let mut x = GRID_STEP;
        while x < VIEW_W {
            let line = Primitive::Line {
                from: Point::new(x, 0.0),
                to: Point::new(x, VIEW_H),
            };
            scene.parts.push(Part::new(line, grid));
            x += GRID_STEP;
        }
        let mut y = GRID_STEP;
        while y < VIEW_H {
            let line = Primitive::Line {
                from: Point::new(0.0, y),
                to: Point::new(VIEW_W, y),
            };
            scene.parts.push(Part::new(line, grid));
            y += GRID_STEP;
        }
        scene
    }

    fn push(&mut self, primitive: Primitive, style: Style) {
        self.parts.push(Part::new(primitive, style));
    }

    /// Solid copper, or another solid layer colour.
    pub fn solid_rect(&mut self, r: Rectangle, radius: f32, color: Color) {
        self.push(
            Primitive::RoundedRect { rect: r, radius },
            filled(color, Tint::Solid),
        );
    }

    pub fn solid_circle(&mut self, center: Point, radius: f32, color: Color) {
        self.push(
            Primitive::Circle { center, radius },
            filled(color, Tint::Solid),
        );
    }

    /// An outline, dashed or not, with an optional low-alpha tint inside.
    pub fn outline_rect(
        &mut self,
        r: Rectangle,
        radius: f32,
        color: Color,
        is_dashed: bool,
        tint: Option<Tint>,
    ) {
        let mut style = stroke(color, Weight::Thin, is_dashed);
        style.fill = tint.map(|tint| Fill {
            paint: Paint::Document(color),
            tint,
        });
        self.push(Primitive::RoundedRect { rect: r, radius }, style);
    }

    pub fn outline_circle(
        &mut self,
        center: Point,
        radius: f32,
        color: Color,
        is_dashed: bool,
        tint: Option<Tint>,
    ) {
        let mut style = stroke(color, Weight::Thin, is_dashed);
        style.fill = tint.map(|tint| Fill {
            paint: Paint::Document(color),
            tint,
        });
        self.push(Primitive::Circle { center, radius }, style);
    }

    pub fn line(&mut self, from: Point, to: Point, color: Color, weight: Weight) {
        self.push(Primitive::Line { from, to }, stroke(color, weight, false));
    }

    /// A drilled, plated hole: the board ground inside a plating ring.
    pub fn plated_hole(&mut self, center: Point, radius: f32) {
        let (ground, plating) = (self.ink.ground, self.ink.plating);
        let mut style = stroke(plating, Weight::Regular, false);
        style.fill = Some(Fill {
            paint: Paint::Document(ground),
            tint: Tint::Solid,
        });
        self.push(Primitive::Circle { center, radius }, style);
    }

    /// A dimension line with a head at each end, in the dimension ink.
    pub fn dimension(&mut self, from: Point, to: Point) {
        let color = self.ink.dimension;
        self.push(
            Primitive::Callout(Callout::new(from, to)),
            stroke(color, Weight::Thin, false),
        );
    }

    /// A dashed extension line in the dimension ink.
    pub fn extension(&mut self, from: Point, to: Point) {
        let color = self.ink.dimension;
        self.push(
            Primitive::Line { from, to },
            stroke(color, Weight::Thin, true),
        );
    }

    /// The value of `field` (or of a pair gap) as an editable label.
    /// Nothing is added for a value that is unset or not finite; a 0 keeps
    /// its label, so it can be changed on the drawing.
    pub fn value(
        &mut self,
        field: Option<LimitField>,
        caption: MessageId,
        value: Option<f64>,
        at: Point,
        anchor: LabelAnchor,
        handle: Option<Handle>,
    ) {
        let Some(value_mm) = value.filter(|v| v.is_finite()) else {
            return;
        };
        self.labels.push(ValueLabel {
            field,
            caption,
            value_mm,
            at,
            anchor,
            handle,
        });
    }

    pub fn note(&mut self, at: Point, text: NoteText, anchor: Anchor, color: Color) {
        self.notes.push(Note {
            at,
            text,
            anchor,
            role: TextRole::Note,
            color,
        });
    }

    /// The drawing's title and the note under it, at the top left.
    pub fn title(&mut self, title: MessageId, note: MessageId) {
        let (title_ink, note_ink) = (self.ink.title, self.ink.note);
        self.notes.push(Note {
            at: TITLE_AT,
            text: NoteText::Id(title),
            anchor: Anchor::Start,
            role: TextRole::Caption,
            color: title_ink,
        });
        self.note(NOTE_AT, NoteText::Id(note), Anchor::Start, note_ink);
    }

    /// Every part, the notes last so they draw on top.
    pub fn parts_with_notes(&self, i18n: &I18n) -> Vec<Part> {
        let mut parts = self.parts.clone();
        for note in &self.notes {
            let text: Label<'static> = match &note.text {
                NoteText::Id(id) => t!(i18n, *id).into(),
                NoteText::User(name) => UserText::new(name.clone()).into(),
            };
            let label = TextLabel {
                at: note.at,
                text,
                anchor: note.anchor,
                role: note.role,
            };
            parts.push(Part::new(
                Primitive::Label(label),
                stroke(note.color, Weight::Thin, false),
            ));
        }
        parts
    }
}
