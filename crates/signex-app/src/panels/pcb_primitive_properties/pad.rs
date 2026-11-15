//! The Pad panel: Net Information, Properties, Pad Stack and Testpoint.

use iced::widget::{column, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{UserText, ids};
use signex_widgets::text;

use super::rows::{
    Look, Section, chip, dropdown, field, fixed_checkbox, fixed_tiles, fixed_value, labelled, lock,
    mm_field, net_row, pair, stack, template_rows, testpoint_section, value_box, with_tip,
};
use super::{PadFacts, PcbPrimitiveMsg, PrimitiveField as F, SP, TS, format_number, net_info};
use crate::panels::{PanelContext, PanelMsg};
use crate::pcb_canvas::layer_style;

pub(super) fn view<'a>(ctx: &'a PanelContext, f: &'a PadFacts) -> Element<'a, PanelMsg> {
    let look = Look::of(ctx);
    let common = &f.common;
    stack(vec![
        net_info::section(look, common),
        properties(look, f),
        pad_stack(look, f),
        testpoint_section(look, common),
    ])
}

fn properties<'a>(look: Look<'a>, f: &'a PadFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_PROPERTIES);
    s.push(
        common,
        ids::PCB_PRIM_DESIGNATOR,
        labelled(
            look,
            ids::PCB_PRIM_DESIGNATOR,
            field(common, F::Designator, f.designator.clone(), None, look),
        ),
    );
    let layer: Element<'a, PanelMsg> = match f.layer.as_ref() {
        Some((name, colour_of)) => {
            let shown = value_box(look, UserText::new(name.clone()));
            let content = match colour_of {
                Some(layer) => row![
                    chip(layer_style::layer_colour(*layer, look.tokens())),
                    shown
                ]
                .spacing(SP.s)
                .align_y(iced::Alignment::Center)
                .boxed(),
                None => shown,
            };
            with_tip(look, content, ids::PCB_PRIM_PAD_LAYER_UNSUPPORTED)
        }
        None => value_box(look, UserText::new(String::new())),
    };
    s.push(
        common,
        ids::PCB_PRIM_LAYER,
        labelled(look, ids::PCB_PRIM_LAYER, layer),
    );
    s.push(common, ids::PCB_PRIM_NET, net_row(look, common));
    s.push(
        common,
        ids::PCB_PRIM_ELECTRICAL_TYPE,
        labelled(
            look,
            ids::PCB_PRIM_ELECTRICAL_TYPE,
            fixed_tiles(
                look,
                &[
                    ids::PCB_PRIM_ELECTRICAL_LOAD,
                    ids::PCB_PRIM_ELECTRICAL_SOURCE,
                    ids::PCB_PRIM_ELECTRICAL_TERMINATOR,
                ],
                0,
                ids::PCB_PRIM_ELECTRICAL_UNSUPPORTED,
            ),
        ),
    );
    let blank = || UserText::new(String::new());
    s.push(
        common,
        ids::PCB_PRIM_PROPAGATION_DELAY,
        labelled(
            look,
            ids::PCB_PRIM_PROPAGATION_DELAY,
            fixed_value(look, blank(), ids::PCB_PRIM_DELAY_UNKNOWN),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_PIN_PACKAGE_LENGTH,
        labelled(
            look,
            ids::PCB_PRIM_PIN_PACKAGE_LENGTH,
            fixed_value(look, blank(), ids::PCB_PRIM_PIN_LENGTH_UNSUPPORTED),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_JUMPER,
        labelled(
            look,
            ids::PCB_PRIM_JUMPER,
            fixed_value(
                look,
                UserText::new("0".to_string()),
                ids::PCB_PRIM_JUMPER_UNSUPPORTED,
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
    let degrees = f.rotation.map(format_number);
    s.push(
        common,
        ids::PCB_PRIM_ROTATION,
        labelled(
            look,
            ids::PCB_PRIM_ROTATION,
            field(common, F::Rotation, degrees, None, look),
        ),
    );
    let locked = common.editability == super::Editability::Locked;
    s.push(
        common,
        ids::PCB_PRIM_LOCK,
        labelled(
            look,
            ids::PCB_PRIM_LOCK,
            lock(look, locked, ids::PCB_PRIM_PAD_LOCK_TIP),
        ),
    );
    s.finish()
}

fn pad_stack<'a>(look: Look<'a>, f: &'a PadFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_PAD_STACK);
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
        fixed_tiles(
            look,
            &[
                ids::PCB_PRIM_STACK_SIMPLE,
                ids::PCB_PRIM_STACK_TOP_MIDDLE_BOTTOM,
                ids::PCB_PRIM_STACK_FULL,
            ],
            0,
            ids::PCB_PRIM_STACK_UNSUPPORTED,
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_COPPER,
        group(look, ids::PCB_PRIM_COPPER),
    );
    s.push(
        common,
        ids::PCB_PRIM_X_SIZE,
        labelled(
            look,
            ids::PCB_PRIM_X_SIZE,
            mm_field(common, F::SizeX, f.size_x, look),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_Y_SIZE,
        labelled(
            look,
            ids::PCB_PRIM_Y_SIZE,
            mm_field(common, F::SizeY, f.size_y, look),
        ),
    );
    let target = common.target.clone();
    let shapes = dropdown(common, &f.shapes, f.shape.clone(), move |choice| {
        PcbPrimitiveMsg::ShapePicked {
            target: target.clone(),
            shape: choice.shape,
        }
    });
    s.push(
        common,
        ids::PCB_PRIM_SHAPE,
        labelled(look, ids::PCB_PRIM_SHAPE, shapes),
    );
    s.push(
        common,
        ids::PCB_PRIM_RELIEF,
        labelled(
            look,
            ids::PCB_PRIM_RELIEF,
            fixed_checkbox(
                look,
                ids::PCB_PRIM_DIRECT,
                false,
                ids::PCB_PRIM_RELIEF_UNSUPPORTED,
            ),
        ),
    );
    let hole = if f.has_hole {
        mm_field(common, F::HoleSize, f.hole, look)
    } else {
        value_box(look, look.t(ids::PCB_PRIM_NO_HOLE))
    };
    s.push(
        common,
        ids::PCB_PRIM_HOLE_SIZE,
        labelled(look, ids::PCB_PRIM_HOLE_SIZE, hole),
    );
    let rule = || {
        fixed_value(
            look,
            look.t(ids::PCB_PRIM_RULE),
            ids::PCB_PRIM_MASK_UNSUPPORTED,
        )
    };
    s.push(
        common,
        ids::PCB_PRIM_PASTE,
        group(look, ids::PCB_PRIM_PASTE),
    );
    s.push(
        common,
        ids::PCB_PRIM_EXPANSION,
        labelled(
            look,
            ids::PCB_PRIM_EXPANSION,
            pair(
                rule(),
                fixed_value(
                    look,
                    look.t(ids::PCB_PRIM_PERCENT),
                    ids::PCB_PRIM_MASK_UNSUPPORTED,
                ),
            ),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_ENABLED,
        labelled(
            look,
            ids::PCB_PRIM_ENABLED,
            fixed_checkbox(
                look,
                ids::PCB_PRIM_ENABLED,
                true,
                ids::PCB_PRIM_MASK_UNSUPPORTED,
            ),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_SOLDER,
        group(look, ids::PCB_PRIM_SOLDER),
    );
    s.push(
        common,
        ids::PCB_PRIM_EXPANSION,
        labelled(look, ids::PCB_PRIM_EXPANSION, rule()),
    );
    s.push(
        common,
        ids::PCB_PRIM_TENTED,
        labelled(
            look,
            ids::PCB_PRIM_TENTED,
            fixed_checkbox(
                look,
                ids::PCB_PRIM_TENTED,
                false,
                ids::PCB_PRIM_MASK_UNSUPPORTED,
            ),
        ),
    );
    s.finish()
}

/// The caption over a layer group of the pad stack.
fn group<'a>(look: Look<'a>, id: signex_i18n::MessageId) -> Element<'a, PanelMsg> {
    column![text(look.t(id)).size(TS.dense).color(look.ink)]
        .padding([SP.xs, super::rows::PAD_X])
        .width(Length::Fill)
        .boxed()
}
