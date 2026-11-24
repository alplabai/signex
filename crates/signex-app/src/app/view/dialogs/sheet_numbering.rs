//! The Number Schematic Sheets dialog (#2247, Graphite #78): the project's
//! pages in hierarchy order, each with a number field, an error banner over
//! clashes and high numbers with Auto-number as the fix, a warning line for
//! a gap, and Apply.
//! Draft and rules: `app/sheet_numbering/`.

use iced::widget::{Column, Row, Space, column, container, scrollable, svg};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_net::NumberingIssue;
use signex_types::theme::ThemeTokens;
use signex_widgets::{text, text_input};

use super::sizes;
use crate::app::sheet_numbering::{
    NumberedRow, NumberingProblem, SheetNumberingMsg, SheetNumberingState,
};
use crate::app::{Message, Signex};
use crate::styles::{
    DialogButtonKind, PillTone, Radii, Spacing, TypeScale, dialog_button, dialog_footer_split, ti,
    tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Width of the Number column.
const NUMBER_W: f32 = TS.body * 8.0;
/// Width of the number field inside it: about four digits.
const FIELD_W: f32 = TS.body * 13.0 / 3.0;
/// Width of the File column.
const FILE_W: f32 = TS.body * 50.0 / 3.0;
/// Side of a banner or button glyph.
const GLYPH: f32 = TS.body * 7.0 / 6.0;

const ERR_GLYPH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><circle cx="7" cy="7" r="5.6" fill="none" stroke="currentColor" stroke-width="1.4"/><path d="M7 4v3.6" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/><circle cx="7" cy="10" r="0.8" fill="currentColor"/></svg>"#;
const WAND_GLYPH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><path d="M2.5 11.5l6.5-6.5" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/><path d="M9.5 1.8v1.8M8.6 2.7h1.8M11.8 4.6v1.6M11 5.4h1.6" stroke="currentColor" stroke-width="1.1" stroke-linecap="round"/></svg>"#;

fn sn(msg: SheetNumberingMsg) -> Message {
    Message::from(msg)
}

fn s(i18n: &I18n, id: signex_i18n::MessageId) -> String {
    t!(i18n, id).into_string()
}

/// A monochrome glyph tinted `ink`.
fn glyph<'a>(bytes: &'static [u8], ink: Color) -> Element<'a, Message> {
    svg(svg::Handle::from_memory(bytes))
        .width(GLYPH)
        .height(GLYPH)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}

/// The Auto-number button: the footer's, and the banner's fix (`small`).
fn auto_number_button<'a>(
    state: &SheetNumberingState,
    i18n: &I18n,
    tokens: &ThemeTokens,
    small: bool,
) -> Element<'a, Message> {
    let ink = ti(tokens.text);
    let pad = if small { [SP.xs, SP.m] } else { [SP.m, SP.l] };
    let label = Row::<Element<'a, Message>>::new()
        .push(glyph(WAND_GLYPH, ink))
        .push(
            text(t!(i18n, ids::DIALOG_SHEET_NUMBERING_AUTO))
                .size(TS.label)
                .boxed(),
        )
        .spacing(SP.s)
        .align_y(Alignment::Center);
    signex_widgets::button(label)
        .padding(pad)
        .on_press_maybe(
            (!state.rows().is_empty() && !state.checking)
                .then(|| sn(SheetNumberingMsg::AutoNumberRequested)),
        )
        .style(crate::styles::outline_button(tokens))
        .boxed()
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
        let body = container(body(state, i18n, tokens))
            .padding([SP.xl, SP.xl + SP.m])
            .height(Length::Fill);
        let footer = dialog_footer_split(
            vec![auto_number_button(state, i18n, tokens, false)],
            vec![
                dialog_button(
                    t!(i18n, ids::DIALOG_SHEET_NUMBERING_CANCEL),
                    DialogButtonKind::Cancel,
                    cancel,
                    tokens,
                ),
                dialog_button(
                    t!(i18n, ids::DIALOG_SHEET_NUMBERING_APPLY),
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
        .height(Length::Fixed(sizes::SHEET_NUMBERING.1))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }
}

/// The intro, then the table (or why there is none), then the error banner
/// and why Apply refused.
fn body<'a>(
    state: &'a SheetNumberingState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let muted = tokens::muted_text(tokens);
    let mut col: Column<Element<'a, Message>> = Column::new().spacing(SP.m + SP.xs);
    col = col.push(
        text(t!(i18n, ids::DIALOG_SHEET_NUMBERING_INTRO))
            .size(TS.body)
            .color(muted)
            .boxed(),
    );
    let status = match (&state.read, &state.error) {
        (None, Some(problem)) => Some(problem_text(problem, i18n)),
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
    let lines = banner_lines(state, i18n);
    if !lines.is_empty() {
        col = col.push(error_banner(state, lines, i18n, tokens));
    }
    // A gap only warns (mockup row 78): a warning line that holds nothing back.
    for line in warning_lines(state, i18n) {
        col = col.push(text(line).size(TS.body).color(ti(tokens.warning)).boxed());
    }
    if let Some(problem) = state.error.as_ref() {
        col = col.push(
            text(problem_text(problem, i18n))
                .size(TS.body)
                .color(tokens::danger(tokens))
                .boxed(),
        );
    }
    col.height(Length::Fill).boxed()
}

/// Why the read failed or Apply refused, in the dialog's language.
fn problem_text(problem: &NumberingProblem, i18n: &I18n) -> Localized {
    let path = |path: &std::path::Path| path.display().to_string();
    match problem {
        NumberingProblem::RootUnreadable(root) => t!(
            i18n,
            ids::DIALOG_SHEET_NUMBERING_ERR_ROOT_UNREADABLE,
            "path" => path(root)
        ),
        NumberingProblem::ReadFailed(why) => t!(
            i18n,
            ids::DIALOG_SHEET_NUMBERING_READ_FAILED,
            "why" => why.as_str()
        ),
        NumberingProblem::Stale => t!(i18n, ids::DIALOG_SHEET_NUMBERING_ERR_STALE),
        NumberingProblem::Busy(sheet) => t!(
            i18n,
            ids::DIALOG_SHEET_NUMBERING_ERR_BUSY,
            "path" => path(sheet)
        ),
        NumberingProblem::ReadOnly(sheet) => t!(
            i18n,
            ids::DIALOG_SHEET_NUMBERING_ERR_READ_ONLY,
            "path" => path(sheet)
        ),
        NumberingProblem::NotLoaded(sheet) => t!(
            i18n,
            ids::DIALOG_SHEET_NUMBERING_ERR_NOT_LOADED,
            "path" => path(sheet)
        ),
        NumberingProblem::NotChanged(sheet) => t!(
            i18n,
            ids::DIALOG_SHEET_NUMBERING_ERR_NOT_CHANGED,
            "path" => path(sheet)
        ),
        NumberingProblem::DiskChanged(sheet) => t!(
            i18n,
            ids::DIALOG_SHEET_NUMBERING_ERR_DISK_CHANGED,
            "path" => path(sheet)
        ),
    }
}

/// What holds Apply back: a typed number that is not a sheet number, then
/// the clashes and numbers above the page count.
fn banner_lines(state: &SheetNumberingState, i18n: &I18n) -> Vec<Localized> {
    let mut lines = Vec::new();
    if state.has_invalid() {
        lines.push(t!(i18n, ids::DIALOG_SHEET_NUMBERING_INVALID));
    }
    let rows = state.rows();
    for issue in state.blocking_issues() {
        lines.push(match issue {
            NumberingIssue::Clash { number, .. } => {
                t!(i18n, ids::DIALOG_SHEET_NUMBERING_CLASH, "number" => number)
            }
            NumberingIssue::OutOfRange { page, number } => t!(
                i18n,
                ids::DIALOG_SHEET_NUMBERING_OUT_OF_RANGE,
                "page" => rows.get(page).map_or("", |r| r.name.as_str()),
                "number" => number,
                "count" => rows.len()
            ),
            // `blocking_issues` leaves these out: typed numbers are checked
            // by `has_invalid` above, and a gap is a warning.
            NumberingIssue::InvalidOverride { .. } | NumberingIssue::Gap { .. } => continue,
        });
    }
    lines
}

/// What only warns: the numbers no page prints.
fn warning_lines(state: &SheetNumberingState, i18n: &I18n) -> Vec<Localized> {
    state
        .warnings()
        .into_iter()
        .filter_map(|issue| match issue {
            NumberingIssue::Gap { missing } => {
                let list: Vec<String> = missing.iter().map(usize::to_string).collect();
                Some(t!(
                    i18n,
                    ids::DIALOG_SHEET_NUMBERING_GAP,
                    "numbers" => list.join(", ")
                ))
            }
            _ => None,
        })
        .collect()
}

/// The err-soft banner: the error glyph, the lines, and Auto-number as the
/// fix.
fn error_banner<'a>(
    state: &SheetNumberingState,
    lines: Vec<Localized>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let (fill, danger, ink) = (r.err_soft, tokens::danger(tokens), ti(tokens.text));
    let mut grow: Column<Element<'a, Message>> = Column::new().spacing(SP.xs).width(Length::Fill);
    for line in lines {
        grow = grow.push(text(line).size(TS.body).color(ink).boxed());
    }
    let bar = Row::<Element<'a, Message>>::new()
        .push(glyph(ERR_GLYPH, danger))
        .push(grow.boxed())
        .push(auto_number_button(state, i18n, tokens, true))
        .spacing(SP.m)
        .align_y(Alignment::Center);
    container(bar)
        .padding([SP.m, SP.m + SP.xs])
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: Radii::GRAPHITE.button.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The page table: a sunken header, then one ruled row per page. A row whose
/// number clashes takes the err-soft fill.
fn table<'a>(
    rows: Vec<NumberedRow<'a>>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let heading = |id, width: Length, right: bool| -> Element<'a, Message> {
        let cell = container(text(t!(i18n, id)).size(TS.caption).color(r.ink4)).width(width);
        if right {
            cell.align_right(width).boxed()
        } else {
            cell.boxed()
        }
    };
    let head = Row::<Element<'a, Message>>::new()
        .push(heading(
            ids::DIALOG_SHEET_NUMBERING_COL_SHEET,
            Length::Fill,
            false,
        ))
        .push(heading(
            ids::DIALOG_SHEET_NUMBERING_COL_FILE,
            Length::Fixed(FILE_W),
            false,
        ))
        .push(heading(
            ids::DIALOG_SHEET_NUMBERING_COL_NUMBER,
            Length::Fixed(NUMBER_W),
            true,
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
    let bad_fill = r.err_soft;
    let mut list: Column<Element<'a, Message>> = Column::new().width(Length::Fill);
    for (index, numbered) in rows.into_iter().enumerate() {
        let fill = numbered.clash.then_some(bad_fill);
        list = list.push(rule());
        list = list.push(
            container(page_row(index, numbered, i18n, tokens))
                .padding([SP.s, SP.m])
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: fill.map(Background::Color),
                    ..container::Style::default()
                })
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
    container(column![
        head,
        scrollable(list)
            .height(Length::Fill)
            .style(crate::styles::scrollbar_style(tokens))
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .clip(true)
    .style(crate::styles::card(tokens))
    .boxed()
}

/// One page: its name indented by depth (a top-level page with its pill),
/// its file, and its number field. An automatic number is faint; a typed one
/// is full ink; a clashing, high or unreadable one is red.
fn page_row<'a>(
    index: usize,
    numbered: NumberedRow<'a>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let ink = ti(tokens.text);
    let danger = tokens::danger(tokens);
    let top_level = numbered.row.placement.is_none();
    let mut name = Row::<Element<'a, Message>>::new()
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
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .width(Length::Fill);
    if top_level {
        name = name.push(
            container(
                text(t!(i18n, ids::DIALOG_SHEET_NUMBERING_TOP_LEVEL))
                    .size(TS.caption)
                    .wrapping(iced::widget::text::Wrapping::None),
            )
            .padding([0.0, SP.s])
            .style(crate::styles::pill(tokens, PillTone::Neutral))
            .boxed(),
        );
    }
    let file = text(UserText::new(numbered.row.sheet.clone()))
        .size(TS.body)
        .color(r.ink3)
        .font(super::graphite::mono())
        .width(FILE_W)
        .boxed();
    let wrong = numbered.clash || numbered.invalid || numbered.out_of_range;
    let value = if wrong {
        danger
    } else if numbered.automatic {
        r.ink4
    } else {
        ink
    };
    let field: Element<'a, Message> = if top_level {
        container(
            text(UserText::new(numbered.row.number.to_string()))
                .size(TS.body)
                .color(value)
                .font(super::graphite::mono()),
        )
        .padding([SP.xs, SP.s])
        .width(FIELD_W)
        .style(crate::styles::field(tokens, false))
        .boxed()
    } else {
        text_input(
            t!(i18n, ids::DIALOG_SHEET_NUMBERING_COL_NUMBER),
            numbered.draft,
        )
        .style(move |theme: &Theme, status| {
            let mut style = crate::panels::form_look::text_input_style(theme, status);
            style.value = value;
            if wrong {
                style.border.color = danger;
            }
            style
        })
        .on_input(move |text| sn(SheetNumberingMsg::NumberEdited { row: index, text }))
        .font(super::graphite::mono())
        .size(TS.body)
        .padding([SP.xs, SP.s])
        .width(FIELD_W)
        .boxed()
    };
    let number = container(field)
        .width(Length::Fixed(NUMBER_W))
        .align_right(Length::Fixed(NUMBER_W));
    Row::<Element<'a, Message>>::new()
        .push(name.boxed())
        .push(file)
        .push(number.boxed())
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .boxed()
}
