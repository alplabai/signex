//! The Track panel (#2523, mockup row 95): the illustration, Net Information,
//! Properties (net, layer, width, ends, position, the two ends, length,
//! Tune Net To),
//! then the paste and solder mask expansion sections. The lock is the
//! header's icon. While placing, Net Information and the geometry rows are
//! left out: the clicks set them.

use iced::widget::{container, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::ids;
use signex_types::pcb_layer::PcbLayer;
use signex_widgets::text;

use super::drawing::{self, keys};
use super::linked::{self, linked_row, prefixed, two_fields, unit};
use super::mask::track_mask_section;
use super::rows::{Look, Section, dropdown, net_value, stack};
use super::{PcbPrimitiveMsg, PrimitiveField as F, SP, TS, TrackFacts, net_info};
use crate::panels::{PanelContext, PanelMsg};
use crate::pcb_canvas::layer_style;
use crate::styles::tokens::resolve;

pub(super) fn view<'a>(ctx: &'a PanelContext, f: &'a TrackFacts) -> Element<'a, PanelMsg> {
    let look = Look::of(ctx);
    let common = &f.common;
    let colour = f
        .layer
        .as_ref()
        .map(|l| layer_style::layer_colour(PcbLayer::Copper(l.layer), look.tokens()));
    let net_section = if common.target.is_placement() {
        None
    } else {
        net_info::open_section(look, common, colour)
    };
    let drawn = drawing::track::illustration(&ctx.i18n, f);
    stack(vec![
        Some(linked::illustration(look, common, drawn)),
        net_section,
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

fn properties<'a>(look: Look<'a>, f: &'a TrackFacts) -> Option<Element<'a, PanelMsg>> {
    let common = &f.common;
    let mut s = Section::new(look, ids::PCB_PRIM_PROPERTIES).collapsible(
        "pcb_prim.track.properties",
        false,
        None,
    );
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
    s.push(
        common,
        ids::PCB_PRIM_LAYER,
        linked_row(
            look,
            common,
            keys::LAYER,
            ids::PCB_PRIM_LAYER,
            layer_value(look, f),
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
            linked::ends(look, true),
        ),
    );
    // The geometry of a track not placed yet is its clicks' (#2393).
    if common.target.is_placement() {
        return s.finish();
    }
    let point = |fx, fy, at: (Option<f64>, Option<f64>)| {
        two_fields(
            prefixed(look, common, fx, at.0, ids::PCB_PRIM_X),
            prefixed(look, common, fy, at.1, ids::PCB_PRIM_Y),
            unit(look, ids::PCB_PRIM_UNIT_MM),
        )
    };
    // The selection's corner: typing it moves every selected track, which
    // Start and End cannot do once they read Mixed.
    s.push(
        common,
        ids::PCB_PRIM_POSITION,
        super::rows::labelled(
            look,
            ids::PCB_PRIM_POSITION,
            point(
                F::X,
                F::Y,
                (f.location.map(|l| l.0), f.location.map(|l| l.1)),
            ),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_POINT_START,
        linked_row(
            look,
            common,
            keys::START,
            ids::PCB_PRIM_POINT_START,
            point(F::StartX, F::StartY, f.start),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_POINT_END,
        linked_row(
            look,
            common,
            keys::END,
            ids::PCB_PRIM_POINT_END,
            point(F::EndX, F::EndY, f.end),
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
    // #2389: a length for the track's net; Enter tunes the one selected
    // track to it.
    let tune = if common.target.items.len() == 1 {
        linked::mm(look, common, F::TuneLength, None)
    } else {
        container(
            text(look.t(ids::PCB_PRIM_TUNE_ONE_TRACK))
                .size(TS.caption)
                .color(resolve(look.tokens()).ink4),
        )
        .padding([SP.xs, SP.s])
        .width(Length::Fill)
        .style(crate::styles::read_only_field(look.tokens()))
        .boxed()
    };
    s.push(
        common,
        ids::PCB_PRIM_TUNE_LENGTH,
        super::rows::labelled(look, ids::PCB_PRIM_TUNE_LENGTH, tune),
    );
    s.finish()
}

/// The layer dot and the Layer dropdown of the board's copper layers.
fn layer_value<'a>(look: Look<'a>, f: &'a TrackFacts) -> Element<'a, PanelMsg> {
    let common = &f.common;
    let target = common.target.clone();
    let layers = dropdown(common, &f.layers, f.layer.clone(), move |choice| {
        PcbPrimitiveMsg::LayerPicked {
            target: target.clone(),
            layer: choice.layer,
        }
    });
    let dot = f.layer.as_ref().map_or_else(
        || {
            container(iced::widget::Space::new())
                .width(Length::Fixed(SP.m))
                .boxed()
        },
        |l| {
            linked::dot(layer_style::layer_colour(
                PcbLayer::Copper(l.layer),
                look.tokens(),
            ))
        },
    );
    row![dot, container(layers).width(Length::Fill)]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .boxed()
}
