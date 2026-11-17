//! The Via panel: Net Information, Definition, Via Stack, Solder Mask
//! Expansion, Via Types & Features and Testpoint.

use iced::Element;
use signex_i18n::{UserText, ids};

use signex_types::pcb_stack::StackMode;

use super::mask::{lock_row, via_mask_section};
use super::rows::{
    Look, Section, fixed_checkbox, fixed_value, labelled, mm_field, net_row, stack, template_rows,
    testpoint_section, value_box,
};
use super::{PrimitiveField as F, ViaFacts, net_info, stack_rows};
use crate::panels::{PanelContext, PanelMsg};

pub(super) fn view<'a>(ctx: &'a PanelContext, f: &'a ViaFacts) -> Element<'a, PanelMsg> {
    let look = Look::of(ctx);
    let common = &f.common;
    stack(vec![
        net_info::section(look, common),
        definition(look, f),
        via_stack(look, f),
        via_mask_section(look, f),
        via_types(look, f),
        testpoint_section(look, common),
    ])
}

fn definition<'a>(look: Look<'a>, f: &'a ViaFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_DEFINITION);
    s.push(common, ids::PCB_PRIM_NET, net_row(look, common));
    let span = UserText::new(f.span.clone().unwrap_or_default());
    s.push(
        common,
        ids::PCB_PRIM_VIA_NAME,
        labelled(look, ids::PCB_PRIM_VIA_NAME, value_box(look, span)),
    );
    stack_rows::span_rows(look, common, &f.layers, &f.span_ends, &mut s);
    s.push(
        common,
        ids::PCB_PRIM_PROPAGATION_DELAY,
        labelled(
            look,
            ids::PCB_PRIM_PROPAGATION_DELAY,
            fixed_value(
                look,
                UserText::new(String::new()),
                ids::PCB_PRIM_DELAY_UNKNOWN,
            ),
        ),
    );
    template_rows(look, common, &mut s);
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
    s.push(common, ids::PCB_PRIM_LOCK, lock_row(look, common, f.locked));
    s.finish()
}

fn via_stack<'a>(look: Look<'a>, f: &'a ViaFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_VIA_STACK);
    s.push(
        common,
        ids::PCB_PRIM_SHOW_PREVIEW,
        fixed_value(
            look,
            look.t(ids::PCB_PRIM_SHOW_PREVIEW),
            ids::PCB_PRIM_PREVIEW_UNSUPPORTED,
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_STACK_SIMPLE,
        stack_rows::mode_tiles(look, common, f.mode),
    );
    if f.mode == Some(StackMode::Simple) || f.layer_sizes.is_empty() {
        s.push(
            common,
            ids::PCB_PRIM_DIAMETER,
            labelled(
                look,
                ids::PCB_PRIM_DIAMETER,
                mm_field(common, F::Diameter, f.diameter, look),
            ),
        );
    } else {
        stack_rows::size_rows(look, common, &f.layer_sizes, false, &mut s);
    }
    s.push(
        common,
        ids::PCB_PRIM_THERMAL_RELIEF,
        labelled(
            look,
            ids::PCB_PRIM_THERMAL_RELIEF,
            fixed_checkbox(
                look,
                ids::PCB_PRIM_DIRECT,
                false,
                ids::PCB_PRIM_RELIEF_UNSUPPORTED,
            ),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_HOLE_SIZE,
        labelled(
            look,
            ids::PCB_PRIM_HOLE_SIZE,
            mm_field(common, F::HoleSize, f.hole, look),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_TOLERANCE,
        labelled(
            look,
            ids::PCB_PRIM_TOLERANCE,
            super::rows::pair(
                fixed_value(
                    look,
                    UserText::new("+".to_string()),
                    ids::PCB_PRIM_TOLERANCE_UNSUPPORTED,
                ),
                fixed_value(
                    look,
                    UserText::new("−".to_string()),
                    ids::PCB_PRIM_TOLERANCE_UNSUPPORTED,
                ),
            ),
        ),
    );
    s.finish()
}

fn via_types<'a>(look: Look<'a>, f: &'a ViaFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_VIA_TYPES);
    s.push(
        common,
        ids::PCB_PRIM_IPC_VIA_TYPE,
        labelled(
            look,
            ids::PCB_PRIM_IPC_VIA_TYPE,
            fixed_value(
                look,
                look.t(ids::PCB_PRIM_NONE),
                ids::PCB_PRIM_VIA_TYPE_UNSUPPORTED,
            ),
        ),
    );
    s.finish()
}
