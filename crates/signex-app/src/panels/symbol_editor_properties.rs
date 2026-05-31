//! Properties panel for the active `.snxsym` standalone editor tab (HI-22 / MD-20).
//!
//! Extracted from `panels/mod.rs`. Pure view code, zero behaviour change.
//! Mirrors Altium SchLib's right-dock Properties: pin selected → pin
//! properties (editable Designator / Name / Length, read-only
//! Electrical / Position / Orientation), graphic selected → per-shape
//! numeric fields, field selected → field properties, nothing selected
//! → symbol-level defaults (Name / UUID / pin count) with Name editable.

use iced::widget::{Column, Space, column, container, row, scrollable, text};
use iced::{Color, Element, Length};

use super::{
    GraphicFieldId, GraphicKindSummary, PanelMsg, SymbolEditorPanelContext, SymbolEditorSelection,
};

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

/// Render one Local Colors row — three click-to-cycle swatches
/// (Fills / Lines / Pins). Each swatch shows the current override
/// colour or a striped "inherit" pattern when `None`. Clicking
/// cycles through a small preset palette + back to None.
fn local_colors_row<'a>(
    label: &'a str,
    fill: Option<[u8; 4]>,
    line: Option<[u8; 4]>,
    pin: Option<[u8; 4]>,
    muted: Color,
) -> Element<'a, PanelMsg> {
    let swatch = |slot_label: &'a str, c: Option<[u8; 4]>, msg: PanelMsg| {
        let bg = match c {
            Some([r, g, b, a]) => iced::Color::from_rgba8(r, g, b, (a as f32) / 255.0),
            None => iced::Color::from_rgba(0.5, 0.5, 0.5, 0.25),
        };
        let border = if c.is_some() {
            iced::Color::from_rgba(0.0, 0.0, 0.0, 0.35)
        } else {
            iced::Color::from_rgba(1.0, 1.0, 1.0, 0.30)
        };
        column![
            text(slot_label).size(9).color(muted),
            iced::widget::button(iced::widget::Space::new())
                .padding(0)
                .width(Length::Fixed(28.0))
                .height(Length::Fixed(16.0))
                .on_press(msg)
                .style(
                    move |_: &iced::Theme, _status: iced::widget::button::Status| {
                        iced::widget::button::Style {
                            background: Some(iced::Background::Color(bg)),
                            border: iced::Border {
                                width: 1.0,
                                radius: 2.0.into(),
                                color: border,
                            },
                            ..iced::widget::button::Style::default()
                        }
                    }
                ),
        ]
        .spacing(2)
        .align_x(iced::Alignment::Center)
    };

    container(
        row![
            text(label.to_string())
                .size(10)
                .color(muted)
                .width(Length::FillPortion(2)),
            row![
                swatch("Fills", fill, PanelMsg::SymEditorCycleLocalFillColor),
                Space::new().width(8),
                swatch("Lines", line, PanelMsg::SymEditorCycleLocalLineColor),
                Space::new().width(8),
                swatch("Pins", pin, PanelMsg::SymEditorCycleLocalPinColor),
            ]
            .width(Length::FillPortion(3)),
        ]
        .spacing(4)
        .align_y(iced::Alignment::Center),
    )
    .padding([3, 8])
    .width(Length::Fill)
    .into()
}

/// Properties panel content for the active `.snxsym` standalone editor
/// tab. Mirrors Altium SchLib's right-dock Properties.
pub(super) fn view_symbol_editor_properties<'a>(
    sym: &'a SymbolEditorPanelContext,
    muted: Color,
    primary: Color,
    border_c: Color,
) -> Element<'a, PanelMsg> {
    let mut col: Column<'a, PanelMsg> = Column::new().spacing(0).width(Length::Fill);

    let header_label = match &sym.selected {
        SymbolEditorSelection::None => "Symbol",
        SymbolEditorSelection::Pin(_) => "Pin",
        SymbolEditorSelection::FieldReference => "Field — Reference",
        SymbolEditorSelection::FieldValue => "Field — Value",
        SymbolEditorSelection::Graphic(g) => match &g.kind {
            GraphicKindSummary::Rectangle { .. } => "Graphic — Rectangle",
            GraphicKindSummary::Line { .. } => "Graphic — Line",
            GraphicKindSummary::Circle { .. } => "Graphic — Circle",
            GraphicKindSummary::Arc { .. } => "Graphic — Arc",
            GraphicKindSummary::Text { .. } => "Graphic — Text",
        },
    };
    col = col.push(
        container(text(header_label).size(11).color(primary))
            .padding([6, 8])
            .width(Length::Fill),
    );
    col = col.push(super::thin_sep(border_c));

    let prop_row_static = |label: &str, value: String| {
        container(
            row![
                text(label.to_string())
                    .size(10)
                    .color(muted)
                    .width(Length::FillPortion(2)),
                text(value)
                    .size(10)
                    .color(primary)
                    .width(Length::FillPortion(3)),
            ]
            .spacing(4),
        )
        .padding([3, 8])
        .width(Length::Fill)
    };

    match &sym.selected {
        SymbolEditorSelection::None => {
            // The .snxsym editor is a SYMBOL editor — symbol-level
            // visual / geometric properties only. Component metadata
            // (Designator / Comment / Description / Type / Parameters)
            // lives on the host ComponentRow in the component library
            // and is edited from the Library Browser / Component
            // Editor, not here.

            // Helper — labelled text_input row.
            let text_field = |label: &'a str,
                              value: &'a str,
                              placeholder: &'a str,
                              on_input: fn(String) -> PanelMsg|
             -> Element<'a, PanelMsg> {
                container(
                    row![
                        text(label)
                            .size(10)
                            .color(muted)
                            .width(Length::FillPortion(2)),
                        iced::widget::text_input(placeholder, value)
                            .padding([2, 4])
                            .size(11)
                            .on_input(on_input)
                            .width(Length::FillPortion(3)),
                    ]
                    .spacing(4)
                    .align_y(iced::Alignment::Center),
                )
                .padding([3, 8])
                .width(Length::Fill)
                .into()
            };

            // ── ▾ Symbol ──
            col = col.push(super::thin_sep(border_c));
            col = col.push(container(text("Symbol").size(11).color(primary)).padding([6, 8]));
            col = col.push(text_field(
                "Design Item ID",
                sym.symbol_name.as_str(),
                "Symbol name",
                PanelMsg::SymEditorSetSymbolName,
            ));
            col = col.push(prop_row_static("UUID", sym.symbol_uuid.to_string()));
            col = col.push(prop_row_static("Pins", sym.pins.len().to_string()));
            col = col.push(prop_row_static("Graphics", sym.graphics.len().to_string()));

            // Part of Parts (Altium "Part B / of Parts: 2") — surfaces
            // the multi-part picker the user already drives via the
            // toolbar arrows or the SCH Library tree-expander.
            let part_label = if sym.active_max_part > 1 {
                format!(
                    "Part {} / of Parts {}",
                    sym.active_part, sym.active_max_part
                )
            } else {
                "Single-part".to_string()
            };
            col = col.push(prop_row_static("Part", part_label));

            // ── ▾ Graphical ──
            col = col.push(super::thin_sep(border_c));
            col = col.push(container(text("Graphical").size(11).color(primary)).padding([6, 8]));
            let mirrored_row: Element<'a, PanelMsg> = container(
                row![
                    text("Mirrored")
                        .size(10)
                        .color(muted)
                        .width(Length::FillPortion(2)),
                    iced::widget::checkbox(sym.symbol_mirrored)
                        .size(14)
                        .on_toggle(|_| PanelMsg::SymEditorToggleSymbolMirrored),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
            .padding([3, 8])
            .width(Length::Fill)
            .into();
            col = col.push(mirrored_row);

            // ── Local Colors (Fills / Lines / Pins) ──
            col = col.push(local_colors_row(
                "Local Colors",
                sym.symbol_local_fill_color,
                sym.symbol_local_line_color,
                sym.symbol_local_pin_color,
                muted,
            ));
        }
        SymbolEditorSelection::Pin(pin) => {
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
            let orient_msg_lookup: Vec<(String, signex_library::PinOrientation)> =
                orientation_options
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
                        .on_input(move |s| PanelMsg::SymEditorSetPinDescription {
                            pin_idx,
                            value: s,
                        })
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
                        .on_input(move |s| PanelMsg::SymEditorSetPinFunctionCsv {
                            pin_idx,
                            value: s,
                        })
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
            let toggle_row =
                |label: &'static str, value: bool, msg: PanelMsg| -> Element<'a, PanelMsg> {
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
            col = col.push(super::thin_sep(border_c));
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
        }
        SymbolEditorSelection::Graphic(g) => {
            let g_idx = g.idx;
            // Per-shape numeric fields.
            let num_field =
                |label: &'static str, field: GraphicFieldId, value: f64| -> Element<'a, PanelMsg> {
                    container(
                        row![
                            text(label.to_string())
                                .size(10)
                                .color(muted)
                                .width(Length::FillPortion(2)),
                            iced::widget::text_input("mm", &format!("{:.3}", value))
                                .padding([2, 4])
                                .size(11)
                                .on_input(move |s| {
                                    let parsed = s.trim().parse::<f64>().unwrap_or(value);
                                    PanelMsg::SymEditorSetGraphicField {
                                        idx: g_idx,
                                        field,
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
                    .into()
                };

            match &g.kind {
                GraphicKindSummary::Rectangle { from, to } => {
                    col = col.push(num_field("From X", GraphicFieldId::FromX, from[0]));
                    col = col.push(num_field("From Y", GraphicFieldId::FromY, from[1]));
                    col = col.push(num_field("To X", GraphicFieldId::ToX, to[0]));
                    col = col.push(num_field("To Y", GraphicFieldId::ToY, to[1]));
                }
                GraphicKindSummary::Line { from, to } => {
                    col = col.push(num_field("Start X", GraphicFieldId::FromX, from[0]));
                    col = col.push(num_field("Start Y", GraphicFieldId::FromY, from[1]));
                    col = col.push(num_field("End X", GraphicFieldId::ToX, to[0]));
                    col = col.push(num_field("End Y", GraphicFieldId::ToY, to[1]));
                }
                GraphicKindSummary::Circle { center, radius } => {
                    col = col.push(num_field("Center X", GraphicFieldId::CenterX, center[0]));
                    col = col.push(num_field("Center Y", GraphicFieldId::CenterY, center[1]));
                    col = col.push(num_field("Radius", GraphicFieldId::Radius, *radius));
                }
                GraphicKindSummary::Arc {
                    center,
                    radius,
                    start_deg,
                    end_deg,
                } => {
                    col = col.push(num_field("Center X", GraphicFieldId::CenterX, center[0]));
                    col = col.push(num_field("Center Y", GraphicFieldId::CenterY, center[1]));
                    col = col.push(num_field("Radius", GraphicFieldId::Radius, *radius));
                    col = col.push(num_field(
                        "Start \u{00B0}",
                        GraphicFieldId::StartDeg,
                        *start_deg,
                    ));
                    col = col.push(num_field("End \u{00B0}", GraphicFieldId::EndDeg, *end_deg));
                }
                GraphicKindSummary::Text {
                    position,
                    content,
                    size: text_size,
                } => {
                    col = col.push(num_field("X", GraphicFieldId::PositionX, position[0]));
                    col = col.push(num_field("Y", GraphicFieldId::PositionY, position[1]));
                    col = col.push(num_field("Size", GraphicFieldId::TextSize, *text_size));
                    let content_row: Element<'a, PanelMsg> = container(
                        row![
                            text("Content")
                                .size(10)
                                .color(muted)
                                .width(Length::FillPortion(2)),
                            iced::widget::text_input("text", content.as_str())
                                .padding([2, 4])
                                .size(11)
                                .on_input(move |s| PanelMsg::SymEditorSetGraphicText {
                                    idx: g_idx,
                                    value: s,
                                })
                                .width(Length::FillPortion(3)),
                        ]
                        .spacing(4)
                        .align_y(iced::Alignment::Center),
                    )
                    .padding([3, 8])
                    .width(Length::Fill)
                    .into();
                    col = col.push(content_row);
                }
            }
            // Stroke width — common to every variant.
            col = col.push(num_field(
                "Stroke (mm)",
                GraphicFieldId::StrokeWidth,
                g.stroke_width,
            ));
        }
        SymbolEditorSelection::FieldReference => {
            col = col.push(prop_row_static(
                "Field",
                "Reference (designator)".to_string(),
            ));
            col = col.push(
                container(
                    text("Bound to the host Component's designator at place-time.")
                        .size(10)
                        .color(muted),
                )
                .padding([4, 8]),
            );
        }
        SymbolEditorSelection::FieldValue => {
            col = col.push(prop_row_static("Field", "Value".to_string()));
            col = col.push(
                container(
                    text("Bound to the host Component's value at place-time.")
                        .size(10)
                        .color(muted),
                )
                .padding([4, 8]),
            );
        }
    }

    col = col.push(super::thin_sep(border_c));
    col = col.push(
        container(
            text(
                "Click on the canvas to select. Drawing tools (rectangle, line, arc) land in v0.9 phase 3c.",
            )
            .size(10)
            .color(muted),
        )
        .padding([6, 8]),
    );

    scrollable(col).width(Length::Fill).into()
}
