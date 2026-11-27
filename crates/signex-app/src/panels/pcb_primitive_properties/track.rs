//! The Track panel (#2523, mockup row 95): the illustration, Net Information,
//! Properties (net, layer, width, ends, the two ends, length, Tune Net To),
//! then the paste and solder mask expansion sections. The lock is the
//! header's icon. While placing, Net Information and the geometry rows are
//! left out: the clicks set them. Lengths read to three decimals.

use iced::widget::{container, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_types::pcb_layer::PcbLayer;
use signex_widgets::{Label, text};

use super::drawing::{self, keys};
use super::linked::{self, linked_row, mm_point};
use super::mask::track_mask_section;
use super::rows::{Look, Section, dropdown, fixed_mm, labelled, net_value, stack};
use super::{
    PairFacts, PcbPrimitiveMsg, PrimitiveCommon, PrimitiveField as F, SP, TS, TrackFacts, net_info,
};
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
    if let Some(pair) = &f.pair {
        pair_rows(look, &mut s, common, pair);
    }
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
            linked::mm_text(look, common, F::Width, f.width.map(fixed_mm)),
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
    // No Position row: the location is the header's lock and Start / End
    // (mockup row 95).
    s.push(
        common,
        ids::PCB_PRIM_POINT_START,
        linked_row(
            look,
            common,
            keys::START,
            ids::PCB_PRIM_POINT_START,
            mm_point(look, common, (F::StartX, F::StartY), f.start),
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
            mm_point(look, common, (F::EndX, F::EndY), f.end),
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
            linked::length(look, f.length_mm.map(fixed_mm)),
        ),
    );
    // #2389: a length for the track's net; Enter tunes the one selected
    // track to it.
    let tune = if common.target.items.len() == 1 {
        linked::mm_text(look, common, F::TuneLength, None)
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

/// The rows of a differential pair's tracks (#2585, mockup row 95): the
/// pair, the net paired with the shown one, and the copper gap between the
/// two tracks. Width above applies to both tracks of the pair.
fn pair_rows<'a>(
    look: Look<'a>,
    s: &mut Section<'a>,
    common: &PrimitiveCommon,
    pair: &'a PairFacts,
) {
    let name = linked::read_only(look, UserText::new(pair.name.clone()));
    s.push(
        common,
        ids::PCB_PRIM_PAIR,
        labelled(look, ids::PCB_PRIM_PAIR, name),
    );
    let paired: Label<'a> = match &pair.paired_net {
        Some(net) => UserText::new(net.clone()).into(),
        None => t!(
            look.ctx.i18n,
            ids::PCB_PRIM_PAIR_BOTH_NETS,
            "positive" => pair.positive.clone(),
            "negative" => pair.negative.clone()
        )
        .into(),
    };
    s.push(
        common,
        ids::PCB_PRIM_PAIRED_NET,
        labelled(
            look,
            ids::PCB_PRIM_PAIRED_NET,
            linked::read_only(look, paired),
        ),
    );
    s.push(
        common,
        ids::PCB_PRIM_PAIR_GAP,
        labelled(
            look,
            ids::PCB_PRIM_PAIR_GAP,
            linked::length(look, pair.gap_mm.map(fixed_mm)),
        ),
    );
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

#[cfg(test)]
#[path = "track_tests.rs"]
mod tests;
