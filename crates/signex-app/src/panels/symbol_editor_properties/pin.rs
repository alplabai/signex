//! Pin-selection Properties rows for the symbol editor.

use iced::widget::{Column, column, container, row, text};
use iced::{Color, Element, Length, Widget};

use super::super::{NumericDraft, NumericFieldKey, PanelMsg, SymbolPinSummary, numeric_input};
use crate::styles::{Spacing, TypeScale};

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// IEEE-symbol pick_list row used four times (Inside / Inside Edge /
/// Outside Edge / Outside) on the pin Properties surface. `slot`
/// matches the `SymEditorSetPinSymbol::slot` numbering: 0 / 1 / 2 / 3.
fn view_pin_symbol_picker<'a>(
    label: &str,
    current: signex_library::PinSymbolKind,
    pin_idx: usize,
    slot: u8,
    muted: Color,
) -> Element<'a, PanelMsg> {
    use signex_library::PinSymbolKind as K;
    let options = [
        ("None", K::None),
        ("Dot (active-low bubble)", K::Dot),
        ("Clock edge", K::ClockEdge),
        ("Active-low input", K::ActiveLowInput),
        ("Active-low output", K::ActiveLowOutput),
        ("Schmitt trigger", K::SchmittTrigger),
        ("Analog (≈)", K::Analog),
        ("Digital (square wave)", K::Digital),
        ("Shift-right (▷)", K::ShiftRight),
        ("Shift-left (◁)", K::ShiftLeft),
        ("Pi (π)", K::Pi),
        ("Sigma (Σ)", K::Sigma),
        ("Open collector", K::OpenCollector),
        ("Open emitter", K::OpenEmitter),
        ("Hi-Z (tri-state)", K::HiZ),
    ];
    let selected = options.iter().find(|(_, v)| *v == current).copied();
    let picker = iced::widget::pick_list(
        selected,
        options.to_vec(),
        |(label, _): &(&'static str, K)| label.to_string(),
    )
    .on_select(
        move |(_, value): (&'static str, K)| PanelMsg::SymEditorSetPinSymbol {
            pin_idx,
            slot,
            value,
        },
    )
    .padding([SP.xs, SP.s])
    .text_size(10);
    container(
        row![
            text(label.to_string())
                .size(TS.body)
                .color(muted)
                .width(Length::FillPortion(2)),
            container(picker).width(Length::FillPortion(3)),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center),
    )
    .padding([3.0, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// Pin Properties rows — Designator / Name / Electrical / Position /
/// Orientation / Length / metadata toggles / IEEE symbols.
pub(super) fn view_pin_selection<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    pin: &'a SymbolPinSummary,
    draft: Option<&NumericDraft>,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Column<Element<'a, PanelMsg>> {
    let pin_idx = pin.idx;

    // ── Designator (text) ──
    let designator_row: Element<'a, PanelMsg> = container(
        row![
            text("Designator")
                .size(TS.body)
                .color(muted)
                .width(Length::FillPortion(2)),
            iced::widget::text_input("number", pin.number.as_str())
                .padding([SP.xs, SP.s])
                .size(TS.body)
                .on_input(move |s| PanelMsg::SymEditorSetPinNumber { pin_idx, value: s })
                .width(Length::FillPortion(3)),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center),
    )
    .padding([3.0, SP.m])
    .width(Length::Fill)
    .boxed();
    col = col.push(designator_row);

    // ── Name (text) ──
    let name_row: Element<'a, PanelMsg> = container(
        row![
            text("Name")
                .size(TS.body)
                .color(muted)
                .width(Length::FillPortion(2)),
            iced::widget::text_input("name", pin.name.as_str())
                .padding([SP.xs, SP.s])
                .size(TS.body)
                .on_input(move |s| PanelMsg::SymEditorSetPinName { pin_idx, value: s })
                .width(Length::FillPortion(3)),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center),
    )
    .padding([3.0, SP.m])
    .width(Length::Fill)
    .boxed();
    col = col.push(name_row);

    // ── Electrical Type (pick_list) ──
    use signex_library::PinDirection as D;
    let electrical_options = [
        ("Input", D::Input),
        ("I/O", D::Bidirectional),
        ("Output", D::Output),
        ("Open Collector", D::OpenDrainLow),
        ("Passive", D::Passive),
        ("HiZ", D::ThreeStatable),
        ("Open Emitter", D::OpenDrainHigh),
        ("Power Input", D::PowerInput),
        ("Not Connected", D::DoNotConnect),
        ("Unspecified", D::Unclassified),
        ("Power Output", D::PowerOutput),
        ("Ground", D::GroundReference),
        ("Differential", D::Differential),
        ("Clock", D::Clock),
    ];
    let electrical_selected = electrical_options
        .iter()
        .find(|(_, v)| format!("{:?}", v) == pin.electrical)
        .copied();
    let electrical_picker = iced::widget::pick_list(
        electrical_selected,
        electrical_options.to_vec(),
        |(label, _): &(&'static str, signex_library::PinDirection)| label.to_string(),
    )
    .placeholder(pin.electrical.clone())
    .on_select(
        move |(_, value): (&'static str, signex_library::PinDirection)| {
            PanelMsg::SymEditorSetPinElectrical { pin_idx, value }
        },
    )
    .padding([SP.xs, SP.s])
    .text_size(11);
    let electrical_row: Element<'a, PanelMsg> = container(
        row![
            text("Electrical")
                .size(TS.body)
                .color(muted)
                .width(Length::FillPortion(2)),
            container(electrical_picker).width(Length::FillPortion(3)),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center),
    )
    .padding([3.0, SP.m])
    .width(Length::Fill)
    .boxed();
    col = col.push(electrical_row);

    // ── Position (X, Y) ──
    let pos_x = pin.position[0];
    let pos_y = pin.position[1];
    let pos_x_row: Element<'a, PanelMsg> = container(
        row![
            text("X")
                .size(TS.body)
                .color(muted)
                .width(Length::FillPortion(2)),
            numeric_input("mm", NumericFieldKey::PinX(pin_idx), pos_x, draft)
                .padding([SP.xs, SP.s])
                .size(TS.body),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center),
    )
    .padding([3.0, SP.m])
    .width(Length::Fill)
    .boxed();
    col = col.push(pos_x_row);

    let pos_y_row: Element<'a, PanelMsg> = container(
        row![
            text("Y")
                .size(TS.body)
                .color(muted)
                .width(Length::FillPortion(2)),
            numeric_input("mm", NumericFieldKey::PinY(pin_idx), pos_y, draft)
                .padding([SP.xs, SP.s])
                .size(TS.body),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center),
    )
    .padding([3.0, SP.m])
    .width(Length::Fill)
    .boxed();
    col = col.push(pos_y_row);

    // ── Orientation (pick_list) ──
    let orientation_options = [
        ("Right", signex_library::PinOrientation::Right),
        ("Up", signex_library::PinOrientation::Up),
        ("Left", signex_library::PinOrientation::Left),
        ("Down", signex_library::PinOrientation::Down),
    ];
    let orientation_selected = orientation_options
        .iter()
        .find(|(_, v)| format!("{:?}", v) == pin.orientation)
        .copied();
    let orientation_picker = iced::widget::pick_list(
        orientation_selected,
        orientation_options.to_vec(),
        |(label, _): &(&'static str, signex_library::PinOrientation)| label.to_string(),
    )
    .placeholder(pin.orientation.clone())
    .on_select(
        move |(_, value): (&'static str, signex_library::PinOrientation)| {
            PanelMsg::SymEditorSetPinOrientation { pin_idx, value }
        },
    )
    .padding([SP.xs, SP.s])
    .text_size(11);
    let orientation_row: Element<'a, PanelMsg> = container(
        row![
            text("Orientation")
                .size(TS.body)
                .color(muted)
                .width(Length::FillPortion(2)),
            container(orientation_picker).width(Length::FillPortion(3)),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center),
    )
    .padding([3.0, SP.m])
    .width(Length::Fill)
    .boxed();
    col = col.push(orientation_row);

    // ── Length (numeric) ──
    let length_row: Element<'a, PanelMsg> = container(
        row![
            text("Length")
                .size(TS.body)
                .color(muted)
                .width(Length::FillPortion(2)),
            numeric_input("mm", NumericFieldKey::PinLength(pin_idx), pin.length, draft)
                .padding([SP.xs, SP.s])
                .size(TS.body),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center),
    )
    .padding([3.0, SP.m])
    .width(Length::Fill)
    .boxed();
    col = col.push(length_row);

    // ── Part Number (multi-part components) ──
    let part_now = pin.details.part_number;
    let part_row: Element<'a, PanelMsg> = container(
        row![
            text("Part Number")
                .size(TS.body)
                .color(muted)
                .width(Length::FillPortion(2)),
            iced::widget::text_input("1, or 0 for shared", part_now.to_string())
                .padding([SP.xs, SP.s])
                .size(TS.body)
                .on_input(move |s| {
                    let parsed = s.trim().parse::<u8>().unwrap_or(part_now);
                    PanelMsg::SymEditorSetPinPartNumber {
                        pin_idx,
                        value: parsed,
                    }
                })
                .width(Length::FillPortion(3)),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center),
    )
    .padding([3.0, SP.m])
    .width(Length::Fill)
    .boxed();
    col = col.push(part_row);

    // ── Description (text) ──
    let description_row: Element<'a, PanelMsg> = container(
        row![
            text("Description")
                .size(TS.body)
                .color(muted)
                .width(Length::FillPortion(2)),
            iced::widget::text_input("free text", pin.details.description.as_str())
                .padding([SP.xs, SP.s])
                .size(TS.body)
                .on_input(move |s| PanelMsg::SymEditorSetPinDescription { pin_idx, value: s })
                .width(Length::FillPortion(3)),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center),
    )
    .padding([3.0, SP.m])
    .width(Length::Fill)
    .boxed();
    col = col.push(description_row);

    // ── Function (comma-separated alt-names) ──
    let function_csv = pin.details.function.join(", ");
    let function_row: Element<'a, PanelMsg> = container(
        row![
            text("Function")
                .size(TS.body)
                .color(muted)
                .width(Length::FillPortion(2)),
            iced::widget::text_input("alt-names, comma-separated", function_csv)
                .padding([SP.xs, SP.s])
                .size(TS.body)
                .on_input(move |s| PanelMsg::SymEditorSetPinFunctionCsv { pin_idx, value: s })
                .width(Length::FillPortion(3)),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center),
    )
    .padding([3.0, SP.m])
    .width(Length::Fill)
    .boxed();
    col = col.push(function_row);

    // ── Visibility / state toggles ──
    let toggle_row = |label: &'static str, value: bool, msg: PanelMsg| -> Element<'a, PanelMsg> {
        row![
            iced::widget::checkbox(value)
                .size(TS.heading)
                .on_toggle(move |_| msg.clone())
                .boxed(),
            text(label.to_string()).size(TS.body).color(muted),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center)
        .boxed()
    };
    let toggles_row: Element<'a, PanelMsg> = container(
        column![
            toggle_row(
                "Designator visible",
                pin.details.designator_visible,
                PanelMsg::SymEditorTogglePinDesignatorVisible(pin_idx),
            ),
            toggle_row(
                "Name visible",
                pin.details.name_visible,
                PanelMsg::SymEditorTogglePinNameVisible(pin_idx),
            ),
            toggle_row(
                "Hidden (Pin Hide)",
                pin.details.hidden,
                PanelMsg::SymEditorTogglePinHidden(pin_idx),
            ),
            toggle_row(
                "Locked",
                pin.details.locked,
                PanelMsg::SymEditorTogglePinLocked(pin_idx),
            ),
        ]
        .spacing(SP.xs),
    )
    .padding([6.0, SP.m])
    .width(Length::Fill)
    .boxed();
    col = col.push(toggles_row);

    // ── IEEE Symbols (Inside / Inside Edge / Outside Edge / Outside) ──
    col = col.push(super::super::thin_sep(border_c));
    col = col.push(
        container(text("Symbols").size(TS.caption).color(primary))
            .padding([SP.s, SP.m])
            .boxed(),
    );
    col = col.push(view_pin_symbol_picker(
        "Inside",
        pin.details.inside_symbol,
        pin_idx,
        0,
        muted,
    ));
    col = col.push(view_pin_symbol_picker(
        "Inside Edge",
        pin.details.inside_edge_symbol,
        pin_idx,
        1,
        muted,
    ));
    col = col.push(view_pin_symbol_picker(
        "Outside Edge",
        pin.details.outside_edge_symbol,
        pin_idx,
        2,
        muted,
    ));
    col = col.push(view_pin_symbol_picker(
        "Outside",
        pin.details.outside_symbol,
        pin_idx,
        3,
        muted,
    ));
    col
}
