//! The area panel (#2393): fills, solid regions, polygon pours and
//! keepouts. Net Information and the net for copper areas, the layer, a
//! pour's clearance, minimum width, priority, island rules and the copper
//! its fill holds (#2216), a keepout's kinds and the lock.

use iced::widget::{container, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{ids, t};

use super::rows::{
    Look, Section, chip, dropdown, field, fixed_value, labelled, live_checkbox, lock, mm_field,
    net_row, stack,
};
use super::{PcbPrimitiveMsg, PrimitiveField as F, SP, ZoneFacts, format_number, net_info};
use crate::panels::{PanelContext, PanelMsg};
use crate::pcb_canvas::layer_style;

pub(super) fn view<'a>(ctx: &'a PanelContext, f: &'a ZoneFacts) -> Element<'a, PanelMsg> {
    let look = Look::of(ctx);
    let common = &f.common;
    let net_section = if f.is_copper {
        net_info::section(look, common)
    } else {
        None
    };
    stack(vec![net_section, properties(look, f), pour(look, f)])
}

fn properties<'a>(look: Look<'a>, f: &'a ZoneFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_PROPERTIES);
    if f.is_copper {
        s.push(common, ids::PCB_PRIM_NET, net_row(look, common));
    }
    let target = common.target.clone();
    let layers = dropdown(common, &f.layers, f.layer.clone(), move |choice| {
        PcbPrimitiveMsg::AreaLayerPicked {
            target: target.clone(),
            layer: choice.layer,
        }
    });
    let swatch = f.layer.as_ref().map_or_else(
        || {
            container(iced::widget::Space::new())
                .width(Length::Fixed(super::TS.body))
                .boxed()
        },
        |l| chip(layer_style::layer_colour(l.layer, look.tokens())),
    );
    s.push(
        common,
        ids::PCB_PRIM_LAYER,
        labelled(
            look,
            ids::PCB_PRIM_LAYER,
            row![swatch, container(layers).width(Length::Fill)]
                .spacing(SP.s)
                .align_y(iced::Alignment::Center)
                .boxed(),
        ),
    );
    if f.kind == Some(super::ZoneKind::Keepout) {
        s.push(
            common,
            ids::PCB_PRIM_KEEPOUT_KINDS,
            labelled(
                look,
                ids::PCB_PRIM_KEEPOUT_KINDS,
                fixed_value(
                    look,
                    look.t(ids::PCB_PRIM_KEEPOUT_ALL_COPPER),
                    ids::PCB_PRIM_KEEPOUT_KINDS_UNSUPPORTED,
                ),
            ),
        );
    }
    s.push(
        common,
        ids::PCB_PRIM_LOCK,
        labelled(
            look,
            ids::PCB_PRIM_LOCK,
            lock(look, false, ids::PCB_PRIM_LOCK_UNSUPPORTED),
        ),
    );
    s.finish()
}

/// A pour's clearance and minimum width; nothing for other areas.
fn pour<'a>(look: Look<'a>, f: &'a ZoneFacts) -> Option<Element<'a, PanelMsg>> {
    if !f.is_pour {
        return None;
    }
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_POUR_SETTINGS);
    s.push(
        common,
        ids::PCB_PRIM_CLEARANCE,
        labelled(
            look,
            ids::PCB_PRIM_CLEARANCE,
            mm_field(common, F::Clearance, f.clearance, look),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_MIN_THICKNESS,
        labelled(
            look,
            ids::PCB_PRIM_MIN_THICKNESS,
            mm_field(common, F::MinThickness, f.min_thickness, look),
        ),
    );
    let priority = f.priority.map(|p| p.to_string());
    s.push(
        common,
        ids::PCB_PRIM_PRIORITY,
        labelled(
            look,
            ids::PCB_PRIM_PRIORITY,
            field(common, F::Priority, priority, None, look),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_MIN_ISLAND_AREA,
        labelled(
            look,
            ids::PCB_PRIM_MIN_ISLAND_AREA,
            mm_field(common, F::MinIslandArea, f.min_island_area, look),
        ),
    );
    let target = common.target.clone();
    s.push(
        common,
        ids::PCB_PRIM_REMOVE_DEAD_ISLANDS,
        live_checkbox(
            look,
            common,
            ids::PCB_PRIM_REMOVE_DEAD_ISLANDS,
            f.remove_dead_islands,
            move |on| PcbPrimitiveMsg::DeadIslandsToggled {
                target: target.clone(),
                on,
            },
        ),
    );
    if let Some((area, pieces)) = f.copper {
        let value = t!(
            look.ctx.i18n,
            ids::PCB_PRIM_POUR_COPPER_VALUE,
            "area" => format_number(area),
            "pieces" => pieces
        );
        s.push(
            common,
            ids::PCB_PRIM_POUR_COPPER,
            labelled(
                look,
                ids::PCB_PRIM_POUR_COPPER,
                fixed_value(look, value, ids::PCB_PRIM_POUR_COPPER_TIP),
            ),
        );
    }
    s.finish()
}
