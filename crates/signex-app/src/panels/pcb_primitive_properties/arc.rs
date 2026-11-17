//! The Arc panel (#2373): Net Information and the net for a copper arc,
//! then the layer, centre, radius, angles, width and length. While the Arc
//! tool is armed it also names how the next arc is drawn.

use iced::Element;
use signex_i18n::ids;

use super::rows::{
    Look, Section, any_layer_row, field, fixed_value, labelled, mm_field, mm_or_na, net_row, pair,
    stack, value_box,
};
use super::{ArcFacts, PrimitiveField as F, format_number, net_info};
use crate::panels::{PanelContext, PanelMsg};

pub(super) fn view<'a>(ctx: &'a PanelContext, f: &'a ArcFacts) -> Element<'a, PanelMsg> {
    let look = Look::of(ctx);
    let common = &f.common;
    let net_section = if f.is_copper {
        net_info::section(look, common)
    } else {
        None
    };
    stack(vec![net_section, properties(look, f)])
}

/// An angle field, in degrees as the pad's Rotation reads.
fn degrees<'a>(
    look: Look<'a>,
    common: &super::PrimitiveCommon,
    id: F,
    value: Option<f64>,
) -> Element<'a, PanelMsg> {
    field(common, id, value.map(format_number), None, look)
}

fn properties<'a>(look: Look<'a>, f: &'a ArcFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_PROPERTIES);
    if let Some(mode) = &f.mode {
        s.push(
            common,
            ids::PCB_PRIM_ARC_MODE,
            labelled(
                look,
                ids::PCB_PRIM_ARC_MODE,
                fixed_value(
                    look,
                    signex_i18n::UserText::new(mode.clone()),
                    ids::PCB_PRIM_ARC_MODE_TIP,
                ),
            ),
        );
    }
    if f.is_copper {
        s.push(common, ids::PCB_PRIM_NET, net_row(look, common));
    }
    s.push(
        common,
        ids::PCB_PRIM_LAYER,
        any_layer_row(look, common, &f.layers, f.layer.as_ref()),
    );
    s.push(
        common,
        ids::PCB_PRIM_WIDTH,
        labelled(
            look,
            ids::PCB_PRIM_WIDTH,
            mm_field(common, F::Width, f.width, look),
        ),
    );
    // The shape of an arc not placed yet is its clicks'.
    if !common.target.is_placement() {
        s.push(
            common,
            ids::PCB_PRIM_CENTER,
            labelled(
                look,
                ids::PCB_PRIM_CENTER,
                pair(
                    mm_field(common, F::X, f.center.0, look),
                    mm_field(common, F::Y, f.center.1, look),
                ),
            ),
        );
        s.push(
            common,
            ids::PCB_PRIM_RADIUS,
            labelled(
                look,
                ids::PCB_PRIM_RADIUS,
                mm_field(common, F::Radius, f.radius, look),
            ),
        );
        s.push(
            common,
            ids::PCB_PRIM_START_ANGLE,
            labelled(
                look,
                ids::PCB_PRIM_START_ANGLE,
                degrees(look, common, F::StartAngle, f.start_angle),
            ),
        );
        s.push(
            common,
            ids::PCB_PRIM_END_ANGLE,
            labelled(
                look,
                ids::PCB_PRIM_END_ANGLE,
                degrees(look, common, F::EndAngle, f.end_angle),
            ),
        );
    }
    s.push(
        common,
        ids::PCB_PRIM_LENGTH,
        labelled(
            look,
            ids::PCB_PRIM_LENGTH,
            value_box(look, mm_or_na(look, f.length_mm)),
        ),
    );
    s.finish()
}
