//! The parts of the Alternate part picker (mockup row 208): the Base part
//! card, a candidate row, a status pill and the compare strip. Pure views.

use iced::widget::{Column, Row, Space, column, container, row};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, MessageId, UserText, ids, t};
use signex_library::{AlternateStatus, LifecycleState};
use signex_types::theme::ThemeTokens;
use signex_widgets::{button, text};

use super::VariantPartEditMsg;
use super::candidates::{
    Candidate, CandidateStatus, PinCompare, alternate_values, compare_pins, same_footprint,
};
use super::state::EditedPart;
use super::wiring::{PinProblem, pin_problem};
use crate::app::Message;
use crate::library::picker::mono;
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, Radii, Spacing, TypeScale, medium_font, pill, pill_colors, ti};

pub(super) const TS: TypeScale = TypeScale::DEFAULT;
pub(super) const SP: Spacing = Spacing::DEFAULT;

/// Widths of the candidate columns, in `body`-size characters: MPN,
/// Manufacturer, Value, Footprint, Stock and Lifecycle.
const COLUMN_EMS: [f32; 6] = [15.0, 13.0, 8.0, 9.0, 5.0, 7.0];

/// A row height, in `body`-size characters.
const ROW_EMS: f32 = 2.2;

/// A word in the catalog's text, sized for the picker.
fn words<'a>(label: Localized, size: f32, ink: Color) -> Element<'a, Message> {
    text(label)
        .size(size)
        .color(ink)
        .wrapping(iced::widget::text::Wrapping::None)
        .boxed()
}

/// User data (a part number, a value) in the mono face.
fn data<'a>(value: &str, ink: Color) -> Element<'a, Message> {
    text(UserText::new(value.to_string()))
        .size(TS.body)
        .font(mono())
        .color(ink)
        .wrapping(iced::widget::text::Wrapping::None)
        .boxed()
}

/// User data in the UI face.
fn plain<'a>(value: &str, ink: Color) -> Element<'a, Message> {
    text(UserText::new(value.to_string()))
        .size(TS.body)
        .color(ink)
        .wrapping(iced::widget::text::Wrapping::None)
        .boxed()
}

/// `content` with `tip` in the shared tooltip card above it.
pub(super) fn with_tip<'a>(
    content: Element<'a, Message>,
    tip: Localized,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let tip = container(text(tip).size(TS.caption).color(ti(tokens.text)))
        .padding([SP.xs, SP.s])
        .style(crate::styles::context_menu(tokens));
    signex_widgets::tooltip(content, tip, iced::widget::tooltip::Position::Top)
        .gap(SP.xs)
        .boxed()
}

/// A small status pill.
pub(super) fn status_pill<'a>(
    label: Localized,
    tone: PillTone,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    container(text(label).size(TS.caption).font(medium_font()))
        .padding([0.0, SP.s])
        .style(pill(tokens, tone))
        .boxed()
}

/// What the library says of a candidate, and its tone.
pub(super) fn status_words(status: CandidateStatus) -> (MessageId, PillTone) {
    match status {
        CandidateStatus::Alternate(AlternateStatus::Approved | AlternateStatus::Primary) => {
            (ids::VARIANT_EDIT_STATUS_APPROVED, PillTone::Ok)
        }
        CandidateStatus::Alternate(AlternateStatus::Conditional) => {
            (ids::VARIANT_EDIT_STATUS_CONDITIONAL, PillTone::Warn)
        }
        CandidateStatus::Alternate(AlternateStatus::Disqualified) => {
            (ids::VARIANT_EDIT_STATUS_DISQUALIFIED, PillTone::Err)
        }
        CandidateStatus::Lifecycle(LifecycleState::Released) => {
            (ids::LIBRARY_LIFECYCLE_RELEASED, PillTone::Ok)
        }
        CandidateStatus::Lifecycle(LifecycleState::Draft) => {
            (ids::LIBRARY_LIFECYCLE_DRAFT, PillTone::Neutral)
        }
        CandidateStatus::Lifecycle(LifecycleState::InReview) => {
            (ids::LIBRARY_LIFECYCLE_IN_REVIEW, PillTone::Neutral)
        }
        CandidateStatus::Lifecycle(LifecycleState::Deprecated) => {
            (ids::LIBRARY_LIFECYCLE_DEPRECATED, PillTone::Warn)
        }
        CandidateStatus::Lifecycle(LifecycleState::Obsolete) => {
            (ids::LIBRARY_LIFECYCLE_OBSOLETE, PillTone::Err)
        }
        _ => (ids::LIBRARY_LIFECYCLE_UNKNOWN, PillTone::Neutral),
    }
}

/// The Base part on top: the designator, its facts, and whether the edited
/// variant fits it (`fitted`).
pub(super) fn base_card<'a>(
    part: &EditedPart,
    fitted: bool,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = resolve(tokens);
    let ink = ti(tokens.text);
    let fact = |label: MessageId, value: &str| -> Element<'a, Message> {
        row![words(t!(i18n, label), TS.caption, r.ink3), data(value, ink)]
            .spacing(SP.xs)
            .align_y(Alignment::Center)
            .boxed()
    };
    let base = &part.base;
    let pins = base.pins.len().to_string();
    let facts = row![
        fact(ids::VARIANT_EDIT_FACT_MPN, &base.mpn),
        fact(ids::VARIANT_EDIT_FACT_MANUFACTURER, &base.manufacturer),
        fact(ids::VARIANT_EDIT_FACT_VALUE, &base.value),
        fact(ids::VARIANT_EDIT_FACT_FOOTPRINT, &base.footprint),
        fact(ids::VARIANT_EDIT_FACT_PINS, &pins),
    ]
    .spacing(SP.l)
    .wrap();
    let who = column![
        text(UserText::new(part.this_part.designator.clone()))
            .size(TS.heading)
            .font(mono())
            .color(ink),
        words(t!(i18n, ids::VARIANT_EDIT_BASE_PART), TS.caption, r.ink4),
    ]
    .spacing(SP.xs / 2.0);
    let (fit, tone) = if fitted {
        (ids::VARIANT_EDIT_FITTED_IN, PillTone::Neutral)
    } else {
        (ids::VARIANT_EDIT_NOT_FITTED_IN, PillTone::Warn)
    };
    let fit = status_pill(
        t!(i18n, fit, "variant" => part.variant.as_str()),
        tone,
        tokens,
    );
    let fill = r.sunken;
    container(
        row![who, facts, fit]
            .spacing(SP.l)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([SP.s, SP.m])
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            radius: Radii::GRAPHITE.control.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// The column captions over the candidate rows.
pub(super) fn head_row<'a>(i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let ink = resolve(tokens).ink3;
    let heads = [
        ids::VARIANT_EDIT_FACT_MPN,
        ids::VARIANT_EDIT_FACT_MANUFACTURER,
        ids::VARIANT_EDIT_FACT_VALUE,
        ids::VARIANT_EDIT_FACT_FOOTPRINT,
        ids::VARIANT_EDIT_FACT_STOCK,
        ids::VARIANT_EDIT_FACT_LIFECYCLE,
    ];
    let mut line: Row<Element<'a, Message>> = Row::new().align_y(Alignment::Center);
    for (head, ems) in heads.into_iter().zip(COLUMN_EMS) {
        line = line.push(cell(words(t!(i18n, head), TS.caption, ink), ems));
    }
    container(line)
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

fn cell<'a>(face: Element<'a, Message>, ems: f32) -> Element<'a, Message> {
    container(face)
        .width(Length::Fixed(TS.body * ems))
        .clip(true)
        .boxed()
}

/// A group heading in the list.
pub(super) fn group_head<'a>(label: Localized, tokens: &ThemeTokens) -> Element<'a, Message> {
    container(words(label, TS.caption, resolve(tokens).ink3))
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// One candidate row; a click picks it. `at` is its index in
/// `Candidates::all`; `used_in` names a variant that already uses it.
#[expect(
    clippy::too_many_arguments,
    reason = "the candidate, its place, the part it would replace and the look are separate inputs of one row"
)]
pub(super) fn candidate_row<'a>(
    candidate: &Candidate,
    at: usize,
    is_picked: bool,
    base_footprint: &str,
    used_in: Option<&str>,
    part: &str,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let ink = ti(tokens.text);
    let warn = ti(tokens.warning);
    let mut mpn: Row<Element<'a, Message>> = row![data(&candidate.mpn, ink)]
        .spacing(SP.xs)
        .align_y(Alignment::Center);
    if let Some(variant) = used_in {
        let tip = t!(i18n, ids::VARIANT_EDIT_USED_IN_TIP, "variant" => variant, "part" => part);
        let used = status_pill(
            t!(i18n, ids::VARIANT_EDIT_USED_IN, "variant" => variant),
            PillTone::Info,
            tokens,
        );
        mpn = mpn.push(with_tip(used, tip, tokens));
    }
    let footprint_ink =
        if candidate.footprint.is_empty() || same_footprint(&candidate.footprint, base_footprint) {
            ink
        } else {
            warn
        };
    let stock = match candidate.stock {
        Some(stock) => data(&stock.to_string(), ink),
        None => words(t!(i18n, ids::VARIANT_EDIT_STOCK_UNKNOWN), TS.body, ink),
    };
    let (status, tone) = status_words(candidate.status);
    let line = row![
        cell(mpn.boxed(), COLUMN_EMS[0]),
        cell(plain(&candidate.manufacturer, ink), COLUMN_EMS[1]),
        cell(data(&candidate.value, ink), COLUMN_EMS[2]),
        cell(data(&candidate.footprint, footprint_ink), COLUMN_EMS[3]),
        cell(stock, COLUMN_EMS[4]),
        cell(status_pill(t!(i18n, status), tone, tokens), COLUMN_EMS[5]),
    ]
    .align_y(Alignment::Center);
    button(
        container(line)
            .height(Length::Fixed(TS.body * ROW_EMS))
            .align_y(iced::alignment::Vertical::Center),
    )
    .padding([0.0, SP.m])
    .width(Length::Fill)
    .on_press(Message::from(VariantPartEditMsg::CandidatePicked(at)))
    .style(crate::styles::hover_row(tokens, is_picked))
    .boxed()
}

/// One cell of the compare strip: a heading in its tone and a detail line.
fn compare_cell<'a>(
    head: Localized,
    detail: Element<'a, Message>,
    tone: PillTone,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let (fill, ink) = pill_colors(tokens, tone);
    let line = resolve(tokens).line_soft;
    container(
        column![
            text(head).size(TS.body).font(medium_font()).color(ink),
            detail
        ]
        .spacing(SP.xs / 2.0),
    )
    .width(Length::Fill)
    .padding([SP.s, SP.m])
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            width: 1.0,
            color: line,
            radius: Radii::GRAPHITE.control.into(),
        },
        ..container::Style::default()
    })
    .boxed()
}

/// The compare strip for `candidate` (mockup row 208): its pins, its
/// footprint, and what the variant gets. A difference is amber and says
/// what changes, naming the pin whose wiring changes once the placed part's
/// wired pins (`wired`) are read; Apply stays live (DECISIONS 2026-11-26).
pub(super) fn compare_strip<'a>(
    part: &EditedPart,
    candidate: &Candidate,
    wired: Option<&[String]>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let detail = |label: Localized| -> Element<'a, Message> {
        text(label)
            .size(TS.caption)
            .color(ti(tokens.text_secondary))
            .boxed()
    };
    let base = &part.base;
    let pins = match compare_pins(&base.pins, candidate.pins.as_deref()) {
        PinCompare::Same { count } => compare_cell(
            t!(i18n, ids::VARIANT_EDIT_PINS_MATCH),
            detail(t!(i18n, ids::VARIANT_EDIT_PINS_MATCH_DETAIL, "count" => count)),
            PillTone::Ok,
            tokens,
        ),
        PinCompare::Differ { theirs, ours } => {
            let problem = wired
                .zip(candidate.pins.as_deref())
                .and_then(|(wired, pins)| pin_problem(&base.pins, wired, pins));
            let words = match problem {
                Some(PinProblem::LosesWire { pin, name }) => t!(
                    i18n,
                    ids::VARIANT_EDIT_PINS_LOSES_WIRE,
                    "theirs" => theirs,
                    "ours" => ours,
                    "pin" => pin,
                    "name" => name
                ),
                Some(PinProblem::NoWire { pin, name }) => t!(
                    i18n,
                    ids::VARIANT_EDIT_PINS_NO_WIRE,
                    "theirs" => theirs,
                    "ours" => ours,
                    "pin" => pin,
                    "name" => name
                ),
                None => t!(
                    i18n,
                    ids::VARIANT_EDIT_PINS_DIFFER_DETAIL,
                    "theirs" => theirs,
                    "ours" => ours
                ),
            };
            compare_cell(
                t!(i18n, ids::VARIANT_EDIT_PINS_DIFFER),
                detail(words),
                PillTone::Warn,
                tokens,
            )
        }
        PinCompare::Unknown => compare_cell(
            t!(i18n, ids::VARIANT_EDIT_PINS_UNKNOWN),
            detail(t!(i18n, ids::VARIANT_EDIT_PINS_UNKNOWN_DETAIL)),
            PillTone::Neutral,
            tokens,
        ),
    };
    let footprint = if candidate.footprint.is_empty()
        || same_footprint(&candidate.footprint, &base.footprint)
    {
        compare_cell(
            t!(i18n, ids::VARIANT_EDIT_SAME_FOOTPRINT_HEAD),
            detail(t!(
                i18n,
                ids::VARIANT_EDIT_SAME_FOOTPRINT_DETAIL,
                "footprint" => base.footprint.as_str(),
                "variant" => part.variant.as_str()
            )),
            PillTone::Ok,
            tokens,
        )
    } else {
        compare_cell(
            t!(i18n, ids::VARIANT_EDIT_FOOTPRINT_DIFFERS),
            detail(t!(
                i18n,
                ids::VARIANT_EDIT_FOOTPRINT_DIFFERS_DETAIL,
                "theirs" => candidate.footprint.as_str(),
                "ours" => base.footprint.as_str(),
                "variant" => part.variant.as_str()
            )),
            PillTone::Warn,
            tokens,
        )
    };
    let (mpn, value, footprint_set) = alternate_values(candidate, &base.value, &base.footprint);
    let gets = compare_cell(
        t!(i18n, ids::VARIANT_EDIT_GETS, "variant" => part.variant.as_str()),
        detail(t!(
            i18n,
            ids::VARIANT_EDIT_GETS_DETAIL,
            "mpn" => mpn,
            "value" => value.unwrap_or_else(|| base.value.clone()),
            "footprint" => footprint_set.unwrap_or_else(|| base.footprint.clone())
        )),
        PillTone::Neutral,
        tokens,
    );
    row![pins, footprint, gets]
        .spacing(SP.s)
        .width(Length::Fill)
        .boxed()
}

/// Whether the compare strip warns for `candidate`: its pins or footprint
/// differ from the placed part's.
pub(super) fn differs(part: &EditedPart, candidate: &Candidate) -> bool {
    let pins_differ = matches!(
        compare_pins(&part.base.pins, candidate.pins.as_deref()),
        PinCompare::Differ { .. }
    );
    let footprint_differs = !candidate.footprint.is_empty()
        && !same_footprint(&candidate.footprint, &part.base.footprint);
    pins_differ || footprint_differs
}

/// A line of the list that is no row: reading, failed or empty.
pub(super) fn list_note<'a>(label: Localized, tokens: &ThemeTokens) -> Element<'a, Message> {
    container(words(label, TS.caption, resolve(tokens).ink4))
        .padding([SP.m, SP.m])
        .width(Length::Fill)
        .boxed()
}

/// Empty space that keeps the footer's buttons right.
pub(super) fn fill<'a>() -> Element<'a, Message> {
    Space::new().width(Length::Fill).boxed()
}

/// Column layout used by the picker body.
pub(super) fn stacked<'a>(children: Vec<Element<'a, Message>>) -> Column<Element<'a, Message>> {
    Column::with_children(children).spacing(SP.m)
}
