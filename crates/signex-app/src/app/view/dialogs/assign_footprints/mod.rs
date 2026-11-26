//! The Assign Footprints dialog (#2248, Graphite #80): the project's placed
//! parts with filter chips, a ticked bar, a tick box and a footprint list
//! each, and a read-only footprint preview column. Draft and rules:
//! `app/assign_footprints/`.

#[cfg(test)]
mod look_tests;
mod preview;
mod table;

use iced::font::Weight;
use iced::widget::{Column, Row, Space, column, container, pick_list, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::chips::{ChipMetrics, ChipStyle, FilterSet, filter_chips};
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{text, text_input};

use super::sizes;
use crate::app::assign_footprints::{
    AssignFilter, AssignFootprintsMsg, AssignFootprintsState, AssignProblem, FootprintOffer,
    PartAccess, TickedBar,
};
use crate::app::{Message, Signex};
use crate::styles::{
    DialogButtonKind, PillTone, Radii, Spacing, TypeScale, dialog_button, dialog_footer_split, ti,
    tokens,
};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
/// Width of the filter field.
const FILTER_W: f32 = TS.body * 20.0;
/// Width of the ticked bar's footprint list.
const BAR_PICK_W: f32 = TS.body * 55.0 / 3.0;
/// Side of a glyph.
const GLYPH: f32 = TS.body * 7.0 / 6.0;

const SEARCH_GLYPH: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 14 14"><circle cx="6" cy="6" r="4.25" fill="none" stroke="currentColor" stroke-width="1.4"/><path d="M9.25 9.25L12.5 12.5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/></svg>"#;

fn af(msg: AssignFootprintsMsg) -> Message {
    Message::from(msg)
}

fn s(i18n: &I18n, id: MessageId) -> String {
    t!(i18n, id).into_string()
}

/// A monochrome glyph tinted `ink`.
fn glyph<'a>(bytes: &'static [u8], ink: Color) -> iced::widget::Svg<'a> {
    svg(svg::Handle::from_memory(bytes))
        .width(GLYPH)
        .height(GLYPH)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
}

/// The semibold face of the default font.
fn semibold() -> Font {
    Font {
        weight: Weight::Semibold,
        ..signex_widgets::ui_font::regular()
    }
}

/// A pill: caption text on a tone fill.
fn pill<'a>(
    label: impl Into<signex_widgets::Label<'a>>,
    tone: PillTone,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    container(
        text(label)
            .size(TS.caption)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .padding([0.0, SP.s])
    .style(crate::styles::pill(tokens, tone))
    .boxed()
}

/// Colours of the filter chips.
fn chip_style(tokens: &ThemeTokens) -> ChipStyle {
    let r = tokens::resolve(tokens);
    ChipStyle {
        fill: r.field,
        edge: ti(tokens.border),
        hover: ti(tokens.hover),
        ink: r.ink3,
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    }
}

/// One entry of a footprint list.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Choice {
    pub(super) binding: String,
    pub(super) label: String,
}

pub(super) fn choices(offers: &[FootprintOffer]) -> Vec<Choice> {
    offers
        .iter()
        .map(|o| Choice {
            binding: o.binding.clone(),
            label: o.label.clone(),
        })
        .collect()
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
        let left = container(body(state, i18n, tokens))
            .padding([SP.xl, SP.xl + SP.m])
            .width(Length::Fill)
            .height(Length::Fill);
        let mut content = Row::<Element<'_, Message>>::new()
            .push(left.boxed())
            .height(Length::Fill);
        if state.parts.as_ref().is_some_and(|parts| !parts.is_empty()) {
            content = content.push(preview::column(state, i18n, tokens));
        }
        let changes = state.changes().len();
        let summary: Element<'_, Message> = if changes > 0 {
            pill(
                t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_CHANGES, "count" => changes),
                PillTone::Accent,
                tokens,
            )
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
                apply_button(
                    i18n,
                    state
                        .can_apply()
                        .then(|| af(AssignFootprintsMsg::ApplyRequested)),
                    tokens,
                ),
            ],
        );
        container(column![header, content, footer].width(Length::Fixed(
            super::widgets::modal_card_width(self.ui_state.window_size, sizes::ASSIGN_FOOTPRINTS.0),
        )))
        .height(Length::Fixed(sizes::ASSIGN_FOOTPRINTS.1))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }
}

/// Apply on the primary fill (dark ink in the light theme), as mockup row 80
/// draws it.
fn apply_button<'a>(
    i18n: &I18n,
    on_press: Option<Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let label = t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_APPLY);
    crate::styles::primary_dialog_button(
        |ink| {
            text(label)
                .size(TS.label)
                .color(ink)
                .height(Length::Fill)
                .align_y(iced::alignment::Vertical::Center)
                .wrapping(iced::widget::text::Wrapping::None)
                .boxed()
        },
        on_press,
        tokens,
    )
}

/// The intro, then the filter row, the ticked bar and the table (or why
/// there is none), then the notes.
fn body<'a>(
    state: &'a AssignFootprintsState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let muted = tokens::muted_text(tokens);
    let mut col: Column<Element<'a, Message>> = Column::new().spacing(SP.m + SP.xs);
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
    col = col.push(filter_row(state, i18n, tokens));
    if let Some(bar) = state.ticked_bar() {
        col = col.push(ticked_bar(state, bar, i18n, tokens));
    }
    col = col.push(table::table(state, i18n, tokens));
    if state
        .parts()
        .iter()
        .any(|p| p.access == PartAccess::SheetClosed)
    {
        col = col.push(
            text(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_CLOSED_NOTE))
                .size(TS.caption)
                .color(tokens::resolve(tokens).ink4)
                .boxed(),
        );
    }
    if let Some(problem) = state.problem() {
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

/// The filter field and the All / Changed / No alternates chips with their
/// counts.
fn filter_row<'a>(
    state: &'a AssignFootprintsState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let (ink, ink4, accent) = (ti(tokens.text), r.ink4, ti(tokens.accent));
    let input = text_input(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_FILTER), &state.query)
        .on_input(|query| af(AssignFootprintsMsg::FilterEdited(query)))
        .padding(iced::Padding::ZERO)
        .size(TS.body)
        .width(Length::Fill)
        .style(move |_: &Theme, _| iced::widget::text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder: ink4,
            value: ink,
            selection: Color { a: 0.4, ..accent },
        });
    let field = container(
        row![glyph(SEARCH_GLYPH, ink4).boxed(), input.boxed()]
            .spacing(SP.s)
            .align_y(Alignment::Center),
    )
    .padding([SP.s, SP.m])
    .width(Length::Fixed(FILTER_W))
    .style(crate::styles::field(tokens, false));
    let metrics = ChipMetrics::from_text_size(TS.body);
    let options: Vec<(AssignFilter, signex_widgets::Label<'a>, Option<u32>)> = AssignFilter::ALL
        .into_iter()
        .map(|filter| {
            let count = u32::try_from(state.filter_count(filter)).unwrap_or(u32::MAX);
            (filter, t!(i18n, filter.label()).into(), Some(count))
        })
        .collect();
    let chips = filter_chips(
        options,
        &FilterSet::new([state.filter]),
        chip_style(tokens),
        metrics,
        i18n,
        |filter| af(AssignFootprintsMsg::FilterPicked(filter)),
    );
    Row::<Element<'a, Message>>::new()
        .push(field.boxed())
        .push(chips)
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .boxed()
}

/// The bar shown while parts are ticked: the count, "Set ticked parts to"
/// and the footprints every ticked part's row offers (or why there are
/// none), and Clear ticks.
fn ticked_bar<'a>(
    state: &'a AssignFootprintsState,
    bar: TickedBar,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let (fill, ink) = (r.accent_soft, r.accent_soft_ink);
    let count = text(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_TICKED, "count" => bar.count))
        .size(TS.body)
        .color(ink)
        .font(semibold());
    let picker: Element<'a, Message> = if bar.offers.is_empty() {
        text(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_NO_SHARED))
            .size(TS.body)
            .color(ink)
            .boxed()
    } else {
        // The footprint every ticked part shows now, if they agree.
        let shared = bar
            .offers
            .iter()
            .find(|offer| {
                state
                    .selected
                    .iter()
                    .all(|&index| state.shown_binding(index) == Some(offer.binding.as_str()))
            })
            .map(|offer| Choice {
                binding: offer.binding.clone(),
                label: offer.label.clone(),
            });
        pick_list(shared, choices(&bar.offers), |c: &Choice| c.label.clone())
            .graphite()
            .on_select(|c: Choice| af(AssignFootprintsMsg::TickedPicked(c.binding)))
            .width(Length::Fixed(BAR_PICK_W))
            .boxed()
    };
    let clear = signex_widgets::button(
        text(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_CLEAR_TICKS))
            .size(TS.label)
            .color(ink),
    )
    .padding([SP.xs, SP.m])
    .on_press(af(AssignFootprintsMsg::AllToggled(false)))
    .style(crate::styles::ghost_button(tokens));
    let line = Row::<Element<'a, Message>>::new()
        .push(count.boxed())
        .push(
            text(t!(i18n, ids::DIALOG_ASSIGN_FOOTPRINTS_SET_TICKED))
                .size(TS.body)
                .color(ink)
                .boxed(),
        )
        .push(picker)
        .push(Space::new().width(Length::Fill).boxed())
        .push(clear.boxed())
        .spacing(SP.m)
        .align_y(Alignment::Center);
    container(line)
        .padding([SP.s, SP.m])
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
