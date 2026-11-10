//! The Number Schematic Sheets dialog (#2247): the project's pages in
//! hierarchy order, each with a number field, Auto-number and Apply. Draft
//! and rules: `app/sheet_numbering/`.

use iced::widget::{Column, Row, Space, column, container, scrollable};
use iced::{Alignment, Background, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_net::NumberingIssue;
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, text_input};

use super::sizes;
use crate::app::sheet_numbering::{NumberedRow, SheetNumberingMsg, SheetNumberingState};
use crate::app::{Message, Signex};
use crate::styles::{
    DialogButtonKind, PillTone, Spacing, TypeScale, dialog_button, dialog_footer_split, ti, tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Width of the Number column: about six digits.
const NUMBER_W: f32 = TS.body * 5.0;
/// Width of the File column.
const FILE_W: f32 = TS.body * 14.0;
/// The table scrolls past about twelve rows.
const TABLE_MAX_H: f32 = TS.body * 2.4 * 12.0;

fn sn(msg: SheetNumberingMsg) -> Message {
    Message::from(msg)
}

fn s(i18n: &I18n, id: signex_i18n::MessageId) -> String {
    t!(i18n, id).into_string()
}

impl Signex {
    pub(in crate::app::view) fn view_sheet_numbering_dialog(&self) -> Element<'_, Message> {
        self.wrap_centred_dialog(self.view_sheet_numbering_card(), sizes::SHEET_NUMBERING)
    }

    fn view_sheet_numbering_card(&self) -> Element<'_, Message> {
        let Some(state) = self.ui_state.sheet_numbering.as_ref() else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let cancel = Some(sn(SheetNumberingMsg::CancelRequested));
        let subtitle = state.read.as_ref().map(|read| {
            t!(
                i18n,
                ids::DIALOG_SHEET_NUMBERING_SUBTITLE,
                "count" => read.rows.len(),
                "project" => state.project_name.as_str()
            )
            .into_string()
        });
        let header = super::header::graphite_header_closing(
            s(i18n, ids::DIALOG_SHEET_NUMBERING_TITLE),
            subtitle,
            None,
            cancel.clone(),
            super::header::DIALOG_HEADER_H,
            tokens,
        );
        let body = container(body(state, i18n, tokens)).padding([SP.xl, SP.xl + SP.m]);
        let auto =
            signex_widgets::button(text(t!(i18n, ids::DIALOG_SHEET_NUMBERING_AUTO)).size(TS.label))
                .padding([SP.m, SP.l])
                .on_press_maybe(
                    (!state.rows().is_empty()).then(|| sn(SheetNumberingMsg::AutoNumberRequested)),
                )
                .style(crate::styles::outline_button(tokens))
                .boxed();
        let footer = dialog_footer_split(
            vec![auto],
            vec![
                dialog_button(
                    &s(i18n, ids::DIALOG_SHEET_NUMBERING_CANCEL),
                    DialogButtonKind::Cancel,
                    cancel,
                    tokens,
                ),
                dialog_button(
                    &s(i18n, ids::DIALOG_SHEET_NUMBERING_APPLY),
                    super::graphite::call_to_action(tokens),
                    state
                        .can_apply()
                        .then(|| sn(SheetNumberingMsg::ApplyRequested)),
                    tokens,
                ),
            ],
        );
        container(column![header, body, footer].width(Length::Fixed(
            super::widgets::modal_card_width(self.ui_state.window_size, sizes::SHEET_NUMBERING.0),
        )))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }
}

/// The intro, then the table (or why there is none), then the notes.
fn body<'a>(
    state: &'a SheetNumberingState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let muted = tokens::muted_text(tokens);
    let mut col: Column<Element<'a, Message>> = Column::new().spacing(SP.l);
    col = col.push(
        text(t!(i18n, ids::DIALOG_SHEET_NUMBERING_INTRO))
            .size(TS.body)
            .color(muted)
            .boxed(),
    );
    let status = match (&state.read, &state.error) {
        (None, Some(why)) => Some(t!(
            i18n,
            ids::DIALOG_SHEET_NUMBERING_READ_FAILED,
            "why" => why.as_str()
        )),
        (None, None) => Some(t!(i18n, ids::DIALOG_SHEET_NUMBERING_READING)),
        (Some(read), _) if read.rows.is_empty() => {
            Some(t!(i18n, ids::DIALOG_SHEET_NUMBERING_NO_PAGES))
        }
        (Some(_), _) => None,
    };
    if let Some(line) = status {
        return col
            .push(text(line).size(TS.body).color(ti(tokens.text)).boxed())
            .boxed();
    }
    col = col.push(table(state.numbered_rows(), i18n, tokens));
    for line in notes(state, i18n) {
        col = col.push(line.into_element(tokens));
    }
    col.boxed()
}

/// One line under the table and how loud it is.
enum Note {
    Danger(Localized),
    Warn(Localized),
}

impl Note {
    fn into_element<'a>(self, tokens: &ThemeTokens) -> Element<'a, Message> {
        let (line, color) = match self {
            Note::Danger(line) => (line, tokens::danger(tokens)),
            Note::Warn(line) => (line, crate::styles::pill_colors(tokens, PillTone::Warn).1),
        };
        text(line).size(TS.body).color(color).boxed()
    }
}

/// Why Apply is off or refused, then the clashes, gaps and high numbers.
fn notes(state: &SheetNumberingState, i18n: &I18n) -> Vec<Note> {
    let mut notes = Vec::new();
    if let Some(why) = state.error.as_deref() {
        notes.push(Note::Danger(t!(
            i18n,
            ids::DIALOG_SHEET_NUMBERING_APPLY_FAILED,
            "why" => why
        )));
    }
    if state.has_invalid() {
        notes.push(Note::Danger(t!(i18n, ids::DIALOG_SHEET_NUMBERING_INVALID)));
    }
    let rows = state.rows();
    for issue in state.issues() {
        let line = match issue {
            NumberingIssue::Clash { number, .. } => {
                t!(i18n, ids::DIALOG_SHEET_NUMBERING_CLASH, "number" => number)
            }
            NumberingIssue::Gap { missing } => {
                let list: Vec<String> = missing.iter().map(usize::to_string).collect();
                t!(
                    i18n,
                    ids::DIALOG_SHEET_NUMBERING_GAP,
                    "numbers" => list.join(", ")
                )
            }
            NumberingIssue::OutOfRange { page, number } => t!(
                i18n,
                ids::DIALOG_SHEET_NUMBERING_OUT_OF_RANGE,
                "page" => rows.get(page).map_or("", |r| r.name.as_str()),
                "number" => number,
                "count" => rows.len()
            ),
            // Typed numbers are checked by `has_invalid` above.
            NumberingIssue::InvalidOverride { .. } => continue,
        };
        notes.push(Note::Warn(line));
    }
    notes
}

/// The page table: a sunken header, then one ruled row per page. A changed
/// row takes the soft accent fill.
fn table<'a>(
    rows: Vec<NumberedRow<'a>>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let heading =
        |id| -> Element<'a, Message> { text(t!(i18n, id)).size(TS.caption).color(r.ink4).boxed() };
    let column =
        |id, width: Length| -> Element<'a, Message> { container(heading(id)).width(width).boxed() };
    let head = Row::<Element<'a, Message>>::new()
        .push(column(ids::DIALOG_SHEET_NUMBERING_COL_SHEET, Length::Fill))
        .push(column(
            ids::DIALOG_SHEET_NUMBERING_COL_FILE,
            Length::Fixed(FILE_W),
        ))
        .push(column(
            ids::DIALOG_SHEET_NUMBERING_COL_NUMBER,
            Length::Fixed(NUMBER_W),
        ))
        .spacing(SP.m);
    let sunken = r.sunken;
    let rule_color = r.line_soft;
    let rule = move || -> Element<'a, Message> {
        container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(rule_color)),
                ..container::Style::default()
            })
            .boxed()
    };
    // Past about twelve rows the list scrolls; shorter lists take their height.
    let height = if rows.len() > 12 {
        Length::Fixed(TABLE_MAX_H)
    } else {
        Length::Shrink
    };
    let mut list: Column<Element<'a, Message>> = Column::new().width(Length::Fill);
    for (index, numbered) in rows.into_iter().enumerate() {
        let changed = numbered.changed;
        list = list.push(rule());
        list = list.push(
            container(page_row(index, numbered, i18n, tokens))
                .padding([SP.s, SP.m])
                .width(Length::Fill)
                .style(super::graphite::changed_row(tokens, changed))
                .boxed(),
        );
    }
    let head = container(head)
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(sunken)),
            ..container::Style::default()
        });
    container(column![head, scrollable(list).height(height)])
        .width(Length::Fill)
        .clip(true)
        .style(crate::styles::card(tokens))
        .boxed()
}

/// One page: its name indented by depth, its file, and its number field (a
/// top-level page shows its number, which follows its position).
fn page_row<'a>(
    index: usize,
    numbered: NumberedRow<'a>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let ink = ti(tokens.text);
    let name = Row::<Element<'a, Message>>::new()
        .push(
            Space::new()
                .width(Length::Fixed(SP.l * numbered.row.depth as f32))
                .boxed(),
        )
        .push(
            text(UserText::new(numbered.row.name.clone()))
                .size(TS.body)
                .color(ink)
                .boxed(),
        )
        .width(Length::Fill);
    let file = text(UserText::new(numbered.row.sheet.clone()))
        .size(TS.body)
        .color(r.ink3)
        .font(super::graphite::mono())
        .width(FILE_W)
        .boxed();
    let invalid = numbered.invalid.then(|| tokens::danger(tokens));
    let number: Element<'a, Message> = if numbered.row.placement.is_some() {
        text_input(
            t!(i18n, ids::DIALOG_SHEET_NUMBERING_COL_NUMBER),
            numbered.draft,
        )
        .style(move |theme: &Theme, status| {
            let mut style = crate::panels::form_look::text_input_style(theme, status);
            if let Some(danger) = invalid {
                style.border.color = danger;
            }
            style
        })
        .on_input(move |text| sn(SheetNumberingMsg::NumberEdited { row: index, text }))
        .font(super::graphite::mono())
        .size(TS.body)
        .padding([SP.xs, SP.s])
        .width(NUMBER_W)
        .boxed()
    } else {
        column![
            text(UserText::new(numbered.row.number.to_string()))
                .size(TS.body)
                .color(ink)
                .font(super::graphite::mono()),
            text(t!(i18n, ids::DIALOG_SHEET_NUMBERING_TOP_LEVEL))
                .size(TS.caption)
                .color(r.ink4),
        ]
        .width(NUMBER_W)
        .boxed()
    };
    Row::<Element<'a, Message>>::new()
        .push(name.boxed())
        .push(file)
        .push(number)
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .boxed()
}
