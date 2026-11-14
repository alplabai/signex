//! The Update PCB table (#2371): the column headings, then each group's
//! heading (fold, tri-state box, `n of m`) and its one-line rows (tick,
//! Action, Object, Detail, Status with the inline fixes).

use iced::widget::{Column, Space, container, mouse_area, row, tooltip as tip};
use iced::{Alignment, Background, Element, Length, Theme, Widget as _};
use signex_engine::pcb::eco::{EcoBlock, EcoChangeList, EcoKey, EcoRow};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{CheckboxLabel as _, checkbox, text, tooltip};

use super::{SP, TS, link, texts, up};
use crate::app::update_pcb::table::{group_ticks, rows_of};
use crate::app::update_pcb::{RowAction, TableGroup, UpdatePcbState};
use crate::app::{Message, UpdatePcbMsg};
use crate::panels::form_look::{checkbox_style, ghost_button, mixed_checkbox_style};
use crate::styles::{ti, tokens};

/// Width of the tick column, from the body font.
const TICK_W: f32 = TS.body * 2.0;
/// Width of the Action column: an icon and the longest action word.
const ACTION_W: f32 = TS.body * 7.0;
/// Object, Detail and Status share the rest in these parts.
const OBJECT_PART: u16 = 2;
const DETAIL_PART: u16 = 4;
const STATUS_PART: u16 = 4;

/// The column headings, fixed above the scrolling table.
pub(super) fn headings<'a>(i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let muted = tokens::muted_text(tokens);
    let head = |id| text(t!(i18n, id)).size(TS.caption).color(muted);
    row![
        Space::new().width(TICK_W),
        head(ids::UPDATE_PCB_COLUMN_ACTION).width(ACTION_W),
        head(ids::UPDATE_PCB_COLUMN_OBJECT).width(Length::FillPortion(OBJECT_PART)),
        head(ids::UPDATE_PCB_COLUMN_DETAIL).width(Length::FillPortion(DETAIL_PART)),
        head(ids::UPDATE_PCB_COLUMN_STATUS).width(Length::FillPortion(STATUS_PART)),
    ]
    .spacing(SP.m)
    .padding([SP.xs, SP.xl])
    .boxed()
}

/// Every group with a shown row: its heading, then its rows unless folded.
pub(super) fn body<'a>(
    state: &UpdatePcbState,
    list: &EcoChangeList,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let mut col = Column::new().spacing(SP.xs);
    let mut any = false;
    for group in TableGroup::ALL {
        let shown: Vec<&EcoRow> = rows_of(list, group)
            .filter(|r| state.table.shows(r))
            .collect();
        if shown.is_empty() {
            continue;
        }
        any = true;
        let folded = state.table.is_folded(list, group);
        col = col.push(group_heading(list, group, folded, i18n, tokens));
        if folded {
            continue;
        }
        for r in shown {
            col = col.push(row_view(state, list, r, i18n, tokens));
        }
    }
    if !any && !list.rows.is_empty() {
        col = col.push(
            container(
                text(t!(i18n, ids::UPDATE_PCB_FILTERED_EMPTY))
                    .size(TS.body)
                    .color(tokens::muted_text(tokens)),
            )
            .padding([SP.m, SP.xl])
            .boxed(),
        );
    }
    col.boxed()
}

/// A group's fold arrow, its tri-state box (on when every open row is
/// ticked, mixed when some are; a click ticks them all unless all are) and
/// its `n of m`.
fn group_heading<'a>(
    list: &EcoChangeList,
    group: TableGroup,
    folded: bool,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ticks = group_ticks(list, group);
    let target = !ticks.all();
    let mut boxed = checkbox(ticks.all() || ticks.mixed())
        .size(TS.title)
        .labelled(t!(i18n, texts::group_id(group)))
        .text_size(TS.label);
    boxed = if ticks.mixed() {
        boxed.style(mixed_checkbox_style)
    } else {
        boxed.style(checkbox_style)
    };
    if ticks.open > 0 {
        boxed = boxed.on_toggle(move |_| up(UpdatePcbMsg::GroupToggled { group, on: target }));
    }
    let fold_id = if folded {
        ids::UPDATE_PCB_FOLD_CLOSED
    } else {
        ids::UPDATE_PCB_FOLD_OPEN
    };
    let fold = signex_widgets::button(text(t!(i18n, fold_id)).size(TS.body))
        .padding([0.0, SP.xs])
        .on_press(up(UpdatePcbMsg::GroupFolded {
            group,
            folded: !folded,
        }))
        .style(ghost_button);
    row![
        fold,
        boxed,
        Space::new().width(Length::Fill),
        text(t!(
            i18n,
            ids::UPDATE_PCB_GROUP_COUNT,
            "on" => ticks.on,
            "open" => ticks.open
        ))
        .size(TS.body)
        .color(tokens::muted_text(tokens))
    ]
    .spacing(SP.s)
    .padding([SP.s, SP.m])
    .align_y(Alignment::Center)
    .boxed()
}

/// One line: tick (none to click when blocked), Action, Object, Detail and
/// Status. A clicked row takes the focus Space ticks; a problem row is
/// greyed with its fixes in the Status column.
fn row_view<'a>(
    state: &UpdatePcbState,
    list: &EcoChangeList,
    r: &EcoRow,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let problem = r.blocked.is_some();
    let ink = if problem {
        tokens::disabled_text(tokens)
    } else {
        ti(tokens.text)
    };
    let mut tick = checkbox(r.on).size(TS.body).style(checkbox_style);
    if !problem {
        let key = r.key.clone();
        tick = tick.on_toggle(move |on| {
            up(UpdatePcbMsg::RowToggled {
                key: key.clone(),
                on,
            })
        });
    }
    let action = RowAction::of(&r.change);
    let (icon, word) = texts::action(action);
    let action_cell = row![
        text(t!(i18n, icon)).size(TS.body).color(if problem {
            ink
        } else {
            action_ink(action, tokens)
        }),
        text(t!(i18n, word)).size(TS.body).color(ink),
    ]
    .spacing(SP.s)
    .width(ACTION_W);
    let object = tooltip(
        text(UserText::new(texts::object(&r.change)))
            .size(TS.body)
            .color(ink)
            .boxed(),
        hint(texts::change(i18n, &r.change), tokens),
        tip::Position::Bottom,
    );
    let line = row![
        container(tick).width(TICK_W),
        action_cell,
        container(object).width(Length::FillPortion(OBJECT_PART)),
        text(UserText::new(texts::detail(i18n, &r.change)))
            .size(TS.body)
            .color(ink)
            .width(Length::FillPortion(DETAIL_PART)),
        container(status(state, list, r, i18n, tokens)).width(Length::FillPortion(STATUS_PART)),
    ]
    .spacing(SP.m)
    .padding([SP.xs, SP.xl])
    .align_y(Alignment::Center);
    let focused = state.table.focused.as_ref() == Some(&r.key);
    let tint = tokens::selection_tint(tokens);
    let line = container(line)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: focused.then_some(Background::Color(tint)),
            ..container::Style::default()
        });
    mouse_area(line)
        .on_press(up(UpdatePcbMsg::RowFocused(r.key.clone())))
        .boxed()
}

/// The Status cell: a problem in short with its fixes, else ✓ after a
/// passing Validate, Skipped when a row it needs is unticked, else blank.
fn status<'a>(
    state: &UpdatePcbState,
    list: &EcoChangeList,
    r: &EcoRow,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    if let Some(why) = &r.blocked {
        return problem_cell(&r.key, why, i18n, tokens);
    }
    let muted = tokens::muted_text(tokens);
    if r.on && !list.applies(r) {
        return tooltip(
            text(t!(i18n, ids::UPDATE_PCB_STATUS_SKIPPED))
                .size(TS.body)
                .color(muted)
                .boxed(),
            hint(t!(i18n, ids::UPDATE_PCB_ROW_SKIPPED), tokens),
            tip::Position::Bottom,
        )
        .boxed();
    }
    if matches!(state.validation, Some(Ok(()))) && list.applies(r) {
        return tooltip(
            text(t!(i18n, ids::UPDATE_PCB_STATUS_OK))
                .size(TS.body)
                .color(ti(tokens.success))
                .boxed(),
            hint(t!(i18n, ids::UPDATE_PCB_STATUS_OK_TOOLTIP), tokens),
            tip::Position::Bottom,
        )
        .boxed();
    }
    Space::new().boxed()
}

/// A problem in a few words (the full reason as its tooltip), then what
/// fixes it: Choose footprint..., Annotate..., and Cross Probe for a part.
fn problem_cell<'a>(
    key: &EcoKey,
    why: &EcoBlock,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let short = tooltip(
        text(texts::problem(i18n, why))
            .size(TS.body)
            .color(ti(tokens.warning))
            .boxed(),
        hint(texts::blocked(i18n, why), tokens),
        tip::Position::Bottom,
    );
    let mut cell = row![short].spacing(SP.s).align_y(Alignment::Center);
    if matches!(why, EcoBlock::MissingFootprint(_)) {
        cell = cell.push(link(
            t!(i18n, ids::UPDATE_PCB_FIX_CHOOSE_FOOTPRINT),
            up(UpdatePcbMsg::ChooseFootprintRequested(key.clone())),
        ));
    }
    if texts::needs_annotate(why) {
        cell = cell.push(link(
            t!(i18n, ids::UPDATE_PCB_FIX_ANNOTATE),
            up(UpdatePcbMsg::AnnotateRequested),
        ));
    }
    if matches!(key, EcoKey::AddFootprint { .. }) {
        let probe = signex_widgets::button(
            text(t!(i18n, ids::UPDATE_PCB_FIX_CROSS_PROBE_ICON)).size(TS.body),
        )
        .padding([0.0, SP.xs])
        .on_press(up(UpdatePcbMsg::CrossProbeRequested(key.clone())))
        .style(ghost_button);
        cell = cell.push(
            tooltip(
                probe,
                hint(t!(i18n, ids::UPDATE_PCB_FIX_CROSS_PROBE), tokens),
                tip::Position::Bottom,
            )
            .boxed(),
        );
    }
    cell.boxed()
}

/// The ink of an action's icon.
fn action_ink(action: RowAction, tokens: &ThemeTokens) -> iced::Color {
    match action {
        RowAction::Add => ti(tokens.success),
        RowAction::Change => tokens::accent(tokens),
        RowAction::Remove => tokens::danger(tokens),
    }
}

/// A small hover hint in the Graphite tooltip chrome.
fn hint<'a>(label: Localized, tokens: &ThemeTokens) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    container(text(label).size(TS.caption).color(ti(tokens.text)))
        .padding([SP.xs, SP.s])
        .style(signex_widgets::active_bar_dropdown::tooltip_style(
            r.raised,
            r.line_strong,
        ))
        .boxed()
}
