//! Thermal relief of a plated pad (mockup row 125, #2593): the shipped
//! switch as the enable header ("off · direct connect" while off), over Air
//! gap, Spoke width and Spokes, each Rule (the pour decides) or Manual (the
//! pad's own value, saved with the footprint and used by a pour connecting
//! to the placed pad).

use iced::widget::Column;
use iced::{Element, Length, Widget as _};
use signex_i18n::{Localized, ids, t};
use signex_widgets::enable_section::EnableSection;

use super::super::super::super::PanelMsg;
use super::super::field::{Refusal, Shown};
use super::super::illustration::{THERMAL_GAP, THERMAL_SPOKE, THERMAL_SPOKES};
use super::super::rule_row::RuleRow;
use super::hole::with_header_caption;
use super::{Form, pad_thermal_relief_msg};
use crate::library::editor::footprint::PadThermalField;
use crate::panels::enable_section::enable_section_style;

/// The text Manual sends: the defaults a pour uses.
const SEED_GAP: &str = "0.508";
const SEED_SPOKE: &str = "0.254";
const SEED_SPOKES: &str = "4";

/// The Thermal relief section of `form`'s pad.
pub(super) fn thermal_relief<'a>(form: &Form<'_>) -> Element<'a, PanelMsg> {
    let (target, i18n, tokens) = (form.target, form.i18n, form.tokens);
    let is_on = form.values.stack.thermal_relief;
    let thermal = form.values.stack.thermal;
    let body = Column::new()
        .width(Length::Fill)
        .push(length_row(
            form,
            PadThermalField::AirGap,
            t!(i18n, ids::EDITORS_FP_PAD_AIR_GAP),
            thermal.air_gap,
        ))
        .push(length_row(
            form,
            PadThermalField::SpokeWidth,
            t!(i18n, ids::EDITORS_FP_PAD_SPOKE_WIDTH),
            thermal.spoke_width,
        ))
        .push(spokes_row(form, thermal.spokes));
    let section = EnableSection::new(t!(i18n, ids::EDITORS_PAD_THERMAL_RELIEF), is_on)
        .on_toggle(move |on| pad_thermal_relief_msg(target, on))
        .view(enable_section_style(tokens), body.boxed());
    let caption = (!is_on).then(|| t!(i18n, ids::EDITORS_FP_PAD_THERMAL_OFF));
    with_header_caption(section, caption, tokens)
}

/// What a row sends: the typed text of `which` on the form's pad.
fn thermal_msg(
    form: &Form<'_>,
    which: PadThermalField,
) -> impl Fn(String) -> PanelMsg + Clone + use<> {
    let idx = form.target.selected_idx();
    move |value| PanelMsg::FpEditorPadThermalTyped {
        idx,
        field: which,
        value,
    }
}

/// Air gap or Spoke width: Rule, or a Manual length above 0 mm. Manual
/// starts from the default gap or width a pour uses.
fn length_row<'a>(
    form: &Form<'_>,
    which: PadThermalField,
    label: Localized,
    value: Option<f64>,
) -> Element<'a, PanelMsg> {
    let (field, seed) = match which {
        PadThermalField::AirGap => (THERMAL_GAP, SEED_GAP),
        _ => (THERMAL_SPOKE, SEED_SPOKE),
    };
    let buffer = form.buffer(which.field());
    let stored = value.map(|v| format!("{v:.3}")).unwrap_or_default();
    let row = RuleRow {
        label,
        field,
        is_manual: value.is_some() || buffer.is_some(),
        seed,
    };
    let shown = Shown::from_buffer(buffer, stored, true, |_| true);
    let send = thermal_msg(form, which);
    row.view(
        shown,
        send,
        form.palette,
        form.tokens,
        form.hover,
        form.i18n,
    )
}

/// Spokes: Rule, or a Manual count from 1 to 4. Manual starts from four.
fn spokes_row<'a>(form: &Form<'_>, value: Option<u8>) -> Element<'a, PanelMsg> {
    let which = PadThermalField::Spokes;
    let buffer = form.buffer(which.field());
    let shown = match buffer {
        Some(text) => Shown {
            refusal: Refusal::of_spokes(text),
            text: text.clone(),
        },
        None => Shown {
            text: value.map(|n| n.to_string()).unwrap_or_default(),
            refusal: None,
        },
    };
    let row = RuleRow {
        label: t!(form.i18n, ids::EDITORS_FP_PAD_SPOKES),
        field: THERMAL_SPOKES,
        is_manual: value.is_some() || buffer.is_some(),
        seed: SEED_SPOKES,
    };
    let send = thermal_msg(form, which);
    row.view(
        shown,
        send,
        form.palette,
        form.tokens,
        form.hover,
        form.i18n,
    )
}

#[cfg(test)]
mod tests {
    use signex_types::pad_thermal::DEFAULT_SPOKES;
    use signex_types::pcb::{PCB_THERMAL_BRIDGE_MM, PCB_THERMAL_GAP_MM};

    use super::*;

    /// The seeds are the defaults a pour uses, so picking Manual draws and
    /// fills as the rule did until a value is typed.
    #[test]
    fn manual_starts_from_the_pours_defaults() {
        assert_eq!(SEED_GAP.parse::<f64>().ok(), Some(PCB_THERMAL_GAP_MM));
        assert_eq!(SEED_SPOKE.parse::<f64>().ok(), Some(PCB_THERMAL_BRIDGE_MM));
        assert_eq!(SEED_SPOKES.parse::<u8>().ok(), Some(DEFAULT_SPOKES));
    }
}
