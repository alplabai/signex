//! The values the Pad, Arc, String and Dimension tools place with (#2373,
//! #2393), set in the Properties form while the tool is armed. A value is
//! checked as the edit of a placed object is, then kept as the tool's for
//! the session. The location and an arc's shape come from the clicks, so
//! their fields set nothing here.

use signex_engine::pcb::BoardCommandError;
use signex_engine::pcb::primitive_edit::{
    DIMENSION_MAX_PRECISION, DimensionStyleEdit, apply_style,
};
use signex_i18n::{MessageId, ids, t};

use crate::app::Signex;
use crate::app::handlers::pcb_primitive::{PrimitiveEdit, dimension_style_edit};
use crate::app::pcb_tools::BoardPlaceTool;
use crate::panels::pcb_primitive_properties::PrimitiveField as F;

/// The number of decimals `value` asks for, when it is a whole number the
/// dimension can show.
pub(crate) fn precision_of(value: f64) -> Option<u8> {
    let whole = value.fract() == 0.0 && (0.0..=f64::from(DIMENSION_MAX_PRECISION)).contains(&value);
    whole.then_some(value as u8)
}

impl Signex {
    /// An edit in the form of the free pad, arc, string or dimension `tool`
    /// places next.
    pub(super) fn edit_free_tool_value(
        &mut self,
        tool: BoardPlaceTool,
        edit: &PrimitiveEdit,
        what: MessageId,
    ) {
        if tool.dimension_kind().is_some()
            && let Some(style_edit) = dimension_style_edit(edit)
        {
            self.edit_dimension_tool_style(&style_edit, what);
            return;
        }
        if let PrimitiveEdit::Number(field, value) = edit
            && matches!(
                field,
                F::SizeX | F::SizeY | F::HoleSize | F::Width | F::TextHeight | F::LineWidth
            )
            && *value <= 0.0
        {
            let why = t!(self.ui_state.i18n, ids::PCB_PRIM_NOT_POSITIVE).into_string();
            self.refuse_primitive_edit(what, why);
            return;
        }
        if let Err(why) = self.check_free_tool_value(tool, edit) {
            self.refuse_primitive_edit(what, why);
            return;
        }
        let defaults = &mut self.interaction_state.pcb_tool_defaults;
        match (tool, edit) {
            (BoardPlaceTool::Pad, PrimitiveEdit::Designator(text)) => {
                defaults.pad_designator = Some(text.trim().to_string());
            }
            (BoardPlaceTool::Pad, PrimitiveEdit::Number(F::SizeX, v)) => {
                defaults.pad_size_x_mm = Some(*v);
            }
            (BoardPlaceTool::Pad, PrimitiveEdit::Number(F::SizeY, v)) => {
                defaults.pad_size_y_mm = Some(*v);
            }
            (BoardPlaceTool::Pad, PrimitiveEdit::Number(F::HoleSize, v)) => {
                defaults.pad_hole_mm = Some(*v);
            }
            (BoardPlaceTool::Pad, PrimitiveEdit::Number(F::Rotation, v)) => {
                defaults.pad_rotation = Some(v.rem_euclid(360.0));
            }
            (BoardPlaceTool::Pad, PrimitiveEdit::Shape(shape)) => defaults.pad_shape = Some(*shape),
            (BoardPlaceTool::Pad, PrimitiveEdit::Net(net)) => defaults.pad_net = Some(*net),
            (BoardPlaceTool::Arc, PrimitiveEdit::Number(F::Width, v)) => {
                defaults.arc_width_mm = Some(*v);
            }
            (BoardPlaceTool::Arc, PrimitiveEdit::Net(net)) => defaults.arc_net = Some(*net),
            (BoardPlaceTool::TextString, PrimitiveEdit::Text(text)) => {
                defaults.text = Some(text.clone());
            }
            (BoardPlaceTool::TextString, PrimitiveEdit::Number(F::TextHeight, v)) => {
                defaults.text_height_mm = Some(*v);
            }
            (BoardPlaceTool::TextString, PrimitiveEdit::Number(F::Rotation, v)) => {
                defaults.text_rotation = Some(v.rem_euclid(360.0));
            }
            (BoardPlaceTool::Dimension(_), PrimitiveEdit::Number(F::LineWidth, v)) => {
                defaults.dimension_line_width_mm = Some(*v);
            }
            (BoardPlaceTool::Dimension(_), PrimitiveEdit::Number(F::TextHeight, v)) => {
                defaults.dimension_text_height_mm = Some(*v);
            }
            (BoardPlaceTool::Dimension(_), PrimitiveEdit::Number(F::Precision, v)) => {
                defaults.dimension_precision = precision_of(*v);
            }
            // The location and an arc's shape are the clicks'.
            _ => return,
        }
        self.refresh_pcb_highlight();
    }

    /// A look edit in the Dimension tool's form (#2868), checked on the
    /// dimension it places next as on a placed one, then kept for the
    /// session.
    fn edit_dimension_tool_style(&mut self, edit: &DimensionStyleEdit, what: MessageId) {
        let Some(mut next) = self.next_board_dimension() else {
            return;
        };
        if let Err(refusal) = apply_style(&mut next, edit) {
            self.refuse_primitive_edit(what, refusal.to_string());
            return;
        }
        let defaults = &mut self.interaction_state.pcb_tool_defaults;
        defaults.dimension_style = Some(next.style);
        defaults.dimension_text_at = Some(next.text_at);
        self.refresh_pcb_highlight();
    }

    /// Why the next object of `tool` cannot take `edit`, if it cannot: a
    /// hole wider than the pad, a precision that is not a whole number of
    /// decimals the dimension can show.
    fn check_free_tool_value(
        &self,
        tool: BoardPlaceTool,
        edit: &PrimitiveEdit,
    ) -> Result<(), String> {
        let i18n = &self.ui_state.i18n;
        match (tool, edit) {
            (BoardPlaceTool::Dimension(_), PrimitiveEdit::Number(F::Precision, v))
                if precision_of(*v).is_none() =>
            {
                Err(t!(i18n, ids::PCB_PRIM_NOT_A_PRECISION).into_string())
            }
            (
                BoardPlaceTool::Pad,
                PrimitiveEdit::Number(field @ (F::SizeX | F::SizeY | F::HoleSize), v),
            ) => {
                let Some(board) = self.active_pcb() else {
                    return Ok(());
                };
                let pad = self.next_board_pad(board);
                let (mut x, mut y) = (pad.size.x, pad.size.y);
                let mut hole = pad.drill.as_ref().map_or(0.0, |d| d.diameter);
                match field {
                    F::SizeX => x = *v,
                    F::SizeY => y = *v,
                    _ => hole = *v,
                }
                if hole > x.min(y) {
                    let refusal = BoardCommandError::HoleTooLarge {
                        hole,
                        diameter: x.min(y),
                    };
                    return Err(refusal.to_string());
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}
