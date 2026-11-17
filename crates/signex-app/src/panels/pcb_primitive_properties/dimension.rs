//! The Dimension panel (#2373): what it measures, where its line stands,
//! its line width, text height, decimals and layer, and the value it
//! reads.

use iced::Element;
use signex_i18n::{UserText, ids};

use super::rows::{
    Look, Section, any_layer_row, field, labelled, mm_field, pair, stack, value_box,
};
use super::{DimensionFacts, PrimitiveField as F};
use crate::panels::{PanelContext, PanelMsg};

pub(super) fn view<'a>(ctx: &'a PanelContext, f: &'a DimensionFacts) -> Element<'a, PanelMsg> {
    let look = Look::of(ctx);
    stack(vec![properties(look, f)])
}

fn properties<'a>(look: Look<'a>, f: &'a DimensionFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_PROPERTIES);
    // What a dimension not placed yet measures is its clicks'.
    if !common.target.is_placement() {
        s.push(
            common,
            ids::PCB_PRIM_START,
            labelled(
                look,
                ids::PCB_PRIM_START,
                pair(
                    mm_field(common, F::StartX, f.start.0, look),
                    mm_field(common, F::StartY, f.start.1, look),
                ),
            ),
        );
        s.push(
            common,
            ids::PCB_PRIM_END,
            labelled(
                look,
                ids::PCB_PRIM_END,
                pair(
                    mm_field(common, F::EndX, f.end.0, look),
                    mm_field(common, F::EndY, f.end.1, look),
                ),
            ),
        );
        s.push(
            common,
            ids::PCB_PRIM_OFFSET,
            labelled(
                look,
                ids::PCB_PRIM_OFFSET,
                mm_field(common, F::Offset, f.offset, look),
            ),
        );
    }
    s.push(
        common,
        ids::PCB_PRIM_LINE_WIDTH,
        labelled(
            look,
            ids::PCB_PRIM_LINE_WIDTH,
            mm_field(common, F::LineWidth, f.line_width, look),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_TEXT_HEIGHT,
        labelled(
            look,
            ids::PCB_PRIM_TEXT_HEIGHT,
            mm_field(common, F::TextHeight, f.text_height, look),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_PRECISION,
        labelled(
            look,
            ids::PCB_PRIM_PRECISION,
            field(
                common,
                F::Precision,
                f.precision.map(|p| p.to_string()),
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
    if let Some(value) = &f.value {
        s.push(
            common,
            ids::PCB_PRIM_VALUE,
            labelled(
                look,
                ids::PCB_PRIM_VALUE,
                value_box(look, UserText::new(value.clone())),
            ),
        );
    }
    s.finish()
}
