//! The shortcut table: a bordered card with the header row, then one row
//! per binding trigger (Command, Category, Context, Shortcut, State). A
//! row shows Edit while the pointer is over it, and its "Overridden by"
//! note as a tooltip. With the recorder open, its popover floats over the
//! table just under the row it edits.

use iced::alignment::Vertical;
use iced::widget::{Column, Row, Space, column, container, hover, mouse_area, row, stack};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, shortcut_ids as sids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::{raw, text, tooltip};

use super::filters::RowState;
use super::{ButtonLook, SP, TS, context_label, page_button, recorder, stroke_labels};
use crate::keymap::{BindingConflict, KeymapEditorRow, Platform, ShortcutTrigger, category_title};
use crate::preferences::{PrefMsg, keycaps};
use crate::styles::{self, HAIRLINE, PillTone, Radii, ti, tokens};

/// Height of a header or body row, its bottom hairline included: the
/// mock's 26 px table row, a field high.
const ROW_H: f32 = styles::ChromeMetrics::DEFAULT.field;

/// Room kept under the row being edited so the recorder popover fits
/// inside the table's stack even on the last row.
const POPOVER_RESERVE: f32 = TS.body * 20.0;

/// The columns' shares of the row: Command, Category, Context, Shortcut,
/// State (34 / 15 / 14 / 22 / 15 %).
const COLUMN_SHARES: [u16; 5] = [34, 15, 14, 22, 15];

/// What the table is built from.
pub(super) struct TableInput<'v> {
    pub(super) i18n: &'v I18n,
    /// The visible rows, already filtered and sorted.
    pub(super) rows: &'v [KeymapEditorRow],
    /// What to say instead of rows when there are none.
    pub(super) empty: Option<Localized>,
    pub(super) conflicts: &'v [BindingConflict],
    pub(super) platform: Platform,
    pub(super) active_is_custom: bool,
    pub(super) recorder: Option<&'v crate::app::KeymapRecorderState>,
    pub(super) tokens: &'v ThemeTokens,
}

pub(super) fn shortcut_table<'a>(input: TableInput<'_>) -> Element<'a, PrefMsg> {
    let TableInput {
        i18n,
        rows,
        empty,
        conflicts,
        platform,
        active_is_custom,
        recorder,
        tokens,
    } = input;
    let mut lines: Vec<Element<'a, PrefMsg>> = vec![header(i18n, tokens)];
    for row_model in rows {
        let state = RowState::of(row_model, conflicts, platform, active_is_custom);
        let is_open = recorder.is_some_and(|r| r.slot == row_model.slot);
        lines.push(table_row(
            i18n,
            row_model,
            state,
            active_is_custom,
            is_open,
            platform,
            tokens,
        ));
    }
    if let Some(message) = empty {
        let ink4 = tokens::resolve(tokens).ink4;
        lines.push(
            container(text(message).size(TS.caption).color(ink4))
                .width(Length::Fill)
                .height(Length::Fixed(ROW_H))
                .center_x(Length::Fill)
                .center_y(Length::Fixed(ROW_H))
                .boxed(),
        );
    }
    let border = ti(tokens.border);
    let card = container(Column::with_children(lines))
        .width(Length::Fill)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: HAIRLINE,
                color: border,
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });

    let Some(recorder) = recorder else {
        return card.boxed();
    };
    // Anchor the popover under its row; a row the search or the chip hides
    // leaves it at the top of the table.
    let index = rows.iter().position(|r| r.slot == recorder.slot);
    let top = match index {
        Some(i) => ROW_H * (i as f32 + 2.0) + SP.xs,
        None => ROW_H + SP.xs,
    };
    let table_h = ROW_H * (rows.len().max(1) as f32 + 1.0);
    let extra = (top + POPOVER_RESERVE - table_h).max(0.0);
    let popover = mouse_area(recorder::recorder_popover(i18n, recorder, platform, tokens))
        .interaction(iced::mouse::Interaction::Idle);
    let layer = column![
        Space::new().height(Length::Fixed(top)),
        row![
            Space::new().width(Length::Fill),
            popover,
            Space::new().width(Length::Fixed(SP.l)),
        ],
    ];
    stack![
        column![card, Space::new().height(Length::Fixed(extra))],
        layer
    ]
    .boxed()
}

fn cell<'a>(content: Element<'a, PrefMsg>, share: u16) -> Element<'a, PrefMsg> {
    container(content)
        .width(Length::FillPortion(share))
        .height(Length::Fill)
        .padding([0.0, SP.m])
        .align_y(Vertical::Center)
        .clip(true)
        .boxed()
}

/// A horizontal hairline in `color`.
fn rule<'a>(color: Color) -> Element<'a, PrefMsg> {
    container(Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(HAIRLINE))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..container::Style::default()
        })
        .boxed()
}

fn medium() -> Font {
    Font {
        weight: iced::font::Weight::Medium,
        ..signex_widgets::ui_font::regular()
    }
}

/// The header: Command (sorted ascending, ▲ in the accent ink), Category,
/// Context, Shortcut and State on the sunken fill, a hairline under it.
fn header<'a>(i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, PrefMsg> {
    let r = tokens::resolve(tokens);
    let (ink, ink3) = (ti(tokens.text), r.ink3);
    let title = |id: MessageId, color: Color| -> Element<'a, PrefMsg> {
        text(t!(i18n, id))
            .size(TS.body)
            .font(medium())
            .color(color)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed()
    };
    let command = row![
        title(sids::PREF_KEYMAP_COL_COMMAND, ink),
        text(t!(i18n, ids::PREFERENCES_KEYMAP_SORTED_ASCENDING))
            .size(TS.caption)
            .color(r.accent_text),
    ]
    .spacing(SP.xs)
    .align_y(Alignment::Center);
    let [command_w, category_w, context_w, shortcut_w, state_w] = COLUMN_SHARES;
    let line = row![
        cell(command.boxed(), command_w),
        cell(title(sids::PREF_KEYMAP_COL_CATEGORY, ink3), category_w),
        cell(title(sids::PREF_KEYMAP_COL_CONTEXT, ink3), context_w),
        cell(title(sids::KEYMAP_COL_SHORTCUT, ink3), shortcut_w),
        cell(title(sids::PREF_KEYMAP_COL_STATE, ink3), state_w),
    ]
    .height(Length::Fixed(ROW_H - HAIRLINE));
    let sunken = r.sunken;
    column![
        container(line)
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(sunken)),
                ..container::Style::default()
            }),
        rule(ti(tokens.border)),
    ]
    .boxed()
}

/// One binding row. `is_open` marks the row the recorder edits.
fn table_row<'a>(
    i18n: &I18n,
    row_model: &KeymapEditorRow,
    state: RowState,
    active_is_custom: bool,
    is_open: bool,
    platform: Platform,
    tokens: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let r = tokens::resolve(tokens);
    let (ink, ink4) = (ti(tokens.text), r.ink4);
    let plain = |label: String, color: Color| -> Element<'a, PrefMsg> {
        // raw: the command label and category title are rendered from the
        // catalog when the row model is built (#2057).
        text(raw(label))
            .size(TS.body)
            .color(color)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed()
    };
    let [command_w, category_w, context_w, shortcut_w, state_w] = COLUMN_SHARES;
    let cells = row![
        cell(plain(row_model.label.clone(), ink), command_w),
        cell(
            plain(category_title(i18n, &row_model.category), ink4),
            category_w
        ),
        cell(
            text(context_label(i18n, row_model.context))
                .size(TS.body)
                .color(ink4)
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed(),
            context_w
        ),
        cell(trigger_cell(i18n, row_model, platform, tokens), shortcut_w),
        cell(state_cell(i18n, state, tokens), state_w),
    ]
    .height(Length::Fixed(ROW_H - HAIRLINE))
    .align_y(Alignment::Center);
    let selected = r.accent_soft;
    let base = container(column![cells, rule(r.line_soft)])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: is_open.then_some(Background::Color(selected)),
            ..container::Style::default()
        });

    let edit = edit_message(row_model, active_is_custom).map(|message| {
        page_button(
            t!(i18n, sids::PREF_KEYMAP_EDIT),
            Some(message),
            ButtonLook::Small,
            tokens,
        )
    });
    let hover_fill = tokens::hover_overlay(tokens);
    let mut top_line: Row<Element<'a, PrefMsg>> = Row::new()
        .height(Length::Fixed(ROW_H - HAIRLINE))
        .align_y(Alignment::Center)
        .push(Space::new().width(Length::Fill).boxed());
    if let Some(edit) = edit {
        top_line = top_line.push(edit);
    }
    let top = container(top_line)
        .width(Length::Fill)
        .padding([0.0, SP.m])
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(hover_fill)),
            ..container::Style::default()
        });
    let line: Element<'a, PrefMsg> = hover(base, top).boxed();

    // A fixed app key that claims the stroke first is named in the tooltip;
    // a row the user unbound does nothing anyway.
    match shadow_note(i18n, &row_model.trigger_config).filter(|_| !row_model.unbound) {
        Some(note) => tooltip(
            line,
            // raw: composed from the catalog and the fixed key's action name.
            container(text(raw(note)).size(TS.caption))
                .padding([SP.xs, SP.m])
                .style(styles::context_menu(tokens)),
            tooltip::Position::FollowCursor,
        )
        .boxed(),
        None => line,
    }
}

/// The recorder-open message of a row the user can re-record: a keyboard
/// binding of a command in a custom profile. Built-in profiles and pointer
/// gestures are read-only.
pub(super) fn edit_message(row_model: &KeymapEditorRow, active_is_custom: bool) -> Option<PrefMsg> {
    if !(active_is_custom && row_model.keyboard_editable) {
        return None;
    }
    let command = row_model.command.clone()?;
    Some(PrefMsg::KeymapRecorderOpen {
        slot: row_model.slot,
        command,
        label: row_model.label.clone(),
        context: row_model.context,
        trigger: row_model.trigger_config.clone(),
    })
}

/// The Shortcut cell: keycaps, "then" between strokes; a gesture as its
/// caption; a dash when unbound; an invalid draft as the warning chip.
fn trigger_cell<'a>(
    i18n: &I18n,
    row_model: &KeymapEditorRow,
    platform: Platform,
    tokens: &ThemeTokens,
) -> Element<'a, PrefMsg> {
    let ink4 = tokens::resolve(tokens).ink4;
    if row_model.trigger.trim().is_empty() {
        return text(t!(i18n, ids::PREFERENCES_KEYMAP_NO_SHORTCUT))
            .size(TS.caption)
            .color(ink4)
            .boxed();
    }
    if !row_model.keyboard_editable {
        return text(UserText::new(row_model.trigger.clone()))
            .size(TS.caption)
            .color(ink4)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed();
    }
    if !row_model.trigger_valid {
        return shortcut_chip(i18n, &row_model.trigger, ChipTone::Warning);
    }
    let then = t!(i18n, ids::PREFERENCES_KEYMAP_THEN);
    match ShortcutTrigger::parse(&row_model.trigger_config) {
        Ok(ShortcutTrigger::KeySequence(strokes)) => {
            keycaps::sequence_caps(&stroke_labels(i18n, &strokes, platform), &then, tokens)
                .unwrap_or_else(|| shortcut_chip(i18n, &row_model.trigger, ChipTone::Neutral))
        }
        _ => shortcut_chip(i18n, &row_model.trigger, ChipTone::Neutral),
    }
}

/// The State cell in its tone: Conflict and Invalid as error pills,
/// Unbound as a muted italic caption, Gesture and Reserved as neutral
/// pills, Editable and Create custom as a quiet caption.
fn state_cell<'a>(i18n: &I18n, state: RowState, tokens: &ThemeTokens) -> Element<'a, PrefMsg> {
    let r = tokens::resolve(tokens);
    let label = t!(i18n, state.label());
    match state {
        RowState::Conflict | RowState::Invalid => pill(label, PillTone::Err, tokens),
        RowState::Gesture | RowState::Reserved => pill(label, PillTone::Neutral, tokens),
        RowState::Unbound => text(label)
            .size(TS.caption)
            .color(r.ink4)
            .font(Font {
                style: iced::font::Style::Italic,
                ..signex_widgets::ui_font::regular()
            })
            .boxed(),
        RowState::Editable | RowState::CreateCustom => {
            text(label).size(TS.caption).color(r.ink3).boxed()
        }
    }
}

/// The mock's `.pill`: 16 px tall at the 12 px body, caption text.
fn pill<'a>(label: Localized, tone: PillTone, tokens: &ThemeTokens) -> Element<'a, PrefMsg> {
    let ink = styles::pill_colors(tokens, tone).1;
    let height = TS.body * 4.0 / 3.0;
    container(
        text(label)
            .size(TS.caption)
            .color(ink)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding([0.0, SP.s + SP.xs])
    .height(Length::Fixed(height))
    .center_y(Length::Fixed(height))
    .style(styles::pill(tokens, tone))
    .boxed()
}

/// Colour intent for a shortcut chip, resolved against the active theme
/// so chips read correctly on light and dark palettes alike.
#[derive(Debug, Clone, Copy)]
pub(super) enum ChipTone {
    /// Regular trigger text, on the chip's own background.
    Neutral,
    /// Invalid or transient-modifier trigger.
    Warning,
}

/// A trigger that is not keycaps (an invalid draft, a stroke the caps
/// cannot split, the recorder's held modifiers) as a small framed chip.
pub(super) fn shortcut_chip<'a>(i18n: &I18n, label: &str, tone: ChipTone) -> Element<'a, PrefMsg> {
    let label = if label.trim().is_empty() {
        i18n.message_plain(sids::PREF_KEYMAP_STATE_UNBOUND, &[])
    } else {
        label.to_string()
    };
    container(
        // raw: a key sequence shown with the catalog's key names (#2314).
        text(raw(label))
            .size(TS.body)
            .style(move |theme: &Theme| text::Style {
                color: Some(match tone {
                    ChipTone::Neutral => theme.palette().background.weak.text,
                    ChipTone::Warning => theme.palette().warning.base.color,
                }),
                ..text::Style::default()
            }),
    )
    .padding([3.0, SP.m])
    .width(Length::Shrink)
    .style(move |theme: &Theme| {
        let palette = theme.palette();
        container::Style {
            background: Some(Background::Color(palette.background.weak.color)),
            border: Border {
                width: 1.0,
                color: palette.background.strong.color,
                radius: 3.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        }
    })
    .boxed()
}

/// The "overridden by <fixed action>" note for a binding whose stroke a
/// fixed app key claims first (F1, Esc), or `None`.
pub(super) fn shadow_note(i18n: &I18n, trigger: &str) -> Option<String> {
    crate::app::dispatch::input::shadowing_fixed_key_for_trigger(trigger).map(|action| {
        i18n.message_plain(sids::PREF_KEYMAP_OVERRIDDEN, &[("action", action.into())])
    })
}
