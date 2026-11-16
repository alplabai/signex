//! The String panel (#2373): the text, its location, height, turn and
//! layer.

use iced::Element;
use signex_i18n::ids;

use super::rows::{Look, Section, any_layer_row, field, labelled, mm_field, stack, text_field};
use super::{PrimitiveField as F, TextFacts, format_number};
use crate::panels::{PanelContext, PanelMsg};

pub(super) fn view<'a>(ctx: &'a PanelContext, f: &'a TextFacts) -> Element<'a, PanelMsg> {
    let look = Look::of(ctx);
    stack(vec![properties(look, f)])
}

fn properties<'a>(look: Look<'a>, f: &'a TextFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_PROPERTIES);
    s.push(
        common,
        ids::PCB_PRIM_TEXT,
        labelled(
            look,
            ids::PCB_PRIM_TEXT,
            text_field(common, F::Text, f.text.as_deref(), look),
        ),
    );
    // Where a string not placed yet goes is the pointer's.
    if !common.target.is_placement() {
        s.push(
            common,
            ids::PCB_PRIM_X,
            labelled(look, ids::PCB_PRIM_X, mm_field(common, F::X, f.x, look)),
        );
        s.push(
            common,
            ids::PCB_PRIM_Y,
            labelled(look, ids::PCB_PRIM_Y, mm_field(common, F::Y, f.y, look)),
        );
    }
    s.push(
        common,
        ids::PCB_PRIM_TEXT_HEIGHT,
        labelled(
            look,
            ids::PCB_PRIM_TEXT_HEIGHT,
            mm_field(common, F::TextHeight, f.height, look),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_ROTATION,
        labelled(
            look,
            ids::PCB_PRIM_ROTATION,
            field(
                common,
                F::Rotation,
                f.rotation.map(format_number),
                None,
                look,
            ),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_LAYER,
        any_layer_row(look, common, &f.layers, f.layer.as_ref()),
    );
    s.finish()
}
