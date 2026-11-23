//! The Linear and Aligned dimension tools (#1267): click the first point,
//! the second, then the line's place. A linear dimension is horizontal or
//! vertical by the drag direction. Dimensions are free once placed: they
//! never follow the objects they measured.

use signex_types::schematic::{
    DimensionArrows, DimensionDirection, DimensionUnits, Point, SchDimension, SchDrawing,
};

use super::super::super::*;

/// The text settings the next dimension is placed with (the tool
/// properties); Properties edits them while a dimension tool is armed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DimensionToolStyle {
    pub units: DimensionUnits,
    pub precision: u8,
    pub arrows: DimensionArrows,
    /// A direction picked in Properties; `None` follows the tool and drag.
    pub direction: Option<DimensionDirection>,
    /// The next dimension's typed value; it does not carry to the one after.
    pub override_text: Option<String>,
    /// Space put the line on the other side of the measured points.
    pub flipped: bool,
}

impl Default for DimensionToolStyle {
    fn default() -> Self {
        Self {
            units: DimensionUnits::Mm,
            precision: 2,
            arrows: DimensionArrows::FilledInside,
            direction: None,
            override_text: None,
            flipped: false,
        }
    }
}

impl DimensionToolStyle {
    /// The override the dimension is placed with; blank means the measured
    /// value.
    pub(crate) fn placed_override(&self) -> Option<String> {
        self.override_text
            .clone()
            .filter(|text| !text.trim().is_empty())
    }

    /// Drop what belongs to one dimension only.
    pub(crate) fn end_dimension(&mut self) {
        self.override_text = None;
        self.flipped = false;
    }
}

/// The Bezier and dimension tools' in-flight state.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct AnnotationTool {
    /// The points clicked so far.
    pub points: Vec<Point>,
    pub dimension: DimensionToolStyle,
    /// The values and points a Tab pause began with; Esc puts them back.
    pub paused_with: Option<Box<(DimensionToolStyle, Vec<Point>)>>,
    /// The Esc press that resumed from the Override box is still down, so
    /// its repeats are not new presses.
    pub is_resume_esc_held: bool,
}

/// The dimension `points` (first, second) and the third click `at` place.
pub(crate) fn dimension_from_clicks(
    tool: Tool,
    from: Point,
    to: Point,
    at: Point,
    style: DimensionToolStyle,
) -> SchDimension {
    let direction = style.direction.unwrap_or(match tool {
        Tool::AlignedDimension => DimensionDirection::Aligned,
        _ => SchDimension::linear_direction(from, to),
    });
    let mut d = SchDimension::new(uuid::Uuid::now_v7(), from, to, 0.0, direction);
    d.offset = d.offset_on_side(at, style.flipped);
    d.units = style.units;
    d.precision = style
        .precision
        .min(signex_types::schematic::DIMENSION_MAX_PRECISION);
    d.arrows = style.arrows;
    d.override_text = style.placed_override();
    d
}

impl Signex {
    /// Feed one snapped click to the Bezier, dimension or table tool.
    pub(crate) fn annotation_tool_point(&mut self, tool: Tool, p: Point) {
        match tool {
            Tool::Table => self.place_table_click(p),
            Tool::Bezier => self.bezier_tool_point(p),
            Tool::LinearDimension | Tool::AlignedDimension => self.dimension_tool_point(tool, p),
            _ => {}
        }
    }

    fn dimension_tool_point(&mut self, tool: Tool, p: Point) {
        let state = &mut self.interaction_state.annotation_tool;
        state.points.push(p);
        let style = state.dimension.clone();
        let clicked = if state.points.len() >= 3 {
            state.dimension.end_dimension();
            std::mem::take(&mut state.points)
        } else {
            Vec::new()
        };
        if let [from, to, at] = clicked[..] {
            self.drop_dimension_form_draft();
            let d = dimension_from_clicks(tool, from, to, at, style);
            // Two clicks on one spot measure nothing: nothing is placed.
            if d.measured_mm() > 0.0 {
                self.apply_engine_command(
                    signex_engine::Command::PlaceSchDrawing {
                        drawing: SchDrawing::Dimension(d),
                    },
                    false,
                    false,
                );
            }
        }
        self.sync_annotation_draft();
    }

    /// Mirror the annotation tool's points into the canvas preview, with the
    /// hint bar's text for the next click.
    pub(crate) fn sync_annotation_draft(&mut self) {
        use crate::canvas::{AnnotationDraft, AnnotationDraftKind, HintInk};
        use signex_i18n::{ids, t};
        self.document_state.panel_ctx.annotation.dimension_tool = self.dimension_tool_form();
        // Points paused in another window are not this canvas's to preview.
        if self.interaction_state.is_sheet_pause_elsewhere() {
            self.interaction_state.active_canvas_mut().annotation_draft = None;
            return;
        }
        let tool = self.interaction_state.current_tool;
        let kind = match tool {
            Tool::Bezier => AnnotationDraftKind::Bezier,
            Tool::LinearDimension => AnnotationDraftKind::LinearDimension,
            Tool::AlignedDimension => AnnotationDraftKind::AlignedDimension,
            _ => {
                self.interaction_state.active_canvas_mut().annotation_draft = None;
                return;
            }
        };
        let state = &self.interaction_state.annotation_tool;
        let i18n = &self.ui_state.i18n;
        let title = match kind {
            AnnotationDraftKind::Bezier => t!(i18n, ids::PROPERTIES_BEZIER_TITLE),
            AnnotationDraftKind::LinearDimension => t!(i18n, ids::PROPERTIES_DIMENSION_LINEAR),
            AnnotationDraftKind::AlignedDimension => t!(i18n, ids::PROPERTIES_DIMENSION_ALIGNED),
        }
        .into_string();
        let step = state.points.len();
        let hint = match (kind, step) {
            (AnnotationDraftKind::Bezier, 0) => t!(i18n, ids::CANVAS_HINT_BEZIER_START),
            (AnnotationDraftKind::Bezier, 1) => t!(i18n, ids::CANVAS_HINT_BEZIER_C1),
            (AnnotationDraftKind::Bezier, 2) => t!(i18n, ids::CANVAS_HINT_BEZIER_C2),
            (AnnotationDraftKind::Bezier, _) => t!(i18n, ids::CANVAS_HINT_BEZIER_END),
            (_, 0) => t!(i18n, ids::CANVAS_HINT_DIMENSION_FIRST),
            (_, 1) => t!(i18n, ids::CANVAS_HINT_DIMENSION_SECOND),
            (_, _) => t!(i18n, ids::CANVAS_HINT_DIMENSION_LINE),
        }
        .into_string();
        let tokens = self.tokens_for(self.ui_state.preferences_draft_theme);
        let r = crate::styles::tokens::resolve(&tokens);
        let ink = HintInk {
            background: r.raised,
            border: r.line_strong,
            title: crate::styles::ti(tokens.text),
            caption: r.ink4,
            accent: crate::styles::ti(tokens.accent),
            on_accent: r.primary_ink,
        };
        let draft = AnnotationDraft {
            kind,
            points: state.points.clone(),
            title,
            hint,
            units: state.dimension.units,
            precision: state.dimension.precision,
            arrows: state.dimension.arrows,
            direction: state.dimension.direction,
            override_text: state.dimension.placed_override(),
            flipped: state.dimension.flipped,
            ink,
        };
        let canvas = self.interaction_state.active_canvas_mut();
        canvas.annotation_draft = Some(draft);
        canvas.clear_overlay_cache();
    }
}
