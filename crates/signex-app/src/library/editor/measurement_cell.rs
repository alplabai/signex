//! The editor of one measurement parameter: a number-only field, an SI
//! prefix picker and the unit (fixed by the class template's slot, or
//! the row's own). Users never type a prefix or a unit.

use iced::widget::{column, row, text, text_input};
use iced::{Element, Length};
use signex_library::{ParamSlot, ParamValue, slot_unit};
use signex_types::quantity::Unit;
use signex_types::theme::ThemeTokens;
use signex_widgets::theme_ext;

use super::super::messages::{EditorMsg, LibraryMessage};
use super::super::prefix_picker::prefix_picker;
use super::super::state::{ComponentPreviewState, EditorAddress};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// The unit token the row edits in: the slot's, else the stored value's.
fn row_unit(
    state: &ComponentPreviewState,
    name: &str,
    unit: Option<String>,
    slot: Option<&ParamSlot>,
) -> String {
    if let Some(u) = slot.and_then(|s| s.unit.clone()) {
        return u;
    }
    match (unit, state.row.parameters.get(name)) {
        (Some(u), _) => u,
        (None, Some(ParamValue::Measurement { unit, .. })) => unit.clone(),
        _ => String::new(),
    }
}

/// What the number field shows: the pending buffer, else the stored
/// value's number in the prefix the picker shows.
fn field_text(state: &ComponentPreviewState, name: &str, typed_unit: bool) -> String {
    if let Some(buf) = state.params_edit_buf.get(name) {
        return buf.clone();
    }
    match state.row.parameters.get(name) {
        Some(v) if typed_unit && v.measurement_parts().is_some() => v
            .measurement_parts()
            .map(|p| p.mantissa)
            .unwrap_or_default(),
        Some(ParamValue::Measurement { value, .. }) => value.to_string(),
        Some(other) => other.display(),
        None => String::new(),
    }
}

pub(super) fn view<'a>(
    state: &'a ComponentPreviewState,
    name: &str,
    unit: Option<String>,
    slot: Option<&ParamSlot>,
    tokens: &'a ThemeTokens,
    address: &EditorAddress,
) -> Element<'a, LibraryMessage> {
    let unit_token = row_unit(state, name, unit, slot);
    let known: Option<Unit> = slot
        .and_then(slot_unit)
        .or_else(|| Unit::from_alias(unit_token.trim()));
    let buf = field_text(state, name, known.is_some());
    let event = |msg| super::params::editor_event(address, msg);

    let input_slot = slot.cloned();
    let (input_address, input_name) = (address.clone(), name.to_string());
    let field = text_input("", &buf)
        .on_input(move |s| {
            super::params::editor_event(
                &input_address,
                EditorMsg::ParamSetMeasurementBuf {
                    name: input_name.clone(),
                    buf: s,
                    slot: input_slot.clone(),
                },
            )
        })
        .on_submit(event(EditorMsg::ParamCommitMeasurement {
            name: name.to_string(),
            unit: unit_token.clone(),
        }))
        .padding([SP.s, SP.m])
        .size(TS.body);

    let pick_slot = slot.cloned();
    let (pick_address, pick_name, pick_unit) =
        (address.clone(), name.to_string(), unit_token.clone());
    let picker = known.and_then(|u| {
        prefix_picker(&u, state.measurement_prefix(name), TS.body, move |prefix| {
            super::params::editor_event(
                &pick_address,
                EditorMsg::ParamPrefixPicked {
                    name: pick_name.clone(),
                    unit: pick_unit.clone(),
                    prefix,
                    slot: pick_slot.clone(),
                },
            )
        })
    });
    let line: Element<'a, LibraryMessage> = match picker {
        Some(pick) => row![field, pick]
            .spacing(SP.s)
            .align_y(iced::Alignment::Center)
            .into(),
        None => field.into(),
    };
    match state.params_error.get(name) {
        Some(why) => column![
            line,
            text(why.clone())
                .size(TS.caption)
                .color(theme_ext::error_color(tokens))
        ]
        .spacing(SP.xs)
        .width(Length::Fill)
        .into(),
        None => line,
    }
}
