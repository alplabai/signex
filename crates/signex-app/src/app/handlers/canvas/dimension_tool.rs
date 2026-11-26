//! The armed dimension tool's Properties (mockup row 157): the form shows
//! what the next dimension is placed with, Tab pauses to edit it and Space
//! flips the side of the dimension line.

use signex_types::schematic::{DimensionDirection, Point, SchDimension, SchDrawing};

use super::super::super::*;
use super::place_dimension::DimensionToolStyle;
use crate::app::dispatch::input::ESCAPE_KEY;
use crate::app::sch_tables::{AnnotationEdit, apply_annotation_edit};
use crate::panels::{AnnotationField, AnnotationPanelMsg, DimensionToolForm};

/// The span the form's drawing shows until two points are clicked (mm).
const SAMPLE_SPAN_MM: f64 = 12.7;
const SAMPLE_OFFSET_MM: f64 = -2.54;

/// Whether `event` shows the Esc press that resumed is no longer down: its
/// release, or a window event after which that release may never arrive.
pub(crate) fn ends_held_resume_esc(event: &iced::Event) -> bool {
    match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyReleased { key, .. }) => {
            ESCAPE_KEY.matches(key)
        }
        iced::Event::Window(iced::window::Event::Unfocused | iced::window::Event::Closed) => true,
        _ => false,
    }
}

/// The form showing `points` and the values the next dimension takes. With
/// From clicked, To shows `cursor` (the snapped pointer) until it is clicked.
fn dimension_form(
    tool: Tool,
    style: &DimensionToolStyle,
    points: &[Point],
    cursor: Option<Point>,
    is_paused: bool,
) -> Box<DimensionToolForm> {
    let from = points.first().copied();
    let to = points.get(1).copied();
    let following = from.and(cursor).filter(|_| to.is_none());
    let shown_from = from.unwrap_or(Point::new(0.0, 0.0));
    let shown_to = to
        .or(following)
        .unwrap_or(Point::new(shown_from.x + SAMPLE_SPAN_MM, shown_from.y));
    let is_aligned = tool == Tool::AlignedDimension;
    let direction = style.direction.unwrap_or(if is_aligned {
        DimensionDirection::Aligned
    } else {
        SchDimension::linear_direction(shown_from, shown_to)
    });
    let mut dimension = SchDimension::new(
        uuid::Uuid::nil(),
        shown_from,
        shown_to,
        SAMPLE_OFFSET_MM,
        direction,
    );
    dimension.units = style.units;
    dimension.precision = style.precision;
    dimension.arrows = style.arrows;
    dimension.override_text = style.override_text.clone();
    Box::new(DimensionToolForm {
        dimension,
        is_aligned,
        has_from: from.is_some(),
        has_to: to.is_some(),
        is_to_following: following.is_some(),
        is_paused,
    })
}

/// The form of a window that does not hold the dimension under way: the
/// tool's settings, without that dimension's points and Override.
fn form_without_points(
    tool: Tool,
    style: &DimensionToolStyle,
    is_paused: bool,
) -> Box<DimensionToolForm> {
    let mut settings = style.clone();
    settings.end_dimension();
    dimension_form(tool, &settings, &[], None, is_paused)
}

impl Signex {
    /// The dimension tool armed on a schematic sheet.
    fn armed_dimension_tool(&self) -> Option<Tool> {
        let tool = self.interaction_state.current_tool;
        (matches!(tool, Tool::LinearDimension | Tool::AlignedDimension)
            && !self.has_active_pcb()
            && self.document_state.active_engine().is_some())
        .then_some(tool)
    }

    /// What Properties shows while a dimension tool is armed.
    pub(crate) fn dimension_tool_form(&self) -> Option<Box<DimensionToolForm>> {
        let tool = self.armed_dimension_tool()?;
        let state = &self.interaction_state;
        let is_paused = state.active_canvas().placement_paused;
        let annotation = &state.annotation_tool;
        if state.is_dimension_held_elsewhere() && !state.is_sheet_pause_elsewhere() {
            return Some(form_without_points(tool, &annotation.dimension, is_paused));
        }
        // Paused, nothing follows the pointer, as on the canvas.
        let cursor = (!is_paused).then(|| {
            let (x, y) = self.snap_to_grid(self.ui_state.cursor_x, self.ui_state.cursor_y);
            Point::new(x, y)
        });
        Some(dimension_form(
            tool,
            &annotation.dimension,
            &annotation.points,
            cursor,
            is_paused,
        ))
    }

    /// The pointer moved: while To follows it, Properties shows the span
    /// the next click would place (mockup row 157).
    pub(crate) fn dimension_tool_cursor_moved(&mut self) {
        if self.armed_dimension_tool().is_none()
            || self.interaction_state.annotation_tool.points.len() != 1
        {
            return;
        }
        let form = self.dimension_tool_form();
        if self.document_state.panel_ctx.annotation.dimension_tool != form {
            self.document_state.panel_ctx.annotation.dimension_tool = form;
        }
    }

    /// The form for windows that do not hold the unpaused dimension under
    /// way, with the window that does.
    pub(crate) fn form_for_other_windows(
        &self,
    ) -> Option<(Option<iced::window::Id>, Box<DimensionToolForm>)> {
        let state = &self.interaction_state;
        let owner = state.dimension_points_window()?;
        if state.paused_annotation_window().is_some() {
            return None;
        }
        let form = form_without_points(state.current_tool, &state.annotation_tool.dimension, false);
        Some((owner, form))
    }

    /// Whether the Properties form belongs to the armed dimension tool.
    pub(crate) fn is_dimension_tool_form_shown(&self) -> bool {
        self.armed_dimension_tool().is_some()
    }

    /// Whether `msg` edits what belongs to a dimension held in another
    /// window: its points, its Override text.
    fn edits_dimension_held_elsewhere(&self, msg: &AnnotationPanelMsg) -> bool {
        use AnnotationField::{DimensionPoint, OverrideText};
        self.interaction_state.is_dimension_held_elsewhere()
            && matches!(
                msg,
                AnnotationPanelMsg::FieldTyped(DimensionPoint(..) | OverrideText, _)
                    | AnnotationPanelMsg::FieldSubmitted(DimensionPoint(..) | OverrideText)
                    | AnnotationPanelMsg::Picked(
                        AnnotationEdit::DimensionPoint(..)
                            | AnnotationEdit::OverrideEnabled(_)
                            | AnnotationEdit::OverrideText(_)
                    )
            )
    }

    /// A message of the armed tool's form: it edits the tool, never the sheet.
    pub(crate) fn handle_dimension_tool_panel(&mut self, msg: AnnotationPanelMsg) -> Task<Message> {
        // The values of a dimension paused in another window are that
        // window's form to edit, and a held point is its window's alone.
        if self.interaction_state.is_sheet_pause_elsewhere()
            || self.edits_dimension_held_elsewhere(&msg)
        {
            return Task::none();
        }
        let id = uuid::Uuid::nil();
        match msg {
            AnnotationPanelMsg::FieldTyped(field, text) => {
                let edit = self.document_state.active_engine().and_then(|engine| {
                    let sheet = engine.document();
                    field.edit(&text, |y| sheet.page_y(y))
                });
                self.document_state.panel_ctx.annotation.draft = Some((id, field, text));
                if let Some(edit) = edit {
                    self.apply_dimension_tool_edit(&edit);
                }
            }
            AnnotationPanelMsg::FieldSubmitted(field) => {
                return self.submit_dimension_tool_field(field);
            }
            AnnotationPanelMsg::Picked(edit) => {
                self.document_state.panel_ctx.annotation.draft = None;
                self.apply_dimension_tool_edit(&edit);
            }
            // The box captured the key: this is the press's one Esc. Only the
            // pause answers, so no menu or dialog of any window closes.
            AnnotationPanelMsg::OverrideEscaped => {
                self.cancel_pending_chord();
                if self.interaction_state.active_canvas().placement_paused {
                    self.interaction_state.annotation_tool.is_resume_esc_held = true;
                    return self.resume_placement_without_edits();
                }
                return Task::none();
            }
            AnnotationPanelMsg::CsvImportRequested
            | AnnotationPanelMsg::CsvExportRequested
            | AnnotationPanelMsg::Image(_) => {}
        }
        self.sync_annotation_draft();
        Task::none()
    }

    /// Enter in the form: the typed value stands and a paused tool goes on
    /// placing, as a second Tab does. Text that does not read, in whichever
    /// box of the form holds it, stays there and keeps the pause.
    fn submit_dimension_tool_field(&mut self, field: AnnotationField) -> Task<Message> {
        let is_paused = self.interaction_state.active_canvas().placement_paused;
        if is_paused && !self.dimension_form_draft_reads() {
            return Task::none();
        }
        let draft = &mut self.document_state.panel_ctx.annotation.draft;
        draft.take_if(|(id, f, _)| id.is_nil() && *f == field);
        self.sync_annotation_draft();
        if !is_paused {
            return Task::none();
        }
        self.update(Message::Tool(ToolMessage::PrePlacementTab))
    }

    /// Whether this key event is a repeat of the Esc press that resumed from
    /// the Override box. A new Esc press or its release ends that hold; the
    /// release is read here too because a quick tap can arrive before the
    /// held-Esc listener is live.
    pub(crate) fn is_repeat_of_resuming_esc(&mut self, event: &iced::keyboard::Event) -> bool {
        use iced::keyboard::Event::{KeyPressed, KeyReleased};
        let held = &mut self.interaction_state.annotation_tool.is_resume_esc_held;
        match event {
            KeyPressed { key, repeat, .. } if ESCAPE_KEY.matches(key) => {
                *held &= *repeat;
                *held
            }
            KeyReleased { key, .. } if ESCAPE_KEY.matches(key) => {
                *held = false;
                false
            }
            _ => false,
        }
    }

    /// Whether the armed tool's form holds text that does not read, in any
    /// of its boxes: Tab keeps the pause for it (mockup row 157).
    pub(crate) fn dimension_form_holds_refused_text(&self) -> bool {
        self.armed_dimension_tool().is_some() && !self.dimension_form_draft_reads()
    }

    /// Whether the text typed in the form, if any, reads for its own box.
    fn dimension_form_draft_reads(&self) -> bool {
        let draft = self.document_state.panel_ctx.annotation.draft.as_ref();
        draft
            .filter(|(id, _, _)| id.is_nil())
            .is_none_or(|(_, field, text)| {
                self.document_state.active_engine().is_some_and(|engine| {
                    let sheet = engine.document();
                    field.edit(text, |y| sheet.page_y(y)).is_some()
                })
            })
    }

    fn apply_dimension_tool_edit(&mut self, edit: &AnnotationEdit) {
        let Some(form) = self.dimension_tool_form() else {
            return;
        };
        // This window's form has no Override of the dimension held elsewhere.
        let keeps_override = self.interaction_state.is_dimension_held_elsewhere();
        let tool = &mut self.interaction_state.annotation_tool;
        match edit {
            // A pick pins the direction even when it matches the drag's.
            AnnotationEdit::Direction(direction) => {
                tool.dimension.direction = Some(*direction);
                return;
            }
            // An emptied box holds nothing typed: Override is off again.
            AnnotationEdit::OverrideText(text) if form.is_paused && text.is_empty() => {
                tool.dimension.override_text = None;
                self.document_state.panel_ctx.annotation.draft = None;
                return;
            }
            // Nothing is measured yet, so there is no value to start from.
            AnnotationEdit::OverrideEnabled(true) if !form.has_to => {
                tool.dimension.override_text = Some(String::new());
                return;
            }
            _ => {}
        }
        let drawing = SchDrawing::Dimension(form.dimension);
        let Some(SchDrawing::Dimension(next)) = apply_annotation_edit(&drawing, edit) else {
            return;
        };
        if let AnnotationEdit::DimensionPoint(second, _, _) = edit {
            let (index, value) = if *second {
                (1, next.to)
            } else {
                (0, next.from)
            };
            if let Some(point) = tool.points.get_mut(index) {
                *point = value;
            }
            return;
        }
        tool.dimension.units = next.units;
        tool.dimension.precision = next.precision;
        tool.dimension.arrows = next.arrows;
        if !keeps_override {
            tool.dimension.override_text = next.override_text;
        }
    }

    /// Space while a dimension is under way: the line goes to the other side
    /// of the measured points. `false` leaves the key to its usual action.
    pub(crate) fn flip_dimension_side(&mut self) -> bool {
        if self.armed_dimension_tool().is_none()
            || self.interaction_state.annotation_tool.points.is_empty()
        {
            return false;
        }
        if !self.interaction_state.active_canvas().placement_paused
            && !self.interaction_state.is_sheet_pause_elsewhere()
            && !self.interaction_state.is_dimension_held_elsewhere()
        {
            let style = &mut self.interaction_state.annotation_tool.dimension;
            style.flipped = !style.flipped;
            self.sync_annotation_draft();
        }
        true
    }

    /// Text typed in the form belongs to the dimension under way.
    pub(crate) fn drop_dimension_form_draft(&mut self) {
        let draft = &mut self.document_state.panel_ctx.annotation.draft;
        if matches!(draft, Some((id, _, _)) if id.is_nil()) {
            *draft = None;
        }
    }

    /// The tool changed: a picked direction and a pause's saved values
    /// belong to the tool they were made under.
    pub(crate) fn leave_dimension_tool(&mut self) {
        let tool = &mut self.interaction_state.annotation_tool;
        tool.dimension.direction = None;
        tool.paused_with = None;
    }

    /// Tab paused the armed tool: keep what Esc puts back.
    pub(crate) fn note_dimension_tool_pause(&mut self) {
        let is_armed = self.armed_dimension_tool().is_some();
        let tool = &mut self.interaction_state.annotation_tool;
        tool.paused_with =
            is_armed.then(|| Box::new((tool.dimension.clone(), tool.points.clone())));
        if is_armed {
            // A folded section has no box to take the keys.
            self.document_state
                .panel_ctx
                .collapsed_sections
                .remove(crate::panels::DIMENSION_TEXT_SECTION);
            self.sync_annotation_draft();
        }
    }

    /// Tab paused the armed tool: the keys go to the Override box.
    pub(crate) fn focus_dimension_override(&self) -> Task<Message> {
        if self.armed_dimension_tool().is_none() {
            return Task::none();
        }
        iced::widget::operation::focus(iced::widget::Id::new(
            crate::panels::DIMENSION_OVERRIDE_INPUT,
        ))
    }

    /// The pause ended: the box lets go, so Space and Tab reach the tool, and
    /// text it refused does not outlive the pause it was typed in.
    pub(crate) fn release_dimension_override(&mut self) -> Task<Message> {
        if self.armed_dimension_tool().is_none()
            || self.interaction_state.active_canvas().placement_paused
        {
            return Task::none();
        }
        self.drop_dimension_form_draft();
        self.sync_annotation_draft();
        super::super::detached_modals::unfocus_text_inputs()
    }

    /// Esc while paused: drop the form's edits.
    pub(crate) fn restore_dimension_tool_pause(&mut self) {
        let tool = &mut self.interaction_state.annotation_tool;
        if let Some(values) = tool.paused_with.take() {
            (tool.dimension, tool.points) = *values;
            self.document_state.panel_ctx.annotation.draft = None;
            self.sync_annotation_draft();
        }
    }
}
