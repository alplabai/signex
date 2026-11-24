//! Preferences ▸ PCB Defaults (#2868): the draft of what each new board
//! dimension starts from. Each typed number or pick is checked as the same
//! edit of a placed dimension is (`pcb_primitive::dimension_edit`,
//! `apply_dimension`); Save commits the draft.

use signex_engine::pcb::primitive_edit::{
    DimensionStyleEdit, PrimitiveEditRefusal, apply_dimension,
};
use signex_i18n::{I18n, ids, t};
use signex_types::pcb::Point;
use signex_types::pcb_dimension::LinearDimension;
use signex_types::pcb_dimension_style::DimensionDefaults;
use uuid::Uuid;

use super::super::pcb_primitive::{PrimitiveEdit, dimension_edit};
use crate::app::Signex;
use crate::panels::pcb_primitive_properties::{PrimitiveField, parse_number};
use crate::pcb_defaults_prefs::PcbDefaultsDraft;
use crate::preferences::PcbDefaultsMsg;

/// `defaults` with `edit` applied, checked on a stand-in dimension as on a
/// placed one; the line width and text height stay above zero.
pub(super) fn applied(
    defaults: &DimensionDefaults,
    edit: &PrimitiveEdit,
) -> Result<DimensionDefaults, PrimitiveEditRefusal> {
    let Some(edit) = dimension_edit(edit) else {
        return Ok(defaults.clone());
    };
    let (start, end) = (Point::new(0.0, 0.0), Point::new(10.0, 0.0));
    let mut d = LinearDimension::new(Uuid::nil(), start, end, 1.0).with_defaults(defaults);
    apply_dimension(&mut d, edit?)?;
    if !(d.line_width > 0.0 && d.text_height > 0.0) {
        return Err(PrimitiveEditRefusal::NotPositive);
    }
    Ok(DimensionDefaults {
        line_width: d.line_width,
        text_height: d.text_height,
        precision: d.precision,
        style: d.style,
    })
}

/// The edit text typed in `field` asks for, or why it asks for none.
fn typed_edit(field: PrimitiveField, text: &str, i18n: &I18n) -> Result<PrimitiveEdit, String> {
    match field {
        PrimitiveField::Prefix => Ok(PrimitiveEdit::DimensionStyle(DimensionStyleEdit::Prefix(
            text.to_string(),
        ))),
        PrimitiveField::Suffix => Ok(PrimitiveEdit::DimensionStyle(DimensionStyleEdit::Suffix(
            text.to_string(),
        ))),
        _ => parse_number(text)
            .map(|v| PrimitiveEdit::Number(field, v))
            .ok_or_else(|| t!(i18n, ids::PCB_PRIM_NOT_A_NUMBER, "text" => text).into_string()),
    }
}

/// `msg` applied to `draft`: a typed number the defaults take sets them,
/// else the field keeps its text and says why; a pick sets its value.
pub(super) fn apply_message(draft: &mut PcbDefaultsDraft, msg: PcbDefaultsMsg, i18n: &I18n) {
    let (field, edit) = match msg {
        PcbDefaultsMsg::Typed(field, text) => {
            let edit = typed_edit(field, &text, i18n);
            draft.texts.insert(field, text);
            (Some(field), edit)
        }
        PcbDefaultsMsg::Picked(edit) => (None, Ok(PrimitiveEdit::DimensionStyle(edit))),
        PcbDefaultsMsg::PrecisionPicked(decimals) => (
            None,
            Ok(PrimitiveEdit::Number(
                PrimitiveField::Precision,
                f64::from(decimals),
            )),
        ),
    };
    let outcome =
        edit.and_then(|edit| applied(&draft.defaults, &edit).map_err(|why| why.to_string()));
    match (outcome, field) {
        (Ok(defaults), field) => {
            draft.defaults = defaults;
            if let Some(field) = field {
                draft.refused.remove(&field);
            }
        }
        (Err(why), Some(field)) => {
            draft.refused.insert(field, why);
        }
        (Err(_), None) => {}
    }
}

impl Signex {
    pub(super) fn handle_pcb_defaults_message(&mut self, msg: PcbDefaultsMsg) {
        let i18n = self.ui_state.i18n.clone();
        apply_message(&mut self.ui_state.preferences_pcb_defaults, msg, &i18n);
        self.recompute_preferences_dirty();
    }
}
