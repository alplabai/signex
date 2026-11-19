//! Board Print / PDF's page plan (m116): one schematic card per page of
//! `BoardPrintOptions::page_plan`, live with the options, naming the page's
//! sets and the layers it holds (`BoardPrintOptions::page_layers`), tagged
//! "mirrored" on a mirrored bottom page. With no set ticked it shows its
//! empty state instead. The cards are not rendered thumbnails.

use iced::widget::{Column, Row, Space, column, container, row};
use iced::{Alignment, Background, Border, Element, Length, Theme, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_output::{BoardLayerSet, BoardPrintOptions};
use signex_types::pcb::PcbBoard;
use signex_types::theme::ThemeTokens;

use super::{SP, TS, kit};
use crate::app::Message;
use crate::app::board_output::set_label;
use crate::styles::{PillTone, Radii, chrome_px, tokens};

/// Height of a card's page (150 px).
const PAGE_H: f32 = chrome_px(150.0 * 11.0 / 12.0);
/// The board sketched on a card's page (110 x 70 px).
const BOARD_W: f32 = chrome_px(110.0 * 11.0 / 12.0);
const BOARD_H: f32 = chrome_px(70.0 * 11.0 / 12.0);
/// Gap between the cards (14 px).
const CARD_GAP: f32 = SP.l + SP.xs;

/// What one card of the plan says.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PageCard {
    /// The page's place in the plan, from 1.
    pub number: usize,
    /// The set names of the page, joined as the title block joins them.
    pub title: String,
    /// The layers the page holds, joined.
    pub layers: String,
    pub mirrored: bool,
}

/// The cards of `options`' page plan, in print order. `board` names the
/// layers; without it a card names its sets alone.
pub(super) fn page_cards(
    options: &BoardPrintOptions,
    board: Option<&PcbBoard>,
    i18n: &I18n,
) -> Vec<PageCard> {
    options
        .page_plan()
        .into_iter()
        .enumerate()
        .map(|(index, (sets, mirrored))| {
            let title = sets
                .iter()
                .map(|set| t!(i18n, set_label(*set)).into_string())
                .collect::<Vec<_>>()
                .join(" + ");
            let layers = board
                .map(|board| options.page_layers(board, &sets))
                .unwrap_or_default()
                .into_iter()
                .map(|layer| crate::pcb_canvas::layer_style::layer_name(i18n, layer).into_string())
                .collect::<Vec<_>>()
                .join(" · ");
            PageCard {
                number: index + 1,
                title,
                layers,
                mirrored,
            }
        })
        .collect()
}

/// The sets left unticked, as the last card lists them.
pub(super) fn unticked_sets(options: &BoardPrintOptions, i18n: &I18n) -> Option<String> {
    let off: Vec<String> = BoardLayerSet::ALL
        .into_iter()
        .filter(|set| !options.sets.contains(set))
        .map(|set| t!(i18n, set_label(set)).into_string())
        .collect();
    (!off.is_empty()).then(|| off.join(", "))
}

/// "PAGE PLAN" with the page count, then the cards two to a row, or the
/// empty state.
pub(super) fn plan<'a>(
    options: &BoardPrintOptions,
    board: Option<&PcbBoard>,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let cards = page_cards(options, board, i18n);
    let count = t!(i18n, ids::BOARD_PRINT_PAGE_COUNT, "count" => cards.len());
    let head = row![
        container(kit::section_label(
            t!(i18n, ids::BOARD_PRINT_PAGE_PLAN),
            i18n,
            tokens
        ))
        .width(Length::Fill),
        kit::status_pill(count, PillTone::Neutral, tokens),
    ]
    .align_y(Alignment::Center);
    if cards.is_empty() {
        return column![head, empty_state(i18n, tokens)]
            .spacing(SP.l - SP.xs)
            .boxed();
    }
    let mut tiles: Vec<Element<'a, Message>> = cards
        .into_iter()
        .map(|card| card_view(card, i18n, tokens))
        .collect();
    if let Some(off) = unticked_sets(options, i18n) {
        tiles.push(add_set_card(off, i18n, tokens));
    }
    let mut grid: Column<Element<'a, Message>> = Column::new().spacing(CARD_GAP);
    let mut tiles = tiles.into_iter();
    while let Some(first) = tiles.next() {
        let second = tiles.next().unwrap_or_else(|| Space::new().boxed());
        let pair: Row<Element<'a, Message>> = Row::new()
            .spacing(CARD_GAP)
            .push(container(first).width(Length::FillPortion(1)).boxed())
            .push(container(second).width(Length::FillPortion(1)).boxed());
        grid = grid.push(pair.boxed());
    }
    column![head, grid].spacing(SP.l - SP.xs).boxed()
}

/// A page as a card: the paper with the board sketched on it, then its
/// number, title and tag, then its layers.
fn card_view<'a>(card: PageCard, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let mut title: Row<Element<'a, Message>> = Row::new()
        .spacing(SP.s + SP.xs)
        .align_y(Alignment::Center)
        .push(
            signex_widgets::text(t!(i18n, ids::BOARD_PRINT_PAGE_NUMBER, "number" => card.number))
                .size(TS.caption)
                .font(super::super::graphite::mono())
                .color(r.ink4)
                .boxed(),
        )
        .push(
            signex_widgets::text(
                t!(i18n, ids::BOARD_PRINT_PAGE_SETS, "sets" => card.title.as_str()),
            )
            .size(TS.body)
            .font(kit::medium())
            .boxed(),
        );
    if card.mirrored {
        title = title.push(kit::status_pill(
            t!(i18n, ids::BOARD_PRINT_MIRRORED),
            PillTone::Neutral,
            tokens,
        ));
    }
    let mut col: Column<Element<'a, Message>> = Column::new()
        .spacing(SP.s)
        .push(paper(true, sketch(tokens), tokens))
        .push(title.boxed());
    if !card.layers.is_empty() {
        col = col.push(kit::caption(
            t!(i18n, ids::BOARD_PRINT_PAGE_LAYERS, "layers" => card.layers.as_str()),
            tokens,
        ));
    }
    col.boxed()
}

/// The last card while sets are left unticked: an empty page and the sets
/// it would take.
fn add_set_card<'a>(off: String, i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let hint = kit::caption(t!(i18n, ids::BOARD_PRINT_ADD_SET), tokens);
    column![
        paper(false, hint, tokens),
        kit::caption(
            t!(i18n, ids::BOARD_PRINT_NOT_PRINTED, "sets" => off.as_str()),
            tokens
        ),
    ]
    .spacing(SP.s)
    .boxed()
}

/// A page: `filled` paper in the raised fill, or an outline alone, with
/// `content` centred on it.
fn paper<'a>(
    filled: bool,
    content: Element<'a, Message>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let fill = filled.then_some(Background::Color(r.raised));
    let line = r.line_strong;
    container(content)
        .width(Length::Fill)
        .height(Length::Fixed(PAGE_H))
        .center_x(Length::Fill)
        .center_y(Length::Fixed(PAGE_H))
        .style(move |_: &Theme| container::Style {
            background: fill,
            border: Border {
                width: 1.0,
                color: line,
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// The board drawn on a card's page: its outline as a rounded frame.
fn sketch<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let ink = tokens::resolve(tokens).ink4;
    container(Space::new())
        .width(Length::Fixed(BOARD_W))
        .height(Length::Fixed(BOARD_H))
        .style(move |_: &Theme| container::Style {
            border: Border {
                width: 1.0,
                color: ink,
                radius: Radii::GRAPHITE.small.into(),
                ..Border::default()
            },
            ..container::Style::default()
        })
        .boxed()
}

/// No set ticked: the plan says what to do, in a framed note.
fn empty_state<'a>(i18n: &I18n, tokens: &ThemeTokens) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let line = r.line_strong;
    let words: Localized = t!(i18n, ids::BOARD_PRINT_NO_SETS);
    container(
        column![
            signex_widgets::text(words)
                .size(TS.body)
                .font(kit::medium())
                .color(crate::styles::ti(tokens.text)),
            signex_widgets::text(t!(i18n, ids::BOARD_PRINT_NO_SETS_HINT))
                .size(TS.body)
                .color(r.ink3),
        ]
        .spacing(SP.xs * 2.0)
        .align_x(Alignment::Center),
    )
    .padding(SP.l)
    .width(Length::Fill)
    .center_x(Length::Fill)
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: 1.0,
            color: line,
            radius: Radii::GRAPHITE.card.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}
