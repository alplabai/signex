//! The armed dimension tool's Properties (mockup row 157): the form shows
//! what the next dimension is placed with, Tab pauses to edit it and Space
//! flips the side of the dimension line.

use signex_types::schematic::{DimensionDirection, Point, SchDimension, SchDrawing};

use super::super::super::*;
use crate::app::sch_tables::{AnnotationEdit, apply_annotation_edit};
use crate::panels::{AnnotationPanelMsg, DimensionToolForm};

/// The span the form's drawing shows until two points are clicked (mm).
const SAMPLE_SPAN_MM: f64 = 12.7;
const SAMPLE_OFFSET_MM: f64 = -2.54;

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
        let state = &self.interaction_state.annotation_tool;
        let style = &state.dimension;
        let from = state.points.first().copied();
        let to = state.points.get(1).copied();
        let shown_from = from.unwrap_or(Point::new(0.0, 0.0));
        let shown_to = to.unwrap_or(Point::new(shown_from.x + SAMPLE_SPAN_MM, shown_from.y));
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
        Some(Box::new(DimensionToolForm {
            dimension,
            is_aligned,
            has_from: from.is_some(),
            has_to: to.is_some(),
            is_paused: self.interaction_state.active_canvas().placement_paused,
        }))
    }

    /// Whether the Properties form belongs to the armed dimension tool.
    pub(crate) fn is_dimension_tool_form_shown(&self) -> bool {
        self.armed_dimension_tool().is_some()
    }

    /// A message of the armed tool's form: it edits the tool, never the sheet.
    pub(crate) fn handle_dimension_tool_panel(&mut self, msg: AnnotationPanelMsg) {
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
                let draft = &mut self.document_state.panel_ctx.annotation.draft;
                if matches!(draft, Some((d, f, _)) if *d == id && *f == field) {
                    *draft = None;
                }
            }
            AnnotationPanelMsg::Picked(edit) => {
                self.document_state.panel_ctx.annotation.draft = None;
                self.apply_dimension_tool_edit(&edit);
            }
            AnnotationPanelMsg::CsvImportRequested | AnnotationPanelMsg::CsvExportRequested => {}
        }
        self.sync_annotation_draft();
    }

    fn apply_dimension_tool_edit(&mut self, edit: &AnnotationEdit) {
        let Some(form) = self.dimension_tool_form() else {
            return;
        };
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
        tool.dimension.override_text = next.override_text;
    }

    /// Space while a dimension is under way: the line goes to the other side
    /// of the measured points. `false` leaves the key to its usual action.
    pub(crate) fn flip_dimension_side(&mut self) -> bool {
        if self.armed_dimension_tool().is_none()
            || self.interaction_state.annotation_tool.points.is_empty()
        {
            return false;
        }
        if !self.interaction_state.active_canvas().placement_paused {
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

    /// The pause ended: the box lets go, so Space and Tab reach the tool.
    pub(crate) fn release_dimension_override(&mut self) -> Task<Message> {
        if self.armed_dimension_tool().is_none()
            || self.interaction_state.active_canvas().placement_paused
        {
            return Task::none();
        }
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
