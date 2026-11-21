//! Properties of a library row (mockup row 127): the row selected in a
//! Library Browser tab (header, pills, Component, Bindings with the Pick
//! buttons, Storage), and the row of the active Component editor tab
//! (Component and Storage). Every field here is read-only; editing a row
//! stays in the Library Browser and the Component editor.

use iced::widget::{Column, column, container, row, svg};
use iced::{Alignment, Background, Border, Element, Font, Length, Padding, Theme, Widget};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{Label, button, text};

use super::PanelMsg;
use super::form_look::section_label;
use crate::styles::tokens::{Overlay, composite_over, overlay, resolve};
use crate::styles::{
    ChromeMetrics, PillTone, Radii, Spacing, TypeScale, dimmed_button, pill_colors, primary_button,
    read_only_field, ti,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
const CM: ChromeMetrics = ChromeMetrics::DEFAULT;
const RADII: Radii = Radii::GRAPHITE;

/// Height of a label-and-field row.
const FIELD_ROW_H: f32 = CM.field + SP.xs;
/// Height of the read-only field inside it.
const FIELD_H: f32 = CM.row;
/// Side of a binding card's status glyph and of the lifecycle dot.
const GLYPH: f32 = TS.label;
const DOT: f32 = SP.s + SP.xs;

const CHECK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M3.5 8.5l3 3 6-7" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const WARN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path d="M8 2.5l6 10.5H2z" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round"/><path d="M8 6.5v3M8 11.3v.2" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>"#;

/// The row selected in the active Library Browser tab, as the Properties
/// panel shows it. Built by `refresh_panel_ctx` from the cached row (no
/// disk read); `None` for any other tab or with no row selected.
#[derive(Debug, Clone)]
pub struct LibraryRowDetail {
    pub library_path: std::path::PathBuf,
    /// The open library's name, for the Storage section.
    pub library: String,
    pub table: String,
    pub row_id: uuid::Uuid,
    /// The row's 1-based place in its table, in stored order.
    pub position: Option<usize>,
    pub internal_pn: String,
    /// Class string stored on the row, derived from the table name at
    /// create time. Read-only metadata.
    pub class: String,
    /// Lifecycle as worded in the library editor ("Released", …).
    pub lifecycle: String,
    /// Tone of the lifecycle dot.
    pub lifecycle_tone: PillTone,
    pub version: String,
    pub mpn: String,
    pub manufacturer: String,
    /// Last update, `YYYY-MM-DD HH:MM` (UTC).
    pub updated: String,
    /// "Symbol bound — 4 pins" / "Symbol unresolved (…)" / "No symbol
    /// bound", worded when the panel context is built.
    pub symbol_summary: Localized,
    /// Same shape for the footprint.
    pub footprint_summary: Localized,
    /// The row lives in a read-only mount (the stock library); the Pick
    /// buttons are disabled.
    pub read_only: bool,
    /// The symbol binding is missing, unresolved or unreadable: its card
    /// turns amber and its Pick button becomes primary.
    pub symbol_warn: bool,
    /// The same for the footprint binding.
    pub footprint_warn: bool,
}

/// The Properties of a selected Library Browser row.
pub fn view_library_row_properties<'a>(
    d: &'a LibraryRowDetail,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let mut body: Column<Element<'a, PanelMsg>> = column![
        header(d, tokens, i18n),
        pills(d, tokens, i18n),
        section_label(
            t!(i18n, ids::PANELS_COMPONENT_SHEET_SECTION_COMPONENT),
            tokens,
            i18n
        ),
    ]
    .width(Length::Fill);
    let component = [
        (
            ids::PANELS_COMPONENT_SHEET_INTERNAL_PN,
            d.internal_pn.as_str(),
            true,
        ),
        (ids::PANELS_COMPONENT_SHEET_MPN, d.mpn.as_str(), true),
        (
            ids::PANELS_COMPONENT_SHEET_MANUFACTURER,
            d.manufacturer.as_str(),
            false,
        ),
        (
            ids::PANELS_COMPONENT_SHEET_LIFECYCLE,
            d.lifecycle.as_str(),
            false,
        ),
        (
            ids::PANELS_COMPONENT_SHEET_VERSION,
            d.version.as_str(),
            true,
        ),
    ];
    for (label, value, mono) in component {
        body = body.push(field_row(label, value, mono, tokens, i18n));
    }
    body = body.push(section_label(
        t!(i18n, ids::PANELS_LIBRARY_ROW_SECTION_BINDINGS),
        tokens,
        i18n,
    ));
    let writable = !d.read_only;
    body = body.push(binding_card(
        Binding {
            title: t!(i18n, ids::PANELS_LIBRARY_ROW_SYMBOL),
            status: d.symbol_summary.clone(),
            warn: d.symbol_warn,
            pick: writable.then_some(PanelMsg::LibraryRowPickSymbol),
            pick_named: t!(i18n, ids::PANELS_LIBRARY_ROW_PICK_SYMBOL),
        },
        tokens,
        i18n,
    ));
    body = body.push(binding_card(
        Binding {
            title: t!(i18n, ids::PANELS_LIBRARY_ROW_FOOTPRINT),
            status: d.footprint_summary.clone(),
            warn: d.footprint_warn,
            pick: writable.then_some(PanelMsg::LibraryRowPickFootprint),
            pick_named: t!(i18n, ids::PANELS_LIBRARY_ROW_PICK_FOOTPRINT),
        },
        tokens,
        i18n,
    ));
    body = body.push(section_label(
        t!(i18n, ids::PANELS_COMPONENT_SHEET_SECTION_STORAGE),
        tokens,
        i18n,
    ));
    let storage = [
        (ids::PANELS_COMPONENT_SHEET_LIBRARY, d.library.as_str()),
        (ids::PANELS_COMPONENT_SHEET_TABLE, d.table.as_str()),
        (ids::PANELS_COMPONENT_SHEET_UPDATED, d.updated.as_str()),
    ];
    for (label, value) in storage {
        body = body.push(field_row(label, value, false, tokens, i18n));
    }
    body.push(iced::widget::Space::new().height(SP.m).boxed())
        .boxed()
}

/// The part number as the title and "Library Row · table · row n" under it.
fn header<'a>(d: &'a LibraryRowDetail, tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let title: Label<'a> = if d.internal_pn.is_empty() {
        t!(i18n, ids::PANELS_LIBRARY_ROW_UNNAMED).into()
    } else {
        UserText::new(d.internal_pn.as_str()).into()
    };
    let caption = match d.position {
        Some(n) => t!(
            i18n,
            ids::PANELS_LIBRARY_ROW_HEADING,
            "table" => d.table.as_str(),
            "row" => n
        ),
        None => t!(
            i18n,
            ids::PANELS_LIBRARY_ROW_HEADING,
            "table" => d.table.as_str(),
            "row" => signex_types::object_id::short_id(&d.row_id)
        ),
    };
    container(
        column![
            text(title)
                .size(TS.label)
                .font(Font {
                    weight: iced::font::Weight::Semibold,
                    ..mono()
                })
                .color(ti(tokens.text))
                .boxed(),
            text(caption).size(TS.caption).color(r.ink4).boxed(),
        ]
        .spacing(SP.xs / 2.0),
    )
    .padding(Padding {
        top: SP.m,
        right: SP.m,
        bottom: SP.xs,
        left: SP.m,
    })
    .width(Length::Fill)
    .boxed()
}

/// The class pill, the lifecycle with its tone dot, and the version.
fn pills<'a>(d: &'a LibraryRowDetail, tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let mut line: iced::widget::Row<Element<'a, PanelMsg>> = iced::widget::Row::new()
        .spacing(SP.s)
        .align_y(Alignment::Center);
    if !d.class.is_empty() {
        line = line.push(crate::library::editor::kit::toned_pill(
            d.class.clone(),
            PillTone::Accent,
            tokens,
        ));
    }
    let (_, tone_ink) = pill_colors(tokens, d.lifecycle_tone);
    let dot = container(iced::widget::Space::new())
        .width(DOT)
        .height(DOT)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(tone_ink)),
            border: Border {
                radius: RADII.pill.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    line = line.push(
        row![
            dot.boxed(),
            text(UserText::new(d.lifecycle.as_str()))
                .size(TS.caption)
                .color(tone_ink)
                .boxed(),
        ]
        .spacing(SP.xs + SP.xs)
        .align_y(Alignment::Center)
        .boxed(),
    );
    if !d.version.is_empty() {
        line = line.push(
            text(t!(i18n, ids::PANELS_LIBRARY_ROW_VERSION, "version" => d.version.as_str()))
                .size(TS.caption)
                .color(r.ink4)
                .boxed(),
        );
    }
    container(line)
        .padding(Padding {
            top: SP.s,
            right: SP.m,
            bottom: SP.xs,
            left: SP.m,
        })
        .width(Length::Fill)
        .boxed()
}

/// The mono face of part numbers.
fn mono() -> Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// A label and its value in a read-only field (the sunken read-only look);
/// an empty value shows the no-value mark.
fn field_row<'a>(
    label: signex_i18n::MessageId,
    value: &'a str,
    is_mono: bool,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let shown: Label<'a> = if value.trim().is_empty() {
        t!(i18n, ids::PANELS_COMPONENT_SHEET_NO_VALUE).into()
    } else {
        UserText::new(value).into()
    };
    let mut v = text(shown)
        .size(TS.body)
        .wrapping(iced::widget::text::Wrapping::None)
        .ellipsis(iced::widget::text::Ellipsis::End);
    if is_mono {
        v = v.font(mono());
    }
    let field = container(v.boxed())
        .padding([0.0, SP.m])
        .height(FIELD_H)
        .center_y(FIELD_H)
        .width(Length::Fill)
        .clip(true)
        .style(read_only_field(tokens));
    container(
        row![
            text(t!(i18n, label))
                .size(TS.body)
                .color(r.ink3)
                .wrapping(iced::widget::text::Wrapping::None)
                .width(super::PROPERTY_LABEL_COLUMN)
                .boxed(),
            field.boxed(),
        ]
        .spacing(super::PROPERTY_ROW_GAP)
        .align_y(Alignment::Center),
    )
    .padding([0.0, SP.m])
    .height(FIELD_ROW_H)
    .center_y(FIELD_ROW_H)
    .width(Length::Fill)
    .boxed()
}

/// What one binding card shows.
struct Binding {
    title: Localized,
    status: Localized,
    /// Missing, unresolved or unreadable: the card turns amber.
    warn: bool,
    /// `None` on a read-only library: Pick is disabled.
    pick: Option<PanelMsg>,
    /// The primary button's label when `warn` ("Pick Symbol…").
    pick_named: Localized,
}

/// A Symbol or Footprint binding card: status glyph, title and status line,
/// and the Pick button. A binding that needs attention turns the card amber
/// and makes Pick the primary action.
fn binding_card<'a>(b: Binding, tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let ink = ti(tokens.text);
    let (ok, warn) = (ti(tokens.success), ti(tokens.warning));
    let (glyph_bytes, glyph_ink, status_ink) = if b.warn {
        (WARN, warn, warn)
    } else {
        (CHECK, ok, r.ink3)
    };
    let (fill, edge) = if b.warn {
        (Some(Background::Color(r.warn_soft)), warn)
    } else {
        (None, ti(tokens.border))
    };
    let pick = if b.warn {
        button(text(b.pick_named).size(TS.caption))
            .padding([SP.xs, SP.m])
            .on_press_maybe(b.pick)
            .style(primary_button(tokens))
            .boxed()
    } else {
        small_button(t!(i18n, ids::PANELS_LIBRARY_ROW_PICK), b.pick, tokens)
    };
    let card = container(
        row![
            svg(svg::Handle::from_memory(glyph_bytes))
                .width(GLYPH)
                .height(GLYPH)
                .style(move |_: &Theme, _| svg::Style {
                    color: Some(glyph_ink),
                    ..Default::default()
                })
                .boxed(),
            column![
                text(b.title)
                    .size(TS.body)
                    .font(Font {
                        weight: iced::font::Weight::Medium,
                        ..Font::DEFAULT
                    })
                    .color(ink)
                    .boxed(),
                text(b.status).size(TS.caption).color(status_ink).boxed(),
            ]
            .spacing(SP.xs)
            .width(Length::Fill)
            .boxed(),
            pick,
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center),
    )
    .padding([SP.m, SP.m + SP.xs])
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: fill,
        border: Border {
            width: 1.0,
            color: edge,
            radius: RADII.card.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    container(card)
        .padding(Padding {
            top: SP.xs,
            right: SP.m,
            bottom: SP.s,
            left: SP.m,
        })
        .width(Length::Fill)
        .boxed()
}

/// A small outlined button (Pick…): field fill, hairline, small radius;
/// faded while disabled.
fn small_button<'a>(
    label: Localized,
    on_press: Option<PanelMsg>,
    tokens: &ThemeTokens,
) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let (ink, fill, edge) = (ti(tokens.text), r.field, ti(tokens.border));
    let hover = composite_over(overlay(tokens, Overlay::Hover), fill);
    button(text(label).size(TS.caption))
        .padding([SP.xs, SP.m])
        .on_press_maybe(on_press)
        .style(move |_: &Theme, status: button::Status| {
            let style = button::Style {
                background: Some(Background::Color(match status {
                    button::Status::Hovered | button::Status::Pressed => hover,
                    _ => fill,
                })),
                text_color: ink,
                border: Border {
                    width: 1.0,
                    color: edge,
                    radius: RADII.small.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            };
            if status == button::Status::Disabled {
                dimmed_button(style)
            } else {
                style
            }
        })
        .boxed()
}

/// What the Graphite Properties dock shows for the active Component editor tab:
/// the row's own data, nothing invented.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentSheet {
    pub internal_pn: String,
    pub manufacturer: String,
    pub mpn: String,
    pub lifecycle: String,
    pub version: String,
    pub library: String,
    pub table: String,
    pub updated: String,
}

/// The Component editor tab's sheet: Component and Storage sections of
/// read-only fields, the look of a library row's Properties.
pub fn view_component_sheet<'a>(
    sheet: &'a ComponentSheet,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let mut body: Column<Element<'a, PanelMsg>> = column![section_label(
        t!(i18n, ids::PANELS_COMPONENT_SHEET_SECTION_COMPONENT),
        tokens,
        i18n
    )]
    .width(Length::Fill);
    let component = [
        (
            ids::PANELS_COMPONENT_SHEET_INTERNAL_PN,
            sheet.internal_pn.as_str(),
            true,
        ),
        (ids::PANELS_COMPONENT_SHEET_MPN, sheet.mpn.as_str(), true),
        (
            ids::PANELS_COMPONENT_SHEET_MANUFACTURER,
            sheet.manufacturer.as_str(),
            false,
        ),
        (
            ids::PANELS_COMPONENT_SHEET_LIFECYCLE,
            sheet.lifecycle.as_str(),
            false,
        ),
        (
            ids::PANELS_COMPONENT_SHEET_VERSION,
            sheet.version.as_str(),
            true,
        ),
    ];
    for (label, value, mono) in component {
        body = body.push(field_row(label, value, mono, tokens, i18n));
    }
    body = body.push(section_label(
        t!(i18n, ids::PANELS_COMPONENT_SHEET_SECTION_STORAGE),
        tokens,
        i18n,
    ));
    let storage = [
        (
            ids::PANELS_COMPONENT_SHEET_LIBRARY,
            sheet.library.as_str(),
            true,
        ),
        (
            ids::PANELS_COMPONENT_SHEET_TABLE,
            sheet.table.as_str(),
            true,
        ),
        (
            ids::PANELS_COMPONENT_SHEET_UPDATED,
            sheet.updated.as_str(),
            false,
        ),
    ];
    for (label, value, mono) in storage {
        body = body.push(field_row(label, value, mono, tokens, i18n));
    }
    body.boxed()
}

#[cfg(test)]
#[path = "library_row_tests.rs"]
mod tests;
