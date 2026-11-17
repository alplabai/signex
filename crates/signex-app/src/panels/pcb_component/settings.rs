//! The rows of the component settings (#2365): the Type dropdown, typed
//! numbers and text (height, opacity, channel offset, footprint description)
//! and the lock and swapping checkboxes. Each edit is one board undo step.

use iced::{Element, Length, Widget as _};
use signex_engine::pcb::component_edit::{ComponentSetting, TypedSetting};
use signex_i18n::{MessageId, ids};
use signex_types::pcb::ComponentType;

use super::general::draft_input;
use super::rows::{self, Form};
use super::{ComponentField, PcbComponentFacts, PcbComponentMsg, Shared};
use crate::panels::PanelMsg;
use crate::styles::Spacing;

const SP: Spacing = Spacing::DEFAULT;

/// The catalog label of a component type.
pub(crate) fn type_label(component_type: ComponentType) -> MessageId {
    match component_type {
        ComponentType::Standard => ids::PCB_COMPONENT_TYPE_STANDARD,
        ComponentType::StandardNoBom => ids::PCB_COMPONENT_TYPE_STANDARD_NO_BOM,
        ComponentType::Mechanical => ids::PCB_COMPONENT_TYPE_MECHANICAL,
        ComponentType::Graphical => ids::PCB_COMPONENT_TYPE_GRAPHICAL,
        ComponentType::NetTie => ids::PCB_COMPONENT_TYPE_NET_TIE,
        ComponentType::NetTieInBom => ids::PCB_COMPONENT_TYPE_NET_TIE_IN_BOM,
        ComponentType::Jumper => ids::PCB_COMPONENT_TYPE_JUMPER,
    }
}

/// One entry of the Type dropdown.
#[derive(Debug, Clone, PartialEq)]
struct TypeChoice {
    component_type: ComponentType,
    label: String,
}

/// The message that writes `setting` on the selected components.
fn picked(facts: &PcbComponentFacts, setting: ComponentSetting) -> PanelMsg {
    PanelMsg::PcbComponent(PcbComponentMsg::SettingPicked {
        board: facts.board.clone(),
        footprints: facts.footprints.clone(),
        setting,
    })
}

/// The Type row: a dropdown of every type, blank when the components differ.
pub(super) fn type_row<'a>(
    form: &Form<'a>,
    facts: &'a PcbComponentFacts,
) -> Option<Element<'a, PanelMsg>> {
    use signex_widgets::dropdown_style::GraphiteDropdown as _;
    let label = form.text(ids::PCB_COMPONENT_TYPE);
    if !form.shows(&label) {
        return None;
    }
    let choice = |component_type: ComponentType| TypeChoice {
        component_type,
        label: form.text(type_label(component_type)),
    };
    let options: Vec<TypeChoice> = ComponentType::ALL.into_iter().map(choice).collect();
    let selected = facts.settings.component_type.same().map(|t| choice(*t));
    let list = iced::widget::pick_list(selected, options, |c: &TypeChoice| c.label.clone())
        .graphite()
        .padding(SP.s)
        .width(Length::Fill);
    let control = if facts.is_editable() {
        list.on_select(move |c: TypeChoice| picked(facts, ComponentSetting::Type(c.component_type)))
            .boxed()
    } else {
        list.boxed()
    };
    Some(rows::row(form, label, control))
}

/// A typed setting's row: its draft, else the shared stored text.
pub(super) fn typed_row<'a>(
    form: &Form<'a>,
    facts: &'a PcbComponentFacts,
    label: MessageId,
    tip: Option<MessageId>,
    setting: TypedSetting,
    stored: Option<String>,
) -> Option<Element<'a, PanelMsg>> {
    let label = form.text(label);
    if !form.shows(&label) {
        return None;
    }
    let mut control = draft_input(facts, ComponentField::Setting(setting), stored);
    if let Some(tip) = tip {
        control = form.with_tip(control, tip);
    }
    Some(rows::row(form, label, control))
}

/// A checkbox setting's row; a mixed value ticks every component.
pub(super) fn flag_row<'a>(
    form: &Form<'a>,
    facts: &'a PcbComponentFacts,
    label: MessageId,
    tip: Option<MessageId>,
    value: &Shared<bool>,
    make: fn(bool) -> ComponentSetting,
) -> Option<Element<'a, PanelMsg>> {
    let label = form.text(label);
    if !form.shows(&label) {
        return None;
    }
    let toggled = facts
        .is_editable()
        .then(|| picked(facts, make(*value != Shared::Same(true))));
    let mut control = rows::check(value, toggled);
    if let Some(tip) = tip {
        control = form.with_tip(control, tip);
    }
    Some(rows::row(form, label, control))
}

/// The Properties rows after Design Item ID and Source: Height, 3D Body
/// Opacity and the two locks.
pub(super) fn property_rows<'a>(
    form: &Form<'a>,
    facts: &'a PcbComponentFacts,
) -> Vec<Element<'a, PanelMsg>> {
    let s = &facts.settings;
    let number = |value: &Shared<f64>| value.same().map(|v| rows::number_text(*v));
    [
        typed_row(
            form,
            facts,
            ids::PCB_COMPONENT_HEIGHT,
            Some(ids::PCB_COMPONENT_HEIGHT_TIP),
            TypedSetting::Height,
            number(&s.height_mm),
        ),
        typed_row(
            form,
            facts,
            ids::PCB_COMPONENT_BODY_OPACITY,
            Some(ids::PCB_COMPONENT_BODY_OPACITY_TIP),
            TypedSetting::BodyOpacity,
            number(&s.body_opacity),
        ),
        flag_row(
            form,
            facts,
            ids::PCB_COMPONENT_LOCK_PRIMITIVES,
            Some(ids::PCB_COMPONENT_LOCK_PRIMITIVES_TIP),
            &s.lock_primitives,
            ComponentSetting::LockPrimitives,
        ),
        flag_row(
            form,
            facts,
            ids::PCB_COMPONENT_LOCK_STRINGS,
            Some(ids::PCB_COMPONENT_LOCK_STRINGS_TIP),
            &s.lock_strings,
            ComponentSetting::LockStrings,
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The Swapping Options rows.
pub(super) fn swapping_rows<'a>(
    form: &Form<'a>,
    facts: &'a PcbComponentFacts,
) -> Vec<Element<'a, PanelMsg>> {
    let s = &facts.settings;
    [
        flag_row(
            form,
            facts,
            ids::PCB_COMPONENT_PIN_SWAP,
            None,
            &s.pin_swapping,
            ComponentSetting::PinSwapping,
        ),
        flag_row(
            form,
            facts,
            ids::PCB_COMPONENT_PART_SWAP,
            None,
            &s.part_swapping,
            ComponentSetting::PartSwapping,
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
}
