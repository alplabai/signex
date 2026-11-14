//! The Assign Footprints dialog (#2248): the project's placed parts with a
//! tick box and a footprint list each, a "set ticked parts to" list, and
//! Apply. Draft and rules: `app/assign_footprints/`.

use iced::widget::{Column, Row, Space, column, container, pick_list, scrollable};
use iced::{Alignment, Background, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{PickListPlaceholder as _, checkbox, text};

use super::sizes;
use crate::app::assign_footprints::{
    AssignFootprintsMsg, AssignFootprintsState, AssignProblem, FootprintOffer, FootprintPart,
    PartAccess,
};
use crate::app::{Message, Signex};
use crate::styles::{
    DialogButtonKind, Spacing, TypeScale, dialog_button, dialog_footer_split, ti, tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Width of the tick column.
const TICK_W: f32 = TS.body * 2.0;
/// Width of the Designator column.
const DESIGNATOR_W: f32 = TS.body * 6.0;
/// Width of the Value and Library row columns.
const TEXT_W: f32 = TS.body * 8.0;
/// Width of the Footprint column.
const FOOTPRINT_W: f32 = TS.body * 16.0;
/// The table scrolls past about twelve rows.
const TABLE_MAX_H: f32 = TS.body * 2.6 * 12.0;

fn af(msg: AssignFootprintsMsg) -> Message {
    Message::from(msg)
}

fn s(i18n: &I18n, id: MessageId) -> String {
    t!(i18n, id).into_string()
}

impl Signex {
    pub(in crate::app::view) fn view_assign_footprints_dialog(&self) -> Element<'_, Message> {
        self.wrap_centred_dialog(self.view_assign_footprints_card(), sizes::ASSIGN_FOOTPRINTS)
    }

    fn view_assign_footprints_card(&self) -> Element<'_, Message> {
        let Some(state) = self.ui_state.assign_footprints.as_ref() else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let cancel = Some(af(AssignFootprintsMsg::CancelRequested));
        let subtitle = state.parts.as_ref().map(|parts| {
            t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_SUBTITLE, "count" => parts.len()).into_string()
        });
        let header = super::header::graphite_header_closing(
            s(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_TITLE),
            subtitle,
            None,
            cancel.clone(),
            super::header::DIALOG_HEADER_H,
            tokens,
        );
        let body = container(body(state, i18n, tokens)).padding([SP.xl, SP.xl + SP.m]);
        let changes = state.changes().len();
        let summary: Element<'_, Message> = if changes > 0 {
            text(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_CHANGES, "count" => changes))
                .size(TS.label)
                .color(tokens::muted_text(tokens))
                .boxed()
        } else {
            Space::new().boxed()
        };
        let footer = dialog_footer_split(
            vec![summary],
            vec![
                dialog_button(
                    t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_CANCEL),
                    DialogButtonKind::Cancel,
                    cancel,
                    tokens,
                ),
                dialog_button(
                    t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_APPLY),
                    super::graphite::call_to_action(tokens),
                    state
                        .can_apply()
                        .then(|| af(AssignFootprintsMsg::ApplyRequested)),
                    tokens,
                ),
            ],
        );
        container(column![header, body, footer].width(Length::Fixed(
            super::widgets::modal_card_width(self.ui_state.window_size, sizes::ASSIGN_FOOTPRINTS.0),
        )))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }
}

/// The intro, then the bulk row and the table (or why there is none), then
/// the notes.
fn body<'a>(
    state: &'a AssignFootprintsState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let muted = tokens::muted_text(tokens);
    let mut col: Column<Element<'a, Message>> = Column::new().spacing(SP.l);
    col = col.push(
        text(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_INTRO))
            .size(TS.body)
            .color(muted)
            .boxed(),
    );
    let status = match (&state.parts, &state.error) {
        (None, Some(problem)) => Some(problem_text(problem, i18n)),
        (None, None) => Some(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_READING)),
        (Some(parts), _) if parts.is_empty() => {
            Some(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_NO_PARTS))
        }
        (Some(_), _) => None,
    };
    if let Some(line) = status {
        return col
            .push(text(line).size(TS.body).color(ti(tokens.text)).boxed())
            .boxed();
    }
    col = col.push(bulk_row(state, i18n, tokens));
    col = col.push(table(state, i18n, tokens));
    if state
        .parts()
        .iter()
        .any(|p| p.access == PartAccess::SheetClosed)
    {
        col = col.push(
            text(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_CLOSED_NOTE))
                .size(TS.body)
                .color(muted)
                .boxed(),
        );
    }
    if let Some(problem) = state.problem().filter(|_| state.parts.is_some()) {
        col = col.push(
            text(problem_text(problem, i18n))
                .size(TS.body)
                .color(tokens::danger(tokens))
                .boxed(),
        );
    }
    col.boxed()
}

/// Why the read failed or Apply refused, in the dialog's language.
fn problem_text(problem: &AssignProblem, i18n: &I18n) -> Localized {
    let path = |path: &std::path::Path| path.display().to_string();
    match problem {
        AssignProblem::ReadFailed(why) => t!(
            i18n,
            ids::DIALOG_ASSIGN_FOOTPRINTS_READ_FAILED,
            "why" => why.as_str()
        ),
        AssignProblem::SheetClosed(sheet) => t!(
            i18n,
            ids::DIALOG_ASSIGN_FOOTPRINTS_ERR_CLOSED,
            "path" => path(sheet)
        ),
        AssignProblem::ReadOnly(sheet) => t!(
            i18n,
            ids::DIALOG_ASSIGN_FOOTPRINTS_ERR_READ_ONLY,
            "path" => path(sheet)
        ),
        AssignProblem::Refused { reference, .. } => t!(
            i18n,
            ids::DIALOG_ASSIGN_FOOTPRINTS_ERR_REFUSED,
            "part" => reference.as_str()
        ),
        AssignProblem::Stale(part) => t!(
            i18n,
            ids::DIALOG_ASSIGN_FOOTPRINTS_ERR_STALE,
            "part" => part.as_str()
        ),
        AssignProblem::NotChanged(sheet) => t!(
            i18n,
            ids::DIALOG_ASSIGN_FOOTPRINTS_ERR_NOT_CHANGED,
            "path" => path(sheet)
        ),
    }
}

/// One entry of a footprint list.
#[derive(Debug, Clone, PartialEq)]
struct Choice {
    binding: String,
    label: String,
}

fn choices(offers: &[FootprintOffer]) -> Vec<Choice> {
    offers
        .iter()
        .map(|o| Choice {
            binding: o.binding.clone(),
            label: o.label.clone(),
        })
        .collect()
}

/// "Set ticked parts to" and the footprints every ticked part's row offers.
fn bulk_row<'a>(
    state: &'a AssignFootprintsState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let muted = tokens::muted_text(tokens);
    let label = text(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_SET_TICKED))
        .size(TS.label)
        .color(ti(tokens.text));
    let shared = choices(&state.shared_offers());
    let picker: Element<'a, Message> = if !state.selected.is_empty() && shared.is_empty() {
        text(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_NO_SHARED))
            .size(TS.body)
            .color(muted)
            .boxed()
    } else {
        pick_list(None::<Choice>, shared, |c: &Choice| c.label.clone())
            .graphite()
            .on_select(|c: Choice| af(AssignFootprintsMsg::TickedPicked(c.binding)))
            .width(Length::Fixed(FOOTPRINT_W))
            .boxed()
    };
    Row::<Element<'a, Message>>::new()
        .push(label.boxed())
        .push(picker)
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .boxed()
}

/// The part table: a sunken header with the tick-all box, then one ruled
/// row per part. A changed row takes the soft accent fill.
fn table<'a>(
    state: &'a AssignFootprintsState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let heading = |id, width: Length| -> Element<'a, Message> {
        container(text(t!(i18n, id)).size(TS.caption).color(r.ink4))
            .width(width)
            .boxed()
    };
    let all = state.all_selected();
    let tick_all = checkbox(all)
        .style(crate::panels::form_look::checkbox_style)
        .size(TS.body)
        .on_toggle(|on| af(AssignFootprintsMsg::AllToggled(on)));
    let head = Row::<Element<'a, Message>>::new()
        .push(container(tick_all).width(Length::Fixed(TICK_W)).boxed())
        .push(heading(
            ids::DIALOG_ASSIGN_FOOTPRINTS_COL_DESIGNATOR,
            Length::Fixed(DESIGNATOR_W),
        ))
        .push(heading(
            ids::DIALOG_ASSIGN_FOOTPRINTS_COL_VALUE,
            Length::Fixed(TEXT_W),
        ))
        .push(heading(ids::DIALOG_ASSIGN_FOOTPRINTS_COL_ROW, Length::Fill))
        .push(heading(
            ids::DIALOG_ASSIGN_FOOTPRINTS_COL_FOOTPRINT,
            Length::Fixed(FOOTPRINT_W),
        ))
        .spacing(SP.m)
        .align_y(Alignment::Center);
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
    let parts = state.parts();
    let height = if parts.len() > 12 {
        Length::Fixed(TABLE_MAX_H)
    } else {
        Length::Shrink
    };
    let mut list: Column<Element<'a, Message>> = Column::new().width(Length::Fill);
    for (index, part) in parts.iter().enumerate() {
        let changed = state.picks.get(index).is_some_and(Option::is_some);
        list = list.push(rule());
        list = list.push(
            container(part_row(state, index, part, i18n, tokens))
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

/// Why a part cannot be changed here, in the dialog's language.
fn access_note(i18n: &I18n, access: PartAccess) -> Option<Localized> {
    Some(match access {
        PartAccess::Editable => return None,
        PartAccess::SheetClosed => t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_SHEET_CLOSED),
        PartAccess::ReadOnly => t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_READ_ONLY),
        PartAccess::NoRow => t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_ROW_NOT_OPEN),
        PartAccess::NoAlternates => t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_NO_ALTERNATES),
    })
}

/// One part: tick box, designator (and sheet), value, library row (with
/// "tick all from this row"), and its footprint list or why it has none.
fn part_row<'a>(
    state: &'a AssignFootprintsState,
    index: usize,
    part: &'a FootprintPart,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let ink = ti(tokens.text);
    let editable = part.access == PartAccess::Editable;
    let tick: Element<'a, Message> = if editable {
        checkbox(state.selected.contains(&index))
            .style(crate::panels::form_look::checkbox_style)
            .size(TS.body)
            .on_toggle(move |_| af(AssignFootprintsMsg::RowToggled(index)))
            .boxed()
    } else {
        Space::new().boxed()
    };
    let designator = column![
        text(UserText::new(part.designator.clone()))
            .size(TS.body)
            .color(ink)
            .font(super::graphite::mono()),
        text(UserText::new(part.sheet_label.clone()))
            .size(TS.caption)
            .color(r.ink4),
    ]
    .width(Length::Fixed(DESIGNATOR_W));
    let value = text(UserText::new(part.value.clone()))
        .size(TS.body)
        .color(ink)
        .width(Length::Fixed(TEXT_W));
    let row_label: signex_widgets::Label<'a> = match &part.row_label {
        Some(label) => UserText::new(label.clone()).into(),
        None => t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_NO_ROW).into(),
    };
    let mut row_cell = Column::<Element<'a, Message>>::new()
        .push(text(row_label).size(TS.body).color(ink).boxed())
        .width(Length::Fill);
    if editable && part.row_key.is_some() {
        row_cell = row_cell.push(
            signex_widgets::button(
                text(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_SAME_ROW))
                    .size(TS.caption)
                    .color(tokens::resolve(tokens).accent_text),
            )
            .padding(0)
            .on_press(af(AssignFootprintsMsg::SameRowTicked(index)))
            .style(crate::styles::ghost_button(tokens))
            .boxed(),
        );
    }
    let footprint: Element<'a, Message> = if editable {
        let options = choices(&part.offers);
        let shown = state.shown_binding(index);
        let current = options
            .iter()
            .find(|c| Some(c.binding.as_str()) == shown)
            .cloned();
        pick_list(current, options, |c: &Choice| c.label.clone())
            .graphite()
            .placeholder_label(UserText::new(part.current.clone()))
            .on_select(move |c: Choice| {
                af(AssignFootprintsMsg::FootprintPicked {
                    row: index,
                    binding: c.binding,
                })
            })
            .width(Length::Fixed(FOOTPRINT_W))
            .boxed()
    } else {
        let mut cell = Column::<Element<'a, Message>>::new().push(
            text(UserText::new(part.current.clone()))
                .size(TS.body)
                .color(ink)
                .font(super::graphite::mono())
                .boxed(),
        );
        if let Some(note) = access_note(i18n, part.access) {
            cell = cell.push(text(note).size(TS.caption).color(r.ink4).boxed());
        }
        cell.width(Length::Fixed(FOOTPRINT_W)).boxed()
    };
    Row::<Element<'a, Message>>::new()
        .push(container(tick).width(Length::Fixed(TICK_W)).boxed())
        .push(designator.boxed())
        .push(value.boxed())
        .push(row_cell.boxed())
        .push(footprint)
        .spacing(SP.m)
        .align_y(Alignment::Center)
        .boxed()
}
