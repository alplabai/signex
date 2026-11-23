//! The Parameters column of the Edit Component modal (mockup row 130): the
//! section header with "+ Add Parameter", the missing-template-slot offer,
//! and a boxed dense table Key | Value | Unit | ✕. Template rows carry a
//! "template" pill and no ✕; a measurement's number and its menu are one
//! measurement slot (mockup row 139).

use iced::widget::{Column, Row, Space, column, container, row};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget};
use signex_i18n::{I18n, UserText, ids, t};
use signex_library::{
    ParamKind, ParamSlot, ParamValue, ParameterTemplate, slot_problem, slot_unit,
};
use signex_types::quantity::{SiPrefix, Unit};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, raw, text, theme_ext};

use super::measurement_slot::{NUMBER_SHARE, PICKER_SHARE, measurement_slot};
use super::messages::{BrowserEditMsg, LibraryMessage};
use super::modal_chrome::secondary_btn;
use super::state::EditRowModalState;
use super::unit_menu::{UnitMenu, measurement_menu, prefix_menu};
use super::unit_picker::unit_of_token;
use crate::styles::{HAIRLINE, PillTone, Radii};

const TS: crate::styles::TypeScale = crate::styles::TypeScale::DEFAULT;
const SP: crate::styles::Spacing = crate::styles::Spacing::DEFAULT;

/// Column shares of the table, then the ✕ column. The measurement slot
/// spans Value and Unit, its picker under the Unit heading.
const KEY_SHARE: u16 = 3;
const VALUE_SHARE: u16 = NUMBER_SHARE;
const UNIT_SHARE: u16 = PICKER_SHARE;
/// Width of the ✕ column.
const REMOVE_COL_W: f32 = 32.0;
/// Horizontal padding of a table cell (8 px).
const CELL_PAD_X: f32 = SP.m;

/// The column, top to bottom. `class_label` is the display name of the
/// class whose template applies, for the caption under the table.
pub(super) fn view<'a>(
    edit: &'a EditRowModalState,
    template: Option<&'a ParameterTemplate>,
    class_label: Option<String>,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    send: impl Fn(BrowserEditMsg) -> LibraryMessage + Clone + 'a,
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let header = row![
        text(t!(i18n, ids::LIBRARY_EDIT_ROW_PARAMETERS))
            .size(TS.caption)
            .color(r.ink4)
            .width(Length::Fill),
        secondary_btn(
            t!(i18n, ids::LIBRARY_EDIT_ROW_ADD_PARAMETER),
            send(BrowserEditMsg::AddParam),
            tokens,
        ),
    ]
    .align_y(Alignment::Center);

    let mut col: Column<Element<'a, LibraryMessage>> = column![header].spacing(SP.m);
    if let Some(offer) = slot_offer(edit, template, tokens, i18n, &send) {
        col = col.push(offer);
    }
    col = col.push(table(edit, template, tokens, i18n, &send));
    if let Some(class) = class_label.filter(|_| template.is_some()) {
        col = col.push(
            text(t!(i18n, ids::LIBRARY_EDIT_ROW_TEMPLATE_CAPTION, "class" => class))
                .size(TS.caption)
                .color(r.ink4)
                .boxed(),
        );
    }
    // A neutral note from the last edit (a unit change) stays a muted
    // caption under the table.
    if let Some(note) = edit.note.as_ref() {
        col = col.push(
            text(UserText::new(note.clone()))
                .size(TS.caption)
                .color(r.ink4)
                .boxed(),
        );
    }
    col.width(Length::Fill).boxed()
}

/// The boxed table: a header row, then one row per parameter (the tags
/// have their own Identity field and are left out).
fn table<'a>(
    edit: &'a EditRowModalState,
    template: Option<&'a ParameterTemplate>,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    send: &(impl Fn(BrowserEditMsg) -> LibraryMessage + Clone + 'a),
) -> Element<'a, LibraryMessage> {
    let r = crate::styles::tokens::resolve(tokens);
    let heading = |id| {
        text(t!(i18n, id))
            .size(TS.caption)
            .color(r.ink3)
            .width(Length::Fill)
            .boxed()
    };
    let sunken = r.sunken;
    let header_row = container(
        row![
            cell(heading(ids::LIBRARY_EDIT_ROW_KEY), KEY_SHARE),
            cell(heading(ids::LIBRARY_EDIT_ROW_VALUE), VALUE_SHARE),
            cell(heading(ids::LIBRARY_EDIT_ROW_UNIT), UNIT_SHARE),
            Space::new().width(REMOVE_COL_W),
        ]
        .align_y(Alignment::Center)
        .padding([SP.s + SP.xs, 0.0]),
    )
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(sunken)),
        ..container::Style::default()
    });

    let mut rows: Column<Element<'a, LibraryMessage>> = column![header_row];
    let visible: Vec<(&String, &ParamValue)> = edit
        .draft
        .parameters
        .iter()
        .filter(|(k, _)| k.as_str() != "tags")
        .collect();
    if visible.is_empty() {
        rows = rows.push(rule(tokens)).push(
            container(
                text(t!(i18n, ids::LIBRARY_EDIT_ROW_NO_PARAMETERS))
                    .size(TS.body)
                    .color(r.ink4),
            )
            .padding([SP.m, CELL_PAD_X])
            .boxed(),
        );
    }
    // BTreeMap key order keeps the layout stable.
    for (key, value) in visible {
        let slot = template.and_then(|t| t.slot(key));
        rows = rows
            .push(rule(tokens))
            .push(param_row(edit, key, value, slot, tokens, i18n, send));
    }

    let line = theme_ext::border_color(tokens);
    container(rows)
        .width(Length::Fill)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: 1.0,
                radius: Radii::GRAPHITE.button.into(),
                color: line,
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// A table cell of `share` width with the cell padding.
fn cell<'a>(body: Element<'a, LibraryMessage>, share: u16) -> Element<'a, LibraryMessage> {
    container(body)
        .padding([0.0, CELL_PAD_X])
        .width(Length::FillPortion(share))
        .boxed()
}

/// The soft hairline between two table rows.
fn rule<'a>(tokens: &ThemeTokens) -> Element<'a, LibraryMessage> {
    let soft = crate::styles::tokens::resolve(tokens).line_soft;
    container(Space::new())
        .width(Length::Fill)
        .height(HAIRLINE)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(soft)),
            ..container::Style::default()
        })
        .boxed()
}

/// The template slots the row holds no value for yet, one button each; a
/// pick adds the slot in its own kind and unit.
fn slot_offer<'a>(
    edit: &'a EditRowModalState,
    template: Option<&'a ParameterTemplate>,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    send: &(impl Fn(BrowserEditMsg) -> LibraryMessage + Clone + 'a),
) -> Option<Element<'a, LibraryMessage>> {
    let buttons: Vec<Element<'a, LibraryMessage>> = template?
        .missing_slots(&edit.draft.parameters)
        .map(|slot| {
            let label = match slot_unit(slot) {
                Some(u) => t!(
                    i18n,
                    ids::LIBRARY_EDIT_ROW_ADD_SLOT_UNIT,
                    "name" => slot.name.as_str(),
                    "unit" => u.glyph()
                ),
                None => t!(i18n, ids::LIBRARY_EDIT_ROW_ADD_SLOT, "name" => slot.name.as_str()),
            };
            secondary_btn(
                label,
                send(BrowserEditMsg::AddSlotParam {
                    name: slot.name.clone(),
                }),
                tokens,
            )
        })
        .collect();
    if buttons.is_empty() {
        return None;
    }
    Some(
        column![
            text(t!(i18n, ids::LIBRARY_EDIT_ROW_TEMPLATE_OFFER))
                .size(TS.caption)
                .color(crate::styles::tokens::resolve(tokens).ink4),
            Row::with_children(buttons).spacing(SP.s).wrap(),
        ]
        .spacing(SP.xs + SP.xs)
        .boxed(),
    )
}

/// One parameter row: the key (with the "template" pill on a template
/// slot), the value and unit as one measurement slot, and ✕ on a custom
/// row. A class-template measurement slot fixes the unit (its menu holds
/// only prefixes) and its sign rules apply; any other parameter picks its
/// unit.
fn param_row<'a>(
    edit: &'a EditRowModalState,
    key: &str,
    value: &ParamValue,
    slot: Option<&ParamSlot>,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    send: &(impl Fn(BrowserEditMsg) -> LibraryMessage + Clone + 'a),
) -> Element<'a, LibraryMessage> {
    let key_owned = key.to_string();

    let fixed_unit = slot
        .filter(|s| s.kind == ParamKind::Measurement)
        .and_then(slot_unit);
    let (cur_value, buf_unit) = match edit.param_buf.get(key) {
        Some((v, u)) => (v.clone(), u.clone()),
        None => EditRowModalState::param_buf_entry(value),
    };
    // The token the row edits in: the slot's, else the buffer's.
    let cur_unit = fixed_unit
        .as_ref()
        .map(|u| u.token().to_string())
        .unwrap_or(buf_unit);

    let picker = slot_picker(
        edit,
        key,
        fixed_unit.as_ref(),
        &cur_unit,
        tokens,
        i18n,
        send,
    );
    let send_v = send.clone();
    let key_v = key_owned.clone();
    let field = measurement_slot(
        &cur_value,
        t!(i18n, ids::LIBRARY_EDIT_ROW_VALUE_PLACEHOLDER),
        move |s| {
            send_v(BrowserEditMsg::SetParamValue {
                key: key_v.clone(),
                value: s,
            })
        },
        Some(send(BrowserEditMsg::CommitParam {
            key: key_owned.clone(),
        })),
        Some(picker),
        tokens,
    );
    // A stored value that already breaks its slot stays as it is until it
    // is edited; say so under it.
    let value_cell: Element<'a, LibraryMessage> = match slot.and_then(|s| slot_problem(s, value)) {
        Some(why) => column![
            field,
            text(t!(i18n, ids::LIBRARY_EDIT_ROW_SLOT_PROBLEM, "reason" => why))
                .size(TS.caption)
                .color(theme_ext::error_color(tokens))
        ]
        .spacing(SP.xs)
        .boxed(),
        None => field,
    };

    let name = text(UserText::new(key_owned.clone()))
        .size(TS.body)
        .color(theme_ext::text_primary(tokens));
    let key_cell: Element<'a, LibraryMessage> = if slot.is_some() {
        row![name, template_pill(tokens, i18n)]
            .spacing(SP.s)
            .align_y(Alignment::Center)
            .boxed()
    } else {
        name.boxed()
    };

    row![
        cell(key_cell, KEY_SHARE),
        cell(value_cell, VALUE_SHARE + UNIT_SHARE),
        remove_cell(slot.is_some(), key_owned, tokens, send),
    ]
    .align_y(Alignment::Center)
    .padding([SP.xs, 0.0])
    .boxed()
}

/// The ✕ that removes a custom row. A template row keeps the column empty:
/// its parameters come from the class.
fn remove_cell<'a>(
    is_template: bool,
    key: String,
    tokens: &ThemeTokens,
    send: &(impl Fn(BrowserEditMsg) -> LibraryMessage + Clone + 'a),
) -> Element<'a, LibraryMessage> {
    if is_template {
        return Space::new().width(REMOVE_COL_W).boxed();
    }
    let send = send.clone();
    let muted = crate::styles::tokens::resolve(tokens).ink4;
    container(
        // raw: a remove glyph, not language.
        button(text(raw("✕")).size(TS.body).color(muted))
            .padding([SP.xs, SP.s])
            .on_press_with(move || send(BrowserEditMsg::DeleteParam { key: key.clone() }))
            .style(crate::styles::ghost_button(tokens)),
    )
    .width(REMOVE_COL_W)
    .align_x(iced::alignment::Horizontal::Center)
    .boxed()
}

/// What a row's slot shows for its unit.
#[derive(Debug, PartialEq)]
enum UnitSegment<'u> {
    /// A fixed unit the prefix menu's face already names (`kΩ`).
    InPrefix,
    /// A fixed unit that takes no prefix: a muted label.
    Fixed(&'u Unit),
    /// A row's own known unit, or none: the unit menu.
    Menu(Option<Unit>),
    /// A unit string the build does not know: read-only, as written.
    KeptAsWritten(&'u str),
}

fn unit_segment_of<'u>(fixed: Option<&'u Unit>, cur_unit: &'u str) -> UnitSegment<'u> {
    match (fixed, unit_of_token(cur_unit)) {
        (Some(u), _) if u.takes_prefix() => UnitSegment::InPrefix,
        (Some(u), _) => UnitSegment::Fixed(u),
        (None, Ok(selected)) => UnitSegment::Menu(selected),
        (None, Err(written)) => UnitSegment::KeptAsWritten(written),
    }
}

/// The right part of a row's slot, always one control (mockup row 139): a
/// fixed unit's prefix menu or label, the menu of a row that picks its own
/// unit, or an unknown unit as written.
fn slot_picker<'a>(
    edit: &'a EditRowModalState,
    key: &str,
    fixed: Option<&Unit>,
    cur_unit: &str,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    send: &(impl Fn(BrowserEditMsg) -> LibraryMessage + Clone + 'a),
) -> Element<'a, LibraryMessage> {
    let fixed_prefix = fixed.and_then(|u| {
        let menu = row_menu(edit, key, Some(u.clone()), send);
        let on_prefix = |prefix| {
            send(BrowserEditMsg::ParamPrefixPicked {
                key: key.to_string(),
                prefix,
            })
        };
        prefix_menu(menu, row_prefix(edit, key), on_prefix, tokens, i18n)
    });
    fixed_prefix
        .or_else(|| {
            let segment = unit_segment_of(fixed, cur_unit);
            unit_segment(edit, key, segment, tokens, i18n, send)
        })
        .unwrap_or_else(|| Space::new().boxed())
}

/// The prefix the row's number is in.
fn row_prefix(edit: &EditRowModalState, key: &str) -> SiPrefix {
    edit.param_prefix.get(key).copied().unwrap_or(SiPrefix::One)
}

/// The open state and events of a row's menu.
fn row_menu(
    edit: &EditRowModalState,
    key: &str,
    selected: Option<Unit>,
    send: &impl Fn(BrowserEditMsg) -> LibraryMessage,
) -> UnitMenu<LibraryMessage> {
    UnitMenu {
        selected,
        is_open: edit.unit_menu_open.as_deref() == Some(key),
        on_open: send(BrowserEditMsg::UnitMenuOpened {
            key: key.to_string(),
        }),
        on_dismiss: send(BrowserEditMsg::UnitMenuDismissed),
    }
}

/// The unit part of a slot; `None` when the prefix menu already names it.
fn unit_segment<'a>(
    edit: &'a EditRowModalState,
    key: &str,
    segment: UnitSegment<'_>,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    send: &(impl Fn(BrowserEditMsg) -> LibraryMessage + Clone + 'a),
) -> Option<Element<'a, LibraryMessage>> {
    let ink4 = crate::styles::tokens::resolve(tokens).ink4;
    let label = |glyph: &str| {
        container(
            text(UserText::new(glyph.to_string()))
                .size(TS.body)
                .font(crate::fonts::iced_font_for_family(
                    crate::fonts::CHROME_MONO_FONT,
                ))
                .color(ink4),
        )
        .padding([SP.xs + SP.xs, SP.s + SP.xs])
    };
    match segment {
        UnitSegment::InPrefix => None,
        UnitSegment::Fixed(u) => Some(label(u.glyph()).boxed()),
        UnitSegment::Menu(selected) => Some(menu_segment(edit, key, selected, tokens, i18n, send)),
        UnitSegment::KeptAsWritten(written) => Some(
            signex_widgets::tooltip(
                label(written),
                container(text(t!(i18n, ids::LIBRARY_EDIT_ROW_UNIT_KEPT)).size(TS.caption))
                    .padding([SP.xs, SP.s + SP.xs])
                    .style(crate::styles::context_menu(tokens)),
                iced::widget::tooltip::Position::Bottom,
            )
            .boxed(),
        ),
    }
}

/// The one menu of a row that picks its own unit: prefixes and units when
/// the unit takes prefixes, else units.
fn menu_segment<'a>(
    edit: &'a EditRowModalState,
    key: &str,
    selected: Option<Unit>,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    send: &(impl Fn(BrowserEditMsg) -> LibraryMessage + Clone + 'a),
) -> Element<'a, LibraryMessage> {
    let menu = row_menu(edit, key, selected, send);
    let on_prefix = |prefix| {
        send(BrowserEditMsg::ParamPrefixPicked {
            key: key.to_string(),
            prefix,
        })
    };
    let on_pick = |unit| {
        send(BrowserEditMsg::SetParamUnit {
            key: key.to_string(),
            unit,
        })
    };
    let prefix = row_prefix(edit, key);
    measurement_menu(menu, prefix, on_prefix, on_pick, tokens, i18n)
}

/// The "template" pill after a template slot's key.
fn template_pill<'a>(tokens: &ThemeTokens, i18n: &I18n) -> Element<'a, LibraryMessage> {
    container(text(t!(i18n, ids::LIBRARY_EDIT_ROW_TEMPLATE_PILL)).size(TS.caption))
        .padding([0.0, SP.s + SP.xs])
        .style(crate::styles::pill(tokens, PillTone::Neutral))
        .boxed()
}

#[cfg(test)]
mod slot_tests {
    use super::*;

    /// Mockup row 139: a fixed unit with prefixes lives in the prefix
    /// picker, so the slot shows no second unit.
    #[test]
    fn the_slot_shows_one_unit() {
        let ohm = Unit::Ohm;
        let pct = Unit::Percent;
        assert_eq!(unit_segment_of(Some(&ohm), "ohm"), UnitSegment::InPrefix);
        assert_eq!(unit_segment_of(Some(&pct), "%"), UnitSegment::Fixed(&pct));
        assert_eq!(
            unit_segment_of(None, "ohm"),
            UnitSegment::Menu(Some(Unit::Ohm))
        );
        assert_eq!(unit_segment_of(None, ""), UnitSegment::Menu(None));
        assert_eq!(
            unit_segment_of(None, "furlong"),
            UnitSegment::KeptAsWritten("furlong")
        );
    }
}
