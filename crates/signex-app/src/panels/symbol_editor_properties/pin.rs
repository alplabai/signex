//! Pin properties: `‹ Pins` back to the list, the header, the pin
//! illustration, and the Pin, Location, Visibility and Symbols sections.

use iced::widget::{column, container, row};
use iced::{Alignment, Element, Length, Widget as _};
use signex_i18n::{ids, t};
use signex_library::{PinDirection, PinOrientation, PinSymbolKind};

use super::super::{NumericFieldKey, PanelMsg, SymbolEditorPanelContext, SymbolPinSummary};
use super::illustration::{self, field};
use super::object_context;
use super::pins_tab::type_tone;
use super::rows::{Choice, Form, SP, props};
use crate::library::editor::pin_labels::{
    PIN_SYMBOLS, direction_by_name, direction_message, electrical_message, orientation_by_name,
    orientation_message, symbol_kind_message,
};
use crate::library::editor::symbol::SymbolPropsMsg;

const PIN: &str = "symprops.pin";
const LOCATION: &str = "symprops.location";
const VISIBILITY: &str = "symprops.visibility";
const SYMBOLS: &str = "symprops.symbols";

/// The orientations in the order the segmented control shows them.
const ORIENTATIONS: [PinOrientation; 4] = [
    PinOrientation::Left,
    PinOrientation::Right,
    PinOrientation::Up,
    PinOrientation::Down,
];

pub(super) fn view<'a>(
    sym: &'a SymbolEditorPanelContext,
    pin: &'a SymbolPinSummary,
    form: &Form<'_>,
) -> Vec<Element<'a, PanelMsg>> {
    let i18n = form.i18n;
    let back = container(form.ghost(
        t!(i18n, ids::EDITORS_SYMPROPS_BACK_TO_PINS),
        Some(props(SymbolPropsMsg::PinsBackPressed)),
        Some(t!(i18n, ids::EDITORS_SYMPROPS_BACK_TO_PINS_TIP)),
    ))
    .padding(iced::Padding {
        top: SP.s,
        right: SP.s,
        bottom: 0.0,
        left: SP.s,
    });
    let electrical: signex_widgets::Label<'a> = match electrical_message(&pin.electrical) {
        Some(id) => t!(i18n, id).into(),
        None => signex_i18n::UserText::new(pin.electrical.as_str()).into(),
    };
    let mut col = vec![
        back.boxed(),
        form.header(
            t!(i18n, ids::EDITORS_SYMPROPS_PIN_TITLE, "number" => pin.number.as_str()).into(),
            Some((electrical, type_tone(direction_by_name(&pin.electrical)))),
            Some((
                object_context(sym, pin.details.part_number, i18n).into(),
                false,
            )),
        ),
        illustration::view(illustration::pin(pin, i18n), form),
    ];
    col.push(form.section(
        PIN,
        t!(i18n, ids::EDITORS_SYMPROPS_SECTION_PIN),
        true,
        None,
        || pin_rows(pin, form),
    ));
    col.push(form.section(
        LOCATION,
        t!(i18n, ids::EDITORS_SYMPROPS_SECTION_LOCATION),
        true,
        None,
        || location_rows(pin, form),
    ));
    col.push(form.section(
        VISIBILITY,
        t!(i18n, ids::EDITORS_SYMPROPS_SECTION_VISIBILITY),
        true,
        None,
        || vec![visibility_grid(pin, form)],
    ));
    col.push(form.section(
        SYMBOLS,
        t!(i18n, ids::EDITORS_SYMPROPS_SECTION_SYMBOLS),
        true,
        None,
        || symbol_rows(pin, form),
    ));
    col
}

fn pin_rows<'a>(pin: &SymbolPinSummary, form: &Form<'_>) -> Vec<Element<'a, PanelMsg>> {
    let i18n = form.i18n;
    let pin_idx = pin.idx;
    let part_now = pin.details.part_number;
    let electrical: Vec<Choice<PinDirection>> = PinDirection::ALL
        .into_iter()
        .map(|d| Choice::new(d, t!(i18n, direction_message(d))))
        .collect();
    let part = row![
        form.input(
            t!(i18n, ids::EDITORS_SYMPROPS_PART_NUMBER),
            part_now.to_string(),
            true,
            move |s| PanelMsg::SymEditorSetPinPartNumber {
                pin_idx,
                value: s.trim().parse::<u8>().unwrap_or(part_now),
            },
        ),
        signex_widgets::text(t!(i18n, ids::EDITORS_SYMPROPS_PART_NUMBER_UNIT))
            .size(super::rows::TS.caption)
            .color(form.look.ink4)
            .wrapping(iced::widget::text::Wrapping::None),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center);
    vec![
        form.field(
            t!(i18n, ids::EDITORS_SYMPROPS_DESIGNATOR_NAME),
            form.pair(
                form.input(
                    t!(i18n, ids::EDITORS_SYMPROPS_DESIGNATOR_PLACEHOLDER),
                    pin.number.clone(),
                    true,
                    move |value| PanelMsg::SymEditorSetPinNumber { pin_idx, value },
                ),
                form.input(
                    t!(i18n, ids::EDITORS_SYMPROPS_NAME_PLACEHOLDER),
                    pin.name.clone(),
                    true,
                    move |value| PanelMsg::SymEditorSetPinName { pin_idx, value },
                ),
            ),
            Some(field::PIN_DN),
        ),
        form.field(
            t!(i18n, ids::EDITORS_SYMPROPS_ELECTRICAL),
            form.dropdown(
                electrical,
                direction_by_name(&pin.electrical),
                move |value| PanelMsg::SymEditorSetPinElectrical { pin_idx, value },
            ),
            Some(field::PIN_ETYPE),
        ),
        form.field(
            t!(i18n, ids::EDITORS_SYMPROPS_DESCRIPTION),
            form.input(
                t!(i18n, ids::EDITORS_SYMPROPS_DESCRIPTION_PLACEHOLDER),
                pin.details.description.clone(),
                false,
                move |value| PanelMsg::SymEditorSetPinDescription { pin_idx, value },
            ),
            None,
        ),
        form.field(
            t!(i18n, ids::EDITORS_SYMPROPS_FUNCTION),
            form.input(
                t!(i18n, ids::EDITORS_SYMPROPS_FUNCTION_PLACEHOLDER),
                pin.details.function.join(", "),
                false,
                move |value| PanelMsg::SymEditorSetPinFunctionCsv { pin_idx, value },
            ),
            None,
        ),
        form.field(
            t!(i18n, ids::EDITORS_SYMPROPS_PART_NUMBER),
            part.boxed(),
            None,
        ),
    ]
}

fn location_rows<'a>(pin: &SymbolPinSummary, form: &Form<'_>) -> Vec<Element<'a, PanelMsg>> {
    let i18n = form.i18n;
    let pin_idx = pin.idx;
    let mm = || Some(t!(i18n, ids::EDITORS_SYMPROPS_UNIT_MM));
    let current = orientation_by_name(&pin.orientation);
    let orientation = ORIENTATIONS
        .into_iter()
        .map(|o| {
            (
                t!(i18n, orientation_message(o)),
                current == Some(o),
                PanelMsg::SymEditorSetPinOrientation { pin_idx, value: o },
            )
        })
        .collect();
    vec![
        form.field(
            t!(i18n, ids::EDITORS_SYMPROPS_X_Y),
            form.pair(
                form.number(NumericFieldKey::PinX(pin_idx), pin.position[0], mm()),
                form.number(NumericFieldKey::PinY(pin_idx), pin.position[1], mm()),
            ),
            Some(field::PIN_XY),
        ),
        form.field(
            t!(i18n, ids::EDITORS_SYMPROPS_LENGTH),
            form.pair(
                form.number(NumericFieldKey::PinLength(pin_idx), pin.length, mm()),
                iced::widget::Space::new().boxed(),
            ),
            Some(field::PIN_LEN),
        ),
        form.field(
            t!(i18n, ids::EDITORS_SYMPROPS_ORIENTATION),
            form.segmented(orientation),
            Some(field::PIN_ORIENT),
        ),
    ]
}

/// Name visible, Designator visible, Hidden and Locked, two to a row.
fn visibility_grid<'a>(pin: &SymbolPinSummary, form: &Form<'_>) -> Element<'a, PanelMsg> {
    let i18n = form.i18n;
    let d = &pin.details;
    let i = pin.idx;
    let half = |e: Element<'a, PanelMsg>| container(e).width(Length::FillPortion(1));
    container(
        column![
            row![
                half(form.check(
                    t!(i18n, ids::EDITORS_SYMPROPS_NAME_VISIBLE),
                    d.name_visible,
                    PanelMsg::SymEditorTogglePinNameVisible(i),
                )),
                half(form.check(
                    t!(i18n, ids::EDITORS_SYMPROPS_DESIGNATOR_VISIBLE),
                    d.designator_visible,
                    PanelMsg::SymEditorTogglePinDesignatorVisible(i),
                )),
            ]
            .spacing(SP.m),
            row![
                half(form.check(
                    t!(i18n, ids::EDITORS_SYMPROPS_HIDDEN),
                    d.hidden,
                    PanelMsg::SymEditorTogglePinHidden(i),
                )),
                half(form.check(
                    t!(i18n, ids::EDITORS_SYMPROPS_LOCKED),
                    d.locked,
                    PanelMsg::SymEditorTogglePinLocked(i),
                )),
            ]
            .spacing(SP.m),
        ]
        .spacing(SP.xs),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// The four symbol slots, each a dropdown over every pin symbol. `slot`
/// numbers match `PanelMsg::SymEditorSetPinSymbol`.
fn symbol_rows<'a>(pin: &SymbolPinSummary, form: &Form<'_>) -> Vec<Element<'a, PanelMsg>> {
    let i18n = form.i18n;
    let d = &pin.details;
    let pin_idx = pin.idx;
    let slot_row =
        |label: signex_i18n::MessageId, slot: u8, current: PinSymbolKind, id: &'static str| {
            let choices: Vec<Choice<PinSymbolKind>> = PIN_SYMBOLS
                .into_iter()
                .map(|k| Choice::new(k, t!(i18n, symbol_kind_message(k))))
                .collect();
            form.field(
                t!(i18n, label),
                form.dropdown(choices, Some(current), move |value| {
                    PanelMsg::SymEditorSetPinSymbol {
                        pin_idx,
                        slot,
                        value,
                    }
                }),
                Some(id),
            )
        };
    vec![
        slot_row(
            ids::EDITORS_SYMPROPS_INSIDE,
            0,
            d.inside_symbol,
            field::PIN_INSIDE,
        ),
        slot_row(
            ids::EDITORS_SYMPROPS_INSIDE_EDGE,
            1,
            d.inside_edge_symbol,
            field::PIN_IN_EDGE,
        ),
        slot_row(
            ids::EDITORS_SYMPROPS_OUTSIDE_EDGE,
            2,
            d.outside_edge_symbol,
            field::PIN_OUT_EDGE,
        ),
        slot_row(
            ids::EDITORS_SYMPROPS_OUTSIDE,
            3,
            d.outside_symbol,
            field::PIN_OUTSIDE,
        ),
    ]
}
