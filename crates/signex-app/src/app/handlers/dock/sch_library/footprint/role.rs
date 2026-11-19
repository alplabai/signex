//! The role sub-form edits that have no flat `FpEditor*` message (#126):
//! the keepout layer and the sketch pad's paste and mask expansions. Each
//! edit goes through the editor's undo history, re-solves and re-bakes the
//! sketch, and marks the tab dirty, like the keepout kinds in `shape.rs`.

use super::super::*;

use super::shape::{rebuild_sketch, sketch_entity_mut};
use crate::panels::{FpSubformMsg, expansion_buffer_key};
use signex_sketch::id::SketchEntityId;
use signex_sketch::parameter::ParameterTable;
use signex_sketch::unit::UnitFamily;
use signex_types::pcb_layer::{BoardSide, PcbLayer};

/// Which expansion of a sketch pad a field edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expansion {
    Paste,
    Mask,
}

impl Signex {
    pub(in crate::app::handlers::dock::sch_library) fn fp_subform_edited(
        &mut self,
        msg: &FpSubformMsg,
    ) -> bool {
        match msg {
            FpSubformMsg::KeepoutLayerPicked { id, layer } => {
                self.fp_keepout_layer_picked(*id, *layer)
            }
            FpSubformMsg::SketchPadPasteExpansionEdited { id, side, value } => {
                self.fp_sketch_pad_expansion_edited(*id, Expansion::Paste, *side, value)
            }
            FpSubformMsg::SketchPadMaskExpansionEdited { id, side, value } => {
                self.fp_sketch_pad_expansion_edited(*id, Expansion::Mask, *side, value)
            }
        }
    }

    fn fp_keepout_layer_picked(&mut self, id: SketchEntityId, layer: PcbLayer) -> bool {
        if let Some(editor) = self.active_footprint_editor_mut() {
            editor.record_edit(None, |editor| {
                let Some(k) = sketch_entity_mut(editor, id).and_then(|e| e.keepout.as_mut()) else {
                    return false;
                };
                if k.layer == layer {
                    return false;
                }
                k.layer = layer;
                rebuild_sketch(editor);
                true
            });
        }
        self.refresh_panel_ctx();
        true
    }

    /// Writes the expansion expression of `value`. Text that is not a
    /// length yet is kept in the editor's field buffers, where the field
    /// shows it refused, and leaves the pad as it is.
    fn fp_sketch_pad_expansion_edited(
        &mut self,
        id: SketchEntityId,
        which: Expansion,
        side: BoardSide,
        value: &str,
    ) -> bool {
        let buffer_key = expansion_buffer_key(id, which == Expansion::Mask, side);
        let Some(editor) = self.active_footprint_editor_mut() else {
            return true;
        };
        let parameters = editor
            .primitive()
            .sketch
            .as_ref()
            .map(|sketch| sketch.parameters.clone())
            .unwrap_or_default();
        let Some(expr) = expansion_expr(value, &parameters) else {
            editor
                .state
                .numeric_buffers
                .insert(buffer_key, value.to_owned());
            self.refresh_panel_ctx();
            return true;
        };
        editor.state.numeric_buffers.remove(&buffer_key);
        let field = match (which, side) {
            (Expansion::Paste, BoardSide::Top) => "sketch_pad_paste_top",
            (Expansion::Paste, BoardSide::Bottom) => "sketch_pad_paste_bottom",
            (Expansion::Mask, BoardSide::Top) => "sketch_pad_mask_top",
            (Expansion::Mask, BoardSide::Bottom) => "sketch_pad_mask_bottom",
        };
        editor.record_edit(Some((field, id.0.as_u128())), |editor| {
            let Some(pad) = sketch_entity_mut(editor, id).and_then(|e| e.pad.as_mut()) else {
                return false;
            };
            let stack = &mut pad.stack;
            let (slot, pct) = match (which, side) {
                (Expansion::Paste, BoardSide::Top) => {
                    (&mut stack.paste_top_expr, Some(&mut stack.paste_top_pct))
                }
                (Expansion::Paste, BoardSide::Bottom) => (
                    &mut stack.paste_bottom_expr,
                    Some(&mut stack.paste_bottom_pct),
                ),
                (Expansion::Mask, BoardSide::Top) => (&mut stack.mask_top_expr, None),
                (Expansion::Mask, BoardSide::Bottom) => (&mut stack.mask_bottom_expr, None),
            };
            if *slot == expr {
                return false;
            }
            // A paste expression and a paste percentage exclude each other.
            if let (Some(pct), Some(_)) = (pct, expr.as_ref()) {
                *pct = None;
            }
            *slot = expr;
            rebuild_sketch(editor);
            true
        });
        self.refresh_panel_ctx();
        true
    }
}

/// The stored expression of an expansion field's text: `Some(None)` for
/// empty text (the design rule applies), `Some(Some(expr))` for an
/// expression that evaluates to a length against the sketch `parameters`
/// (a bare number is millimetres), `None` for anything else: text that does
/// not parse yet, an unknown parameter, or a value without a length unit.
fn expansion_expr(value: &str, parameters: &ParameterTable) -> Option<Option<String>> {
    let text = value.trim();
    if text.is_empty() {
        return Some(None);
    }
    if let Ok(mm) = text.parse::<f64>() {
        return mm.is_finite().then(|| Some(format!("{mm}mm")));
    }
    let q = signex_sketch::parameter::eval_with_parameters(text, parameters).ok()?;
    let is_length = q.unit.family() == UnitFamily::Length && q.value.is_finite();
    is_length.then(|| Some(text.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::{ParameterTable, expansion_expr};

    fn table() -> ParameterTable {
        ParameterTable(
            [("mask_margin".to_owned(), "0.05mm".to_owned())]
                .into_iter()
                .collect(),
        )
    }

    #[test]
    fn empty_text_follows_the_rule() {
        assert_eq!(expansion_expr("  ", &table()), Some(None));
    }

    #[test]
    fn a_bare_number_is_stored_in_millimetres() {
        assert_eq!(
            expansion_expr("0.05", &table()),
            Some(Some("0.05mm".into()))
        );
        assert_eq!(
            expansion_expr("-0.040", &table()),
            Some(Some("-0.04mm".into()))
        );
        assert_eq!(expansion_expr("0", &table()), Some(Some("0mm".into())));
    }

    #[test]
    fn a_length_expression_is_stored_as_written() {
        assert_eq!(
            expansion_expr("mask_margin * 2", &table()),
            Some(Some("mask_margin * 2".into()))
        );
        assert_eq!(
            expansion_expr("0.1mm", &table()),
            Some(Some("0.1mm".into()))
        );
    }

    #[test]
    fn half_typed_text_is_not_stored() {
        assert_eq!(expansion_expr("-", &table()), None);
        assert_eq!(expansion_expr("0.1 +", &table()), None);
        assert_eq!(expansion_expr("NaN", &table()), None);
    }

    #[test]
    fn text_that_is_not_a_length_is_not_stored() {
        assert_eq!(expansion_expr("2*3", &table()), None, "no length unit");
        assert_eq!(expansion_expr("m", &table()), None, "half-typed name");
        assert_eq!(expansion_expr("pitch * 2", &table()), None, "unknown name");
    }
}
