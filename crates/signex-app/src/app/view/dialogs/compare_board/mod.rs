//! The Compare Board with Schematic dialog (#2462, mockup m162): header with
//! Compare again, the kind chips and "Set shown rows", the table
//! (`table.rs`), the in-sync card, and the Export report… / Close / Apply
//! footer. The confirmation listing board deletions: `confirm.rs`. State
//! and rules: `app/compare_board/`.

use std::sync::LazyLock;

use iced::widget::{Column, Row, Space, column, container, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Length, Theme, Widget as _};
use signex_engine::pcb::eco::{CompareKind, CompareReport, Direction};
use signex_i18n::{I18n, Localized, MessageId, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::chips::{ChipMetrics, ChipStyle, chip};
use signex_widgets::{Label, SegmentedStyle, button, segmented, text};

use crate::app::compare_board::CompareBoardState;
use crate::app::{CompareBoardMsg, Message, Signex};
use crate::styles::{DialogButtonKind, Spacing, TypeScale, dialog_button, ti, tokens};

mod confirm;
mod table;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Design size of the dialog (m162).
const SIZE: (f32, f32) = (1060.0, 640.0);
/// Height of the header's small button (22 px at the 12 px body).
const SMALL_BUTTON_H: f32 = TS.body * 22.0 / 12.0;
/// Side of a button glyph (13 px at the 12 px body).
const BUTTON_GLYPH: f32 = TS.body * 13.0 / 12.0;

fn cb(msg: CompareBoardMsg) -> Message {
    Message::from(msg)
}

/// The kinds' chip labels, in report order.
fn kind_chip_id(kind: CompareKind) -> MessageId {
    match kind {
        CompareKind::MissingOnBoard => ids::COMPARE_BOARD_KIND_MISSING,
        CompareKind::ExtraOnBoard => ids::COMPARE_BOARD_KIND_EXTRA,
        CompareKind::NetDifference => ids::COMPARE_BOARD_KIND_NETS,
        CompareKind::FootprintMismatch => ids::COMPARE_BOARD_KIND_FOOTPRINTS,
        CompareKind::ValueChange => ids::COMPARE_BOARD_KIND_VALUES,
    }
}

fn direction_id(direction: Direction) -> MessageId {
    match direction {
        Direction::UpdatePcb => ids::COMPARE_BOARD_UPDATE_PCB,
        Direction::UpdateSchematic => ids::COMPARE_BOARD_UPDATE_SCHEMATIC,
        Direction::Skip => ids::COMPARE_BOARD_SKIP,
    }
}

/// The dialog's stroke glyphs (16 x 16, m162's sprite), drawn in the ink
/// they are given.
#[derive(Debug, Clone, Copy)]
enum Glyph {
    Refresh,
    Down,
    Ok,
    Schematic,
    Board,
    Route,
    Edit,
    ArrowRight,
    ArrowLeft,
}

const GLYPH_REFRESH: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"><path d="M13 8a5 5 0 11-1.5-3.6M13 2.5v3h-3"/></svg>"##;
const GLYPH_DOWN: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"><path d="M8 2.5v8M4.5 7.5L8 11l3.5-3.5M3 13.5h10"/></svg>"##;
const GLYPH_OK: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-linecap="round" stroke-linejoin="round"><circle cx="8" cy="8" r="5.8" stroke-width="1.3"/><path d="M5.5 8.2l1.8 1.8 3.3-3.6" stroke-width="1.4"/></svg>"##;
const GLYPH_SCHEMATIC: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.3" stroke-linecap="round"><path d="M2 8h3M11 8h3"/><rect x="5" y="5" width="6" height="6" rx="1"/></svg>"##;
const GLYPH_BOARD: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.3"><rect x="2" y="2.5" width="12" height="11" rx="1.5"/><circle cx="5" cy="5.5" r="1" fill="#000" stroke="none"/><path d="M5 5.5h3.5v5H11" stroke-width="1.2"/></svg>"##;
const GLYPH_ROUTE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M2.5 12.5h4l3-6h4"/><circle cx="2.5" cy="12.5" r="1.3" fill="#000" stroke="none"/><circle cx="13.5" cy="6.5" r="1.3" fill="#000" stroke="none"/></svg>"##;
const GLYPH_EDIT: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.3" stroke-linejoin="round"><path d="M3 13l.6-2.8 7-7 2.2 2.2-7 7z"/></svg>"##;
const GLYPH_ARROW_RIGHT: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d="M3 8h9.5M9.5 5l3 3-3 3"/></svg>"##;
const GLYPH_ARROW_LEFT: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" fill="none" stroke="#000" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round"><path d="M13 8H3.5M6.5 5l-3 3 3 3"/></svg>"##;

/// The glyphs parsed once, in [`Glyph`] order.
static GLYPHS: LazyLock<[svg::Handle; 9]> = LazyLock::new(|| {
    [
        GLYPH_REFRESH,
        GLYPH_DOWN,
        GLYPH_OK,
        GLYPH_SCHEMATIC,
        GLYPH_BOARD,
        GLYPH_ROUTE,
        GLYPH_EDIT,
        GLYPH_ARROW_RIGHT,
        GLYPH_ARROW_LEFT,
    ]
    .map(svg::Handle::from_memory)
});

/// `which` at `side` px in `ink`.
fn glyph<'a>(which: Glyph, side: f32, ink: Color) -> Element<'a, Message> {
    svg(GLYPHS[which as usize].clone())
        .width(Length::Fixed(side))
        .height(Length::Fixed(side))
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}

/// A 1 px rule in `color`.
fn rule<'a>(color: Color) -> Element<'a, Message> {
    container(Space::new())
        .width(Length::Fill)
        .height(Length::Fixed(1.0))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..container::Style::default()
        })
        .boxed()
}

impl Signex {
    pub(in crate::app::view) fn view_compare_board_dialog(&self) -> Element<'_, Message> {
        self.wrap_centred_dialog(self.view_compare_board_card(), SIZE)
    }

    fn view_compare_board_card(&self) -> Element<'_, Message> {
        let Some(state) = self.ui_state.compare_board.as_ref() else {
            return container(Space::new()).boxed();
        };
        let tokens = &self.document_state.panel_ctx.tokens;
        let i18n = self.ui_state.i18n.as_ref();
        let r = tokens::resolve(tokens);
        let report = state.report().filter(|_| state.failure().is_none());
        let in_sync = report.is_some_and(CompareReport::in_sync) && state.is_ready();
        let mut card = Column::new().push(header(state, i18n, tokens));
        match report {
            Some(report) if !in_sync => {
                card = card
                    .push(chips_bar(state, report, i18n, tokens))
                    .push(rule(r.line_soft));
                if let Some(line) = self.compare_board_banner(state, tokens) {
                    card = card.push(line);
                }
                card = card.push(table::view(state, report, i18n, tokens));
            }
            Some(report) => card = card.push(in_sync_card(report, i18n, tokens)),
            None => card = card.push(phase_card(state, i18n, tokens)),
        }
        let rows = report.filter(|_| !in_sync);
        card = card
            .push(rule(r.line_soft))
            .push(self.compare_board_footer(state, rows, i18n, tokens));
        container(card.width(Length::Fixed(super::widgets::modal_card_width(
            self.ui_state.window_size,
            SIZE.0,
        ))))
        .height(Length::Fixed(SIZE.1))
        .style(crate::styles::modal_card(tokens))
        .clip(true)
        .boxed()
    }

    /// Why Apply did nothing (or only the board side), else why it cannot
    /// run now; `None` when it can.
    fn compare_board_banner<'a>(
        &self,
        state: &CompareBoardState,
        tokens: &ThemeTokens,
    ) -> Option<Element<'a, Message>> {
        let (why, ink) = match state.error() {
            Some(why) => (why.to_string(), tokens::danger(tokens)),
            None if state.is_ready() => (self.compare_board_apply_block()?, ti(tokens.warning)),
            None => return None,
        };
        Some(
            container(
                text(signex_i18n::UserText::new(why))
                    .size(TS.body)
                    .color(ink),
            )
            .padding([SP.s, SP.l])
            .width(Length::Fill)
            .boxed(),
        )
    }

    /// Export report… on the left; the counts, Close and Apply on the
    /// right. With no rows (in sync, reading, failed) only Close.
    fn compare_board_footer<'a>(
        &self,
        state: &CompareBoardState,
        report: Option<&CompareReport>,
        i18n: &I18n,
        tokens: &ThemeTokens,
    ) -> Element<'a, Message> {
        let close = dialog_button(
            t!(i18n, ids::COMPARE_BOARD_CLOSE),
            DialogButtonKind::Cancel,
            Some(cb(CompareBoardMsg::CloseRequested)),
            tokens,
        );
        let Some(report) = report else {
            return footer_strip(Vec::new(), vec![close], tokens);
        };
        let ready = state.is_ready() && !state.checking;
        let counts = report.counts();
        let export = export_button(i18n, tokens, ready);
        let can_apply = ready && counts.applied() > 0 && self.compare_board_apply_block().is_none();
        let apply = dialog_button(
            t!(i18n, ids::COMPARE_BOARD_APPLY, "count" => counts.applied()),
            DialogButtonKind::Primary,
            can_apply.then(|| cb(CompareBoardMsg::ApplyRequested)),
            tokens,
        );
        let caption = text(footer_caption(state, report, i18n))
            .size(TS.body)
            .color(tokens::resolve(tokens).ink3)
            .boxed();
        footer_strip(vec![export], vec![caption, close, apply], tokens)
    }
}

/// The footer: `left` actions, a spacer, `right` ones, on the chrome strip.
fn footer_strip<'a>(
    left: Vec<Element<'a, Message>>,
    right: Vec<Element<'a, Message>>,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let line = row![
        Row::with_children(left)
            .spacing(SP.m)
            .align_y(Alignment::Center),
        Space::new().width(Length::Fill),
        Row::with_children(right)
            .spacing(SP.m)
            .align_y(Alignment::Center),
    ]
    .align_y(Alignment::Center);
    container(line)
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .style(crate::styles::modal_footer_strip(tokens))
        .boxed()
}

/// `[n of N shown · ]X to PCB · Y to schematic[ · Z skipped]`.
fn footer_caption(state: &CompareBoardState, report: &CompareReport, i18n: &I18n) -> Localized {
    let counts = report.counts();
    let (to_pcb, to_schematic, skipped) = (counts.to_pcb, counts.to_schematic, counts.skipped);
    match (state.filter(), skipped > 0) {
        (None, false) => t!(
            i18n,
            ids::COMPARE_BOARD_FOOTER,
            "to_pcb" => to_pcb,
            "to_schematic" => to_schematic
        ),
        (None, true) => t!(
            i18n,
            ids::COMPARE_BOARD_FOOTER_SKIPPED,
            "to_pcb" => to_pcb,
            "to_schematic" => to_schematic,
            "skipped" => skipped
        ),
        (Some(kind), false) => t!(
            i18n,
            ids::COMPARE_BOARD_FOOTER_SHOWN,
            "shown" => report.kind_count(Some(kind)),
            "total" => counts.total,
            "to_pcb" => to_pcb,
            "to_schematic" => to_schematic
        ),
        (Some(kind), true) => t!(
            i18n,
            ids::COMPARE_BOARD_FOOTER_SHOWN_SKIPPED,
            "shown" => report.kind_count(Some(kind)),
            "total" => counts.total,
            "to_pcb" => to_pcb,
            "to_schematic" => to_schematic,
            "skipped" => skipped
        ),
    }
}

/// The ghost Export report… button with its down glyph.
fn export_button<'a>(i18n: &I18n, tokens: &ThemeTokens, live: bool) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let ink = if live { r.accent_text } else { r.ink4 };
    let face = row![
        glyph(Glyph::Down, BUTTON_GLYPH, ink),
        text(t!(i18n, ids::COMPARE_BOARD_EXPORT))
            .size(TS.body)
            .color(ink)
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    button(face)
        .padding([SP.xs, SP.m])
        .on_press_maybe(live.then(|| cb(CompareBoardMsg::ExportRequested)))
        .style(crate::styles::ghost_button(tokens))
        .boxed()
}

/// Title, `<root sheet> ↔ <board> · compared HH:MM:SS`, Compare again and
/// the close x.
fn header<'a>(
    state: &CompareBoardState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let subtitle = match &state.compared_at {
        Some(time) => t!(
            i18n,
            ids::COMPARE_BOARD_SUBTITLE,
            "schematic" => state.schematic_name.clone(),
            "board" => state.board_name(),
            "time" => time.clone()
        ),
        None => t!(
            i18n,
            ids::COMPARE_BOARD_SUBTITLE_READING,
            "schematic" => state.schematic_name.clone(),
            "board" => state.board_name()
        ),
    };
    let r = tokens::resolve(tokens);
    let again_live = !state.checking;
    let again = button(
        row![
            glyph(Glyph::Refresh, BUTTON_GLYPH, ti(tokens.text)),
            text(t!(i18n, ids::COMPARE_BOARD_AGAIN))
                .size(TS.caption)
                .color(if again_live { ti(tokens.text) } else { r.ink4 })
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center),
    )
    .padding([0.0, SP.m])
    .height(Length::Fixed(SMALL_BUTTON_H))
    .on_press_maybe(again_live.then(|| cb(CompareBoardMsg::CompareAgainRequested)))
    .style(crate::styles::outline_button(tokens));
    super::header::graphite_header_closing(
        t!(i18n, ids::COMPARE_BOARD_TITLE).into_string(),
        Some(subtitle.into_string()),
        Some(container(again).boxed()),
        Some(cb(CompareBoardMsg::CloseRequested)),
        super::header::DIALOG_HEADER_H,
        tokens,
    )
}

/// The colours of a filter chip (m162 `.chip`).
fn chip_style(tokens: &ThemeTokens) -> ChipStyle {
    let r = tokens::resolve(tokens);
    ChipStyle {
        fill: r.field,
        edge: ti(tokens.border),
        hover: tokens::hover_overlay(tokens),
        ink: r.ink3,
        on_fill: r.accent_soft,
        on_ink: r.accent_soft_ink,
    }
}

/// The kind chips (All and the five kinds, each with its count), then
/// "Set shown rows:" and the three directions.
fn chips_bar<'a>(
    state: &CompareBoardState,
    report: &CompareReport,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let metrics = ChipMetrics::from_text_size(TS.body);
    let style = chip_style(tokens);
    let count = |kind| u32::try_from(report.kind_count(kind)).unwrap_or(u32::MAX);
    let filter = state.filter();
    let mut chips = Row::new()
        .spacing(SP.s)
        .align_y(Alignment::Center)
        .push(chip(
            t!(i18n, ids::COMPARE_BOARD_ALL),
            Some(count(None)),
            filter.is_none(),
            style,
            metrics,
            i18n,
            Some(cb(CompareBoardMsg::KindFilterPicked(None))),
        ));
    for kind in CompareKind::ALL {
        chips = chips.push(chip(
            t!(i18n, kind_chip_id(kind)),
            Some(count(Some(kind))),
            filter == Some(kind),
            style,
            metrics,
            i18n,
            Some(cb(CompareBoardMsg::KindFilterPicked(Some(kind)))),
        ));
    }
    let r = tokens::resolve(tokens);
    let choices = Direction::ALL.map(|d| (Some(d), Label::from(t!(i18n, direction_id(d)))));
    let shown = segmented(
        choices,
        state.shown_choice,
        SegmentedStyle::new(
            r.sunken,
            r.raised,
            r.shadow,
            r.ink3,
            ti(tokens.text),
            TS.caption,
        ),
        |d| {
            cb(CompareBoardMsg::ShownRowsDirectionPicked(
                d.unwrap_or(Direction::Skip),
            ))
        },
    );
    let line = chips
        .push(Space::new().width(Length::Fill))
        .push(
            text(t!(i18n, ids::COMPARE_BOARD_SET_SHOWN))
                .size(TS.caption)
                .color(r.ink4),
        )
        .push(shown);
    container(line)
        .padding([SP.m, SP.l])
        .width(Length::Fill)
        .boxed()
}

/// Board and schematic match: the ok card, centred.
fn in_sync_card<'a>(
    report: &CompareReport,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let r = tokens::resolve(tokens);
    let compared = report.compared;
    let title = row![
        glyph(Glyph::Ok, TS.body * 16.0 / 12.0, ti(tokens.success)),
        text(t!(i18n, ids::COMPARE_BOARD_IN_SYNC_TITLE))
            .size(TS.body)
            .color(ti(tokens.text))
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    let body = text(t!(
        i18n,
        ids::COMPARE_BOARD_IN_SYNC_BODY,
        "parts" => compared.parts,
        "nets" => compared.nets,
        "footprints" => compared.footprints
    ))
    .size(TS.body)
    .color(r.ink3)
    .align_x(iced::alignment::Horizontal::Center);
    let radius = crate::styles::Radii::GRAPHITE.card;
    let fill = r.ok_soft;
    let card = container(
        column![title, body]
            .spacing(SP.xs)
            .align_x(Alignment::Center),
    )
    .padding(SP.l + SP.xs)
    .max_width(TS.body * 420.0 / 12.0)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            radius: radius.into(),
            ..Border::default()
        },
        ..container::Style::default()
    });
    container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .boxed()
}

/// While reading, or why the read failed, centred.
fn phase_card<'a>(
    state: &CompareBoardState,
    i18n: &I18n,
    tokens: &ThemeTokens,
) -> Element<'a, Message> {
    let line = match state.failure() {
        Some(why) => text(signex_i18n::UserText::new(why.to_string()))
            .size(TS.body)
            .color(tokens::danger(tokens)),
        None => text(t!(i18n, ids::COMPARE_BOARD_READING))
            .size(TS.body)
            .color(tokens::resolve(tokens).ink3),
    };
    container(line)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .boxed()
}
