//! The live preview under the cards (mockup m36): a small schematic in the
//! canvas colours drawn with the resolved defaults, each value written
//! beside its item. A pure `canvas::Program`: nothing on the drawing is
//! editable and it has no handles.

use std::collections::HashMap;

use iced::advanced::text::Alignment as TextAlign;
use iced::alignment::Vertical;
use iced::widget::canvas::{self, Frame, LineDash, Path, Stroke};
use iced::{Color, Font, Pixels, Point, Rectangle, Renderer, Size, Theme, mouse};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::net_class::NetLineStyle;
use signex_types::project::{SchematicDefaultField, SchematicDefaultOverrides};
use signex_types::schematic::{FillType, SchematicDefaults};
use signex_types::theme::{CanvasColors, ThemeTokens};

use super::{clamp_note, shown_number};
use crate::styles::{ti, tokens as ink};

/// Height the drawing is laid out for; it scales with the canvas height.
const VIEW_HEIGHT: f32 = 210.0;
/// Pixels per millimetre of a drawn width at [`VIEW_HEIGHT`].
const PX_PER_MM: f32 = 8.0;
/// Thinnest drawn stroke, so a tiny width still shows.
const MIN_STROKE_PX: f32 = 0.6;
/// Pitch of the grid dots.
const GRID_PX: f32 = 20.0;
/// Size of the value labels.
const LABEL_PX: f32 = 10.0;
/// Text size in px per pt of the text size default.
const PX_PER_PT: f32 = 1.15;
/// Stroke of the power and ground symbols.
const PORT_STROKE_PX: f32 = 2.0;

// Sample design names drawn on the preview: net and part names a sheet
// would hold, not words of the interface.
const POWER_NET: &str = "+3V3";
const GROUND_NET: &str = "GND";
const DESIGNATOR: &str = "R1";
const RESISTANCE: &str = "10k";
const NET_LABEL: &str = "VOUT";
const BUS_LABEL: &str = "D[0..3]";
const BUS_MEMBER: &str = "D";

/// How a value label reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::app::project_tab) enum LabelLook {
    /// The project sets the value.
    Set,
    /// The field is empty: the built-in value, grey italic.
    BuiltIn,
    /// The typed number was clamped: the value used and the typed one.
    Clamped,
}

/// The words beside one item of the drawing.
#[derive(Debug, Clone, PartialEq)]
pub(in crate::app::project_tab) struct ValueLabel {
    pub field: SchematicDefaultField,
    pub words: String,
    pub look: LabelLook,
}

/// The colours the preview draws in: the schematic canvas colours, and
/// the label inks from the UI tokens.
#[derive(Debug, Clone, Copy)]
pub(super) struct PreviewInk {
    paper: Color,
    grid: Color,
    wire: Color,
    bus: Color,
    junction: Color,
    power: Color,
    body: Color,
    body_fill: Color,
    reference: Color,
    value: Color,
    net_label: Color,
    label: Color,
    built_in: Color,
    clamped: Color,
}

impl PreviewInk {
    pub(super) fn new(canvas: &CanvasColors, tokens: &ThemeTokens) -> Self {
        Self {
            paper: ti(canvas.paper),
            grid: ti(canvas.grid),
            wire: ti(canvas.wire),
            bus: ti(canvas.bus),
            junction: ti(canvas.junction),
            power: signex_widgets::theme_ext::power_port(canvas),
            body: ti(canvas.body),
            body_fill: ti(canvas.body_fill),
            reference: ti(canvas.reference),
            value: ti(canvas.value),
            net_label: ti(canvas.net_label),
            label: ti(tokens.text_secondary),
            built_in: ink::resolve(tokens).ink4,
            clamped: ti(tokens.warning),
        }
    }
}

/// The preview program: resolved values, fonts, colours and labels.
#[derive(Debug, Clone)]
pub(super) struct Preview {
    pub values: SchematicDefaults,
    /// The project's text font, for the net names.
    pub text_font: Font,
    /// The face of the value labels.
    pub label_font: Font,
    pub ink: PreviewInk,
    pub labels: Vec<ValueLabel>,
}

/// The words of `field`'s value label.
fn item_words(field: SchematicDefaultField) -> MessageId {
    match field {
        SchematicDefaultField::WireWidth => ids::PROJECT_DEFAULTS_PREVIEW_WIRE,
        SchematicDefaultField::BusWidth => ids::PROJECT_DEFAULTS_PREVIEW_BUS,
        SchematicDefaultField::SymbolLineWidth => ids::PROJECT_DEFAULTS_PREVIEW_LINE,
        SchematicDefaultField::ShapeStroke => ids::PROJECT_DEFAULTS_PREVIEW_STROKE,
        SchematicDefaultField::JunctionDiameter => ids::PROJECT_DEFAULTS_PREVIEW_JUNCTION,
        SchematicDefaultField::TextSize => ids::PROJECT_DEFAULTS_PREVIEW_TEXT,
    }
}

/// One label per numeric field: the value in effect, grey while the field
/// is empty, the used and typed numbers when the typed one was clamped.
pub(in crate::app::project_tab) fn value_labels(
    overrides: &SchematicDefaultOverrides,
    drafts: Option<&HashMap<SchematicDefaultField, String>>,
    i18n: &I18n,
) -> Vec<ValueLabel> {
    let resolved = overrides.resolve();
    SchematicDefaultField::ALL
        .into_iter()
        .map(|field| {
            let value = shown_number(field.resolved(&resolved));
            let words = t!(i18n, item_words(field), "value" => value).into_string();
            let draft = drafts.and_then(|drafts| drafts.get(&field));
            if clamp_note(field, draft).is_some() {
                let typed = draft.map_or("", |text| text.trim()).to_string();
                return ValueLabel {
                    field,
                    words: t!(
                        i18n,
                        ids::PROJECT_DEFAULTS_PREVIEW_CLAMPED,
                        "label" => words,
                        "typed" => typed,
                    )
                    .into_string(),
                    look: LabelLook::Clamped,
                };
            }
            let look = if overrides.get(field).is_some() {
                LabelLook::Set
            } else {
                LabelLook::BuiltIn
            };
            ValueLabel { field, words, look }
        })
        .collect()
}

/// Draws in the layout units of [`VIEW_HEIGHT`], scaled to the canvas.
struct Pen<'f> {
    frame: &'f mut Frame,
    unit: f32,
}

impl Pen<'_> {
    fn at(&self, x: f32, y: f32) -> Point {
        Point::new(x * self.unit, y * self.unit)
    }

    fn line(&mut self, from: (f32, f32), to: (f32, f32), stroke: Stroke<'_>) {
        let path = Path::line(self.at(from.0, from.1), self.at(to.0, to.1));
        self.frame.stroke(&path, stroke);
    }

    fn words(
        &mut self,
        content: String,
        at: (f32, f32),
        size: f32,
        color: Color,
        font: Font,
        align: TextAlign,
    ) {
        self.frame.fill_text(canvas::Text {
            content,
            position: self.at(at.0, at.1),
            color,
            size: Pixels(size * self.unit),
            font,
            align_x: align,
            align_y: Vertical::Bottom,
            ..canvas::Text::default()
        });
    }
}

/// A stroke `px` wide in layout units.
fn stroke(color: Color, px: f32, unit: f32) -> Stroke<'static> {
    Stroke::default().with_color(color).with_width(px * unit)
}

/// The drawn width of a default in mm.
fn width_px(mm: f64) -> f32 {
    (mm as f32 * PX_PER_MM).max(MIN_STROKE_PX)
}

impl<M> canvas::Program<M> for Preview {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let unit = (bounds.height / VIEW_HEIGHT).max(f32::EPSILON);
        // The width of the drawing in layout units.
        let wide = bounds.width / unit;
        frame.fill_rectangle(Point::ORIGIN, bounds.size(), self.ink.paper);
        let mut pen = Pen {
            frame: &mut frame,
            unit,
        };
        self.draw_grid(&mut pen, wide);
        self.draw_left(&mut pen);
        self.draw_bus(&mut pen, wide);
        self.draw_shape(&mut pen, wide);
        self.draw_labels(&mut pen, wide);
        vec![frame.into_geometry()]
    }
}

impl Preview {
    fn text_px(&self) -> f32 {
        self.values.text_size_pt as f32 * PX_PER_PT
    }

    fn draw_grid(&self, pen: &mut Pen<'_>, wide: f32) {
        let mut y = GRID_PX;
        while y < VIEW_HEIGHT {
            let mut x = GRID_PX;
            while x < wide {
                let dot = Path::circle(pen.at(x, y), 0.8 * pen.unit);
                pen.frame.fill(&dot, self.ink.grid);
                x += GRID_PX;
            }
            y += GRID_PX;
        }
    }

    /// The wire stroke in the wire line style.
    fn wire_stroke<'d>(&self, unit: f32, dash: &'d [f32]) -> Stroke<'d> {
        let mut wire = stroke(self.ink.wire, width_px(self.values.wire_width_mm), unit);
        if self.values.wire_line_style != NetLineStyle::Solid {
            wire.line_dash = LineDash {
                segments: dash,
                offset: 0,
            };
        }
        wire
    }

    /// Power port, wires with the T-junction, ground, resistor and the net
    /// label.
    fn draw_left(&self, pen: &mut Pen<'_>) {
        let unit = pen.unit;
        let dash = match self.values.wire_line_style {
            NetLineStyle::Dotted => [1.5 * unit, 3.0 * unit],
            _ => [6.0 * unit, 4.0 * unit],
        };
        let wire = self.wire_stroke(unit, &dash);
        let port = stroke(self.ink.power, PORT_STROKE_PX, unit);
        let fs = self.text_px();
        let bold = Font {
            weight: iced::font::Weight::Semibold,
            ..self.text_font
        };
        pen.line((58.0, 36.0), (82.0, 36.0), port);
        pen.words(
            POWER_NET.into(),
            (70.0, 30.0),
            fs,
            self.ink.power,
            bold,
            TextAlign::Center,
        );
        pen.line((70.0, 36.0), (70.0, 100.0), wire);
        pen.line((16.0, 100.0), (120.0, 100.0), wire);
        pen.line((70.0, 100.0), (70.0, 150.0), wire);
        let radius = width_px(self.values.junction_diameter_mm) / 2.0;
        let junction = Path::circle(pen.at(70.0, 100.0), radius * unit);
        pen.frame.fill(&junction, self.ink.junction);
        for (y, half) in [(150.0, 12.0), (155.0, 8.0), (160.0, 4.0)] {
            pen.line((70.0 - half, y), (70.0 + half, y), port);
        }
        pen.words(
            GROUND_NET.into(),
            (88.0, 160.0),
            fs * 0.85,
            self.ink.power,
            bold,
            TextAlign::Left,
        );
        let symbol = stroke(
            self.ink.body,
            width_px(self.values.symbol_line_width_mm),
            unit,
        );
        pen.line((120.0, 100.0), (136.0, 100.0), symbol);
        pen.line((184.0, 100.0), (200.0, 100.0), symbol);
        let body = Path::rectangle(pen.at(136.0, 92.0), Size::new(48.0 * unit, 16.0 * unit));
        pen.frame.stroke(&body, symbol);
        let font = self.text_font;
        let center = TextAlign::Center;
        pen.words(
            DESIGNATOR.into(),
            (160.0, 86.0),
            fs,
            self.ink.reference,
            font,
            center,
        );
        pen.words(
            RESISTANCE.into(),
            (160.0, 112.0 + fs),
            fs,
            self.ink.value,
            font,
            center,
        );
        pen.line((200.0, 100.0), (310.0, 100.0), wire);
        pen.words(
            NET_LABEL.into(),
            (255.0, 96.0),
            fs,
            self.ink.net_label,
            font,
            center,
        );
    }

    /// The bus with its name and four entries.
    fn draw_bus(&self, pen: &mut Pen<'_>, wide: f32) {
        let unit = pen.unit;
        let bus_px = width_px(self.values.bus_width_mm);
        let mut bus = stroke(self.ink.bus, bus_px, unit);
        bus.line_cap = canvas::LineCap::Round;
        pen.line((340.0, 48.0), (wide - 20.0, 48.0), bus);
        let fs = self.text_px();
        let font = self.text_font;
        pen.words(
            BUS_LABEL.into(),
            (342.0, 48.0 - bus_px / 2.0 - 5.0),
            fs,
            self.ink.net_label,
            font,
            TextAlign::Left,
        );
        let entry = stroke(self.ink.wire, width_px(self.values.wire_width_mm), unit);
        for (index, x) in [400.0_f32, 450.0, 500.0, 550.0].into_iter().enumerate() {
            pen.line((x, 48.0), (x + 10.0, 58.0), entry);
            pen.line((x + 10.0, 58.0), (x + 10.0, 94.0), entry);
            pen.words(
                format!("{BUS_MEMBER}{index}"),
                (x + 14.0, 86.0),
                fs * 0.85,
                self.ink.net_label,
                font,
                TextAlign::Left,
            );
        }
    }

    /// The rectangle shape at the shape stroke and fill.
    fn draw_shape(&self, pen: &mut Pen<'_>, wide: f32) {
        let unit = pen.unit;
        let (top_left, size) = shape_box(wide);
        let shape = Path::rectangle(
            pen.at(top_left.0, top_left.1),
            Size::new(size.0 * unit, size.1 * unit),
        );
        match self.values.shape_fill {
            FillType::None => {}
            FillType::Outline => pen.frame.fill(&shape, self.ink.body),
            FillType::Background => pen.frame.fill(&shape, self.ink.body_fill),
        }
        let edge = stroke(self.ink.body, width_px(self.values.shape_stroke_mm), unit);
        pen.frame.stroke(&shape, edge);
    }

    /// Each value beside its item.
    fn draw_labels(&self, pen: &mut Pen<'_>, wide: f32) {
        let (top_left, size) = shape_box(wide);
        for label in &self.labels {
            let (at, align) = match label.field {
                SchematicDefaultField::WireWidth => ((96.0, 92.0), TextAlign::Center),
                SchematicDefaultField::JunctionDiameter => ((80.0, 122.0), TextAlign::Left),
                SchematicDefaultField::SymbolLineWidth => ((160.0, 150.0), TextAlign::Center),
                SchematicDefaultField::TextSize => ((255.0, 122.0), TextAlign::Center),
                SchematicDefaultField::BusWidth => ((wide - 22.0, 70.0), TextAlign::Right),
                SchematicDefaultField::ShapeStroke => (
                    (
                        top_left.0 + size.0 / 2.0,
                        top_left.1 + size.1 / 2.0 + LABEL_PX / 2.0,
                    ),
                    TextAlign::Center,
                ),
            };
            let (color, font) = match label.look {
                LabelLook::Set => (self.ink.label, self.label_font),
                LabelLook::BuiltIn => (
                    self.ink.built_in,
                    Font {
                        style: iced::font::Style::Italic,
                        ..self.label_font
                    },
                ),
                LabelLook::Clamped => (self.ink.clamped, self.label_font),
            };
            pen.words(label.words.clone(), at, LABEL_PX, color, font, align);
        }
    }
}

/// Top-left corner and size of the shape, in layout units.
fn shape_box(wide: f32) -> ((f32, f32), (f32, f32)) {
    (
        (340.0, 118.0),
        ((wide - 360.0).max(40.0), VIEW_HEIGHT - 136.0),
    )
}
