//! Properties panel for the active `.snxsym` standalone editor tab (HI-22 / MD-20).
//!
//! Extracted from `panels/mod.rs`. Pure view code, zero behaviour change.
//! Mirrors Altium SchLib's right-dock Properties: pin selected → pin
//! properties (editable Designator / Name / Length, read-only
//! Electrical / Position / Orientation), graphic selected → per-shape
//! numeric fields, field selected → field properties, nothing selected
//! → symbol-level defaults (Name / UUID / pin count) with Name editable.

use iced::widget::{Column, container, row, scrollable, text};
use iced::{Color, Element, Length};

use super::{GraphicKindSummary, PanelMsg, SymbolEditorPanelContext, SymbolEditorSelection};

mod graphic;
mod pin;
mod symbol;

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
            GraphicKindSummary::Polygon { .. } => "Graphic — Polygon",
        },
    };
    col = col.push(
        container(text(header_label).size(11).color(primary))
            .padding([6, 8])
            .width(Length::Fill),
    );
    col = col.push(super::thin_sep(border_c));

    match &sym.selected {
        SymbolEditorSelection::None => {
            col = symbol::view_symbol_selection(col, sym, muted, primary, border_c);
        }
        SymbolEditorSelection::Pin(pin) => {
            col = pin::view_pin_selection(col, pin, muted, primary, border_c);
        }
        SymbolEditorSelection::Graphic(g) => {
            col = graphic::view_graphic_selection(col, g, muted, border_c, sym.graphic_fill_picker);
        }
        SymbolEditorSelection::FieldReference => {
            col = col.push(prop_row_static(
                "Field",
                "Reference (designator)".to_string(),
                muted,
                primary,
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
            col = col.push(prop_row_static(
                "Field",
                "Value".to_string(),
                muted,
                primary,
            ));
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

/// Read-only label + value Properties row shared by the symbol-level
/// and field selections.
fn prop_row_static<'a>(
    label: &str,
    value: String,
    muted: Color,
    primary: Color,
) -> Element<'a, PanelMsg> {
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
    .into()
}
