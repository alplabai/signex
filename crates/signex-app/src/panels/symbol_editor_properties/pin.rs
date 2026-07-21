//! Pin-selection Properties rows for the symbol editor.

use iced::widget::{Column, column, container, row, text};
use iced::{Color, Element, Length};

use super::super::{PanelMsg, SymbolPinSummary};

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
    let labels: Vec<String> = options.iter().map(|(l, _)| l.to_string()).collect();
    let lookup: Vec<(String, K)> = options.iter().map(|(l, v)| (l.to_string(), *v)).collect();
    let current_label = options
        .iter()
        .find(|(_, v)| *v == current)
        .map(|(l, _)| l.to_string())
        .unwrap_or_else(|| "None".to_string());
    let picker = iced::widget::pick_list(labels, Some(current_label), move |chosen: String| {
        let value = lookup
            .iter()
            .find(|(l, _)| l == &chosen)
            .map(|(_, v)| *v)
            .unwrap_or(K::None);
        PanelMsg::SymEditorSetPinSymbol {
            pin_idx,
            slot,
            value,
        }
    })
    .padding([2, 4])
    .text_size(10);
    container(
        row![
            text(label.to_string())
                .size(10)
                .color(muted)
                .width(Length::FillPortion(2)),
            container(picker).width(Length::FillPortion(3)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .into()
}

/// Pin Properties rows — Designator / Name / Electrical / Position /
/// Orientation / Length / metadata toggles / IEEE symbols.
pub(super) fn view_pin_selection<'a>(
    mut col: Column<'a, PanelMsg>,
    pin: &'a SymbolPinSummary,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Column<'a, PanelMsg> {
    let pin_idx = pin.idx;

    // ── Designator (text) ──
    let designator_row: Element<'a, PanelMsg> = container(
        row![
            text("Designator")
                .size(10)
                .color(muted)
                .width(Length::FillPortion(2)),
            iced::widget::text_input("number", pin.number.as_str())
                .padding([2, 4])
                .size(11)
                .on_input(move |s| PanelMsg::SymEditorSetPinNumber { pin_idx, value: s })
                .width(Length::FillPortion(3)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .into();
    col = col.push(designator_row);

    // ── Name (text) ──
    let name_row: Element<'a, PanelMsg> = container(
        row![
            text("Name")
                .size(10)
                .color(muted)
                .width(Length::FillPortion(2)),
            iced::widget::text_input("name", pin.name.as_str())
                .padding([2, 4])
                .size(11)
                .on_input(move |s| PanelMsg::SymEditorSetPinName { pin_idx, value: s })
                .width(Length::FillPortion(3)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .into();
    col = col.push(name_row);

    // ── Electrical Type (pick_list) ──
    let electrical_options = [
        ("Input", signex_library::PinDirection::Input),
        ("I/O", signex_library::PinDirection::Bidirectional),
        ("Output", signex_library::PinDirection::Output),
        (
            "Open Collector",
            signex_library::PinDirection::OpenCollector,
        ),
        ("Passive", signex_library::PinDirection::Passive),
        ("HiZ", signex_library::PinDirection::Tristate),
        ("Open Emitter", signex_library::PinDirection::OpenEmitter),
        ("Power", signex_library::PinDirection::Power),
        ("Not Connected", signex_library::PinDirection::NotConnected),
        ("Unspecified", signex_library::PinDirection::Unspecified),
    ];
    let current_label = electrical_options
        .iter()
        .find(|(_, v)| format!("{:?}", v) == pin.electrical)
        .map(|(label, _)| label.to_string())
        .unwrap_or_else(|| pin.electrical.clone());
    let labels: Vec<String> = electrical_options
        .iter()
        .map(|(label, _)| label.to_string())
        .collect();
    let labels_for_msg: Vec<(String, signex_library::PinDirection)> = electrical_options
        .iter()
        .map(|(label, v)| (label.to_string(), *v))
        .collect();
    let electrical_picker =
        iced::widget::pick_list(labels, Some(current_label), move |chosen: String| {
            let value = labels_for_msg
                .iter()
                .find(|(label, _)| label == &chosen)
                .map(|(_, v)| *v)
                .unwrap_or(signex_library::PinDirection::Unspecified);
            PanelMsg::SymEditorSetPinElectrical { pin_idx, value }
        })
        .padding([2, 4])
        .text_size(11);
    let electrical_row: Element<'a, PanelMsg> = container(
        row![
            text("Electrical")
                .size(10)
                .color(muted)
                .width(Length::FillPortion(2)),
            container(electrical_picker).width(Length::FillPortion(3)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .into();
    col = col.push(electrical_row);

    // ── Position (X, Y) ──
    let pos_x = pin.position[0];
    let pos_y = pin.position[1];
    let pos_x_row: Element<'a, PanelMsg> = container(
        row![
            text("X")
                .size(10)
                .color(muted)
                .width(Length::FillPortion(2)),
            iced::widget::text_input("mm", &format!("{:.3}", pos_x))
                .padding([2, 4])
                .size(11)
                .on_input(move |s| {
                    let parsed = s.trim().parse::<f64>().unwrap_or(pos_x);
                    PanelMsg::SymEditorSetPinX {
                        pin_idx,
                        value: parsed,
                    }
                })
                .width(Length::FillPortion(3)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .into();
    col = col.push(pos_x_row);

    let pos_y_row: Element<'a, PanelMsg> = container(
        row![
            text("Y")
                .size(10)
                .color(muted)
                .width(Length::FillPortion(2)),
            iced::widget::text_input("mm", &format!("{:.3}", pos_y))
                .padding([2, 4])
                .size(11)
                .on_input(move |s| {
                    let parsed = s.trim().parse::<f64>().unwrap_or(pos_y);
                    PanelMsg::SymEditorSetPinY {
                        pin_idx,
                        value: parsed,
                    }
                })
                .width(Length::FillPortion(3)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .into();
    col = col.push(pos_y_row);

    // ── Orientation (pick_list) ──
    let orientation_options = [
        ("Right", signex_library::PinOrientation::Right),
        ("Up", signex_library::PinOrientation::Up),
        ("Left", signex_library::PinOrientation::Left),
        ("Down", signex_library::PinOrientation::Down),
    ];
    let current_orient = orientation_options
        .iter()
        .find(|(_, v)| format!("{:?}", v) == pin.orientation)
        .map(|(label, _)| label.to_string())
        .unwrap_or_else(|| pin.orientation.clone());
    let orient_labels: Vec<String> = orientation_options
        .iter()
        .map(|(label, _)| label.to_string())
        .collect();
    let orient_msg_lookup: Vec<(String, signex_library::PinOrientation)> = orientation_options
        .iter()
        .map(|(label, v)| (label.to_string(), *v))
        .collect();
    let orientation_picker = iced::widget::pick_list(
        orient_labels,
        Some(current_orient),
        move |chosen: String| {
            let value = orient_msg_lookup
                .iter()
                .find(|(label, _)| label == &chosen)
                .map(|(_, v)| *v)
                .unwrap_or(signex_library::PinOrientation::Right);
            PanelMsg::SymEditorSetPinOrientation { pin_idx, value }
        },
    )
    .padding([2, 4])
    .text_size(11);
    let orientation_row: Element<'a, PanelMsg> = container(
        row![
            text("Orientation")
                .size(10)
                .color(muted)
                .width(Length::FillPortion(2)),
            container(orientation_picker).width(Length::FillPortion(3)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .into();
    col = col.push(orientation_row);

    // ── Length (numeric) ──
    let length_row: Element<'a, PanelMsg> = container(
        row![
            text("Length")
                .size(10)
                .color(muted)
                .width(Length::FillPortion(2)),
            iced::widget::text_input("mm", &format!("{:.3}", pin.length))
                .padding([2, 4])
                .size(11)
                .on_input(move |s| {
                    let parsed = s.trim().parse::<f64>().unwrap_or(0.0);
                    PanelMsg::SymEditorSetPinLength {
                        pin_idx,
                        value: parsed,
                    }
                })
                .width(Length::FillPortion(3)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .into();
    col = col.push(length_row);

    // ── Part Number (multi-part components) ──
    let part_now = pin.details.part_number;
    let part_row: Element<'a, PanelMsg> = container(
        row![
            text("Part Number")
                .size(10)
                .color(muted)
                .width(Length::FillPortion(2)),
            iced::widget::text_input("1, or 0 for shared", &part_now.to_string(),)
                .padding([2, 4])
                .size(11)
                .on_input(move |s| {
                    let parsed = s.trim().parse::<u8>().unwrap_or(part_now);
                    PanelMsg::SymEditorSetPinPartNumber {
                        pin_idx,
                        value: parsed,
                    }
                })
                .width(Length::FillPortion(3)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .into();
    col = col.push(part_row);

    // ── Description (text) ──
    let description_row: Element<'a, PanelMsg> = container(
        row![
            text("Description")
                .size(10)
                .color(muted)
                .width(Length::FillPortion(2)),
            iced::widget::text_input("free text", pin.details.description.as_str())
                .padding([2, 4])
                .size(11)
                .on_input(move |s| PanelMsg::SymEditorSetPinDescription { pin_idx, value: s })
                .width(Length::FillPortion(3)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .into();
    col = col.push(description_row);

    // ── Function (comma-separated alt-names) ──
    let function_csv = pin.details.function.join(", ");
    let function_row: Element<'a, PanelMsg> = container(
        row![
            text("Function")
                .size(10)
                .color(muted)
                .width(Length::FillPortion(2)),
            iced::widget::text_input("alt-names, comma-separated", &function_csv)
                .padding([2, 4])
                .size(11)
                .on_input(move |s| PanelMsg::SymEditorSetPinFunctionCsv { pin_idx, value: s })
                .width(Length::FillPortion(3)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .into();
    col = col.push(function_row);

    // ── Visibility / state toggles ──
    let toggle_row = |label: &'static str, value: bool, msg: PanelMsg| -> Element<'a, PanelMsg> {
        row![
            iced::widget::checkbox(value)
                .size(14)
                .on_toggle(move |_| msg.clone()),
            text(label.to_string()).size(10).color(muted),
        ]
        .spacing(6)
        .align_y(iced::Alignment::Center)
        .into()
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
        .spacing(2),
    )
    .padding([6, 8])
    .width(Length::Fill)
    .into();
    col = col.push(toggles_row);

    // ── IEEE Symbols (Inside / Inside Edge / Outside Edge / Outside) ──
    col = col.push(super::super::thin_sep(border_c));
    col = col.push(container(text("Symbols").size(10).color(primary)).padding([4, 8]));
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
