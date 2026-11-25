//! Editing on the class preview (mockup row 153, #2661): the width, gap,
//! impedance and tolerance written on the board-style drawing are values
//! the user can type into (Enter or Tab commits, Esc cancels), a handle on
//! the N trace drags the gap, and the expand button draws the pair taller.
//! A value set on the drawing is the same edit as typing it in its field,
//! so the field and the drawing always agree. Where the drawing puts
//! things is decided here, once, for the view and for the labels.

use iced::keyboard::{Key, Modifiers};
use iced::{Point, Task, Vector};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::coord::Unit;
use signex_types::diff_pair_class::DiffPairClass;
use signex_widgets::LengthFormat;
use signex_widgets::dimension_label::{
    DimensionLabel, DimensionLabelMessage, DimensionLabels, DragHandle, LabelAnchor, ValueLimits,
    label_box,
};

use super::{DiffPairClassesState, PairClassField, field_value, set_field};
use crate::styles::TypeScale;

const TS: TypeScale = TypeScale::DEFAULT;

/// Width of the drawing, in view units.
pub(crate) const VIEW_W: f32 = 480.0;
/// Height of the drawing, in view units, before it is expanded.
const VIEW_H: f32 = 150.0;
/// Where the traces run, left and right.
pub(crate) const TRACE_X: (f32, f32) = (70.0, 410.0);
/// The pair (two widths and the gap) fills this many view units.
const PAIR_SPAN: f64 = 56.0;
/// View units per millimetre, at least and at most.
const SCALE: (f64, f64) = (20.0, 400.0);
/// The expanded drawing is this much taller, and its pair this much larger.
const EXPANDED_ZOOM: f32 = 2.0;
/// One view unit, in captions, as the Rules tab drawings use it.
const UNIT_EM: f32 = 1.0 / 11.0;
/// What a class with no width or gap is drawn with, in mm.
pub(crate) const DRAWN_WIDTH: f64 = 0.2;
pub(crate) const DRAWN_GAP: f64 = 0.15;
/// Clear space between a callout and its value.
const LABEL_DX: f32 = 9.0;
/// Where the impedance starts.
const VALUES_AT: (f32, f32) = (12.0, 18.0);
/// Clear space the traces keep from the drawing's bottom edge while the
/// gap is dragged.
const EDGE_MARGIN: f32 = 4.0;
/// A dragged gap snaps to this, in mm, as the other rule drawings do.
const DRAG_STEP_MM: f64 = 0.005;

/// The field each label writes, in label order.
pub(crate) const LABEL_FIELDS: [PairClassField; 4] = [
    PairClassField::Width,
    PairClassField::Gap,
    PairClassField::Impedance,
    PairClassField::Tolerance,
];
/// The index of the gap label, the one with the handle.
const GAP_LABEL: usize = 1;

/// The gap handle held: the scale the drawing keeps meanwhile, so it does
/// not rescale under the pointer, and the gap the class had, which Esc
/// puts back (a class with no gap gets none again).
#[derive(Debug, Clone, Copy, PartialEq)]
struct HeldGap {
    scale: f64,
    original: Option<f64>,
}

/// The preview's interaction state, in the dialog's state.
#[derive(Debug, Clone, Default)]
pub struct PairPreview {
    pub(crate) labels: DimensionLabels,
    pub(crate) expanded: bool,
    held: Option<HeldGap>,
}

impl PartialEq for PairPreview {
    /// The label widget's field id is unique per instance, so two previews
    /// are equal by what they show: the open label and its text, the held
    /// handle and the size.
    fn eq(&self, other: &Self) -> bool {
        self.expanded == other.expanded
            && self.held == other.held
            && self.labels.editing_index() == other.labels.editing_index()
            && self.labels.draft() == other.labels.draft()
            && self.labels.dragging_index() == other.labels.dragging_index()
    }
}

impl PairPreview {
    /// Close an open label and let go of the handle; the size is kept.
    pub(crate) fn close_labels(&mut self) {
        self.labels = DimensionLabels::default();
        self.held = None;
    }

    /// Tab and Shift+Tab while a label is open, Esc while one is open or
    /// the handle is held.
    pub(crate) fn key_message(&self, key: &Key, modifiers: Modifiers) -> Option<PairPreviewMsg> {
        self.labels
            .key_message(key, modifiers)
            .map(PairPreviewMsg::Labels)
    }
}

/// An event of the class preview.
#[derive(Debug, Clone, PartialEq)]
pub enum PairPreviewMsg {
    Labels(DimensionLabelMessage),
    /// The expand button was pressed: the drawing grows taller, or back.
    ExpandToggled,
}

/// `value` without trailing zeros, or `—` when unset.
pub(crate) fn shown_value(value: Option<f64>) -> String {
    value.map_or_else(
        || "—".to_string(),
        |v| {
            let text = format!("{v:.3}");
            text.trim_end_matches('0').trim_end_matches('.').to_string()
        },
    )
}

/// A set value above zero, or what the drawing uses instead.
fn drawn(value: Option<f64>, fallback: f64) -> f64 {
    value.filter(|v| *v > 0.0).unwrap_or(fallback)
}

/// The texts the drawing writes for its values, in [`LABEL_FIELDS`] order.
pub(crate) fn value_texts(class: &DiffPairClass, i18n: &I18n) -> [Localized; 4] {
    [
        t!(i18n, ids::DIALOG_DPC_WIDTH_LABEL, "value" => shown_value(class.width_mm)),
        t!(i18n, ids::DIALOG_DPC_GAP_LABEL, "value" => shown_value(class.gap_mm)),
        t!(i18n, ids::DIALOG_DPC_ZDIFF_VALUE, "ohm" => shown_value(class.impedance_ohm)),
        t!(
            i18n,
            ids::DIALOG_DPC_TOLERANCE_VALUE,
            "pct" => shown_value(class.impedance_tolerance_pct)
        ),
    ]
}

/// Characters of `text` a reader sees: Fluent's isolation marks take no
/// room.
fn visible_chars(text: &Localized) -> usize {
    text.clone()
        .into_string()
        .chars()
        .filter(|c| !matches!(c, '\u{2068}' | '\u{2069}'))
        .count()
}

/// Where the drawing puts the pair of one class, in view units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PairGeometry {
    /// Height of the drawing.
    pub view_h: f32,
    /// View units per millimetre.
    pub scale: f64,
    pub centre_y: f32,
    /// Drawn width of a trace.
    pub trace_w: f32,
    /// Centre lines of the P and N traces.
    pub y_p: f32,
    pub y_n: f32,
    /// The mm width the drawing uses (the class's, or [`DRAWN_WIDTH`]).
    width_mm: f64,
}

impl PairGeometry {
    /// The drawing of `class`; one scale across the pair, held while the
    /// gap handle is dragged.
    pub(crate) fn of(class: &DiffPairClass, preview: &PairPreview) -> Self {
        let zoom = if preview.expanded { EXPANDED_ZOOM } else { 1.0 };
        let z = f64::from(zoom);
        let width_mm = drawn(class.width_mm, DRAWN_WIDTH);
        let gap = drawn(class.gap_mm, DRAWN_GAP);
        let scale = preview.held.map_or_else(
            || (PAIR_SPAN * z / (2.0 * width_mm + gap)).clamp(SCALE.0 * z, SCALE.1 * z),
            |held| held.scale,
        );
        let view_h = VIEW_H * zoom;
        let centre_y = view_h / 2.0 + 8.0;
        let trace_w = ((width_mm * scale) as f32).max(1.0);
        let half = ((gap + width_mm) * scale) as f32 / 2.0;
        Self {
            view_h,
            scale,
            centre_y,
            trace_w,
            y_p: centre_y - half,
            y_n: centre_y + half,
            width_mm,
        }
    }

    /// Drawing pixels per view unit.
    pub(crate) fn unit_px() -> f32 {
        TS.caption * UNIT_EM
    }

    pub(crate) fn width_px(self) -> f32 {
        VIEW_W * Self::unit_px()
    }

    pub(crate) fn height_px(self) -> f32 {
        self.view_h * Self::unit_px()
    }

    /// The drawing height in captions, for the illustration widget.
    pub(crate) fn height_em(self) -> f32 {
        self.height_px() / TS.caption
    }

    /// Where along the traces the width callout stands.
    pub(crate) fn width_x() -> f32 {
        TRACE_X.0 + (TRACE_X.1 - TRACE_X.0) * 0.28
    }

    /// Where along the traces the gap callout stands.
    pub(crate) fn gap_x() -> f32 {
        TRACE_X.0 + (TRACE_X.1 - TRACE_X.0) * 0.62
    }

    /// The gap handle, on the N trace near its right end.
    pub(crate) fn handle_at(self) -> Point {
        Point::new(TRACE_X.1 - 26.0, self.y_n)
    }

    /// Where each value is written, and how it is anchored, in
    /// [`LABEL_FIELDS`] order. The tolerance follows the impedance text.
    pub(crate) fn value_spots(self, impedance: &Localized) -> [(Point, LabelAnchor); 4] {
        let unit = Self::unit_px();
        let chars = visible_chars(impedance);
        let impedance_box = label_box(chars, 0, TS.caption, Point::ORIGIN, LabelAnchor::Start);
        let impedance_w = impedance_box.width / unit;
        [
            (
                Point::new(Self::width_x() - LABEL_DX, self.y_p),
                LabelAnchor::End,
            ),
            (
                Point::new(Self::gap_x() + LABEL_DX, self.centre_y),
                LabelAnchor::Start,
            ),
            (Point::new(VALUES_AT.0, VALUES_AT.1), LabelAnchor::Start),
            (
                Point::new(VALUES_AT.0 + impedance_w, VALUES_AT.1),
                LabelAnchor::Start,
            ),
        ]
    }

    /// The widest gap the handle drags to at this scale: the N trace stays
    /// inside the drawing.
    fn max_dragged_gap(self) -> f64 {
        let room = self.view_h - self.centre_y - self.trace_w / 2.0 - EDGE_MARGIN;
        (2.0 * f64::from(room) / self.scale - self.width_mm).max(DRAG_STEP_MM)
    }
}

/// The drawing's values as labels, in [`LABEL_FIELDS`] order. The drawing
/// writes each value itself; a label is the press target over it and the
/// field it opens. A value not set opens at what the drawing shows (the
/// drawn width or gap, 0 otherwise), and an unchanged field commits
/// nothing. Values are plain numbers in the field's unit (mm, Ω, %), read
/// as the mm fields read them.
pub(crate) fn preview_labels<'a>(
    class: &DiffPairClass,
    preview: &PairPreview,
    i18n: &'a I18n,
) -> Vec<DimensionLabel<'a>> {
    let g = PairGeometry::of(class, preview);
    let texts = value_texts(class, i18n);
    let spots = g.value_spots(&texts[2]);
    let format = LengthFormat::new(i18n, Unit::Mm);
    let unit = PairGeometry::unit_px();
    texts
        .into_iter()
        .zip(spots)
        .zip(LABEL_FIELDS)
        .enumerate()
        .map(|(index, ((text, (at, anchor)), field))| {
            let fallback = match field {
                PairClassField::Width => DRAWN_WIDTH,
                PairClassField::Gap => DRAWN_GAP,
                PairClassField::Impedance | PairClassField::Tolerance => 0.0,
            };
            let value = field_value(class, field).unwrap_or(fallback);
            let label = DimensionLabel::new(value, format, Point::new(at.x * unit, at.y * unit))
                .caption(text)
                .anchor(anchor)
                .decimals(3)
                .bare();
            if index != GAP_LABEL {
                return label;
            }
            // The N trace moves by half the gap change: the pair is drawn
            // about its centre line.
            let handle = g.handle_at();
            label
                .handle(DragHandle {
                    at: Point::new(handle.x * unit, handle.y * unit),
                    axis: Vector::new(0.0, 1.0),
                    value_per_px: 2.0 / (g.scale * f64::from(unit)),
                })
                .limits(ValueLimits {
                    min: Some(DRAG_STEP_MM),
                    max: Some(g.max_dragged_gap()),
                    step: DRAG_STEP_MM,
                })
        })
        .collect()
}

impl DiffPairClassesState {
    /// A preview event. A value set on the drawing goes through its field's
    /// own edit, so the field shows it too.
    pub(crate) fn act_on_preview(&mut self, msg: PairPreviewMsg) -> Task<PairPreviewMsg> {
        let message = match msg {
            PairPreviewMsg::ExpandToggled => {
                self.preview.close_labels();
                self.preview.expanded = !self.preview.expanded;
                return Task::none();
            }
            PairPreviewMsg::Labels(message) => message,
        };
        let Some(class) = self.selected_class() else {
            return Task::none();
        };
        // Values read and show `.` in every locale (`LengthFormat`), so the
        // fallback bundle parses exactly what the view shows.
        let en = I18n::en_us();
        let shown = preview_labels(class, &self.preview, &en);
        let scale = PairGeometry::of(class, &self.preview).scale;
        let gap_before = class.gap_mm;
        let was_held = self.preview.labels.dragging_index().is_some();
        let is_escape = message == DimensionLabelMessage::EscapePressed;
        let outcome = self.preview.labels.update(message, &shown);
        if let Some(done) = outcome.committed
            && let Some(&field) = LABEL_FIELDS.get(done.index)
        {
            super::update::edit_field(self, field, format!("{}", done.value));
        }
        let held = self.preview.held;
        self.preview.held = self.preview.labels.dragging_index().map(|_| {
            held.unwrap_or(HeldGap {
                scale,
                original: gap_before,
            })
        });
        if was_held
            && is_escape
            && let Some(held) = held
        {
            self.restore_gap(held.original);
        }
        if outcome.focus_input {
            self.preview.labels.focus_input()
        } else {
            Task::none()
        }
    }

    /// Esc on a held handle: the gap the class had, and no draft over it.
    fn restore_gap(&mut self, gap: Option<f64>) {
        if let Some(class) = self.classes.get_mut(self.selected) {
            set_field(class, PairClassField::Gap, gap);
        }
        self.drafts.retain(|(f, _)| *f != PairClassField::Gap);
    }
}

impl crate::app::Signex {
    /// A key press for the class preview of the open dialog: Tab, Shift+Tab
    /// and Esc while a value is open or the handle is held.
    pub(crate) fn diff_pair_preview_key(
        &self,
        event: &iced::keyboard::Event,
    ) -> Option<crate::app::Message> {
        let iced::keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
            return None;
        };
        let state = self.ui_state.diff_pair_classes.as_deref()?;
        state
            .preview
            .key_message(key, *modifiers)
            .map(|msg| crate::app::Message::from(super::DiffPairClassesMsg::PreviewActed(msg)))
    }
}
