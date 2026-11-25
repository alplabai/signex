//! The pad's Properties section (mockup row 125): Designator with Locked, Layer,
//! Electrical Type (not for a hole), Net, the read-only position, Rotation
//! and the Template dropdown (`template.rs`). The Layer dropdown carries
//! the layer's colour dot.

use iced::widget::{Column, Space, container, pick_list, row};
use iced::{Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{CheckboxLabel as _, Label, text_input};

use super::super::super::super::PanelMsg;
use super::super::field::{Shown, labelled_row, number_input, pair, read_only_cell, with_refusals};
use super::{
    Form, PadClass, pad_designator_msg, pad_electrical_type_msg, pad_locked_msg, pad_net_msg,
    pad_pick_row_w, pad_rotation_msg, pad_side_msg,
};
use crate::library::editor::footprint::{PadSide, pad_field};
use crate::pcb_canvas::layer_style;
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

pub(super) fn push<'a>(
    col: Column<Element<'a, PanelMsg>>,
    form: &Form<'_>,
) -> Column<Element<'a, PanelMsg>> {
    let (mut col, is_open) = form.section(
        col,
        "fp_pad_properties",
        ids::EDITORS_FP_PAD_SECTION_PROPERTIES,
    );
    if !is_open {
        return col;
    }
    let (v, target, i18n) = (form.values, form.target, form.i18n);
    let fw = form.fw();
    let muted = form.palette.muted;

    let designator = text_input(t!(i18n, ids::EDITORS_FP_PAD_AUTO), v.designator.clone())
        .size(TS.caption)
        .width(Length::Fill)
        .padding(SP.xs)
        .style(crate::panels::form_look::text_input_style)
        .on_input(move |s| pad_designator_msg(target, s))
        .boxed();
    let locked = signex_widgets::checkbox(v.locked)
        .labelled(t!(i18n, ids::EDITORS_PAD_LOCKED))
        .on_toggle(move |on| pad_locked_msg(target, on))
        .size(TS.label)
        .text_size(TS.caption)
        .spacing(SP.s)
        .style(crate::panels::form_look::checkbox_style)
        .boxed();
    col = col.push(labelled_row(
        t!(i18n, ids::EDITORS_PAD_DESIGNATOR),
        pair(designator, locked),
        fw,
        muted,
    ));

    // The pad's kind has no picker in this form, so Layer stays the way a
    // pad becomes through-hole (Multi-Layer) and back.
    col = col.push(layer_row(
        t!(i18n, ids::EDITORS_PAD_LAYER),
        v.side,
        move |s: PadSide| pad_side_msg(target, s),
        fw,
        muted,
    ));
    if form.class != PadClass::Npth {
        col = col.push(pad_pick_row_w(
            fw,
            t!(i18n, ids::EDITORS_PAD_ELECTRICAL_TYPE),
            signex_sketch::attr::ElectricalType::ALL,
            v.electrical_type,
            move |e: signex_sketch::attr::ElectricalType| pad_electrical_type_msg(target, e),
            muted,
        ));
    }
    let net = text_input(t!(i18n, ids::EDITORS_FP_PAD_UNASSIGNED), v.net.clone())
        .size(TS.caption)
        .width(Length::Fill)
        .padding(SP.xs)
        .style(crate::panels::form_look::text_input_style)
        .on_input(move |s| pad_net_msg(target, s))
        .boxed();
    col = col.push(labelled_row(t!(i18n, ids::EDITORS_PAD_NET), net, fw, muted));

    // Position is read-only: a pad moves by dragging it on the canvas.
    if let Some([x, y]) = v.position_mm {
        let cell = |n: f64| read_only_cell(UserText::new(format!("{n:.3}")), form.tokens);
        col = col.push(labelled_row(
            t!(i18n, ids::EDITORS_FP_PAD_POSITION),
            pair(cell(x), cell(y)),
            fw,
            muted,
        ));
    }

    let rotation = Shown::from_buffer(
        form.buffer(pad_field::ROTATION),
        format!("{:.1}", v.rotation_deg),
        false,
        |_| true,
    );
    let input = number_input(&rotation, move |s| pad_rotation_msg(target, s), form.tokens);
    let row = labelled_row(
        t!(i18n, ids::EDITORS_PAD_ROTATION),
        pair(input, Space::new().width(Length::Fill).boxed()),
        fw,
        muted,
    );
    col = col.push(with_refusals(row, &[rotation.refusal], form.tokens, i18n));
    super::template::push(col, form)
}

/// The colour the Layer dropdown's dot shows for `side` (mockup row 125):
/// the colour the canvas draws that copper in, Multi-Layer's for a
/// through-hole pad. Opaque, as a swatch.
fn layer_dot_colour(side: PadSide) -> Color {
    let colour = match side {
        PadSide::Top => layer_style::board_layer_colour(PcbLayer::Copper(CopperLayer::Top)),
        PadSide::Bottom => layer_style::board_layer_colour(PcbLayer::Copper(CopperLayer::Bottom)),
        PadSide::All => layer_style::multi_layer_colour(),
    };
    Color { a: 1.0, ..colour }
}

/// A round dot of `colour`, sized from the spacing scale.
fn layer_dot<'a>(colour: Color) -> Element<'a, PanelMsg> {
    let side = SP.m + SP.xs / 2.0;
    container(Space::new())
        .width(Length::Fixed(side))
        .height(Length::Fixed(side))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(colour)),
            border: Border {
                radius: (side / 2.0).into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The Layer row: the layer's colour dot, then the dropdown.
fn layer_row<'a>(
    label: impl Into<Label<'a>>,
    side: PadSide,
    on_change: impl Fn(PadSide) -> PanelMsg + 'a,
    fw: (Length, Length),
    muted: Color,
) -> Element<'a, PanelMsg> {
    let picker = pick_list(Some(side), PadSide::ALL_OPTIONS, |s: &PadSide| {
        s.to_string()
    })
    .graphite()
    .on_select(on_change)
    .padding([SP.xs, SP.m])
    .width(Length::Fill);
    let content = row![layer_dot(layer_dot_colour(side)), picker]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .width(Length::Fill)
        .boxed();
    labelled_row(label, content, fw, muted)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WINDOW: iced::Size = iced::Size::new(300.0, 60.0);

    fn rgb(c: Color) -> [i32; 3] {
        [c.r, c.g, c.b].map(|v| (v * 255.0).round() as i32)
    }

    /// Whether any pixel of the drawn `side` row is `colour`.
    fn row_shows(side: PadSide, colour: Color) -> bool {
        let fw = (Length::FillPortion(4), Length::FillPortion(5));
        let element = layer_row(
            UserText::new(String::from("Layer")),
            side,
            |_| PanelMsg::FpEditorEditPadInSketch { pad_idx: 0 },
            fw,
            Color::BLACK,
        );
        let want = rgb(colour);
        crate::layout_probe::draw_pixels(element, WINDOW)
            .chunks_exact(4)
            .any(|px| {
                let got = [px[0], px[1], px[2]].map(i32::from);
                got.iter().zip(want).all(|(g, w)| (g - w).abs() <= 3)
            })
    }

    /// Mockup row 125: Top copper and Bottom copper differ, and Multi-Layer
    /// has its own colour.
    #[test]
    fn each_layer_has_its_own_dot_colour() {
        let [top, bottom, all] =
            [PadSide::Top, PadSide::Bottom, PadSide::All].map(layer_dot_colour);
        assert_ne!(rgb(top), rgb(bottom));
        assert_ne!(rgb(top), rgb(all));
        assert_eq!(top.a, 1.0);
    }

    /// Mockup row 125: the Layer dropdown carries a dot in its layer's colour.
    #[test]
    fn the_layer_row_draws_its_colour_dot() {
        let top = layer_dot_colour(PadSide::Top);
        let bottom = layer_dot_colour(PadSide::Bottom);
        assert!(row_shows(PadSide::Top, top), "no Top Layer dot");
        assert!(row_shows(PadSide::Bottom, bottom), "no Bottom Layer dot");
        assert!(
            !row_shows(PadSide::Top, bottom),
            "the Top row drew a Bottom dot"
        );
    }
}
