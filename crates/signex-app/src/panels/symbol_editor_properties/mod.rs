//! Properties panel for the active `.snxsym` standalone editor tab (HI-22 / MD-20).
//!
//! Extracted from `panels/mod.rs`. Pure view code, zero behaviour change.
//! Mirrors Altium SchLib's right-dock Properties: pin selected → pin
//! properties (editable Designator / Name / Length, read-only
//! Electrical / Position / Orientation), graphic selected → per-shape
//! numeric fields, field selected → field properties, nothing selected
//! → symbol-level defaults (Name / UUID / pin count) with Name editable.

use iced::widget::{Column, Widget, container, row, scrollable};
use iced::{Color, Element, Length};
use signex_widgets::{raw, text};

use super::{
    GraphicKindSummary, NumericDraft, PanelMsg, SymbolEditorPanelContext, SymbolEditorSelection,
};
use crate::styles::{Spacing, TypeScale};
use signex_types::theme::ThemeTokens;

/// Default type / spacing scales; equal to the old literals at the default size.
const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

use super::form_look;

mod graphic;
mod pin;
mod symbol;

/// Properties panel content for the active `.snxsym` standalone editor
/// tab. Mirrors Altium SchLib's right-dock Properties.
pub(super) fn view_symbol_editor_properties<'a>(
    sym: &'a SymbolEditorPanelContext,
    draft: Option<&NumericDraft>,
    muted: Color,
    primary: Color,
    border_c: Color,
    tokens: &ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> Element<'a, PanelMsg> {
    let mut col: Column<Element<'a, PanelMsg>> = Column::new().spacing(0).width(Length::Fill);

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
    // The symbol-level form opens on its own "Symbol" section label, so the
    // generic header would only repeat it.
    if !matches!(sym.selected, SymbolEditorSelection::None) {
        col = col.push(section_title(header_label, tokens, i18n));
    }

    match &sym.selected {
        SymbolEditorSelection::None => {
            col = symbol::view_symbol_selection(col, sym, muted, primary, border_c, tokens, i18n);
        }
        SymbolEditorSelection::Pin(pin) => {
            col = pin::view_pin_selection(col, pin, draft, muted, primary, border_c);
        }
        SymbolEditorSelection::Graphic(g) => {
            col = graphic::view_graphic_selection(
                col,
                g,
                draft,
                muted,
                border_c,
                sym.graphic_fill_picker,
                i18n,
            );
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
                    text(raw(
                        "Bound to the host Component's designator at place-time.",
                    ))
                    .size(TS.caption)
                    .color(muted),
                )
                .padding([SP.s, SP.m])
                .boxed(),
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
                    text(raw("Bound to the host Component's value at place-time."))
                        .size(TS.caption)
                        .color(muted),
                )
                .padding([SP.s, SP.m])
                .boxed(),
            );
        }
    }

    col = col.push(
        container(
            text(raw("Click on the canvas to select."))
                .size(TS.caption)
                .color(muted),
        )
        .padding([6.0, SP.m])
        .boxed(),
    );

    scrollable(col)
        .width(Length::Fill)
        .style(form_look::scrollbar_style)
        .boxed()
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
            text(raw(label.to_string()))
                .size(TS.body)
                .color(muted)
                .width(Length::FillPortion(2)),
            text(raw(value))
                .size(TS.body)
                .color(primary)
                .width(Length::FillPortion(3)),
        ]
        .spacing(SP.s),
    )
    .padding([3.0, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// [`prop_row_static`] for an identifier (the UUID): one line in the mono
/// face, ending in an ellipsis when the dock is narrower than the value.
fn prop_row_mono<'a>(
    label: &str,
    value: String,
    muted: Color,
    primary: Color,
) -> Element<'a, PanelMsg> {
    use iced::widget::text::{Ellipsis, Wrapping};
    container(
        row![
            text(raw(label.to_string()))
                .size(TS.body)
                .color(muted)
                .width(Length::FillPortion(2)),
            text(raw(value))
                .size(TS.body)
                .color(primary)
                .font(crate::fonts::iced_font_for_family(
                    crate::fonts::CHROME_MONO_FONT,
                ))
                .wrapping(Wrapping::None)
                .ellipsis(Ellipsis::End)
                .width(Length::FillPortion(3)),
        ]
        .spacing(SP.s),
    )
    .padding([3.0, SP.m])
    .width(Length::Fill)
    .boxed()
}

/// A section heading: the uppercase label.
fn section_title<'a>(
    title: &str,
    tokens: &ThemeTokens,
    i18n: &signex_i18n::I18n,
) -> Element<'a, PanelMsg> {
    form_look::section_label(title, tokens, i18n)
}

#[cfg(test)]
mod uuid_row_tests {
    use super::*;
    use crate::layout_probe::{find, lay_out_texts, natural};

    const UUID: &str = "a1cf3b2e-4cdd-4cdd-9420-8bdf89c0ffee";

    /// #2262: the UUID wrapped onto a second line and ran past the dock
    /// edge. It stays on one line inside the row now, cut with an ellipsis.
    #[test]
    fn the_uuid_row_stays_on_one_line_inside_a_narrow_dock() {
        let dock = iced::Size::new(240.0, 400.0);
        let row = prop_row_mono("UUID", UUID.to_string(), Color::BLACK, Color::BLACK);
        let value = find(&lay_out_texts(row, dock), UUID).bounds;
        let mono = crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT);
        let one_line = natural::<PanelMsg>(text(raw(UUID)).size(TS.body).font(mono).boxed(), UUID);
        assert!(
            one_line.width > value.width,
            "the dock must be narrower than the UUID: {value:?}, natural {one_line:?}"
        );
        assert!(
            value.height <= one_line.height + 0.5,
            "the UUID wraps: {value:?}, one line is {one_line:?}"
        );
        assert!(
            value.x + value.width <= dock.width + 0.5,
            "the UUID runs past the dock edge: {value:?}"
        );
    }
}
