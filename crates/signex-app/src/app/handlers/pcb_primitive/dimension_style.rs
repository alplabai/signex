//! The look of the selected dimensions in Properties (#2868): the facts the
//! panel shows and the edits its rows send, each one undo step through
//! `signex_engine::pcb::primitive_edit::DimensionStyleEdit`.

use signex_engine::pcb::primitive_edit::DimensionStyleEdit as E;
use signex_i18n::{MessageId, ids};
use signex_types::pcb_dimension::LinearDimension;

use super::PrimitiveEdit;
use crate::panels::pcb_primitive_properties::{DimensionStyleFacts, PrimitiveField as F, shared};

/// What `dimensions` share of their look.
pub(super) fn style_facts(dimensions: &[&LinearDimension]) -> DimensionStyleFacts {
    let each = |f: &dyn Fn(&LinearDimension) -> f64| shared(dimensions.iter().map(|d| f(*d)));
    let pick = |f: &dyn Fn(&LinearDimension) -> bool| shared(dimensions.iter().map(|d| f(*d)));
    let words = |f: &dyn Fn(&LinearDimension) -> String| shared(dimensions.iter().map(|d| f(*d)));
    DimensionStyleFacts {
        arrow_style: shared(dimensions.iter().map(|d| d.style.arrow_style)),
        arrow_length: each(&|d| d.style.arrow_length),
        arrow_width: each(&|d| d.style.arrow_width),
        font: shared(dimensions.iter().map(|d| d.style.font)),
        bold: pick(&|d| d.style.bold),
        italic: pick(&|d| d.style.italic),
        text_width: each(&|d| d.style.text_width),
        text_position: shared(dimensions.iter().map(|d| d.style.text_position)),
        text_gap: each(&|d| d.style.text_gap),
        text_at: (each(&|d| d.text_at.x), each(&|d| d.text_at.y)),
        extension_offset: each(&|d| d.style.extension_offset),
        extension_overshoot: each(&|d| d.style.extension_overshoot),
        extension_width: each(&|d| d.style.extension_width),
        unit: shared(dimensions.iter().map(|d| d.style.unit)),
        show_unit: pick(&|d| d.style.show_unit),
        prefix: words(&|d| d.style.prefix.clone()),
        suffix: words(&|d| d.style.suffix.clone()),
        trailing_zeros: pick(&|d| d.style.trailing_zeros),
    }
}

/// Text typed in the Prefix or Suffix field.
pub(super) fn words_edit(field: F, text: &str) -> Option<E> {
    match field {
        F::Prefix => Some(E::Prefix(text.to_string())),
        F::Suffix => Some(E::Suffix(text.to_string())),
        _ => None,
    }
}

/// The look field `edit` sets, if it sets one: a picked choice, or a number
/// typed in a look row.
pub(crate) fn dimension_style_edit(edit: &PrimitiveEdit) -> Option<E> {
    Some(match edit {
        PrimitiveEdit::DimensionStyle(e) => e.clone(),
        PrimitiveEdit::Number(F::ArrowLength, v) => E::ArrowLength(*v),
        PrimitiveEdit::Number(F::ArrowWidth, v) => E::ArrowWidth(*v),
        PrimitiveEdit::Number(F::TextWidth, v) => E::TextWidth(*v),
        PrimitiveEdit::Number(F::TextGap, v) => E::TextGap(*v),
        PrimitiveEdit::Number(F::TextAlong, v) => E::TextAlong(*v),
        PrimitiveEdit::Number(F::TextAcross, v) => E::TextAcross(*v),
        PrimitiveEdit::Number(F::ExtensionOffset, v) => E::ExtensionOffset(*v),
        PrimitiveEdit::Number(F::ExtensionOvershoot, v) => E::ExtensionOvershoot(*v),
        PrimitiveEdit::Number(F::ExtensionWidth, v) => E::ExtensionWidth(*v),
        _ => return None,
    })
}

/// The catalog name of the row `edit` came from, for undo labels and
/// notices.
pub(super) fn edit_name(edit: &E) -> MessageId {
    match edit {
        E::ArrowStyle(_) => ids::PCB_PRIM_DIM_ARROW_STYLE,
        E::ArrowLength(_) => ids::PCB_PRIM_DIM_ARROW_LENGTH,
        E::ArrowWidth(_) => ids::PCB_PRIM_DIM_ARROW_WIDTH,
        E::Font(_) => ids::PCB_PRIM_DIM_FONT,
        E::Bold(_) => ids::PCB_PRIM_DIM_BOLD,
        E::Italic(_) => ids::PCB_PRIM_DIM_ITALIC,
        E::TextWidth(_) => ids::PCB_PRIM_DIM_TEXT_WIDTH,
        E::TextPosition(_) => ids::PCB_PRIM_DIM_TEXT_POSITION,
        E::TextGap(_) => ids::PCB_PRIM_DIM_TEXT_GAP,
        E::TextAlong(_) | E::TextAcross(_) => ids::PCB_PRIM_DIM_TEXT_AT,
        E::ExtensionOffset(_) => ids::PCB_PRIM_DIM_EXT_OFFSET,
        E::ExtensionOvershoot(_) => ids::PCB_PRIM_DIM_EXT_OVERSHOOT,
        E::ExtensionWidth(_) => ids::PCB_PRIM_DIM_EXT_WIDTH,
        E::Unit(_) => ids::PCB_PRIM_DIM_UNIT,
        E::ShowUnit(_) => ids::PCB_PRIM_DIM_SHOW_UNIT,
        E::Prefix(_) => ids::PCB_PRIM_DIM_PREFIX,
        E::Suffix(_) => ids::PCB_PRIM_DIM_SUFFIX,
        E::TrailingZeros(_) => ids::PCB_PRIM_DIM_TRAILING_ZEROS,
    }
}
