//! The preview's values as editable labels and handles (mockup row 36,
//! #2685). The canvas draws the words; a bare label is the press target over
//! them. A value typed on the drawing is the same edit as typing it in its
//! field, and a drag is one undo step.

use iced::{Point, Task, Vector};
use signex_engine::ProjectGeneralCommand;
use signex_i18n::{I18n, UserText};
use signex_types::coord::Unit;
use signex_types::project::{SchematicDefaultField, SchematicDefaultOverrides};
use signex_types::schematic::SchematicDefaults;
use signex_types::theme::{CanvasColors, ThemeTokens};
use signex_widgets::LengthFormat;
use signex_widgets::dimension_label::{
    DimensionLabel, DimensionLabelMessage, DimensionLabelStyle, DimensionLabels, DragHandle,
    ValueLimits,
};

use super::super::ProjectTabMsg;
use super::super::table::TS;
use super::preview::{
    BUS_Y, JUNCTION, LABEL_PX, PX_PER_MM, VIEW_WIDTH, label_place, value_labels, width_px,
};
use super::{DefaultInput, parse_override};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};
use crate::styles::{ti, tokens::resolve};

/// Decimals an open label's value is read to on the tag.
const DECIMALS: usize = 3;
/// A dragged value snaps to this.
const DRAG_STEP: f64 = 0.01;
/// Clear space between a drawn item and its handle, in layout units.
const HANDLE_GAP: f32 = 5.0;
/// Where the junction's handle sits on its circle: down and to the right.
const DIAGONAL: f32 = std::f32::consts::FRAC_1_SQRT_2;
/// Where along the bus its handle sits, from the drawing's right edge.
const BUS_HANDLE_INSET: f32 = 40.0;
/// Middle of a label's text above the baseline the canvas draws it on.
const TEXT_MID_EM: f32 = 0.65;

fn limits(field: SchematicDefaultField) -> ValueLimits {
    let range = field.range();
    ValueLimits {
        min: Some(*range.start()),
        max: Some(*range.end()),
        step: DRAG_STEP,
    }
}

/// The handle of `field`: the junction's size and the bus width. A pixel of
/// drag moves the drawn edge a pixel, half of the drawn width.
fn handle_of(
    field: SchematicDefaultField,
    values: &SchematicDefaults,
    zoom: f32,
) -> Option<DragHandle> {
    let (at, axis) = match field {
        SchematicDefaultField::JunctionDiameter => {
            let radius = width_px(values.junction_diameter_mm) / 2.0;
            let off = radius * DIAGONAL + HANDLE_GAP;
            ((JUNCTION.0 + off, JUNCTION.1 + off), Vector::new(1.0, 1.0))
        }
        SchematicDefaultField::BusWidth => {
            let half = width_px(values.bus_width_mm) / 2.0;
            (
                (VIEW_WIDTH - BUS_HANDLE_INSET, BUS_Y - half - HANDLE_GAP),
                Vector::new(0.0, -1.0),
            )
        }
        _ => return None,
    };
    Some(DragHandle {
        at: Point::new(at.0 * zoom, at.1 * zoom),
        axis,
        value_per_px: 2.0 / f64::from(PX_PER_MM * zoom),
    })
}

/// One label per numeric field, in [`SchematicDefaultField::ALL`] order. The
/// value is the one in effect; lengths read in mm, the text size in pt.
pub(in crate::app::project_tab) fn preview_labels<'a>(
    overrides: &SchematicDefaultOverrides,
    zoom: f32,
    i18n: &'a I18n,
) -> Vec<DimensionLabel<'a>> {
    let format = LengthFormat::new(i18n, Unit::Mm);
    let resolved = overrides.resolve();
    value_labels(overrides, i18n)
        .into_iter()
        .map(|shown| {
            let field = shown.field;
            let ((x, y), anchor) = label_place(field, VIEW_WIDTH);
            let at = Point::new(x * zoom, (y - LABEL_PX * TEXT_MID_EM) * zoom);
            let label = DimensionLabel::new(field.resolved(&resolved), format, at)
                .caption(UserText::new(shown.words))
                .anchor(anchor)
                .decimals(DECIMALS)
                .limits(limits(field))
                .bare();
            match handle_of(field, &resolved, zoom) {
                Some(handle) => label.handle(handle),
                None => label,
            }
        })
        .collect()
}

/// The open field and handles, in the theme's accent on the canvas paper.
pub(super) fn label_style(
    canvas: &CanvasColors,
    tokens: &ThemeTokens,
    zoom: f32,
) -> DimensionLabelStyle {
    DimensionLabelStyle {
        ink: ti(tokens.accent),
        fill: ti(canvas.paper),
        field: resolve(tokens).field,
        selection: ti(tokens.selection),
        handle: ti(canvas.paper),
        text_size: TS.caption * zoom,
    }
}

/// Whether `message` closes the open field, so its text is written first.
fn leaves_open_field(labels: &DimensionLabels, message: &DimensionLabelMessage) -> bool {
    use DimensionLabelMessage as M;
    let open = labels.editing_index();
    match message {
        M::DraftSubmitted | M::TabPressed { .. } => open.is_some(),
        M::DraftBlurred(index) => open == Some(*index),
        M::LabelPressed(index) | M::HandlePressed(index) => open.is_some_and(|o| o != *index),
        _ => false,
    }
}

/// The open field's field and text, unless it still holds what it opened
/// with: that text is rounded and must not be written back.
fn typed_draft(
    labels: &DimensionLabels,
    shown: &[DimensionLabel<'_>],
) -> Option<(SchematicDefaultField, String)> {
    let index = labels.editing_index()?;
    if !labels.draft_refused(shown) && labels.pending_value(shown).is_none() {
        return None;
    }
    let field = *SchematicDefaultField::ALL.get(index)?;
    Some((field, labels.draft()?.to_string()))
}

impl Signex {
    /// A message of the drawing's labels: a typed value goes through the
    /// field's edit, a drag writes the value it reaches, Esc puts it back.
    pub(in crate::app::project_tab) fn act_on_default_labels(
        &mut self,
        project_id: ProjectId,
        message: DimensionLabelMessage,
    ) -> Task<Message> {
        use DimensionLabelMessage as M;
        self.ui_state.project_preview.adopt(project_id);
        // Lengths read `.` in every locale, so the fallback bundle parses
        // exactly what the view shows.
        let en = I18n::en_us();
        if !self.commit_open_default(project_id, &message, &en) {
            return Task::none();
        }
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return Task::none();
        };
        let zoom = self.ui_state.project_preview.zoom();
        let shown = preview_labels(&project.data.schematic_defaults, zoom, &en);
        let open = self.ui_state.project_preview.labels.editing_index();
        let event = match message {
            M::DraftBlurred(index) if open == Some(index) => M::DraftSubmitted,
            other => other,
        };
        let is_focused = self.update_default_labels(project_id, event, &shown);
        if is_focused {
            self.ui_state.project_preview.labels.focus_input()
        } else {
            Task::none()
        }
    }

    /// Hand `event` to the labels and apply what a drag did. Returns whether
    /// a field opened and wants the focus.
    fn update_default_labels(
        &mut self,
        project_id: ProjectId,
        event: DimensionLabelMessage,
        shown: &[DimensionLabel<'_>],
    ) -> bool {
        use DimensionLabelMessage as M;
        let labels = &self.ui_state.project_preview.labels;
        let was_held = labels.dragging_index().is_some();
        let is_escape = matches!(event, M::EscapePressed);
        let is_move = matches!(event, M::PointerMoved(_));
        let is_release = matches!(event, M::PointerReleased);
        if is_escape {
            self.drop_label_draft(project_id);
        }
        if matches!(event, M::HandlePressed(_)) {
            self.close_general_run(project_id);
        }
        let outcome = self.ui_state.project_preview.labels.update(event, shown);
        if is_move {
            self.write_dragged_default(project_id, outcome.committed);
        }
        if is_escape && was_held && outcome.committed.is_some() {
            // The whole drag is one step, so one undo restores the value
            // it started from, an unset default included.
            self.step_project_tab_history(project_id, true);
        }
        if is_release {
            self.close_general_run(project_id);
        }
        outcome.focus_input
    }

    /// Write the open field's typed text before `message` closes it, as one
    /// undo step. `false` when the text is refused: the field stays open and
    /// its form box shows the same refusal.
    fn commit_open_default(
        &mut self,
        project_id: ProjectId,
        message: &DimensionLabelMessage,
        en: &I18n,
    ) -> bool {
        let ui = &self.ui_state.project_preview;
        if !leaves_open_field(&ui.labels, message) {
            return true;
        }
        let Some(project) = self.document_state.project_by_id(project_id) else {
            return false;
        };
        let shown = preview_labels(&project.data.schematic_defaults, ui.zoom(), en);
        let Some((field, text)) = typed_draft(&ui.labels, &shown) else {
            return true;
        };
        let is_accepted = parse_override(field, &text) != DefaultInput::Keep;
        self.close_general_run(project_id);
        self.apply_project_tab_message(ProjectTabMsg::SchematicDefaultEdited {
            project_id,
            field,
            text,
        });
        self.close_general_run(project_id);
        if is_accepted && let Some(tab) = self.document_view.project_tabs.get_mut(&project_id) {
            tab.default_drafts.remove(&field);
        }
        is_accepted
    }

    /// A drag reached a new value: it joins the drag's undo step.
    fn write_dragged_default(
        &mut self,
        project_id: ProjectId,
        committed: Option<signex_widgets::dimension_label::DimensionCommitted>,
    ) {
        let Some(done) = committed else { return };
        let Some(field) = SchematicDefaultField::ALL.get(done.index).copied() else {
            return;
        };
        self.apply_project_general(
            project_id,
            ProjectGeneralCommand::SetSchematicDefault {
                field,
                value: Some(done.value),
            },
        );
    }

    /// Esc on a refused label takes its text back out of the form box; text
    /// typed in the box itself stays.
    fn drop_label_draft(&mut self, project_id: ProjectId) {
        let labels = &self.ui_state.project_preview.labels;
        let (Some(index), Some(text)) = (labels.editing_index(), labels.draft()) else {
            return;
        };
        let Some(field) = SchematicDefaultField::ALL.get(index).copied() else {
            return;
        };
        if let Some(tab) = self.document_view.project_tabs.get_mut(&project_id)
            && tab.default_drafts.get(&field).map(String::as_str) == Some(text)
        {
            tab.default_drafts.remove(&field);
        }
    }
}
