//! The Variant Manager's right pane (mockup row 32): the selected cell's
//! part in its variant. The Fitting switch fits or leaves the part out (or
//! lets the sheets decide again), the parameter overrides give it values of
//! its own with a reset back to Base's, greyed while it is not fitted, and
//! the last lines show the part in the other variants. Base's cell is the
//! sheets and is shown, not edited. No sheet preview yet: no preview widget
//! draws one placed part.

use iced::widget::{Column, Row, Space, column, container, row, scrollable};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{MessageId, UserText, ids, t};
use signex_output::{VariantMatrix, VariantRow};
use signex_types::project::{FOOTPRINT_PARAMETER, VALUE_PARAMETER};
use signex_widgets::segmented::segmented_disabled;
use signex_widgets::{segmented, text, text_input};

use super::super::ProjectTabMsg;
use super::super::general::tab_message;
use super::super::look::{ButtonKind, Glyph, field_label, icon_button, text_button};
use super::super::table::{SP, TS, note};
use super::{Manager, VariantCell, VariantManagerMsg, mono, semibold};
use crate::app::Message;
use crate::styles::ti;
use crate::styles::tokens::resolve;

/// Width of the pane, in `body`-size characters.
const PANE_EMS: f32 = 300.0 / 12.0;
/// Width of an override's name and Base columns, in `body`-size characters.
const OVERRIDE_COL_EMS: f32 = 76.0 / 12.0;

/// The pane: the selected cell's part, or a hint to click a cell.
pub(super) fn view<'a>(m: &Manager<'_>) -> Element<'a, Message> {
    let selected = m.cell().zip(m.matrix()).and_then(|(cell, matrix)| {
        let part = matrix
            .rows
            .iter()
            .find(|row| row.designator == cell.designator)?;
        Some(cell_view(m, cell, matrix, part))
    });
    let body =
        selected.unwrap_or_else(|| note(t!(m.i18n, ids::VARIANT_MANAGER_DETAIL_EMPTY), m.tokens));
    let chrome = ti(m.tokens.toolbar_bg);
    let side = iced::border::Side::default()
        .width(1.0)
        .color(ti(m.tokens.border));
    container(
        scrollable(container(body).padding(SP.l))
            .direction(crate::styles::thin_vertical())
            .style(crate::styles::scrollbar_style(m.tokens))
            .height(Length::Fill),
    )
    .width(Length::Fixed(TS.body * PANE_EMS))
    .height(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(chrome)),
        border: Border::default().left(side),
        ..container::Style::default()
    })
    .boxed()
}

/// One cell: its heading, fitting, overrides and the other variants.
fn cell_view<'a>(
    m: &Manager<'_>,
    cell: &VariantCell,
    matrix: &VariantMatrix,
    part: &VariantRow,
) -> Element<'a, Message> {
    let fitted = matrix
        .column(cell.variant.as_deref())
        .and_then(|column| part.fitted.get(column).copied())
        .unwrap_or(true);
    let mut pane = column![
        heading(m, cell, part),
        field_label(
            t!(m.i18n, ids::VARIANT_MANAGER_FITTING, "variant" => m.name(cell.variant.as_deref())),
            m.tokens
        ),
        fitting(m, cell, part, fitted),
    ]
    .spacing(SP.m);
    if let Some(every) = super::channel_rows::every_channel(m, cell, matrix, part) {
        pane = pane.push(every);
    }
    pane.push(field_label(
        t!(m.i18n, ids::VARIANT_MANAGER_OVERRIDES),
        m.tokens,
    ))
    .push(overrides(m, cell, part, fitted))
    .push(field_label(
        t!(m.i18n, ids::VARIANT_MANAGER_OTHER_VARIANTS, "designator" => part.designator.as_str()),
        m.tokens,
    ))
    .push(others(m, cell, matrix, part))
    .boxed()
}

/// "R5 in Lite", one catalog message so a language can order its words, and
/// the part's value and footprint as the sheets give them.
fn heading<'a>(m: &Manager<'_>, cell: &VariantCell, part: &VariantRow) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let title = text(t!(
        m.i18n,
        ids::VARIANT_MANAGER_DETAIL_IN,
        "designator" => part.designator.as_str(),
        "variant" => m.name(cell.variant.as_deref()),
    ))
    .size(TS.title)
    .font(semibold());
    let mut facts: Row<Element<'a, Message>> = row![
        text(UserText::new(part.value.clone()))
            .size(TS.caption)
            .font(mono())
    ]
    .spacing(SP.xs);
    if !part.footprint.is_empty() {
        facts = facts.push(
            text(
                t!(m.i18n, ids::PROJECT_VARIANTS_FOOTPRINT, "footprint" => part.footprint.as_str()),
            )
            .size(TS.caption)
            .color(r.ink4)
            .boxed(),
        );
    }
    column![title, facts].spacing(SP.xs).boxed()
}

/// Fitted / Not fitted for a variant's cell, Alternate (the picker), and
/// As on the sheets while the cell sets a fit of its own. Base's cell shows
/// its fit greyed: Base is what the sheets say.
fn fitting<'a>(
    m: &Manager<'_>,
    cell: &VariantCell,
    part: &VariantRow,
    fitted: bool,
) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let style = r.segmented(TS.body);
    let options = [
        (true, t!(m.i18n, ids::PROJECT_VARIANTS_FITTED).into()),
        (false, t!(m.i18n, ids::PROJECT_VARIANTS_NOT_FITTED).into()),
    ];
    if cell.variant.is_none() || m.settings_locked {
        let words = if m.settings_locked {
            ids::VARIANT_MANAGER_SETTINGS_UNREADABLE
        } else {
            ids::VARIANT_MANAGER_BASE_FITTING
        };
        let grey = Color {
            a: crate::styles::DISABLED_OPACITY,
            ..r.ink4
        };
        let mut greyed = style.resting_ink(grey);
        greyed.active_ink = grey;
        return column![
            segmented_disabled(options, fitted, greyed),
            note(t!(m.i18n, words), m.tokens),
        ]
        .spacing(SP.s)
        .boxed();
    }
    let send = fit_message(m, cell);
    let switch = segmented(options, fitted, style, move |fitted| send(Some(fitted)));
    // The canvas's Alternate part picker for this part (mockup row 208).
    let alternate = text_button(
        None,
        t!(m.i18n, ids::VARIANT_MANAGER_ALTERNATE),
        Some(tab_message(ProjectTabMsg::VariantManager {
            project_id: m.project_id,
            msg: VariantManagerMsg::AlternateRequested { cell: cell.clone() },
        })),
        ButtonKind::Small,
        m.tokens,
    );
    let mut line: Row<Element<'a, Message>> = row![switch, alternate]
        .spacing(SP.s)
        .align_y(Alignment::Center);
    let own = m
        .own(cell.variant.as_deref(), part)
        .is_some_and(|own| own.fitted.is_some());
    if own {
        line = line.push(text_button(
            None,
            t!(m.i18n, ids::VARIANT_MANAGER_AS_ON_SHEETS),
            Some(fit_message(m, cell)(None)),
            ButtonKind::Ghost,
            m.tokens,
        ));
    }
    line.wrap().boxed()
}

/// The message that sets `cell`'s fit.
fn fit_message(m: &Manager<'_>, cell: &VariantCell) -> impl Fn(Option<bool>) -> Message + 'static {
    let project_id = m.project_id;
    let cell = cell.clone();
    move |fitted| {
        tab_message(ProjectTabMsg::VariantManager {
            project_id,
            msg: VariantManagerMsg::FitPicked {
                cell: cell.clone(),
                fitted,
            },
        })
    }
}

/// The part's parameters: name, Base's value and the variant's own (typed
/// in place, Enter stores it, the arrow resets it). A part not fitted has
/// none to override.
fn overrides<'a>(
    m: &Manager<'_>,
    cell: &VariantCell,
    part: &VariantRow,
    fitted: bool,
) -> Element<'a, Message> {
    if !fitted {
        return note(t!(m.i18n, ids::VARIANT_MANAGER_OVERRIDES_OFF), m.tokens);
    }
    let mut rows: Column<Element<'a, Message>> =
        Column::new().spacing(SP.xs).push(override_header(m, cell));
    for (parameter, base) in parameter_rows(m, cell, part) {
        rows = rows.push(override_row(m, cell, part, &parameter, &base));
    }
    rows.boxed()
}

/// The override table's column names: Parameter, Base and the variant.
fn override_header<'a>(m: &Manager<'_>, cell: &VariantCell) -> Element<'a, Message> {
    let ink = resolve(m.tokens).ink4;
    let column = Length::Fixed(TS.body * OVERRIDE_COL_EMS);
    let mut line: Row<Element<'a, Message>> = row![
        container(
            text(t!(m.i18n, ids::VARIANT_MANAGER_COL_PARAMETER))
                .size(TS.caption)
                .color(ink)
        )
        .width(column),
        container(
            text(t!(m.i18n, ids::PROJECT_VARIANTS_BASE))
                .size(TS.caption)
                .color(ink)
        )
        .width(column),
    ]
    .spacing(SP.s);
    if let Some(variant) = cell.variant.as_deref() {
        line = line.push(
            text(UserText::new(variant.to_string()))
                .size(TS.caption)
                .color(ink)
                .boxed(),
        );
    }
    line.boxed()
}

/// Value, Footprint, then every field the part or the variant sets, each
/// with Base's value.
fn parameter_rows(m: &Manager<'_>, cell: &VariantCell, part: &VariantRow) -> Vec<(String, String)> {
    let mut rows = vec![
        (VALUE_PARAMETER.to_string(), part.value.clone()),
        (FOOTPRINT_PARAMETER.to_string(), part.footprint.clone()),
    ];
    rows.extend(part.parameters.iter().cloned());
    if let Some(own) = m.own(cell.variant.as_deref(), part) {
        for name in own.parameters.keys() {
            if !rows.iter().any(|(listed, _)| listed == name) {
                rows.push((name.clone(), String::new()));
            }
        }
    }
    rows
}

/// One override line: the parameter, Base's value, the variant's.
fn override_row<'a>(
    m: &Manager<'_>,
    cell: &VariantCell,
    part: &VariantRow,
    parameter: &str,
    base: &str,
) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let label: Element<'a, Message> = match catalog_name(parameter) {
        Some(id) => text(t!(m.i18n, id)).size(TS.caption).boxed(),
        None => text(UserText::new(parameter.to_string()))
            .size(TS.caption)
            .boxed(),
    };
    let column = Length::Fixed(TS.body * OVERRIDE_COL_EMS);
    let base_face = text(UserText::new(base.to_string()))
        .size(TS.caption)
        .font(mono())
        .color(r.ink3)
        .wrapping(iced::widget::text::Wrapping::None);
    row![
        container(label).width(column).clip(true),
        container(base_face).width(column).clip(true),
        own_value(m, cell, part, parameter),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

/// The parameters whose name comes from the catalog.
fn catalog_name(parameter: &str) -> Option<MessageId> {
    match parameter {
        VALUE_PARAMETER => Some(ids::VARIANT_MANAGER_PARAMETER_VALUE),
        FOOTPRINT_PARAMETER => Some(ids::VARIANT_MANAGER_PARAMETER_FOOTPRINT),
        _ => None,
    }
}

/// The variant's own value for `parameter`: a box for a variant's cell (the
/// text being typed, else the value set, else "same as Base"), with the
/// reset arrow while a value is set; Base's own value for Base.
fn own_value<'a>(
    m: &Manager<'_>,
    cell: &VariantCell,
    part: &VariantRow,
    parameter: &str,
) -> Element<'a, Message> {
    if cell.variant.is_none() {
        return Space::new().boxed();
    }
    let set = m
        .own(cell.variant.as_deref(), part)
        .and_then(|own| own.parameters.get(parameter))
        .cloned();
    let typed = m
        .tab
        .and_then(|tab| tab.variant_manager.draft.as_ref())
        .filter(|draft| draft.parameter == parameter)
        .map(|draft| draft.text.clone());
    let shown = typed.or_else(|| set.clone()).unwrap_or_default();
    let project_id = m.project_id;
    let send = move |msg| tab_message(ProjectTabMsg::VariantManager { project_id, msg });
    let mut input = text_input(t!(m.i18n, ids::VARIANT_MANAGER_SAME_AS_BASE), shown)
        .style(crate::panels::form_look::text_input_style)
        .size(TS.caption)
        .font(mono())
        .padding(SP.xs)
        .width(Length::Fill);
    if !m.settings_locked {
        let name = parameter.to_string();
        input = input
            .on_input(move |text| {
                send(VariantManagerMsg::OverrideEdited {
                    parameter: name.clone(),
                    text,
                })
            })
            .on_submit(send(VariantManagerMsg::OverrideSubmitted {
                cell: cell.clone(),
            }));
    }
    let mut line: Row<Element<'a, Message>> = row![input].align_y(Alignment::Center);
    if set.is_some() && !m.settings_locked {
        let reset = send(VariantManagerMsg::OverrideReset {
            cell: cell.clone(),
            parameter: parameter.to_string(),
        });
        line = line.push(icon_button(
            Glyph::Restore,
            t!(m.i18n, ids::VARIANT_MANAGER_RESET),
            Some(reset),
            m.tokens,
        ));
    }
    line.width(Length::Fill).boxed()
}

/// The part in every other variant: its value there, or Not fitted.
fn others<'a>(
    m: &Manager<'_>,
    cell: &VariantCell,
    matrix: &VariantMatrix,
    part: &VariantRow,
) -> Element<'a, Message> {
    let r = resolve(m.tokens);
    let mut rows: Column<Element<'a, Message>> = Column::new().spacing(SP.xs);
    for variant in m.variants().filter(|v| *v != cell.variant.as_deref()) {
        let fitted = matrix
            .column(variant)
            .and_then(|column| part.fitted.get(column).copied())
            .unwrap_or(true);
        let value: Element<'a, Message> = if fitted {
            text(UserText::new(shown_value(m, variant, part)))
                .size(TS.caption)
                .font(mono())
                .boxed()
        } else {
            text(t!(m.i18n, ids::PROJECT_VARIANTS_NOT_FITTED))
                .size(TS.caption)
                .color(r.ink4)
                .boxed()
        };
        rows = rows.push(
            row![
                text(UserText::new(m.name(variant)))
                    .size(TS.caption)
                    .color(r.ink3),
                Space::new().width(Length::Fill),
                value,
            ]
            .spacing(SP.s)
            .boxed(),
        );
    }
    rows.boxed()
}

/// The value `variant` shows for `part`: its own, else the sheets'.
fn shown_value(m: &Manager<'_>, variant: Option<&str>, part: &VariantRow) -> String {
    m.own(variant, part)
        .and_then(|own| own.parameters.get(VALUE_PARAMETER))
        .cloned()
        .unwrap_or_else(|| part.value.clone())
}
