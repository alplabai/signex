//! The Library Updates card: a header naming the sheet, the summary line with
//! one pill per bump kind, the drift table (a conflict line under its part)
//! and the footer. Pure: it reads [`LibraryUpdatesState`] and nothing else.

use iced::alignment::{Horizontal, Vertical};
use iced::widget::text::Wrapping;
use iced::widget::{Row, Space, column, container, row, scrollable};
use iced::{Alignment, Background, Border, Element, Font, Length, Theme, Widget};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::schematic::StampField;
use signex_types::theme::ThemeTokens;
use signex_widgets::{checkbox, radio, raw, text};
use uuid::Uuid;

use super::{BumpKind, ConflictChoice, DriftRow, LibraryUpdateEntry, LibraryUpdatesState};
use crate::library::messages::LibraryMessage;
use crate::library::modal_chrome::{
    cancel_btn, footer_strip, header_with_note_closing, primary_btn_if, secondary_btn,
};
use crate::library::part_sync::Choice;
use crate::styles::tokens::resolve;
use crate::styles::{
    HAIRLINE, PillTone, Radii, Spacing, TypeScale, chrome_px, dialog_body, pill, ti,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Card width: five table columns, and a conflict line with two values and
/// two answers under its part.
const MODAL_W: f32 = 760.0;
/// Cap on the height of the table body; it scrolls past this, so a sheet of
/// a hundred drifted parts does not make a window-tall card.
const LIST_MAX_H: f32 = 320.0;
/// The checkbox column.
const CHECK_COL_W: f32 = chrome_px(28.0);
/// The Change column: the widest pill ("⚠ Major") and the cell padding.
const CHANGE_COL_W: f32 = chrome_px(76.0);
/// A table row.
const ROW_H: f32 = chrome_px(26.0);
/// The heading row.
const HEAD_H: f32 = chrome_px(24.0);
/// A status pill.
const PILL_H: f32 = chrome_px(16.0);
/// Gap between the summary line and the table, and between the items of a
/// conflict line.
const GAP: f32 = SP.m + SP.xs;
/// Gap between a radio and its label.
const RADIO_GAP: f32 = chrome_px(5.0);

/// Render the modal card. Returns an `Element<LibraryMessage>` so the
/// caller can `.map(Message::Library)`.
pub fn view<'a>(
    state: &'a LibraryUpdatesState,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
    windowed: bool,
) -> Element<'a, LibraryMessage> {
    let file_name = state
        .schematic_path
        .file_name()
        .and_then(|s| s.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| state.schematic_path.display().to_string());
    let title = t!(i18n, ids::DIALOG_TITLE_LIBRARY_UPDATES).into_string();
    let header = header_with_note_closing(
        &title,
        Some(file_name),
        LibraryMessage::LibraryUpdatesCancel,
        windowed,
        tokens,
    );

    let body = column![
        summary_line(state, i18n, tokens),
        drift_table(state, i18n, tokens)
    ]
    .spacing(GAP);

    container(
        column![
            header,
            dialog_body(body),
            Space::new().height(SP.l),
            footer(state, i18n, tokens)
        ]
        .width(crate::library::card_len(windowed, MODAL_W)),
    )
    .style(crate::styles::modal_card(tokens))
    .clip(true)
    .boxed()
}

/// "Skip All" at the left; Cancel and "Update Selected Components (n)",
/// inert while nothing is ticked, at the right.
fn footer<'a>(
    state: &LibraryUpdatesState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let selected = state.selected_count();
    footer_strip(
        vec![secondary_btn(
            t!(i18n, ids::LIBRARY_UPDATES_SKIP_ALL),
            LibraryMessage::LibraryUpdatesSkipAll,
            tokens,
        )],
        vec![
            cancel_btn(
                t!(i18n, ids::DIALOG_BUTTON_CANCEL),
                LibraryMessage::LibraryUpdatesCancel,
                tokens,
            ),
            primary_btn_if(
                t!(
                    i18n,
                    ids::LIBRARY_UPDATES_APPLY,
                    "count" => selected
                ),
                LibraryMessage::LibraryUpdatesApply,
                selected > 0,
                tokens,
            ),
        ],
        tokens,
    )
}

/// The message, then right-aligned one pill per bump kind present, most
/// severe first. A kind with no entry has no pill.
fn summary_line<'a>(
    state: &LibraryUpdatesState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    // Counts table rows, as the pills do: "R10, R11" is one line.
    let message = t!(i18n, ids::LIBRARY_UPDATES_SUMMARY, "count" => state.rows().len());
    let mut line = Row::<Element<'a, LibraryMessage>>::new()
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .push(
            text(message)
                .size(TS.body)
                .color(ti(tokens.text))
                .width(Length::Fill)
                .boxed(),
        );
    for kind in BumpKind::ALL {
        let count = state.count_of(kind);
        if count > 0 {
            let (id, tone) = summary_pill(kind);
            line = line.push(status_pill(t!(i18n, id, "count" => count), tone, tokens));
        }
    }
    line.boxed()
}

/// The catalog message and tone of a kind's summary pill.
fn summary_pill(kind: BumpKind) -> (MessageId, PillTone) {
    match kind {
        BumpKind::Major => (ids::LIBRARY_UPDATES_COUNT_MAJOR, PillTone::Err),
        BumpKind::Minor => (ids::LIBRARY_UPDATES_COUNT_MINOR, PillTone::Warn),
        BumpKind::Patch => (ids::LIBRARY_UPDATES_COUNT_PATCH, PillTone::Ok),
    }
}

/// The catalog message and tone of a row's Change pill.
fn change_pill(kind: BumpKind) -> (MessageId, PillTone) {
    match kind {
        BumpKind::Major => (ids::LIBRARY_UPDATES_CHANGE_MAJOR, PillTone::Err),
        BumpKind::Minor => (ids::LIBRARY_UPDATES_CHANGE_MINOR, PillTone::Warn),
        BumpKind::Patch => (ids::LIBRARY_UPDATES_CHANGE_PATCH, PillTone::Ok),
    }
}

/// The name of the field a conflict is about.
fn field_label(field: StampField) -> MessageId {
    match field {
        StampField::Footprint => ids::LIBRARY_UPDATES_FIELD_FOOTPRINT,
        StampField::Manufacturer => ids::LIBRARY_UPDATES_FIELD_MANUFACTURER,
        StampField::Mpn => ids::LIBRARY_UPDATES_FIELD_MPN,
    }
}

/// A fully rounded status chip: soft fill, toned ink, caption text.
fn status_pill<'a>(
    label: Localized,
    tone: PillTone,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    container(text(label).size(TS.caption).wrapping(Wrapping::None))
        .height(Length::Fixed(PILL_H))
        .align_y(Vertical::Center)
        .padding([0.0, SP.s + SP.xs])
        .style(pill(tokens, tone))
        .boxed()
}

/// The chrome mono face, at `weight`: versions, designators, PNs, values.
fn mono(weight: iced::font::Weight) -> Font {
    Font {
        weight,
        ..crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
    }
}

/// The UI face at medium weight: column headings and conflict field names.
fn medium() -> Font {
    Font {
        weight: iced::font::Weight::Medium,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

/// A table cell: the column's width, the cell padding, clipped, centred
/// vertically.
fn cell<'a>(content: Element<'a, LibraryMessage>, width: Length) -> Element<'a, LibraryMessage> {
    container(content)
        .width(width)
        .height(Length::Fill)
        .padding([0.0, SP.m])
        .align_y(Vertical::Center)
        .clip(true)
        .boxed()
}

/// The checkbox cell, the box centred in its column.
fn check_cell<'a>(content: Element<'a, LibraryMessage>) -> Element<'a, LibraryMessage> {
    container(content)
        .width(Length::Fixed(CHECK_COL_W))
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .boxed()
}

/// The five columns of a table row, in order: check, Part, Library,
/// Version, Change.
fn columns<'a>(
    check: Element<'a, LibraryMessage>,
    part: Element<'a, LibraryMessage>,
    library: Element<'a, LibraryMessage>,
    version: Element<'a, LibraryMessage>,
    change: Element<'a, LibraryMessage>,
) -> Row<Element<'a, LibraryMessage>> {
    Row::<Element<'a, LibraryMessage>>::new()
        .push(check_cell(check))
        .push(cell(part, Length::FillPortion(3)))
        .push(cell(library, Length::FillPortion(2)))
        .push(cell(version, Length::FillPortion(2)))
        .push(cell(change, Length::Fixed(CHANGE_COL_W)))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Alignment::Center)
}

/// A row line: `fill` behind it (none: the card's own) and a hairline rule
/// under it.
fn ruled(
    tokens: &ThemeTokens,
    fill: Option<iced::Color>,
) -> impl Fn(&Theme) -> container::Style + 'static {
    let rule = resolve(tokens).line_soft;
    move |_| container::Style {
        background: fill.map(Background::Color),
        border: Border {
            color: rule,
            bottom: iced::border::Side::default().width(HAIRLINE),
            ..Border::default()
        },
        ..container::Style::default()
    }
}

/// The bordered table: the heading row, then the rows (each followed by
/// its conflict lines) in a list that scrolls past [`LIST_MAX_H`].
fn drift_table<'a>(
    state: &'a LibraryUpdatesState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let mut rows: Vec<Element<'a, LibraryMessage>> = Vec::new();
    for drift in state.rows() {
        rows.push(entry_row(&drift, i18n, tokens));
        // A row with a conflict holds one entry (see `DriftRow`).
        let entry = drift.lead();
        for conflict in &entry.conflicts {
            rows.push(conflict_row(entry.symbol_uuid, conflict, i18n, tokens));
        }
    }
    let list = scrollable(iced::widget::column(rows).width(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fit.max(LIST_MAX_H))
        .direction(crate::styles::thin_vertical())
        .style(crate::styles::scrollbar_style(tokens));
    let edge = ti(tokens.border);
    container(column![heading_row(state, i18n, tokens), list].width(Length::Fill))
        .width(Length::Fill)
        .clip(true)
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: HAIRLINE,
                color: edge,
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The sunken heading row: the select-all checkbox (checked when every
/// entry is), then the column names in capitals.
fn heading_row<'a>(
    state: &LibraryUpdatesState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let heading = |id: MessageId| -> Element<'a, LibraryMessage> {
        // raw: the catalog heading in capitals, for the message language.
        text(raw(i18n.upper_for_display(&t!(i18n, id).into_string())))
            .size(TS.caption)
            .color(r.ink3)
            .font(medium())
            .wrapping(Wrapping::None)
            .boxed()
    };
    let select_all = checkbox(state.all_selected())
        .style(crate::panels::form_look::checkbox_style)
        .size(TS.heading);
    // Nothing to tick: the box stays inert.
    let select_all = if state.entries.is_empty() {
        select_all
    } else {
        select_all.on_toggle(|_| LibraryMessage::LibraryUpdatesSelectAllToggled)
    };
    container(columns(
        select_all.boxed(),
        heading(ids::LIBRARY_UPDATES_COLUMN_PART),
        heading(ids::LIBRARY_UPDATES_COLUMN_LIBRARY),
        heading(ids::LIBRARY_UPDATES_COLUMN_VERSION),
        heading(ids::LIBRARY_UPDATES_COLUMN_CHANGE),
    ))
    .width(Length::Fill)
    .height(Length::Fixed(HEAD_H))
    .style(ruled(tokens, Some(r.sunken)))
    .boxed()
}

/// One drifted part: its checkbox, the designators in bold mono with the
/// Internal PN muted beside them, the library, "current → latest" in mono
/// and the Change pill.
fn entry_row<'a>(
    drift: &DriftRow<'a>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let ink = ti(tokens.text);
    let entry: &'a LibraryUpdateEntry = drift.lead();
    let symbol_uuid = entry.symbol_uuid;
    let check = checkbox(entry.selected)
        .style(crate::panels::form_look::checkbox_style)
        .size(TS.heading)
        .on_toggle(move |_| LibraryMessage::LibraryUpdatesToggleSelection(symbol_uuid))
        .boxed();

    let mut part = Row::<Element<'a, LibraryMessage>>::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(
            text(UserText::new(drift.ref_des()))
                .size(TS.body)
                .color(ink)
                // Medium is the heaviest bundled mono face.
                .font(mono(iced::font::Weight::Medium))
                .wrapping(Wrapping::None)
                .boxed(),
        );
    if !entry.internal_pn.is_empty() {
        part = part.push(
            text(UserText::new(&entry.internal_pn))
                .size(TS.body)
                .color(r.ink4)
                .font(mono(iced::font::Weight::Normal))
                .wrapping(Wrapping::None)
                .boxed(),
        );
    }

    let library = text(UserText::new(&entry.library_name))
        .size(TS.body)
        .color(ink)
        .wrapping(Wrapping::None)
        .boxed();
    let version = text(t!(
        i18n,
        ids::LIBRARY_UPDATES_VERSION,
        "current" => entry.current_version.as_str(),
        "latest" => entry.latest_version.as_str()
    ))
    .size(TS.body)
    .color(ink)
    .font(mono(iced::font::Weight::Normal))
    .wrapping(Wrapping::None)
    .boxed();
    let (change_id, tone) = change_pill(entry.bump_kind);
    let change = status_pill(t!(i18n, change_id), tone, tokens);

    container(columns(check, part.boxed(), library, version, change))
        .width(Length::Fill)
        .height(Length::Fixed(ROW_H))
        .style(ruled(tokens, None))
        .boxed()
}

/// A conflict under its part, on the soft warning fill from the Part
/// column on: the field, "yours" and the part's value, "library now" and the
/// row's value, then the two answers at the right.
fn conflict_row<'a>(
    symbol_uuid: Uuid,
    conflict: &'a ConflictChoice,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, LibraryMessage> {
    let r = resolve(tokens);
    let ink = ti(tokens.text);
    let field = conflict.field;
    let pick = move |choice| LibraryMessage::LibraryUpdatesConflictAnswered {
        symbol_uuid,
        field,
        choice,
    };
    let caption = |id: MessageId| -> Element<'a, LibraryMessage> {
        text(t!(i18n, id))
            .size(TS.caption)
            .color(r.ink4)
            .wrapping(Wrapping::None)
            .boxed()
    };
    let value = |v: &'a String| -> Element<'a, LibraryMessage> {
        text(UserText::new(v))
            .size(TS.body)
            .color(ink)
            .font(mono(iced::font::Weight::Normal))
            .wrapping(Wrapping::None)
            .boxed()
    };
    let answer = |id: MessageId, choice: Choice| -> Element<'a, LibraryMessage> {
        radio(t!(i18n, id), choice, Some(conflict.choice), pick)
            .size(TS.heading)
            .spacing(RADIO_GAP)
            .text_size(TS.body)
            .boxed()
    };
    let line = row![
        text(t!(i18n, field_label(field)))
            .size(TS.body)
            .color(ink)
            .font(medium())
            .wrapping(Wrapping::None),
        caption(ids::LIBRARY_UPDATES_CONFLICT_YOURS),
        value(&conflict.mine),
        caption(ids::LIBRARY_UPDATES_CONFLICT_LIBRARY_NOW),
        value(&conflict.library),
        Space::new().width(Length::Fill),
        answer(ids::LIBRARY_UPDATES_KEEP_MINE, Choice::KeepMine),
        answer(ids::LIBRARY_UPDATES_USE_LIBRARY, Choice::UseLibrary),
    ]
    .spacing(GAP)
    .align_y(Alignment::Center);

    let lit = container(line)
        .width(Length::Fill)
        .padding(iced::Padding {
            top: SP.s + SP.xs,
            right: SP.m,
            bottom: SP.m,
            left: SP.m,
        })
        .style(ruled(tokens, Some(r.warn_soft)));
    container(row![Space::new().width(Length::Fixed(CHECK_COL_W)), lit].width(Length::Fill))
        .width(Length::Fill)
        .style(ruled(tokens, None))
        .boxed()
}
