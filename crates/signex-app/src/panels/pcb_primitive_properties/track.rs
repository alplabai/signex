//! The Track panel: Net Information, Location, Properties, and the paste
//! and solder mask expansion sections.

use iced::widget::{container, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::ids;
use signex_types::pcb_layer::PcbLayer;

use super::mask::{lock_row, track_mask_section};
use super::rows::{
    Look, Section, chip, dropdown, labelled, mm_field, mm_or_na, net_row, pair, stack, value_box,
};
use super::{PcbPrimitiveMsg, PrimitiveField as F, SP, TrackFacts, net_info};
use crate::panels::{PanelContext, PanelMsg};
use crate::pcb_canvas::layer_style;

pub(super) fn view<'a>(ctx: &'a PanelContext, f: &'a TrackFacts) -> Element<'a, PanelMsg> {
    let look = Look::of(ctx);
    let common = &f.common;
    stack(vec![
        net_info::section(look, common),
        location(look, f),
        properties(look, f),
        track_mask_section(
            look,
            common,
            ids::PCB_PRIM_PASTE_MASK,
            F::PasteMask,
            f.paste_mask,
        ),
        track_mask_section(
            look,
            common,
            ids::PCB_PRIM_SOLDER_MASK,
            F::SolderMask,
            f.solder_mask,
        ),
    ])
}

fn location<'a>(look: Look<'a>, f: &'a TrackFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let (x, y) = (f.location.map(|l| l.0), f.location.map(|l| l.1));
    let mut s = Section::new(look, ids::PCB_PRIM_LOCATION);
    s.push(
        common,
        ids::PCB_PRIM_X,
        labelled(look, ids::PCB_PRIM_X, mm_field(common, F::X, x, look)),
    );
    s.push(
        common,
        ids::PCB_PRIM_Y,
        labelled(look, ids::PCB_PRIM_Y, mm_field(common, F::Y, y, look)),
    );
    s.push(common, ids::PCB_PRIM_LOCK, lock_row(look, common, f.locked));
    s.finish()
}

fn properties<'a>(look: Look<'a>, f: &'a TrackFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_PROPERTIES);
    s.push(common, ids::PCB_PRIM_NET, net_row(look, common));
    let target = common.target.clone();
    let layers = dropdown(common, &f.layers, f.layer.clone(), move |choice| {
        PcbPrimitiveMsg::LayerPicked {
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
        |l| {
            chip(layer_style::layer_colour(
                PcbLayer::Copper(l.layer),
                look.tokens(),
            ))
        },
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
    s.push(
        common,
        ids::PCB_PRIM_WIDTH,
        labelled(
            look,
            ids::PCB_PRIM_WIDTH,
            mm_field(common, F::Width, f.width, look),
        ),
    );
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
        ids::PCB_PRIM_LENGTH,
        labelled(
            look,
            ids::PCB_PRIM_LENGTH,
            value_box(look, mm_or_na(look, f.length_mm)),
        ),
    );
    // #2389: a length for the track's net; Enter tunes the track to it. A
    // track not placed yet has nothing to tune (#2393).
    if !common.target.is_placement() {
        s.push(
            common,
            ids::PCB_PRIM_TUNE_LENGTH,
            labelled(
                look,
                ids::PCB_PRIM_TUNE_LENGTH,
                mm_field(common, F::TuneLength, None, look),
            ),
        );
    }
    s.finish()
}
