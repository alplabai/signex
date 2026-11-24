//! Properties panel for the active `.snxsym` editor tab (mockup row 123).
//!
//! Nothing selected: the symbol, with a Symbol tab (illustration, General,
//! Parts, Graphical, Fields) and a Pins tab (the pin list that replaced the
//! Pins dock panel, mockup row 122). A selected pin, graphic or field shows its own
//! form under a property illustration. Pure view code: the tab, the Pins
//! filter, the part chip and the illustration hover are editor state
//! (`SymbolPropsUi`), routed through `PanelMsg::SymbolProps`.

use iced::widget::{Column, scrollable};
use iced::{Color, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::theme::ThemeTokens;

use super::{
    CollapsedSections, NumericDraft, PanelMsg, SymbolEditorPanelContext, SymbolEditorSelection,
    form_look,
};
use crate::library::editor::pin_labels::part_label;

mod graphic;
mod illustration;
mod pin;
mod pin_table;
mod pins_tab;
mod rows;
mod symbol;

pub(crate) use pin_table::view_pin_table;
use rows::{Form, Look};

/// Properties panel content for the active `.snxsym` editor tab.
/// `toggled` is the live `PanelContext::collapsed_sections`, so a
/// `PanelMsg::ToggleSection` shows on the next frame.
pub(super) fn view_symbol_editor_properties<'a>(
    sym: &'a SymbolEditorPanelContext,
    toggled: &CollapsedSections,
    draft: Option<&NumericDraft>,
    border_c: Color,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let form = Form {
        look: Look::of(tokens),
        tokens,
        i18n,
        hover: &sym.props.hover,
        toggled,
        draft,
    };
    let body = match &sym.selected {
        SymbolEditorSelection::None => symbol::view(sym, &form, border_c),
        SymbolEditorSelection::Pin(pin) => pin::view(sym, pin, &form),
        SymbolEditorSelection::Graphic(g) => graphic::view(sym, g, &form, border_c),
        SymbolEditorSelection::FieldReference => field_view(sym, &form, true),
        SymbolEditorSelection::FieldValue => field_view(sym, &form, false),
    };
    scrollable(Column::with_children(body).spacing(0).width(Length::Fill))
        .width(Length::Fill)
        .style(form_look::scrollbar_style)
        .boxed()
}

/// The name of part `part`: "Part 0 (shared)", "Part 1 (A)", or past Z
/// "Part 27".
fn part_name(part: u8, i18n: &I18n) -> Localized {
    match part {
        0 => t!(i18n, ids::EDITORS_SYMPROPS_PART_SHARED),
        1..=26 => t!(
            i18n,
            ids::EDITORS_SYMPROPS_PART_LETTERED,
            "part" => u32::from(part),
            "letter" => part_label(part)
        ),
        _ => t!(i18n, ids::EDITORS_SYMPROPS_PART_NUMBERED, "part" => u32::from(part)),
    }
}

/// "<symbol> · Part <n>", the caption under a pin or graphic title.
fn object_context(sym: &SymbolEditorPanelContext, part: u8, i18n: &I18n) -> Localized {
    t!(
        i18n,
        ids::EDITORS_SYMPROPS_OBJECT_CONTEXT,
        "symbol" => sym.symbol_name.as_str(),
        "part" => u32::from(part)
    )
}

/// The Reference or Value field selected on the canvas: both are bound to
/// the placed component; the Reference template is edited here.
fn field_view<'a>(
    sym: &'a SymbolEditorPanelContext,
    form: &Form<'_>,
    is_reference: bool,
) -> Vec<Element<'a, PanelMsg>> {
    let i18n = form.i18n;
    let (title, note) = if is_reference {
        (
            ids::EDITORS_SYMPROPS_REFERENCE_FIELD,
            ids::EDITORS_SYMPROPS_REFERENCE_BOUND,
        )
    } else {
        (
            ids::EDITORS_SYMPROPS_VALUE_FIELD,
            ids::EDITORS_SYMPROPS_VALUE_BOUND,
        )
    };
    let mut col = vec![form.header(
        t!(i18n, title).into(),
        None,
        Some((
            signex_i18n::UserText::new(sym.symbol_name.as_str()).into(),
            false,
        )),
    )];
    if is_reference {
        col.push(form.field(
            t!(i18n, ids::EDITORS_SYMPROPS_REFERENCE),
            form.input(
                t!(i18n, ids::EDITORS_SYMPROPS_REFERENCE),
                sym.symbol_designator.clone(),
                true,
                PanelMsg::SymEditorSetSymbolDesignator,
            ),
            None,
        ));
    }
    let label = if is_reference {
        ids::EDITORS_SYMPROPS_REFERENCE
    } else {
        ids::EDITORS_SYMPROPS_VALUE
    };
    col.push(form.field(t!(i18n, label), form.note(t!(i18n, note)), None));
    col
}

#[cfg(test)]
mod tests;
