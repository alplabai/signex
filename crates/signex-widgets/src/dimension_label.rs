//! Editable dimension labels over a rule drawing (Rules tab and Rule editor).
//!
//! Each label shows one value of the rule. A press opens an inline field:
//! Enter commits, Tab commits and opens the next label (Shift+Tab the
//! previous one), Esc cancels. A label may carry a drag handle that changes
//! the value along an axis. The caller owns the values; [`DimensionLabels`]
//! holds only the interaction (which label is open, which handle is held)
//! and reports a [`DimensionCommitted`] the caller writes to its rule.
//!
//! The view is an overlay the caller stacks above its drawing:
//! `stack![drawing, labels.view(&items, style).map(Message::RuleLabels)]`.
//! Positions are drawing pixels from the overlay's top-left corner; every
//! size is derived from [`DimensionLabelStyle::text_size`].
//!
//! Tab is not handled by the text input, so the caller routes key presses
//! through [`DimensionLabels::key_message`]. A pointer released outside the
//! drawing does not reach the overlay; route it as
//! [`DimensionLabelMessage::PointerReleased`].

use iced::alignment::{Horizontal, Vertical};
use iced::keyboard::{Key, Modifiers, key::Named};
use iced::widget::{Id, Row, Space, Stack, container, mouse_area, operation};
use iced::{
    Background, Border, Color, Element, Font, Length, Point, Rectangle, Task, Theme, Vector,
    Widget, mouse,
};
use signex_i18n::UserText;

use crate::{Label, Translate, button, text, text_input};

/// Average advance of a monospace glyph, in ems; sizes a label before layout.
const GLYPH_EM: f32 = 0.62;
/// Line height of a label, in ems.
const LINE_EM: f32 = 1.3;
/// Vertical padding of a label, in ems.
const PAD_V_EM: f32 = 0.2;
/// Horizontal padding of a label, in ems.
const PAD_H_EM: f32 = 0.5;
/// Gap between the caption and the value, in ems.
const GAP_EM: f32 = 0.35;
/// Corner radius of a label, in ems.
const RADIUS_EM: f32 = 0.35;
/// Narrowest inline field, in ems, so a short value still has room to type.
const FIELD_MIN_EM: f32 = 7.5;
/// Diameter of a drag handle's dot, in ems.
const HANDLE_EM: f32 = 1.0;
/// Extra hit area around a handle's dot, in ems on each side.
const HANDLE_HIT_EM: f32 = 0.5;
/// Ring width of a drag handle, in ems.
const HANDLE_RING_EM: f32 = 0.2;
/// Decimals the inline field opens with (trailing zeros dropped).
const DRAFT_DECIMALS: usize = 3;
/// Drag results keep nine decimals, which drops float noise.
const NOISE_SCALE: f64 = 1e9;

/// Where a label sits relative to its anchor point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LabelAnchor {
    /// The label starts at the point.
    Start,
    /// The label is centred on the point.
    #[default]
    Middle,
    /// The label ends at the point.
    End,
}

/// The range and step a dragged value keeps to. A typed value is passed
/// through unclamped; validating it is the rule's job.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ValueLimits {
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// Drag snaps to multiples of this; `0.0` turns snapping off.
    pub step: f64,
}

/// `value` rounded to nine decimals, so `0.15 + 0.1` reads as `0.25`.
fn denoise(value: f64) -> f64 {
    (value * NOISE_SCALE).round() / NOISE_SCALE
}

impl ValueLimits {
    /// Snap `value` to the step, then clamp it to the range.
    pub fn apply(&self, value: f64) -> f64 {
        let mut v = denoise(value);
        if self.step > 0.0 {
            v = denoise((v / self.step).round() * self.step);
        }
        if let Some(min) = self.min {
            v = v.max(min);
        }
        if let Some(max) = self.max {
            v = v.min(max);
        }
        v
    }
}

/// A drag handle: dragging along `axis` changes the value by
/// `value_per_px` for each drawing pixel moved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragHandle {
    /// Centre of the handle, in drawing pixels.
    pub at: Point,
    /// Direction that increases the value; need not be unit length.
    pub axis: Vector,
    pub value_per_px: f64,
}

impl DragHandle {
    /// Value change for a pointer moved by `delta` pixels.
    pub fn value_delta(&self, delta: Vector) -> f64 {
        let len = f64::from(self.axis.x).hypot(f64::from(self.axis.y));
        if len == 0.0 {
            return 0.0;
        }
        let along = (f64::from(delta.x) * f64::from(self.axis.x)
            + f64::from(delta.y) * f64::from(self.axis.y))
            / len;
        along * self.value_per_px
    }

    fn interaction(&self) -> mouse::Interaction {
        let (x, y) = (self.axis.x.abs(), self.axis.y.abs());
        if y == 0.0 && x > 0.0 {
            mouse::Interaction::ResizingHorizontally
        } else if x == 0.0 && y > 0.0 {
            mouse::Interaction::ResizingVertically
        } else {
            mouse::Interaction::Move
        }
    }
}

/// One value shown on the drawing. The caller builds these from its rule
/// every frame; `value` is in the units the caller displays.
#[derive(Debug, Clone)]
pub struct DimensionLabel<'a> {
    /// Text before the value ("Width", "Gap", "≥").
    pub caption: Option<Label<'a>>,
    pub value: f64,
    /// Unit symbol after the value ("mm", "mil", "°"); empty for none.
    pub unit: &'a str,
    /// Decimals shown on the label.
    pub decimals: usize,
    /// Anchor point, in drawing pixels.
    pub at: Point,
    pub anchor: LabelAnchor,
    pub handle: Option<DragHandle>,
    pub limits: ValueLimits,
}

impl<'a> DimensionLabel<'a> {
    /// A label at `at` showing `value` with two decimals and no handle.
    pub fn new(value: f64, unit: &'a str, at: Point) -> Self {
        Self {
            caption: None,
            value,
            unit,
            decimals: 2,
            at,
            anchor: LabelAnchor::Middle,
            handle: None,
            limits: ValueLimits::default(),
        }
    }

    pub fn caption(mut self, caption: impl Into<Label<'a>>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = decimals;
        self
    }

    pub fn anchor(mut self, anchor: LabelAnchor) -> Self {
        self.anchor = anchor;
        self
    }

    pub fn handle(mut self, handle: DragHandle) -> Self {
        self.handle = Some(handle);
        self
    }

    pub fn limits(mut self, limits: ValueLimits) -> Self {
        self.limits = limits;
        self
    }

    /// The value as shown on the label, unit included.
    pub fn value_text(&self) -> String {
        let v = format_value(self.value, self.decimals);
        if self.unit.is_empty() {
            v
        } else {
            format!("{v} {}", self.unit)
        }
    }

    /// The label's box for `text_size`, in drawing pixels.
    pub fn bounds(&self, text_size: f32) -> Rectangle {
        let caption_chars = self
            .caption
            .as_ref()
            .map_or(0, |c| c.as_str().chars().count());
        let value_chars = self.value_text().chars().count();
        label_box(caption_chars, value_chars, text_size, self.at, self.anchor)
    }
}

/// Colours and size of the labels; colours come from the caller's tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DimensionLabelStyle {
    /// Label text and outline.
    pub ink: Color,
    /// Label fill, so the text reads over the drawing.
    pub fill: Color,
    /// Fill of the open inline field.
    pub field: Color,
    /// Selection inside the inline field.
    pub selection: Color,
    /// Centre of a drag handle.
    pub handle: Color,
    /// Text size from the type scale; every other size derives from it.
    pub text_size: f32,
}

/// A value the user set on the drawing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DimensionCommitted {
    /// Index into the labels the caller passed.
    pub index: usize,
    pub value: f64,
}

/// What [`DimensionLabels::update`] asks of the caller.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DimensionLabelOutcome {
    /// A value to write to the rule.
    pub committed: Option<DimensionCommitted>,
    /// An inline field opened: return [`DimensionLabels::focus_input`].
    pub focus_input: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DimensionLabelMessage {
    /// A label was pressed (or activated from the keyboard).
    LabelPressed(usize),
    /// The text in the open field changed.
    DraftEdited(String),
    /// Enter was pressed in the open field.
    DraftSubmitted,
    /// The field of this label lost focus (Esc, or a press elsewhere).
    DraftBlurred(usize),
    /// Tab was pressed while a field was open.
    TabPressed { backward: bool },
    /// Esc was pressed.
    EscapePressed,
    /// A drag handle was pressed.
    HandlePressed(usize),
    /// The pointer moved over the drawing during a drag.
    PointerMoved(Point),
    /// The pointer button was released.
    PointerReleased,
}

#[derive(Debug, Clone, PartialEq)]
struct Editing {
    index: usize,
    draft: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Drag {
    index: usize,
    /// Pointer position the drag measures from; set by the first move.
    origin: Option<Point>,
    start_value: f64,
    last_value: f64,
}

/// Interaction state of one drawing's labels; lives in the slice's model.
#[derive(Debug, Clone)]
pub struct DimensionLabels {
    input_id: Id,
    editing: Option<Editing>,
    drag: Option<Drag>,
}

impl Default for DimensionLabels {
    fn default() -> Self {
        Self {
            input_id: Id::unique(),
            editing: None,
            drag: None,
        }
    }
}

impl DimensionLabels {
    /// Index of the label whose field is open.
    pub fn editing_index(&self) -> Option<usize> {
        self.editing.as_ref().map(|e| e.index)
    }

    /// Index of the label whose handle is held.
    pub fn dragging_index(&self) -> Option<usize> {
        self.drag.map(|d| d.index)
    }

    /// The message a key press means here, if any: Tab and Shift+Tab while a
    /// field is open, Esc while a field is open or a handle is held.
    pub fn key_message(&self, key: &Key, modifiers: Modifiers) -> Option<DimensionLabelMessage> {
        match key {
            Key::Named(Named::Tab) if self.editing.is_some() => {
                Some(DimensionLabelMessage::TabPressed {
                    backward: modifiers.shift(),
                })
            }
            Key::Named(Named::Escape) if self.editing.is_some() || self.drag.is_some() => {
                Some(DimensionLabelMessage::EscapePressed)
            }
            _ => None,
        }
    }

    /// Focus the open field and select its text.
    pub fn focus_input<T: 'static>(&self) -> Task<T> {
        Task::batch([
            operation::focus(self.input_id.clone()),
            operation::text_input::select_all(self.input_id.clone()),
        ])
    }

    pub fn update(
        &mut self,
        message: DimensionLabelMessage,
        labels: &[DimensionLabel<'_>],
    ) -> DimensionLabelOutcome {
        use DimensionLabelMessage as M;
        match message {
            M::LabelPressed(index) => self.open(index, labels),
            M::DraftEdited(draft) => {
                if let Some(editing) = &mut self.editing {
                    editing.draft = draft;
                }
                DimensionLabelOutcome::default()
            }
            M::DraftSubmitted => DimensionLabelOutcome {
                committed: self.take_draft(),
                focus_input: false,
            },
            M::DraftBlurred(index) => {
                if self.editing_index() == Some(index) {
                    self.editing = None;
                }
                DimensionLabelOutcome::default()
            }
            M::TabPressed { backward } => self.tab(backward, labels),
            M::EscapePressed => self.escape(),
            M::HandlePressed(index) => {
                self.press_handle(index, labels);
                DimensionLabelOutcome::default()
            }
            M::PointerMoved(position) => DimensionLabelOutcome {
                committed: self.drag_to(position, labels),
                focus_input: false,
            },
            M::PointerReleased => {
                self.drag = None;
                DimensionLabelOutcome::default()
            }
        }
    }

    fn open(&mut self, index: usize, labels: &[DimensionLabel<'_>]) -> DimensionLabelOutcome {
        let Some(label) = labels.get(index) else {
            return DimensionLabelOutcome::default();
        };
        self.drag = None;
        self.editing = Some(Editing {
            index,
            draft: draft_text(label.value),
        });
        DimensionLabelOutcome {
            committed: None,
            focus_input: true,
        }
    }

    fn take_draft(&mut self) -> Option<DimensionCommitted> {
        let editing = self.editing.take()?;
        parse_draft(&editing.draft).map(|value| DimensionCommitted {
            index: editing.index,
            value,
        })
    }

    fn tab(&mut self, backward: bool, labels: &[DimensionLabel<'_>]) -> DimensionLabelOutcome {
        let Some(index) = self.editing_index() else {
            return DimensionLabelOutcome::default();
        };
        let committed = self.take_draft();
        let mut outcome = match next_index(index, labels.len(), backward) {
            Some(next) => self.open(next, labels),
            None => DimensionLabelOutcome::default(),
        };
        outcome.committed = committed;
        outcome
    }

    fn escape(&mut self) -> DimensionLabelOutcome {
        self.editing = None;
        let committed = self.drag.take().and_then(|d| {
            (d.last_value != d.start_value).then_some(DimensionCommitted {
                index: d.index,
                value: d.start_value,
            })
        });
        DimensionLabelOutcome {
            committed,
            focus_input: false,
        }
    }

    fn press_handle(&mut self, index: usize, labels: &[DimensionLabel<'_>]) {
        let Some(label) = labels.get(index).filter(|l| l.handle.is_some()) else {
            return;
        };
        self.editing = None;
        self.drag = Some(Drag {
            index,
            origin: None,
            start_value: label.value,
            last_value: label.value,
        });
    }

    fn drag_to(
        &mut self,
        position: Point,
        labels: &[DimensionLabel<'_>],
    ) -> Option<DimensionCommitted> {
        let drag = self.drag.as_mut()?;
        let Some(origin) = drag.origin else {
            drag.origin = Some(position);
            return None;
        };
        let label = labels.get(drag.index)?;
        let handle = label.handle?;
        let value = label
            .limits
            .apply(drag.start_value + handle.value_delta(position - origin));
        if value == drag.last_value {
            return None;
        }
        drag.last_value = value;
        Some(DimensionCommitted {
            index: drag.index,
            value,
        })
    }

    /// The overlay for `labels`; stack it above the drawing.
    pub fn view<'a>(
        &'a self,
        labels: &'a [DimensionLabel<'a>],
        style: DimensionLabelStyle,
    ) -> Element<'a, DimensionLabelMessage> {
        let mut layers: Vec<Element<'a, DimensionLabelMessage>> = Vec::new();
        for (index, label) in labels.iter().enumerate() {
            if let Some(handle) = label.handle {
                layers.push(handle_view(index, handle, style));
            }
        }
        for (index, label) in labels.iter().enumerate() {
            let body = match &self.editing {
                Some(editing) if editing.index == index => {
                    self.field_view(index, label, &editing.draft, style)
                }
                _ => tag_view(index, label, style),
            };
            layers.push(body);
        }
        let overlay = Stack::with_children(layers)
            .width(Length::Fill)
            .height(Length::Fill);
        let mut area = mouse_area(overlay);
        if self.drag.is_some() {
            area = area
                .on_move(DimensionLabelMessage::PointerMoved)
                .on_release(DimensionLabelMessage::PointerReleased);
        }
        area.boxed()
    }

    fn field_view<'a>(
        &'a self,
        index: usize,
        label: &DimensionLabel<'a>,
        draft: &'a str,
        style: DimensionLabelStyle,
    ) -> Element<'a, DimensionLabelMessage> {
        let size = style.text_size;
        let tag = label.bounds(size);
        let width = tag.width.max(FIELD_MIN_EM * size);
        let x = match label.anchor {
            LabelAnchor::Start => tag.x,
            LabelAnchor::Middle => tag.center_x() - width / 2.0,
            LabelAnchor::End => tag.x + tag.width - width,
        };
        let s = style;
        let field = text_input(UserText::new(""), draft)
            .id(self.input_id.clone())
            .on_input(DimensionLabelMessage::DraftEdited)
            .on_submit(DimensionLabelMessage::DraftSubmitted)
            .on_blur(DimensionLabelMessage::DraftBlurred(index))
            .font(Font::MONOSPACE)
            .size(size)
            .padding([PAD_V_EM * size, PAD_H_EM * size])
            .width(width)
            .style(move |_: &Theme, _| text_input::Style {
                background: Background::Color(s.field),
                border: Border {
                    color: s.ink,
                    width: 1.0,
                    radius: (RADIUS_EM * s.text_size).into(),
                    ..Border::default()
                },
                placeholder: s.ink,
                value: s.ink,
                selection: s.selection,
            });
        Translate::new(field.boxed(), (x, tag.y)).into()
    }
}

fn tag_view<'a>(
    index: usize,
    label: &DimensionLabel<'a>,
    style: DimensionLabelStyle,
) -> Element<'a, DimensionLabelMessage> {
    let size = style.text_size;
    let bounds = label.bounds(size);
    let mut content: Row<Element<'a, DimensionLabelMessage>> = Row::new().spacing(GAP_EM * size);
    if let Some(caption) = &label.caption {
        content = content.push(
            text(caption.clone())
                .size(size)
                .font(Font::MONOSPACE)
                .color(style.ink)
                .boxed(),
        );
    }
    content = content.push(
        text(UserText::new(label.value_text()))
            .size(size)
            .font(Font::MONOSPACE)
            .color(style.ink)
            .boxed(),
    );
    let s = style;
    let tag = container(content)
        .width(bounds.width)
        .height(bounds.height)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(s.fill)),
            border: Border {
                color: s.ink,
                width: 1.0,
                radius: (RADIUS_EM * s.text_size).into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let pressable = button(tag)
        .padding(0.0)
        .style(|_: &Theme, _| button::Style::default())
        .on_press(DimensionLabelMessage::LabelPressed(index));
    Translate::new(pressable.boxed(), (bounds.x, bounds.y)).into()
}

fn handle_view<'a>(
    index: usize,
    handle: DragHandle,
    style: DimensionLabelStyle,
) -> Element<'a, DimensionLabelMessage> {
    let size = style.text_size;
    let dot = HANDLE_EM * size;
    let hit = dot + 2.0 * HANDLE_HIT_EM * size;
    let s = style;
    let ring =
        container(Space::new().width(dot).height(dot)).style(move |_: &Theme| container::Style {
            background: Some(Background::Color(s.handle)),
            border: Border {
                color: s.ink,
                width: HANDLE_RING_EM * s.text_size,
                radius: (dot / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let target = container(ring)
        .width(hit)
        .height(hit)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center);
    let grip = mouse_area(target)
        .on_press(DimensionLabelMessage::HandlePressed(index))
        .interaction(handle.interaction());
    Translate::new(
        grip.boxed(),
        (handle.at.x - hit / 2.0, handle.at.y - hit / 2.0),
    )
    .into()
}

/// Box of a label holding `caption_chars` + `value_chars` glyphs at
/// `text_size`, placed on `at` by `anchor`.
pub fn label_box(
    caption_chars: usize,
    value_chars: usize,
    text_size: f32,
    at: Point,
    anchor: LabelAnchor,
) -> Rectangle {
    let glyphs = (caption_chars + value_chars) as f32;
    let gap = if caption_chars > 0 { GAP_EM } else { 0.0 };
    let width = (glyphs * GLYPH_EM + gap + 2.0 * PAD_H_EM) * text_size;
    let height = (LINE_EM + 2.0 * PAD_V_EM) * text_size;
    let x = match anchor {
        LabelAnchor::Start => at.x,
        LabelAnchor::Middle => at.x - width / 2.0,
        LabelAnchor::End => at.x - width,
    };
    Rectangle {
        x,
        y: at.y - height / 2.0,
        width,
        height,
    }
}

/// `value` with `decimals` decimals, `.` as the separator, never `-0`.
pub fn format_value(value: f64, decimals: usize) -> String {
    let text = format!("{value:.decimals$}");
    if text.starts_with('-') && text[1..].chars().all(|c| c == '0' || c == '.') {
        text[1..].to_owned()
    } else {
        text
    }
}

/// The text an inline field opens with: up to three decimals, trailing
/// zeros dropped.
pub fn draft_text(value: f64) -> String {
    let text = format_value(value, DRAFT_DECIMALS);
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    }
}

/// A typed value: `,` is read as `.` and `−` as `-`. `None` when it is not
/// a finite number.
pub fn parse_draft(draft: &str) -> Option<f64> {
    let cleaned = draft.trim().replace(',', ".").replace('\u{2212}', "-");
    cleaned.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// The label Tab moves to from `index`, wrapping at both ends.
pub fn next_index(index: usize, count: usize, backward: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(if backward {
        (index + count - 1) % count
    } else {
        (index + 1) % count
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels() -> Vec<DimensionLabel<'static>> {
        let handle = DragHandle {
            at: Point::new(100.0, 50.0),
            axis: Vector::new(1.0, 0.0),
            value_per_px: 0.01,
        };
        let limits = ValueLimits {
            min: Some(0.005),
            max: None,
            step: 0.005,
        };
        vec![
            DimensionLabel::new(0.15, "mm", Point::new(40.0, 20.0))
                .handle(handle)
                .limits(limits),
            DimensionLabel::new(0.2, "mm", Point::new(80.0, 20.0)),
            DimensionLabel::new(1.0, "mm", Point::new(120.0, 20.0)),
        ]
    }

    fn committed(index: usize, value: f64) -> Option<DimensionCommitted> {
        Some(DimensionCommitted { index, value })
    }

    #[test]
    fn draft_opens_with_three_decimals_and_no_trailing_zeros() {
        assert_eq!(draft_text(0.15), "0.15");
        assert_eq!(draft_text(1.0), "1");
        assert_eq!(draft_text(0.12345), "0.123");
        assert_eq!(draft_text(-0.0001), "0");
    }

    #[test]
    fn parse_reads_comma_and_minus_sign_and_refuses_junk() {
        assert_eq!(parse_draft(" 0,25 "), Some(0.25));
        assert_eq!(parse_draft("\u{2212}0.1"), Some(-0.1));
        assert_eq!(parse_draft("abc"), None);
        assert_eq!(parse_draft("inf"), None);
        assert_eq!(parse_draft(""), None);
    }

    #[test]
    fn format_never_shows_negative_zero() {
        assert_eq!(format_value(-0.001, 2), "0.00");
        assert_eq!(format_value(-0.5, 2), "-0.50");
        assert_eq!(format_value(0.2, 2), "0.20");
    }

    #[test]
    fn limits_snap_then_clamp() {
        let l = ValueLimits {
            min: Some(0.005),
            max: Some(1.0),
            step: 0.005,
        };
        assert_eq!(l.apply(0.1234), 0.125);
        assert_eq!(l.apply(-3.0), 0.005);
        assert_eq!(l.apply(9.0), 1.0);
        assert_eq!(ValueLimits::default().apply(0.1234), 0.1234);
    }

    #[test]
    fn tab_wraps_both_ways() {
        assert_eq!(next_index(2, 3, false), Some(0));
        assert_eq!(next_index(0, 3, true), Some(2));
        assert_eq!(next_index(0, 0, false), None);
    }

    #[test]
    fn press_opens_the_field_and_enter_commits() {
        let items = labels();
        let mut s = DimensionLabels::default();
        let out = s.update(DimensionLabelMessage::LabelPressed(1), &items);
        assert!(out.focus_input);
        assert_eq!(s.editing_index(), Some(1));
        s.update(DimensionLabelMessage::DraftEdited("0.3".into()), &items);
        let out = s.update(DimensionLabelMessage::DraftSubmitted, &items);
        assert_eq!(out.committed, committed(1, 0.3));
        assert_eq!(s.editing_index(), None);
    }

    #[test]
    fn enter_on_junk_closes_without_committing() {
        let items = labels();
        let mut s = DimensionLabels::default();
        s.update(DimensionLabelMessage::LabelPressed(0), &items);
        s.update(DimensionLabelMessage::DraftEdited("x".into()), &items);
        let out = s.update(DimensionLabelMessage::DraftSubmitted, &items);
        assert_eq!(out.committed, None);
        assert_eq!(s.editing_index(), None);
    }

    #[test]
    fn tab_commits_and_opens_the_next_label() {
        let items = labels();
        let mut s = DimensionLabels::default();
        s.update(DimensionLabelMessage::LabelPressed(2), &items);
        s.update(DimensionLabelMessage::DraftEdited("2".into()), &items);
        let out = s.update(
            DimensionLabelMessage::TabPressed { backward: false },
            &items,
        );
        assert_eq!(out.committed, committed(2, 2.0));
        assert!(out.focus_input);
        assert_eq!(s.editing_index(), Some(0));
        let out = s.update(DimensionLabelMessage::TabPressed { backward: true }, &items);
        assert_eq!(out.committed, committed(0, 0.15));
        assert_eq!(s.editing_index(), Some(2));
    }

    #[test]
    fn escape_and_blur_cancel_but_a_stale_blur_is_ignored() {
        let items = labels();
        let mut s = DimensionLabels::default();
        s.update(DimensionLabelMessage::LabelPressed(0), &items);
        s.update(DimensionLabelMessage::LabelPressed(1), &items);
        let out = s.update(DimensionLabelMessage::DraftBlurred(0), &items);
        assert_eq!(out, DimensionLabelOutcome::default());
        assert_eq!(s.editing_index(), Some(1));
        s.update(DimensionLabelMessage::DraftBlurred(1), &items);
        assert_eq!(s.editing_index(), None);
        s.update(DimensionLabelMessage::LabelPressed(2), &items);
        let out = s.update(DimensionLabelMessage::EscapePressed, &items);
        assert_eq!(out.committed, None);
        assert_eq!(s.editing_index(), None);
    }

    #[test]
    fn drag_moves_along_the_axis_snapped_and_clamped() {
        let items = labels();
        let mut s = DimensionLabels::default();
        s.update(DimensionLabelMessage::HandlePressed(0), &items);
        assert_eq!(s.dragging_index(), Some(0));
        let first = s.update(
            DimensionLabelMessage::PointerMoved(Point::new(10.0, 10.0)),
            &items,
        );
        assert_eq!(first.committed, None);
        let out = s.update(
            DimensionLabelMessage::PointerMoved(Point::new(20.0, 99.0)),
            &items,
        );
        assert_eq!(out.committed, committed(0, 0.25));
        let same = s.update(
            DimensionLabelMessage::PointerMoved(Point::new(20.0, 0.0)),
            &items,
        );
        assert_eq!(same.committed, None);
        let out = s.update(
            DimensionLabelMessage::PointerMoved(Point::new(-90.0, 0.0)),
            &items,
        );
        assert_eq!(out.committed, committed(0, 0.005));
        s.update(DimensionLabelMessage::PointerReleased, &items);
        assert_eq!(s.dragging_index(), None);
    }

    #[test]
    fn escape_during_a_drag_restores_the_start_value() {
        let items = labels();
        let mut s = DimensionLabels::default();
        s.update(DimensionLabelMessage::HandlePressed(0), &items);
        s.update(
            DimensionLabelMessage::PointerMoved(Point::new(0.0, 0.0)),
            &items,
        );
        s.update(
            DimensionLabelMessage::PointerMoved(Point::new(5.0, 0.0)),
            &items,
        );
        let out = s.update(DimensionLabelMessage::EscapePressed, &items);
        assert_eq!(out.committed, committed(0, 0.15));
        assert_eq!(s.dragging_index(), None);
    }

    #[test]
    fn a_label_without_a_handle_does_not_drag() {
        let items = labels();
        let mut s = DimensionLabels::default();
        s.update(DimensionLabelMessage::HandlePressed(1), &items);
        assert_eq!(s.dragging_index(), None);
        s.update(DimensionLabelMessage::HandlePressed(9), &items);
        assert_eq!(s.dragging_index(), None);
    }

    #[test]
    fn keys_route_only_while_a_field_is_open_or_a_handle_is_held() {
        let items = labels();
        let mut s = DimensionLabels::default();
        let tab = Key::Named(Named::Tab);
        let esc = Key::Named(Named::Escape);
        assert_eq!(s.key_message(&tab, Modifiers::empty()), None);
        assert_eq!(s.key_message(&esc, Modifiers::empty()), None);
        s.update(DimensionLabelMessage::LabelPressed(0), &items);
        assert_eq!(
            s.key_message(&tab, Modifiers::SHIFT),
            Some(DimensionLabelMessage::TabPressed { backward: true })
        );
        assert_eq!(
            s.key_message(&esc, Modifiers::empty()),
            Some(DimensionLabelMessage::EscapePressed)
        );
    }

    #[test]
    fn label_box_sizes_from_the_text_size_and_anchors() {
        let at = Point::new(100.0, 50.0);
        let mid = label_box(0, 7, 10.0, at, LabelAnchor::Middle);
        let doubled = label_box(0, 7, 20.0, at, LabelAnchor::Middle);
        assert!((doubled.width - 2.0 * mid.width).abs() < 1e-3);
        assert!((mid.center_x() - 100.0).abs() < 1e-3);
        assert!((mid.center_y() - 50.0).abs() < 1e-3);
        assert_eq!(label_box(0, 7, 10.0, at, LabelAnchor::Start).x, 100.0);
        let end = label_box(0, 7, 10.0, at, LabelAnchor::End);
        assert!((end.x + end.width - 100.0).abs() < 1e-3);
    }

    #[test]
    fn view_builds_idle_editing_and_dragging_states() {
        let items = labels();
        let style = DimensionLabelStyle {
            ink: Color::WHITE,
            fill: Color::BLACK,
            field: Color::BLACK,
            selection: Color::WHITE,
            handle: Color::WHITE,
            text_size: 11.0,
        };
        let mut s = DimensionLabels::default();
        let _ = s.view(&items, style);
        s.update(DimensionLabelMessage::LabelPressed(1), &items);
        let _ = s.view(&items, style);
        s.update(DimensionLabelMessage::HandlePressed(0), &items);
        let _ = s.view(&items, style);
    }
}
