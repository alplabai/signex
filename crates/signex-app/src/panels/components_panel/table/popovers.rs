//! The popovers of the table, drawn as a card under the toolbar: the
//! Columns picker, the Filter popover with its range filters, a header's
//! menu and the "+ Group" column menu.

use iced::widget::{Column, column, container, pick_list, row};
use iced::{Alignment, Element, Length, Widget};
use signex_i18n::{I18n, ids, t};
use signex_library::templates::ColumnKey;
use signex_library::{RangeOn, RangeOp};
use signex_types::quantity::{SiPrefix, Unit};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{PickListPlaceholder as _, button, text, text_input};

use super::super::{HINT_TEXT_SIZE, ROW_TEXT_SIZE, SP};
use super::columns::caption;
use super::state::{ComponentsTable, TablePopover};
use super::update::group_choices;
use super::view::user_text;
use crate::library::messages::{
    ComponentsTableMsg, HeaderAction, LibraryMessage, RangeBound, RangeFilterMsg,
};
use crate::library::range_filter::{BoundDraft, FilterColumn, RangeFilterDraft};
use crate::styles::ti;
use crate::styles::tokens::resolve;

fn msg(m: ComponentsTableMsg) -> LibraryMessage {
    LibraryMessage::ComponentsTable(m)
}

fn range(m: RangeFilterMsg) -> LibraryMessage {
    msg(ComponentsTableMsg::RangeFilter(m))
}

pub(super) fn view<'a>(
    table: &'a ComponentsTable,
    popover: &'a TablePopover,
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
) -> Element<'a, LibraryMessage> {
    let body = match popover {
        TablePopover::Columns => columns_picker(table, tokens, i18n),
        TablePopover::Filter => filter_popover(table, tokens, i18n),
        TablePopover::HeaderMenu(column) => header_menu(table, column, tokens, i18n),
        TablePopover::GroupMenu => group_menu(table, tokens, i18n),
    };
    container(
        container(body)
            .padding(SP.s)
            .style(crate::styles::context_menu(tokens)),
    )
    .padding([SP.xs, 10.0])
    .width(Length::Fill)
    .boxed()
}

/// A popover's title line with its ✕.
fn title_line<'a>(
    title: String,
    extra: Option<Element<'a, LibraryMessage>>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let ink = ti(tokens.text);
    let mut line = row![
        text(user_text(title))
            .size(ROW_TEXT_SIZE)
            .color(ink)
            .width(Length::Fill)
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    if let Some(extra) = extra {
        line = line.push(extra);
    }
    line.push(
        button(text(t!(i18n, ids::PANELS_COMPONENTS_POPOVER_CLOSE)).size(HINT_TEXT_SIZE))
            .padding([1.0, 6.0])
            .on_press(msg(ComponentsTableMsg::PopoverDismissed))
            .style(crate::panels::form_look::ghost_button),
    )
    .boxed()
}

/// One entry of a menu.
fn menu_entry<'a>(
    label: String,
    pressed: LibraryMessage,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    button(
        text(user_text(label))
            .size(ROW_TEXT_SIZE)
            .color(ti(tokens.text)),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .on_press(pressed)
    .style(crate::styles::menu_item(tokens))
    .boxed()
}

fn header_menu<'a>(
    table: &'a ComponentsTable,
    column: &'a ColumnKey,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let setup = &table.cache.setup;
    let in_sort = setup.sort.iter().any(|k| k.column == *column);
    let in_group = setup.group.iter().any(|k| k.column == *column);
    let say = |id| i18n.message_plain(id, &[]);
    let pick = |a| msg(ComponentsTableMsg::HeaderMenuPicked(a));
    let mut entries: Vec<(String, HeaderAction)> = vec![
        (
            say(ids::PANELS_COMPONENTS_SORT_ASCENDING),
            HeaderAction::SortAscending,
        ),
        (
            say(ids::PANELS_COMPONENTS_SORT_DESCENDING),
            HeaderAction::SortDescending,
        ),
        (
            say(if in_sort {
                ids::PANELS_COMPONENTS_SORT_REMOVE
            } else {
                ids::PANELS_COMPONENTS_SORT_ADD
            }),
            HeaderAction::ToggleSort,
        ),
    ];
    if super::columns::is_groupable(column) || in_group {
        entries.push((
            say(if in_group {
                ids::PANELS_COMPONENTS_GROUP_REMOVE
            } else {
                ids::PANELS_COMPONENTS_GROUP_BY
            }),
            HeaderAction::ToggleGroup,
        ));
    }
    if *column != ColumnKey::PartNumber {
        entries.push((say(ids::PANELS_COMPONENTS_HIDE_COLUMN), HeaderAction::Hide));
    }
    entries.push((
        say(ids::PANELS_COMPONENTS_RESET_WIDTH),
        HeaderAction::ResetWidth,
    ));
    entries.push((
        say(ids::PANELS_COMPONENTS_COLUMNS_MENU),
        HeaderAction::OpenColumns,
    ));
    let mut col: Column<Element<'a, LibraryMessage>> =
        column![title_line(caption(column, i18n), None, tokens, i18n)].spacing(0);
    for (label, action) in entries {
        col = col.push(menu_entry(label, pick(action), tokens));
    }
    col.boxed()
}

fn group_menu<'a>(
    table: &'a ComponentsTable,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let title = i18n.message_plain(ids::PANELS_COMPONENTS_GROUP_BY_TITLE, &[]);
    let mut col: Column<Element<'a, LibraryMessage>> =
        column![title_line(title, None, tokens, i18n)].spacing(0);
    for column in group_choices(&table.cache.setup) {
        let label = caption(&column, i18n);
        col = col.push(menu_entry(
            label,
            msg(ComponentsTableMsg::GroupAdded(column)),
            tokens,
        ));
    }
    col.boxed()
}

/// The Columns picker: the category's parameters, then the shared row
/// fields, each with its check box, how many parts hold it and ↑ ↓; which
/// setup is in effect and Reset to category default under them.
fn columns_picker<'a>(
    table: &'a ComponentsTable,
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
) -> Element<'a, LibraryMessage> {
    let cache = &table.cache;
    let category = cache
        .categories
        .iter()
        .find(|e| e.key == table.category)
        .map(|e| e.label.clone())
        .unwrap_or_default();
    let title = i18n.message_plain(
        ids::PANELS_COMPONENTS_COLUMNS_TITLE,
        &[("category", category.as_str().into())],
    );
    let search = text_input(
        t!(i18n, ids::PANELS_COMPONENTS_COLUMNS_SEARCH),
        &table.column_search,
    )
    .on_input(|s| msg(ComponentsTableMsg::ColumnSearchChanged(s)))
    .padding(SP.s)
    .size(ROW_TEXT_SIZE);
    let needle = i18n.fold_for_search(table.column_search.trim());
    let choices = cache.setup.column_choices();
    let matches = |c: &ColumnKey| {
        needle.is_empty() || i18n.fold_for_search(&caption(c, i18n)).contains(&needle)
    };
    let (params, shared): (Vec<_>, Vec<_>) = choices
        .into_iter()
        .partition(|(c, _)| c.param_name().is_some());
    let mut col: Column<Element<'a, LibraryMessage>> =
        column![title_line(title, None, tokens, i18n), search,].spacing(SP.xs);
    let total = cache.base.len();
    let mut any = false;
    for (heading, list) in [
        (ids::PANELS_COMPONENTS_COLUMNS_PARAMETERS, params),
        (ids::PANELS_COMPONENTS_COLUMNS_SHARED, shared),
    ] {
        let shown: Vec<_> = list.iter().filter(|(c, _)| matches(c)).collect();
        if shown.is_empty() {
            continue;
        }
        any = true;
        col = col.push(
            text(t!(i18n, heading))
                .size(HINT_TEXT_SIZE)
                .color(resolve(tokens).ink3),
        );
        for (column, visible) in shown {
            let covered = cache.coverage.get(column).copied().unwrap_or(0);
            let origin = cache.column_origin.get(column).map(String::as_str);
            col = col.push(picker_row(
                column, *visible, covered, total, origin, tokens, i18n,
            ));
        }
    }
    if !any {
        col = col.push(
            text(t!(i18n, ids::PANELS_COMPONENTS_COLUMNS_NO_MATCH))
                .size(HINT_TEXT_SIZE)
                .color(resolve(tokens).ink3),
        );
    }
    let is_default = cache
        .setup
        .is_category_default(&cache.default_view, &cache.offered);
    let tag = if is_default {
        t!(i18n, ids::PANELS_COMPONENTS_USING_DEFAULT)
    } else {
        t!(i18n, ids::PANELS_COMPONENTS_SAVED_SETUP)
    };
    col = col.push(
        text(tag)
            .size(HINT_TEXT_SIZE)
            .color(resolve(tokens).accent_soft_ink),
    );
    col = col.push(
        text(user_text(default_summary(table, i18n)))
            .size(HINT_TEXT_SIZE)
            .color(resolve(tokens).ink3),
    );
    let mut reset =
        button(text(t!(i18n, ids::PANELS_COMPONENTS_RESET_DEFAULT)).size(HINT_TEXT_SIZE))
            .padding([2.0, 6.0])
            .style(crate::panels::form_look::ghost_button);
    if !is_default {
        reset = reset.on_press(msg(ComponentsTableMsg::ResetPressed));
    }
    col.push(reset).boxed()
}

#[allow(clippy::too_many_arguments)]
fn picker_row<'a>(
    column: &ColumnKey,
    visible: bool,
    covered: usize,
    total: usize,
    origin: Option<&str>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let ink = ti(tokens.text);
    let locked = *column == ColumnKey::PartNumber;
    let mut tick = signex_widgets::checkbox(visible)
        .size(ROW_TEXT_SIZE)
        .style(crate::panels::form_look::checkbox_style);
    if !locked {
        let column = column.clone();
        tick = tick.on_toggle(move |shown| {
            msg(ComponentsTableMsg::ColumnShown {
                column: column.clone(),
                shown,
            })
        });
    }
    let coverage = if covered == total {
        total.to_string()
    } else {
        format!("{covered}/{total}")
    };
    let mover = |later: bool, glyph: &'static str| {
        let mut b = button(text(user_text(glyph)).size(HINT_TEXT_SIZE))
            .padding([0.0, 4.0])
            .style(crate::panels::form_look::ghost_button);
        if !locked {
            b = b.on_press(msg(ComponentsTableMsg::ColumnMoved {
                column: column.clone(),
                later,
            }));
        }
        b
    };
    row![
        tick,
        text(user_text(column_label(column, origin, i18n)))
            .size(ROW_TEXT_SIZE)
            .color(ink)
            .width(Length::Fill)
            .wrapping(iced::widget::text::Wrapping::None),
        text(user_text(coverage))
            .size(HINT_TEXT_SIZE)
            .color(resolve(tokens).ink3),
        mover(false, "↑"),
        mover(true, "↓"),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center)
    .boxed()
}

/// A Columns picker entry: the caption, and the library whose own
/// template the column comes from when the global one does not offer it
/// (Caner, 2026-11-27).
pub(super) fn column_label(column: &ColumnKey, origin: Option<&str>, i18n: &I18n) -> String {
    let name = caption(column, i18n);
    match origin {
        Some(library) => i18n.message_plain(
            ids::PANELS_COMPONENTS_COLUMN_FROM,
            &[
                ("column", name.as_str().into()),
                ("library", library.into()),
            ],
        ),
        None => name,
    }
}

/// "Grouped by Package › Value · sorted by Voltage ↑ · 8 columns".
pub(super) fn default_summary(table: &ComponentsTable, i18n: &I18n) -> String {
    let view = &table.cache.default_view;
    let keys = |keys: &[signex_library::templates::SortKey], sep: &str, arrows: bool| {
        keys.iter()
            .map(|k| {
                let name = caption(&k.column, i18n);
                match (arrows, k.descending) {
                    (false, _) => name,
                    (true, false) => format!("{name} ↑"),
                    (true, true) => format!("{name} ↓"),
                }
            })
            .collect::<Vec<_>>()
            .join(sep)
    };
    let grouped = if view.group.is_empty() {
        i18n.message_plain(ids::PANELS_COMPONENTS_DEFAULT_UNGROUPED, &[])
    } else {
        i18n.message_plain(
            ids::PANELS_COMPONENTS_DEFAULT_GROUPED,
            &[("keys", keys(&view.group, " › ", false).into())],
        )
    };
    i18n.message_plain(
        ids::PANELS_COMPONENTS_DEFAULT_SUMMARY,
        &[
            ("grouped", grouped.into()),
            ("sorted", keys(&view.sort, ", ", true).into()),
            ("columns", view.columns.len().into()),
        ],
    )
}

/// The Filter popover: In stock only, the category's range filters and
/// "Add range filter…", Clear filters and the part count.
fn filter_popover<'a>(
    table: &'a ComponentsTable,
    tokens: &'a ThemeTokens,
    i18n: &'a I18n,
) -> Element<'a, LibraryMessage> {
    let filters = table.filters();
    let in_stock = filters.is_some_and(|f| f.in_stock);
    let stock = row![
        signex_widgets::checkbox(in_stock)
            .size(ROW_TEXT_SIZE)
            .style(crate::panels::form_look::checkbox_style)
            .on_toggle(|on| msg(ComponentsTableMsg::InStockToggled(on))),
        text(t!(i18n, ids::PANELS_COMPONENTS_IN_STOCK_ONLY)).size(HINT_TEXT_SIZE),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center);
    let title = i18n.message_plain(ids::PANELS_COMPONENTS_FILTER_TITLE, &[]);
    let mut col: Column<Element<'a, LibraryMessage>> =
        column![title_line(title, Some(stock.boxed()), tokens, i18n)].spacing(SP.s);
    let columns = &table.cache.range_columns;
    if !columns.is_empty() {
        col = col.push(
            text(t!(i18n, ids::PANELS_COMPONENTS_FILTER_RANGE))
                .size(HINT_TEXT_SIZE)
                .color(resolve(tokens).ink3),
        );
    }
    if let Some(f) = filters {
        for (key, draft) in f.range.iter() {
            let label = columns
                .iter()
                .find(|c| &c.key == key)
                .map_or_else(|| key.clone(), |c| c.label.clone());
            let min_to_max = table.cache.is_range(key);
            let excluded = table.cache.excluded.get(key).copied().unwrap_or(0);
            col = col.push(range_row(
                key, label, draft, min_to_max, excluded, tokens, i18n,
            ));
        }
    }
    let free: Vec<FilterColumn> = columns
        .iter()
        .filter(|c| filters.is_none_or(|f| f.range.get(&c.key).is_none()))
        .cloned()
        .collect();
    if !free.is_empty() {
        col = col.push(
            pick_list(None::<FilterColumn>, free, |c: &FilterColumn| c.to_string())
                .graphite()
                .on_select(|c: FilterColumn| {
                    range(RangeFilterMsg::ColumnPicked {
                        key: c.key,
                        unit: c.unit,
                    })
                })
                .placeholder_label(t!(i18n, ids::LIBRARY_RANGE_FILTER_ADD))
                .padding(SP.s),
        );
    }
    let count = t!(i18n, ids::PANELS_COMPONENTS_FILTER_COUNT, "count" => table.cache.shown.len());
    col.push(
        row![
            button(text(t!(i18n, ids::PANELS_COMPONENTS_CLEAR_FILTERS)).size(HINT_TEXT_SIZE))
                .padding([2.0, 6.0])
                .on_press(msg(ComponentsTableMsg::FiltersCleared))
                .style(crate::panels::form_look::ghost_button),
            iced::widget::Space::new().width(Length::Fill),
            text(count).size(HINT_TEXT_SIZE).color(resolve(tokens).ink3),
        ]
        .align_y(Alignment::Center),
    )
    .boxed()
}

/// The target of a min-to-max filter, for its picker.
#[derive(Debug, Clone, PartialEq)]
struct TargetChoice {
    on: RangeOn,
    label: String,
}

impl std::fmt::Display for TargetChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

/// A bound's prefix: Any prefix (`None`) or one the unit takes.
#[derive(Debug, Clone, PartialEq)]
struct PrefixChoice {
    prefix: Option<SiPrefix>,
    label: String,
}

impl std::fmt::Display for PrefixChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

#[allow(clippy::too_many_arguments)]
fn range_row<'a>(
    key: &str,
    label: String,
    draft: &'a RangeFilterDraft,
    min_to_max: bool,
    excluded: usize,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let ink = ti(tokens.text);
    let muted = resolve(tokens).ink3;
    let mut head = row![
        text(user_text(label))
            .size(ROW_TEXT_SIZE)
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::None)
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    if min_to_max {
        let targets: Vec<TargetChoice> = [
            (RangeOn::Covers, ids::PANELS_COMPONENTS_RANGE_COVERS),
            (RangeOn::Min, ids::PANELS_COMPONENTS_RANGE_MIN),
            (RangeOn::Max, ids::PANELS_COMPONENTS_RANGE_MAX),
        ]
        .into_iter()
        .map(|(on, id)| TargetChoice {
            on,
            label: i18n.message_plain(id, &[]),
        })
        .collect();
        let current = targets.iter().find(|c| c.on == draft.on).cloned();
        let k = key.to_owned();
        head = head.push(
            pick_list(current, targets, |c: &TargetChoice| c.to_string())
                .graphite()
                .on_select(move |c: TargetChoice| {
                    msg(ComponentsTableMsg::RangeTargetPicked {
                        key: k.clone(),
                        on: c.on,
                    })
                })
                .padding([3, 6]),
        );
    }
    let covers = draft.on == RangeOn::Covers;
    if !covers {
        let k = key.to_owned();
        head = head.push(
            pick_list(Some(draft.op), RangeOp::ALL.to_vec(), |op: &RangeOp| {
                op.to_string()
            })
            .graphite()
            .on_select(move |op: RangeOp| {
                range(RangeFilterMsg::OperatorPicked { key: k.clone(), op })
            })
            .padding([3, 6]),
        );
    }
    head = head.push(
        button(text(t!(i18n, ids::LIBRARY_RANGE_FILTER_REMOVE)).size(HINT_TEXT_SIZE))
            .padding([1.0, 6.0])
            .on_press(range(RangeFilterMsg::Cleared {
                key: key.to_owned(),
            }))
            .style(crate::panels::form_look::ghost_button),
    );
    let mut bounds = row![bound(key, false, &draft.low, &draft.unit, tokens, i18n)]
        .spacing(SP.s)
        .align_y(Alignment::Center);
    if !covers && draft.op.is_between() {
        bounds = bounds
            .push(
                text(t!(i18n, ids::LIBRARY_RANGE_FILTER_AND))
                    .size(HINT_TEXT_SIZE)
                    .color(muted),
            )
            .push(bound(key, true, &draft.high, &draft.unit, tokens, i18n));
    }
    let mut col = column![head, bounds].spacing(SP.xs);
    if let Some((words, error)) = range_status(draft, excluded, i18n) {
        let color = if error {
            signex_widgets::theme_ext::error_color(tokens)
        } else {
            muted
        };
        col = col.push(text(user_text(words)).size(HINT_TEXT_SIZE).color(color));
    }
    col.boxed()
}

/// The line under a range filter: a refused keystroke, "Enter a number",
/// how Any prefix compares, and how many parts it excluded.
pub(super) fn range_status(
    draft: &RangeFilterDraft,
    excluded: usize,
    i18n: &I18n,
) -> Option<(String, bool)> {
    if let Some(why) = [&draft.low, &draft.high]
        .into_iter()
        .find_map(|b| b.refusal.clone())
    {
        return Some((why, true));
    }
    if draft.filter().is_none() {
        return Some((
            i18n.message_plain(ids::LIBRARY_RANGE_FILTER_ENTER_NUMBER, &[]),
            false,
        ));
    }
    let mut bits = Vec::new();
    if draft.low.any_prefix || (draft.op.is_between() && draft.high.any_prefix) {
        bits.push(i18n.message_plain(ids::PANELS_COMPONENTS_ANY_PREFIX_NOTE, &[]));
    }
    if excluded > 0 {
        bits.push(i18n.message_plain(
            ids::LIBRARY_RANGE_FILTER_EXCLUDED,
            &[
                ("count", excluded.into()),
                ("unit", draft.unit.glyph().into()),
            ],
        ));
    }
    (!bits.is_empty()).then(|| (bits.join(" "), false))
}

/// A number field with its prefix picker (Any prefix first), or the plain
/// unit for a unit that takes no prefix.
fn bound<'a>(
    key: &str,
    high: bool,
    b: &'a BoundDraft,
    unit: &Unit,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, LibraryMessage> {
    let which = if high {
        RangeBound::High
    } else {
        RangeBound::Low
    };
    let k = key.to_owned();
    let number = text_input(
        t!(i18n, ids::LIBRARY_RANGE_FILTER_NUMBER_PLACEHOLDER),
        &b.text,
    )
    .on_input(move |text| {
        range(RangeFilterMsg::NumberEdited {
            key: k.clone(),
            bound: which,
            text,
        })
    })
    .padding(SP.s)
    .size(ROW_TEXT_SIZE)
    .width(Length::Fixed(84.0));
    let unit_part: Element<'a, LibraryMessage> = if unit.takes_prefix() {
        let mut choices = vec![PrefixChoice {
            prefix: None,
            label: i18n.message_plain(ids::PANELS_COMPONENTS_ANY_PREFIX, &[]),
        }];
        choices.extend(
            crate::library::prefix_picker::prefix_choices(unit)
                .into_iter()
                .map(|c| PrefixChoice {
                    prefix: Some(c.prefix),
                    label: c.to_string(),
                }),
        );
        let current = if b.any_prefix {
            choices.first().cloned()
        } else {
            let p = crate::library::prefix_picker::effective_prefix(unit, b.prefix);
            choices.iter().find(|c| c.prefix == Some(p)).cloned()
        };
        let k = key.to_owned();
        pick_list(current, choices, |c: &PrefixChoice| c.to_string())
            .graphite()
            .on_select(move |c: PrefixChoice| {
                msg(ComponentsTableMsg::BoundPrefixPicked {
                    key: k.clone(),
                    high,
                    prefix: c.prefix,
                })
            })
            .padding([3, 6])
            .boxed()
    } else {
        text(signex_i18n::UserText::new(unit.glyph()))
            .size(ROW_TEXT_SIZE)
            .color(resolve(tokens).ink3)
            .boxed()
    };
    row![number, unit_part]
        .spacing(SP.xs)
        .align_y(Alignment::Center)
        .boxed()
}
