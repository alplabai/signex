//! The Arc panel (#2373, #2523, mockup row 98): the illustration, Net Information
//! (closed, copper arcs only), then Properties: while the Arc tool is armed
//! how the next arc is drawn, the net of a copper arc, layer, width, ends,
//! and for a placed arc its centre, radius, angles and length.

use iced::Element;
use signex_engine::pcb::draw::ArcMode;
use signex_i18n::{MessageId, ids};

use super::drawing::{self, keys};
use super::linked::{self, Segment, linked_row, prefixed, segments, two_fields, unit};
use super::rows::{Look, Section, any_layer_value, labelled, net_value, stack, with_tip};
use super::{ArcFacts, PcbPrimitiveMsg, PrimitiveField as F, net_info};
use crate::panels::{PanelContext, PanelMsg};

/// The short name of `mode` on the Placement control.
fn arc_mode_name(mode: ArcMode) -> MessageId {
    match mode {
        ArcMode::Centre => ids::PCB_ARC_MODE_CENTRE_NAME,
        ArcMode::Edge => ids::PCB_ARC_MODE_EDGE_NAME,
        ArcMode::AnyAngle => ids::PCB_ARC_MODE_ANY_NAME,
    }
}

pub(super) fn view<'a>(ctx: &'a PanelContext, f: &'a ArcFacts) -> Element<'a, PanelMsg> {
    let look = Look::of(ctx);
    let common = &f.common;
    let net_section = if f.is_copper && !common.target.is_placement() {
        net_info::section(look, common)
    } else {
        None
    };
    let drawn = drawing::arc::illustration(&ctx.i18n, f);
    stack(vec![
        Some(linked::illustration(look, common, drawn)),
        net_section,
        properties(look, f),
    ])
}

/// Centre, Edge or Any angle: how the armed Arc tool draws its next arc,
/// as Cycle Arc Mode steps it.
fn placement<'a>(look: Look<'a>, f: &'a ArcFacts, mode: ArcMode) -> Element<'a, PanelMsg> {
    let live = f.common.is_editable();
    let cells = ArcMode::ALL
        .into_iter()
        .map(|m| Segment {
            label: look.t(arc_mode_name(m)),
            is_on: m == mode,
            msg: live.then_some(PcbPrimitiveMsg::ArcModePicked(m)),
        })
        .collect();
    with_tip(look, segments(cells), ids::PCB_PRIM_ARC_MODE_TIP)
}

fn properties<'a>(look: Look<'a>, f: &'a ArcFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_PROPERTIES).collapsible(
        "pcb_prim.arc.properties",
        false,
        None,
    );
    if let Some(mode) = f.mode {
        s.push(
            common,
            ids::PCB_PRIM_ARC_MODE,
            labelled(look, ids::PCB_PRIM_ARC_MODE, placement(look, f, mode)),
        );
    }
    if f.is_copper {
        s.push(
            common,
            ids::PCB_PRIM_NET,
            linked_row(
                look,
                common,
                keys::NET,
                ids::PCB_PRIM_NET,
                net_value(look, common),
            ),
        );
    }
    s.push(
        common,
        ids::PCB_PRIM_LAYER,
        linked_row(
            look,
            common,
            keys::LAYER,
            ids::PCB_PRIM_LAYER,
            any_layer_value(look, common, &f.layers, f.layer.as_ref()),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_WIDTH,
        linked_row(
            look,
            common,
            keys::WIDTH,
            ids::PCB_PRIM_WIDTH,
            linked::mm(look, common, F::Width, f.width),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_ENDS,
        linked_row(
            look,
            common,
            keys::ENDS,
            ids::PCB_PRIM_ENDS,
            linked::ends(look, f.is_copper),
        ),
    );
    // The shape of an arc not placed yet is its clicks'.
    if common.target.is_placement() {
        return s.finish();
    }
    s.push(
        common,
        ids::PCB_PRIM_CENTRE,
        linked_row(
            look,
            common,
            keys::CENTRE,
            ids::PCB_PRIM_CENTRE,
            two_fields(
                prefixed(look, common, F::X, f.center.0, ids::PCB_PRIM_X),
                prefixed(look, common, F::Y, f.center.1, ids::PCB_PRIM_Y),
                unit(look, ids::PCB_PRIM_UNIT_MM),
            ),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_RADIUS,
        linked_row(
            look,
            common,
            keys::RADIUS,
            ids::PCB_PRIM_RADIUS,
            linked::mm(look, common, F::Radius, f.radius),
        ),
    );
    // Angles are read from the board origin with Y up, as the pad's
    // Rotation reads.
    s.push(
        common,
        ids::PCB_PRIM_ANGLES,
        linked_row(
            look,
            common,
            keys::ANGLES,
            ids::PCB_PRIM_ANGLES,
            two_fields(
                prefixed(
                    look,
                    common,
                    F::StartAngle,
                    f.start_angle,
                    ids::PCB_PRIM_POINT_START,
                ),
                prefixed(
                    look,
                    common,
                    F::EndAngle,
                    f.end_angle,
                    ids::PCB_PRIM_POINT_END,
                ),
                unit(look, ids::PCB_PRIM_UNIT_DEGREES),
            ),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_LENGTH,
        linked_row(
            look,
            common,
            keys::LENGTH,
            ids::PCB_PRIM_LENGTH,
            linked::length(look, f.length_mm),
        ),
    );
    s.finish()
}
