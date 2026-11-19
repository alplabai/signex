//! Hole Details of a through-hole pad, a slot or a non-plated hole (#125):
//! the hole shape the model stores (Round or Slot), its size with Plated,
//! the tolerance, the rotation of a slot, and the thermal relief of a
//! plated pad as an enable header over its rule values (lib-17).

use iced::alignment::Horizontal;
use iced::widget::{Column, Space, container, stack};
use iced::{Element, Length, Padding, Widget as _};
use signex_i18n::{Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::enable_section::EnableSection;
use signex_widgets::{CheckboxLabel as _, Label, segmented, text};

use super::super::super::super::PanelMsg;
use super::super::choices::HoleShapeChoice;
use super::super::field::{Shown, labelled_row, number_input, pair, segment_style, with_refusals};
use super::super::illustration::{HOLE, SLOT};
use super::super::rule_row::rule_only_row;
use super::{
    Form, PadClass, pad_drill_diameter_msg, pad_drill_slot_length_msg, pad_hole_rotation_msg,
    pad_hole_tolerance_minus_msg, pad_hole_tolerance_plus_msg, pad_plated_msg,
    pad_thermal_relief_msg,
};
use crate::library::editor::footprint::pad_field;
use crate::panels::enable_section::enable_section_style;
use crate::styles::tokens::resolve;
use crate::styles::{Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

pub(super) fn push<'a>(
    col: Column<Element<'a, PanelMsg>>,
    form: &Form<'_>,
) -> Column<Element<'a, PanelMsg>> {
    if !form.class.has_hole() {
        return col;
    }
    let (mut col, is_open) = form.section(col, "fp_pad_hole", ids::EDITORS_PAD_HOLE_DETAILS);
    if !is_open {
        return col;
    }
    let (v, target, i18n, tokens) = (form.values, form.target, form.i18n, form.tokens);
    let (fw, muted) = (form.fw(), form.palette.muted);

    let current = HoleShapeChoice::of(v.drill_slot_length_mm);
    let slot_default = v.drill_diameter_mm.map(|d| d * 1.5).unwrap_or(1.0);
    let shapes = HoleShapeChoice::ALL.map(|c| (c, Label::from(t!(i18n, c.label()))));
    let shape = segmented(shapes, current, segment_style(tokens), move |c| {
        match (c, current) {
            (picked, now) if picked == now => PanelMsg::Noop,
            (HoleShapeChoice::Round, _) => pad_drill_slot_length_msg(target, String::new()),
            (HoleShapeChoice::Slot, _) => {
                pad_drill_slot_length_msg(target, format!("{slot_default:.3}"))
            }
        }
    });
    col = col.push(labelled_row(
        t!(i18n, ids::EDITORS_PAD_SHAPE),
        shape,
        fw,
        muted,
    ));

    let mm = |v: Option<f64>| v.map(|v| format!("{v:.3}")).unwrap_or_default();
    let drill = Shown::from_buffer(
        form.buffer(pad_field::DRILL_DIAMETER),
        mm(v.drill_diameter_mm),
        false,
        |_| true,
    );
    let drill_input = form.linked(
        number_input(&drill, move |s| pad_drill_diameter_msg(target, s), tokens),
        HOLE,
    );
    let plated = signex_widgets::checkbox(!matches!(form.class, PadClass::Npth))
        .labelled(t!(i18n, ids::EDITORS_FP_PAD_PLATED))
        .on_toggle(move |on| pad_plated_msg(target, on))
        .size(TS.label)
        .text_size(TS.caption)
        .spacing(SP.s)
        .style(crate::panels::form_look::checkbox_style)
        .boxed();
    if current == HoleShapeChoice::Slot {
        let length = Shown::from_buffer(
            form.buffer(pad_field::DRILL_SLOT_LENGTH),
            mm(v.drill_slot_length_mm),
            false,
            |_| true,
        );
        let length_input = form.linked(
            number_input(
                &length,
                move |s| pad_drill_slot_length_msg(target, s),
                tokens,
            ),
            SLOT,
        );
        let row = labelled_row(
            t!(i18n, ids::EDITORS_FP_PAD_HOLE_WIDTH_LENGTH),
            pair(drill_input, length_input),
            fw,
            muted,
        );
        col = col.push(with_refusals(
            row,
            &[drill.refusal, length.refusal],
            tokens,
            i18n,
        ));
        col = col.push(labelled_row(
            UserText::new(String::new()),
            pair(plated, Space::new().width(Length::Fill).boxed()),
            fw,
            muted,
        ));
    } else {
        let row = labelled_row(
            t!(i18n, ids::EDITORS_FP_PAD_HOLE_SIZE),
            pair(drill_input, plated),
            fw,
            muted,
        );
        col = col.push(with_refusals(row, &[drill.refusal], tokens, i18n));
    }

    let tolerance = pair(
        number_input(
            &Shown::value(mm(v.hole_tolerance_plus_mm)),
            move |s| pad_hole_tolerance_plus_msg(target, s),
            tokens,
        ),
        number_input(
            &Shown::value(mm(v.hole_tolerance_minus_mm)),
            move |s| pad_hole_tolerance_minus_msg(target, s),
            tokens,
        ),
    );
    col = col.push(labelled_row(
        t!(i18n, ids::EDITORS_FP_PAD_TOLERANCE),
        tolerance,
        fw,
        muted,
    ));
    if current == HoleShapeChoice::Slot {
        let rotation = Shown::value(
            v.hole_rotation_deg
                .map(|r| format!("{r:.1}"))
                .unwrap_or_default(),
        );
        let input = number_input(&rotation, move |s| pad_hole_rotation_msg(target, s), tokens);
        col = col.push(labelled_row(
            t!(i18n, ids::EDITORS_PAD_HOLE_ROTATION),
            pair(input, Space::new().width(Length::Fill).boxed()),
            fw,
            muted,
        ));
    }
    if form.class.has_copper_stack() {
        col = col.push(thermal_relief(form));
    }
    col
}

/// Thermal relief: the shipped switch as the enable header ("off · direct
/// connect" while off), over Air gap, Spoke width and Spokes, which the
/// board's rule decides for a library pad (lib-17).
fn thermal_relief<'a>(form: &Form<'_>) -> Element<'a, PanelMsg> {
    let (target, i18n, tokens, palette) = (form.target, form.i18n, form.tokens, form.palette);
    let is_on = form.values.stack.thermal_relief;
    let body = Column::new()
        .width(Length::Fill)
        .push(rule_only_row(
            t!(i18n, ids::EDITORS_FP_PAD_AIR_GAP),
            palette,
            tokens,
            i18n,
        ))
        .push(rule_only_row(
            t!(i18n, ids::EDITORS_FP_PAD_SPOKE_WIDTH),
            palette,
            tokens,
            i18n,
        ))
        .push(rule_only_row(
            t!(i18n, ids::EDITORS_FP_PAD_SPOKES),
            palette,
            tokens,
            i18n,
        ));
    let section = EnableSection::new(t!(i18n, ids::EDITORS_PAD_THERMAL_RELIEF), is_on)
        .on_toggle(move |on| pad_thermal_relief_msg(target, on))
        .view(enable_section_style(tokens), body.boxed());
    let caption = (!is_on).then(|| t!(i18n, ids::EDITORS_FP_PAD_THERMAL_OFF));
    with_header_caption(section, caption, tokens)
}

/// `section` with `caption` at the right end of its enable header ("off",
/// "opening", "tented"), in the faint ink.
pub(super) fn with_header_caption<'a>(
    section: Element<'a, PanelMsg>,
    caption: Option<Localized>,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let Some(caption) = caption else {
        return section;
    };
    let note = container(text(caption).size(TS.caption).color(resolve(tokens).ink4))
        .width(Length::Fill)
        .align_x(Horizontal::Right)
        .padding(Padding {
            top: SP.m,
            right: SP.l,
            bottom: 0.0,
            left: 0.0,
        });
    stack![section, note].boxed()
}
