//! The Pin table document tab (mockup row 122b): the symbol's pins with
//! all seven columns typed in place, a Check column, a filter and an
//! issues-only view, `+ Pin`, Delete selected, the bulk edit of the ticked
//! rows and Paste rows. Cell edits send the Pin properties' own messages
//! (`SymEditorSetPinName`, `SymEditorTogglePinHidden`, …), so the Pins tab,
//! Pin properties, the canvas and this table show one pin model. The tab's
//! own state is `PinTableUi`, routed through `PanelMsg::SymbolProps`.

use iced::widget::{Column, Space, column, container, row, scrollable};
use iced::{Alignment, Background, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, text_input};

use super::super::{
    CollapsedSections, NumericDraft, PanelMsg, SymbolEditorPanelContext, SymbolPinSummary,
    form_look,
};
use super::pins_tab::{PinIssue, listed_pins, part_chips, pin_issues};
use super::rows::{Form, Look, SP, TS, props};
use crate::library::editor::symbol::{PartChip, PinTableMsg, PinTableUi, SymbolPropsMsg};
use crate::styles::HAIRLINE;

mod bulk;
mod grid;
mod paste;

/// The filter field's width, in body sizes.
const FILTER_EM: f32 = 260.0 / 12.0;

/// A message of the Pin table tab.
fn table(msg: PinTableMsg) -> PanelMsg {
    props(SymbolPropsMsg::PinTable(msg))
}

/// The rows the table lists: every pin whose number or name holds the
/// filter, and only those with an issue while the issues-only view is on.
fn listed_rows<'p>(
    sym: &'p SymbolEditorPanelContext,
    issues: &[PinIssue],
    ui: &PinTableUi,
    i18n: &I18n,
) -> Vec<&'p SymbolPinSummary> {
    listed_pins(&sym.pins, PartChip::All, &ui.filter, i18n)
        .into_iter()
        .filter(|p| !ui.issues_only || issues.get(p.idx).is_some_and(|i| i.any()))
        .collect()
}

/// The tab's body for the symbol `sym`. `ui` is the tab state as the
/// Model holds it, so the paste box's editor keeps its cursor.
pub(crate) fn view_pin_table<'a>(
    sym: &'a SymbolEditorPanelContext,
    ui: &'a PinTableUi,
    toggled: &CollapsedSections,
    draft: Option<&NumericDraft>,
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
    let issues = pin_issues(&sym.pins);
    let listed = listed_rows(sym, &issues, ui, i18n);
    let mut body: Column<Element<'a, PanelMsg>> = Column::new().width(Length::Fill);
    body = body.push(head(sym, ui, &issues, &form));
    body = body.push(rule(&form));
    body = body.push(bar(ui, &form));
    if let Some(paste) = &ui.paste {
        body = body.push(rule(&form));
        body = body.push(paste::view(sym, paste, &form));
    }
    body = body.push(rule(&form));
    body = body.push(bulk::view(ui, &form));
    let rows = if listed.is_empty() {
        empty(sym.pins.is_empty(), &form)
    } else {
        grid::view(sym, &listed, &issues, ui, &form)
    };
    body = body.push(
        scrollable(rows)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(form_look::scrollbar_style)
            .boxed(),
    );
    body = body.push(rule(&form));
    body = body.push(foot(sym, &form));
    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(crate::styles::panel_region(tokens))
        .boxed()
}

/// "OPA2333 pins", "opamps.snxsym · 8 pins · 3 parts", and the issues
/// button, which shows only the rows with issues.
fn head<'a>(
    sym: &SymbolEditorPanelContext,
    ui: &PinTableUi,
    issues: &[PinIssue],
    form: &Form<'_>,
) -> Element<'a, PanelMsg> {
    let i18n = form.i18n;
    let bad = issues.iter().filter(|i| i.any()).count();
    let parts = part_chips(&sym.pins).len().saturating_sub(1);
    let file = sym
        .path
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    let title = column![
        text(t!(i18n, ids::EDITORS_PIN_TABLE_HEADING, "symbol" => sym.symbol_name.as_str()))
            .size(TS.title)
            .color(form.look.ink)
            .boxed(),
        text(t!(
            i18n,
            ids::EDITORS_PIN_TABLE_SUMMARY,
            "file" => file.as_str(),
            "pins" => sym.pins.len(),
            "parts" => parts
        ))
        .size(TS.caption)
        .color(form.look.ink4)
        .boxed(),
    ]
    .spacing(SP.xs);
    let label = if bad == 0 {
        t!(i18n, ids::EDITORS_PIN_TABLE_NO_ISSUES)
    } else if ui.issues_only {
        t!(i18n, ids::EDITORS_PIN_TABLE_ISSUES_ONLY, "count" => bad)
    } else {
        t!(i18n, ids::EDITORS_PIN_TABLE_ISSUES, "count" => bad)
    };
    let issues_button = form.ghost(
        label,
        (bad > 0 || ui.issues_only).then_some(table(PinTableMsg::IssuesOnlyToggled)),
        Some(t!(i18n, ids::EDITORS_PIN_TABLE_ISSUES_TIP)),
    );
    band(
        row![
            title.boxed(),
            Space::new().width(Length::Fill).boxed(),
            issues_button
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .boxed(),
        Some(form.look.sunken),
    )
}

/// The filter, `+ Pin`, Delete selected and Paste rows….
fn bar<'a>(ui: &'a PinTableUi, form: &Form<'_>) -> Element<'a, PanelMsg> {
    let i18n = form.i18n;
    let filter = text_input(
        t!(i18n, ids::EDITORS_SYMPROPS_PIN_FILTER),
        ui.filter.as_str(),
    )
    .on_input(|q| table(PinTableMsg::FilterChanged(q)))
    .size(TS.body)
    .padding([SP.xs, SP.s])
    .style(form_look::text_input_style)
    .width(TS.body * FILTER_EM);
    let add = form.ghost(
        t!(i18n, ids::EDITORS_SYMPROPS_ADD_PIN),
        Some(table(PinTableMsg::AddPinPressed)),
        None,
    );
    let delete = form.ghost(
        t!(i18n, ids::EDITORS_PIN_TABLE_DELETE),
        (!ui.checked.is_empty()).then_some(table(PinTableMsg::DeleteSelectedPressed)),
        None,
    );
    let paste = form.ghost(
        t!(i18n, ids::EDITORS_PIN_TABLE_PASTE),
        Some(table(PinTableMsg::PasteToggled)),
        None,
    );
    band(
        row![
            filter.boxed(),
            add,
            delete,
            Space::new().width(Length::Fill).boxed(),
            paste
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .boxed(),
        None,
    )
}

/// How edits land, under the table.
fn foot<'a>(sym: &SymbolEditorPanelContext, form: &Form<'_>) -> Element<'a, PanelMsg> {
    let line = t!(
        form.i18n,
        ids::EDITORS_PIN_TABLE_FOOT,
        "symbol" => sym.symbol_name.as_str()
    );
    band(
        text(line).size(TS.caption).color(form.look.ink4).boxed(),
        Some(form.look.sunken),
    )
}

/// No pin passes the filter or the issues-only view, or the symbol has no
/// pins yet.
fn empty<'a>(no_pins: bool, form: &Form<'_>) -> Element<'a, PanelMsg> {
    let i18n = form.i18n;
    let (big, hint) = if no_pins {
        (
            ids::EDITORS_SYMPROPS_NO_PINS,
            ids::EDITORS_SYMPROPS_NO_PINS_HINT,
        )
    } else {
        (
            ids::EDITORS_SYMPROPS_NO_MATCH,
            ids::EDITORS_PIN_TABLE_NO_MATCH_HINT,
        )
    };
    container(
        column![
            text(t!(i18n, big))
                .size(TS.body)
                .color(form.look.ink)
                .boxed(),
            text(t!(i18n, hint))
                .size(TS.body)
                .color(form.look.ink3)
                .boxed(),
        ]
        .spacing(SP.xs)
        .align_x(Alignment::Center),
    )
    .padding(SP.xl)
    .center_x(Length::Fill)
    .boxed()
}

/// A full-width strip with the tab's side padding, on `fill` when set.
fn band<'a>(content: Element<'a, PanelMsg>, fill: Option<iced::Color>) -> Element<'a, PanelMsg> {
    container(content)
        .padding([SP.s, SP.l])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: fill.map(Background::Color),
            ..container::Style::default()
        })
        .boxed()
}

/// A hairline between the strips.
fn rule<'a>(form: &Form<'_>) -> Element<'a, PanelMsg> {
    let line = form.look.line_soft;
    container(Space::new())
        .height(HAIRLINE)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        })
        .boxed()
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
