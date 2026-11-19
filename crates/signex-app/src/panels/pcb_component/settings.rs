//! The rows of the component settings (#2365, Graphite m94): the Type
//! dropdown, Height, the 3D Opacity slider, the Lock row with its two
//! checkboxes, and the swapping checkboxes. Each edit is one board undo step.

use iced::widget::{Row, slider};
use iced::{Background, Border, Element, Length, Theme, Widget as _};
use signex_engine::pcb::component_edit::{ComponentSetting, TypedSetting};
use signex_i18n::{MessageId, ids, t};
use signex_types::pcb::ComponentType;
use signex_widgets::text;

use super::general::draft_input;
use super::rows::{self, Form};
use super::{ComponentField, PcbComponentFacts, PcbComponentMsg, Shared};
use crate::panels::{PanelMsg, form_look};
use crate::styles::tokens::resolve;
use crate::styles::{HAIRLINE, Spacing, TypeScale};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The slider's step: the two decimals its value is shown with.
const OPACITY_STEP: f64 = 0.01;

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

/// The Footprint rows after Footprint Name and Library: Type, Height, 3D
/// Opacity and Lock.
pub(super) fn footprint_rows<'a>(
    form: &Form<'a>,
    facts: &'a PcbComponentFacts,
) -> Vec<Element<'a, PanelMsg>> {
    [
        type_row(form, facts),
        height_row(form, facts),
        opacity_row(form, facts),
        lock_row(form, facts),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The Type row: a dropdown of every type, blank when the components differ.
fn type_row<'a>(form: &Form<'a>, facts: &'a PcbComponentFacts) -> Option<Element<'a, PanelMsg>> {
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

/// Height: its draft, else the shared stored height, then the unit.
fn height_row<'a>(form: &Form<'a>, facts: &'a PcbComponentFacts) -> Option<Element<'a, PanelMsg>> {
    let label = form.text(ids::PCB_COMPONENT_HEIGHT);
    if !form.shows(&label) {
        return None;
    }
    let stored = facts
        .settings
        .height_mm
        .same()
        .map(|v| rows::number_text(*v));
    let input = draft_input(facts, ComponentField::Setting(TypedSetting::Height), stored);
    let control = form.with_tip(input, ids::PCB_COMPONENT_HEIGHT_TIP);
    Some(rows::row(
        form,
        label,
        rows::with_unit(form, control, ids::PCB_COMPONENT_UNIT_MM),
    ))
}

/// 3D Opacity: a slider from 0 (clear) to 1 (solid) and its value. Dragging
/// keeps the value as the field's draft; letting go applies it as one step,
/// the same edit the typed field made.
fn opacity_row<'a>(form: &Form<'a>, facts: &'a PcbComponentFacts) -> Option<Element<'a, PanelMsg>> {
    let label = form.text(ids::PCB_COMPONENT_OPACITY);
    if !form.shows(&label) && !form.shows(&form.text(ids::PCB_COMPONENT_BODY_OPACITY)) {
        return None;
    }
    let field = ComponentField::Setting(TypedSetting::BodyOpacity);
    let dragged = facts
        .draft(&field)
        .and_then(|text| text.trim().parse::<f64>().ok());
    let value = dragged.or_else(|| facts.settings.body_opacity.same().copied());
    let editable = facts.is_editable();
    let bar: slider::Slider<'a, f64, PanelMsg> = if editable {
        let (board, footprints) = (facts.board.clone(), facts.footprints.clone());
        let released = PanelMsg::PcbComponent(PcbComponentMsg::FieldSubmitted {
            board: board.clone(),
            footprints: footprints.clone(),
            field: field.clone(),
        });
        slider(0.0..=1.0, value.unwrap_or(1.0), move |v: f64| {
            PanelMsg::PcbComponent(PcbComponentMsg::FieldTyped {
                board: board.clone(),
                footprints: footprints.clone(),
                field: field.clone(),
                text: format!("{v:.2}"),
            })
        })
        .on_release(released)
    } else {
        // Locked or read-only: the slider shows the value and moves nothing.
        slider(0.0..=1.0, value.unwrap_or(1.0), |_: f64| PanelMsg::Noop)
    };
    let bar = bar
        .step(OPACITY_STEP)
        .width(Length::Fill)
        .style(move |theme: &Theme, _| opacity_slider(theme, editable));
    let shown: signex_widgets::Label<'a> = match value {
        Some(v) => signex_i18n::UserText::new(format!("{v:.2}")).into(),
        None => t!(form.ctx.i18n, ids::PCB_COMPONENT_MIXED).into(),
    };
    let control = Row::new()
        .push_boxed(bar)
        .push_boxed(
            text(shown)
                .size(TS.caption)
                .font(crate::fonts::iced_font_for_family(
                    crate::fonts::CHROME_MONO_FONT,
                ))
                .color(resolve(&form.ctx.tokens).ink3),
        )
        .spacing(SP.m)
        .align_y(iced::Alignment::Center)
        .boxed();
    Some(rows::row_with_label_tip(
        form,
        label,
        ids::PCB_COMPONENT_BODY_OPACITY,
        control,
    ))
}

/// The Graphite slider: an accent fill on the strong rule, a raised handle
/// ringed in the accent; the faint ink while it cannot move.
fn opacity_slider(theme: &Theme, editable: bool) -> slider::Style {
    let l = form_look::FormLook::of(theme);
    let lit = if editable { l.accent } else { l.ink4 };
    slider::Style {
        rail: slider::Rail {
            backgrounds: (Background::Color(lit), Background::Color(l.line_strong)),
            width: SP.s,
            border: Border {
                radius: SP.xs.into(),
                ..Border::default()
            },
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Circle {
                radius: SP.s + SP.xs,
            },
            background: Background::Color(l.raised),
            border_width: HAIRLINE,
            border_color: lit,
        },
    }
}

/// The Lock row: Primitives and Strings, each a checkbox with its tooltip.
/// The row is found by its own label or by either lock's full name.
fn lock_row<'a>(form: &Form<'a>, facts: &'a PcbComponentFacts) -> Option<Element<'a, PanelMsg>> {
    let label = form.text(ids::PCB_COMPONENT_LOCK);
    let found = [
        ids::PCB_COMPONENT_LOCK_PRIMITIVES,
        ids::PCB_COMPONENT_LOCK_STRINGS,
    ]
    .into_iter()
    .any(|id| form.shows(&form.text(id)));
    if !form.shows(&label) && !found {
        return None;
    }
    let s = &facts.settings;
    let flag = |value: &Shared<bool>,
                make: fn(bool) -> ComponentSetting,
                short: MessageId,
                tip: MessageId| {
        let toggled = facts
            .is_editable()
            .then(|| picked(facts, make(*value != Shared::Same(true))));
        form.with_tip(
            rows::check_labelled(value, t!(form.ctx.i18n, short), toggled),
            tip,
        )
    };
    let control = Row::new()
        .push(flag(
            &s.lock_primitives,
            ComponentSetting::LockPrimitives,
            ids::PCB_COMPONENT_LOCK_PRIMITIVES_SHORT,
            ids::PCB_COMPONENT_LOCK_PRIMITIVES_TIP,
        ))
        .push(flag(
            &s.lock_strings,
            ComponentSetting::LockStrings,
            ids::PCB_COMPONENT_LOCK_STRINGS_SHORT,
            ids::PCB_COMPONENT_LOCK_STRINGS_TIP,
        ))
        .spacing(SP.l)
        .align_y(iced::Alignment::Center)
        .boxed();
    Some(rows::row(form, label, control))
}

/// Whether both swapping switches are off on every selected component: the
/// Swapping Options header then says "off".
pub(super) fn swapping_off(facts: &PcbComponentFacts) -> bool {
    let s = &facts.settings;
    s.pin_swapping == Shared::Same(false) && s.part_swapping == Shared::Same(false)
}

/// The Swapping Options rows: a labelled checkbox each; a mixed value ticks
/// every component.
pub(super) fn swapping_rows<'a>(
    form: &Form<'a>,
    facts: &'a PcbComponentFacts,
) -> Vec<Element<'a, PanelMsg>> {
    let s = &facts.settings;
    let flag = |label: MessageId, value: &Shared<bool>, make: fn(bool) -> ComponentSetting| {
        if !form.shows(&form.text(label)) {
            return None;
        }
        let toggled = facts
            .is_editable()
            .then(|| picked(facts, make(*value != Shared::Same(true))));
        Some(rows::wide(rows::check_labelled(
            value,
            t!(form.ctx.i18n, label),
            toggled,
        )))
    };
    [
        flag(
            ids::PCB_COMPONENT_PIN_SWAP,
            &s.pin_swapping,
            ComponentSetting::PinSwapping,
        ),
        flag(
            ids::PCB_COMPONENT_PART_SWAP,
            &s.part_swapping,
            ComponentSetting::PartSwapping,
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
}
