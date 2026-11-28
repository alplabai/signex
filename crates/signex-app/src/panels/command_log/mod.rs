//! The Command Log panel (#2458, mockup row 161): every catalog command this
//! session ran, newest last, with its source, arguments, result and time.
//! A pure view over [`CommandLogState`]; every action is a [`CommandLogMsg`]
//! the app runs in `app::handlers::command_log`. Rules: `docs/COMMAND_LOG.md`.

mod detail;
mod layout;
mod model;
mod table;
mod toolbar;

use std::path::PathBuf;
use std::time::Duration;

use iced::widget::{Column, Row, Space, column, container, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Widget as _};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::{ThemeId, ThemeTokens};
use signex_widgets::{button, text};

use super::{PanelContext, PanelMsg};
use crate::styles::{
    self, ChromeMetrics, HAIRLINE, Radii, Spacing, TypeScale, ti,
    tokens::{composite_over, hover_overlay, resolve},
};

pub(crate) use toolbar::title_actions;

pub use model::{
    CLI_CLIENT, CommandLog, JumpTarget, LogCounts, LogFilters, LogOutcome, LogRow, LogSource,
    MAX_ROWS, ResultFilter, SourceKind, failure_code, operand_text,
};
pub(crate) use model::{RECORDED_INPUT, RECORDED_TARGETS, RecordedTargets};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;
const CM: ChromeMetrics = ChromeMetrics::DEFAULT;

/// Width of the detail pane (mockup row 161).
const DETAIL_WIDTH: f32 = 320.0;
/// Width of the empty-session hint (the mock's max width).
const HINT_WIDTH: f32 = 440.0;
/// A tooltip longer than this many characters wraps.
const TOOLTIP_WRAP_CHARS: usize = 60;
/// Edge of a glyph in a button or the filter field.
const ICON: f32 = TS.body * 13.0 / 12.0;
/// The live dot of the footer.
const LIVE_DOT: f32 = TS.body * 7.0 / 12.0;
/// Theme the glyphs are fetched for. Every glyph is tinted with an ink
/// token, which hides the theme's accent swap, so any theme gives the same.
const GLYPH_THEME: ThemeId = ThemeId::GraphiteDark;
/// How long a copy button says "Copied".
pub const COPIED_FOR: Duration = Duration::from_millis(1400);

/// Which copy button says "Copied".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyTarget {
    Error,
    Cli,
}

/// The panel's state: the log and what the panel shows of it. Lives in
/// `UiState`; the panel context holds a copy taken after each change.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandLogState {
    pub log: CommandLog,
    /// The row picked; `None` (or a row the filters hide) shows the newest
    /// listed row.
    pub selected: Option<u64>,
    /// The list keeps the newest row in view.
    pub follow_newest: bool,
    /// The More menu is open.
    pub more_open: bool,
    /// The copy button that says "Copied", and the flash that ends it.
    pub copied: Option<(CopyTarget, u64)>,
    /// The agent socket's endpoint while it listens.
    pub endpoint: Option<String>,
    flashes: u64,
    /// Commands run inside a logged one: they are not logged again.
    pub(crate) nesting: u32,
}

impl Default for CommandLogState {
    fn default() -> Self {
        Self {
            log: CommandLog::default(),
            selected: None,
            follow_newest: true,
            more_open: false,
            copied: None,
            endpoint: None,
            flashes: 0,
            nesting: 0,
        }
    }
}

impl CommandLogState {
    /// The row the detail pane shows: the picked one while the filters list
    /// it, else the newest listed row.
    pub fn shown_row<'r>(&self, visible: &[&'r LogRow]) -> Option<&'r LogRow> {
        self.selected
            .and_then(|seq| visible.iter().copied().find(|row| row.seq == seq))
            .or_else(|| visible.last().copied())
    }

    /// Show "Copied" on `target`; returns the flash to end it with.
    pub(crate) fn start_flash(&mut self, target: CopyTarget) -> u64 {
        self.flashes += 1;
        self.copied = Some((target, self.flashes));
        self.flashes
    }
}

/// What the user did in the Command Log panel. Rows are named by their
/// sequence number.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandLogMsg {
    /// The search text changed.
    FilterChanged(String),
    /// A source chip was pressed.
    SourceToggled(SourceKind),
    /// All, Failed or Undone was picked.
    ResultFilterPicked(ResultFilter),
    /// A row was clicked.
    RowSelected(u64),
    /// Replay: run the row's command again with its arguments.
    ReplayRequested(u64),
    /// Copy the row's CLI line to the clipboard.
    CopyCliRequested(u64),
    /// Copy the row's full error to the clipboard.
    CopyErrorRequested(u64),
    /// The "Copied" flash with this number ran out.
    CopiedFlashEnded(u64),
    /// Save log as… was pressed.
    SaveRequested,
    /// The save finished: the file written, `None` when the dialog was
    /// cancelled, or why it failed.
    Saved(Result<Option<PathBuf>, String>),
    /// Pause logging or Resume logging was pressed.
    PauseToggled,
    /// Clear was pressed.
    Cleared,
    /// Follow newest was switched.
    FollowNewestToggled,
    /// More was pressed.
    MoreToggled,
    /// More ▸ Show view commands was switched.
    ViewCommandsToggled,
    /// Jump to was pressed on a row.
    JumpRequested(u64),
}

fn message(msg: CommandLogMsg) -> PanelMsg {
    PanelMsg::CommandLog(msg)
}

/// The panel, laid out for the width its dock gives it.
pub fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    iced::widget::responsive(move |size| sized_view(ctx, size.width)).boxed()
}

/// The list beside the detail pane of the row it shows.
fn split_view<'a>(
    ctx: &'a PanelContext,
    state: &'a CommandLogState,
    visible: &[&'a LogRow],
    width: f32,
) -> Element<'a, PanelMsg> {
    let shown = state.shown_row(visible);
    let detail_width = layout::detail_width(width);
    let table_width = if shown.is_some() {
        width - detail_width
    } else {
        width
    };
    let main = if visible.is_empty() {
        no_match(ctx)
    } else {
        table::view(ctx, state, visible, shown.map(|row| row.seq), table_width)
    };
    let mut split: Row<Element<'a, PanelMsg>> = Row::new().height(Length::Fill);
    split = split.push(
        container(main)
            .width(Length::Fill)
            .height(Length::Fill)
            .boxed(),
    );
    if let Some(row) = shown {
        split = split.push(detail::view(ctx, state, row, detail_width));
    }
    split.boxed()
}

fn sized_view<'a>(ctx: &'a PanelContext, width: f32) -> Element<'a, PanelMsg> {
    let state: &'a CommandLogState = &ctx.command_log;
    let visible = state.log.visible();
    let counts = state.log.counts();
    let empty = state.log.is_empty();
    let body = if empty {
        empty_hint(ctx)
    } else {
        split_view(ctx, state, &visible, width)
    };
    let mut col: Column<Element<'a, PanelMsg>> =
        Column::new().width(Length::Fill).height(Length::Fill);
    col = col.push(toolbar::bar(ctx, state, &counts, empty));
    if state.more_open {
        col = col.push(toolbar::more_strip(ctx, state));
    }
    col = col.push(rule(&ctx.tokens));
    col = col.push(body);
    col = col.push(rule(&ctx.tokens));
    col = col.push(footer(ctx, state, visible.len(), counts.total));
    col.boxed()
}

/// The footer: whether it logs and where the agent socket listens, how many
/// rows are listed, and that the log is kept for the session only.
fn footer<'a>(
    ctx: &'a PanelContext,
    state: &'a CommandLogState,
    shown: usize,
    total: usize,
) -> Element<'a, PanelMsg> {
    let i18n: &I18n = &ctx.i18n;
    let tokens = &ctx.tokens;
    let ink4 = resolve(tokens).ink4;
    let (dot, status) = if state.log.paused {
        (ti(tokens.warning), t!(i18n, ids::COMMAND_LOG_FOOTER_PAUSED))
    } else {
        let status = match &state.endpoint {
            Some(endpoint) => t!(
                i18n, ids::COMMAND_LOG_FOOTER_LISTENING,
                "endpoint" => endpoint.as_str()
            ),
            None => t!(i18n, ids::COMMAND_LOG_FOOTER_OFF),
        };
        (ti(tokens.success), status)
    };
    let count = if state.log.is_empty() {
        t!(i18n, ids::COMMAND_LOG_FOOTER_NONE)
    } else {
        t!(
            i18n, ids::COMMAND_LOG_FOOTER_COUNT,
            "shown" => shown, "total" => total
        )
    };
    let caption = |label: Localized| -> Element<'a, PanelMsg> {
        text(label)
            .size(TS.caption)
            .color(ink4)
            .wrapping(iced::widget::text::Wrapping::None)
            .boxed()
    };
    let line = row![
        container(Space::new().width(LIVE_DOT).height(LIVE_DOT)).style(move |_: &iced::Theme| {
            container::Style {
                background: Some(Background::Color(dot)),
                border: Border {
                    radius: (LIVE_DOT / 2.0).into(),
                    ..Border::default()
                },
                ..container::Style::default()
            }
        }),
        caption(status),
        Space::new().width(Length::Fill),
        caption(count),
        caption(t!(i18n, ids::COMMAND_LOG_FOOTER_SESSION)),
    ]
    .spacing(SP.m + SP.xs)
    .align_y(Alignment::Center);
    let chrome = ti(tokens.toolbar_bg);
    container(line)
        .padding([0.0, SP.m + SP.xs])
        .width(Length::Fill)
        .height(CM.status)
        .center_y(CM.status)
        .clip(true)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(chrome)),
            ..container::Style::default()
        })
        .boxed()
}

/// The empty session: what the panel will list, and the CLI command that
/// lists the catalog.
fn empty_hint<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let i18n: &I18n = &ctx.i18n;
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let key = container(
        text(t!(i18n, ids::COMMAND_LOG_EMPTY_KEY))
            .size(TS.caption)
            .font(mono()),
    )
    .padding([0.0, SP.xs * 2.0])
    .style(styles::key_cap(tokens));
    let card = column![
        text(t!(i18n, ids::COMMAND_LOG_EMPTY_TITLE))
            .size(TS.body)
            .font(weighted(iced::font::Weight::Medium))
            .color(ti(tokens.text)),
        text(t!(i18n, ids::COMMAND_LOG_EMPTY_BODY))
            .size(TS.body)
            .color(r.ink3)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center),
        row![
            key,
            text(t!(i18n, ids::COMMAND_LOG_EMPTY_KEY_CAPTION))
                .size(TS.caption)
                .color(r.ink4),
        ]
        .spacing(SP.m)
        .align_y(Alignment::Center),
    ]
    .spacing(SP.xs * 2.0)
    .align_x(Alignment::Center);
    let line = r.line_strong;
    let framed = container(card)
        .padding(SP.l + SP.xs)
        .width(HINT_WIDTH)
        .style(move |_: &iced::Theme| container::Style {
            border: Border {
                width: HAIRLINE,
                color: line,
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    container(framed)
        .padding(SP.m)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .boxed()
}

/// The list when the filters leave no row.
fn no_match<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    container(
        text(t!(ctx.i18n, ids::COMMAND_LOG_NO_MATCH))
            .size(TS.body)
            .color(resolve(&ctx.tokens).ink4),
    )
    .center_x(Length::Fill)
    .height(CM.row * 2.0 + SP.m)
    .center_y(CM.row * 2.0 + SP.m)
    .boxed()
}

/// A hairline across the panel.
fn rule<'a>(tokens: &ThemeTokens) -> Element<'a, PanelMsg> {
    let line = resolve(tokens).line_soft;
    container(Space::new())
        .width(Length::Fill)
        .height(HAIRLINE)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(line)),
            ..container::Style::default()
        })
        .boxed()
}

/// A monochrome glyph `side` px square, tinted `ink`.
fn icon<'a>(handle: svg::Handle, side: f32, ink: Color) -> Element<'a, PanelMsg> {
    svg(handle)
        .width(side)
        .height(side)
        .style(move |_: &iced::Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
        .boxed()
}

/// The UI face at a heavier weight.
fn weighted(weight: iced::font::Weight) -> iced::Font {
    iced::Font {
        weight,
        ..crate::fonts::iced_font_for_family(crate::fonts::DEFAULT_UI_FONT)
    }
}

fn mono() -> iced::Font {
    crate::fonts::iced_font_for_family(crate::fonts::CHROME_MONO_FONT)
}

/// The mock's `.btn` looks: a field-filled bordered button, or the primary
/// fill.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Look {
    Field,
    Primary,
}

/// A button with a leading glyph in `look`; `small` is the mock's `.btn.sm`.
/// Greyed and inert without `on_press`.
fn labelled_button<'a>(
    tokens: &ThemeTokens,
    look: Look,
    small: bool,
    glyph: svg::Handle,
    label: Localized,
    on_press: Option<PanelMsg>,
) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let enabled = on_press.is_some();
    let ink = match look {
        Look::Primary => r.primary_ink,
        Look::Field => ti(tokens.text),
    };
    let ink = if enabled { ink } else { styles::dimmed(ink) };
    let size = if small { TS.caption } else { TS.body };
    let content = row![icon(glyph, ICON, ink), text(label).size(size).color(ink)]
        .spacing(SP.xs * 2.0)
        .align_y(Alignment::Center);
    let (height, pad, radius) = if small {
        (TS.body * 11.0 / 6.0, SP.m, Radii::GRAPHITE.small)
    } else {
        (CM.field, SP.m + SP.xs, Radii::GRAPHITE.button)
    };
    let (field, line, hover) = (r.field, ti(tokens.border), hover_overlay(tokens));
    let primary = styles::primary_button(tokens);
    let b = button(container(content).center_y(Length::Fill))
        .height(height)
        .padding([0.0, pad])
        .on_press_maybe(on_press)
        .style(move |theme: &iced::Theme, status: button::Status| {
            let mut style = match look {
                Look::Primary => primary(theme, status),
                Look::Field => button::Style {
                    background: Some(Background::Color(match status {
                        button::Status::Hovered | button::Status::Pressed => {
                            composite_over(hover, field)
                        }
                        _ => field,
                    })),
                    border: Border {
                        width: HAIRLINE,
                        color: line,
                        radius: radius.into(),
                        ..Border::default()
                    },
                    ..button::Style::default()
                },
            };
            style.text_color = ink;
            style.border.radius = radius.into();
            if !enabled {
                style = styles::dimmed_button(style);
            }
            style
        });
    b.boxed()
}

/// A 24 px square icon button with a hover fill and a tooltip.
fn icon_button<'a>(
    tokens: &ThemeTokens,
    glyph: svg::Handle,
    tip: Localized,
    on_press: PanelMsg,
) -> Element<'a, PanelMsg> {
    let ink = resolve(tokens).ink3;
    let hover = hover_overlay(tokens);
    let side = CM.field - SP.xs;
    let face = button(container(icon(glyph, ICON, ink)).center(Length::Fill))
        .width(side)
        .height(side)
        .padding(Padding::ZERO)
        .on_press(on_press)
        .style(
            move |_: &iced::Theme, status: button::Status| button::Style {
                background: matches!(status, button::Status::Hovered | button::Status::Pressed)
                    .then_some(Background::Color(hover)),
                border: Border {
                    radius: Radii::GRAPHITE.small.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            },
        );
    super::graphite::hint_tooltip(
        face.boxed(),
        tip,
        tokens,
        iced::widget::tooltip::Position::Bottom,
    )
}

/// `content` with the user text `tip` shown on hover.
fn text_tooltip<'a>(
    tokens: &ThemeTokens,
    content: Element<'a, PanelMsg>,
    tip: String,
) -> Element<'a, PanelMsg> {
    // A long text wraps in the detail pane's width instead of running off.
    let wraps = tip.chars().count() > TOOLTIP_WRAP_CHARS;
    let mut tip = container(
        text(UserText::new(tip))
            .size(TS.caption)
            .color(ti(tokens.text)),
    )
    .padding([SP.xs, SP.s])
    .style(styles::context_menu(tokens));
    if wraps {
        tip = tip.width(DETAIL_WIDTH);
    }
    signex_widgets::tooltip(content, tip, iced::widget::tooltip::Position::Bottom)
        .gap(SP.xs)
        .boxed()
}

/// How long a row took: "182 ms", "4.8 s".
fn took_text(i18n: &I18n, took: Duration) -> Localized {
    let ms = took.as_millis();
    if ms < 1000 {
        t!(i18n, ids::COMMAND_LOG_TOOK_MS, "ms" => u64::try_from(ms).unwrap_or(u64::MAX))
    } else {
        let tenths = (took.as_secs_f64() * 10.0).round() / 10.0;
        t!(i18n, ids::COMMAND_LOG_TOOK_S, "s" => signex_i18n::LocalNumber(tenths))
    }
}

/// The source pill of a row: GUI, CLI or Agent with its glyph.
fn source_pill<'a>(tokens: &ThemeTokens, i18n: &I18n, source: &LogSource) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let (fill, ink, glyph, label) = match source.kind() {
        SourceKind::Gui => (
            r.sunken,
            r.ink3,
            crate::icons::icon_log_mouse(GLYPH_THEME),
            ids::COMMAND_LOG_PILL_GUI,
        ),
        SourceKind::Cli => (
            r.info_soft,
            r.info,
            crate::icons::icon_log_terminal(GLYPH_THEME),
            ids::COMMAND_LOG_PILL_CLI,
        ),
        SourceKind::Agent => (
            r.accent_soft,
            r.accent_soft_ink,
            crate::icons::icon_log_plug(GLYPH_THEME),
            ids::COMMAND_LOG_PILL_AGENT,
        ),
    };
    let pill_height = TS.body * 1.5;
    container(
        row![
            icon(glyph, TS.body, ink),
            text(t!(i18n, label))
                .size(TS.caption)
                .font(weighted(iced::font::Weight::Medium))
                .color(ink)
                .wrapping(iced::widget::text::Wrapping::None),
        ]
        .spacing(SP.xs * 2.0)
        .align_y(Alignment::Center),
    )
    .height(pill_height)
    .center_y(pill_height)
    .padding([0.0, SP.s + SP.xs])
    .style(move |_: &iced::Theme| container::Style {
        background: Some(Background::Color(fill)),
        border: Border {
            radius: Radii::GRAPHITE.pill.into(),
            ..Border::default()
        },
        ..container::Style::default()
    })
    .boxed()
}

/// The glyph and its tint for a result.
fn result_glyph(tokens: &ThemeTokens, outcome: &LogOutcome) -> (svg::Handle, Color) {
    match outcome {
        LogOutcome::Ok(_) => (crate::icons::icon_log_ok(GLYPH_THEME), ti(tokens.success)),
        LogOutcome::Warn(_) => (crate::icons::icon_log_warn(GLYPH_THEME), ti(tokens.warning)),
        LogOutcome::Failed(_) => (crate::icons::icon_log_error(GLYPH_THEME), ti(tokens.error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_probe::{LaidOutText, lay_out_texts};

    /// The short bottom dock of the audit capture (mockup row 161): the
    /// panel's content below its title bar, at the 12 px body.
    const SHORT_DOCK: iced::Size = iced::Size::new(890.0, 180.0);

    fn context_with_one_row() -> PanelContext {
        let (app, _boot) = crate::app::Signex::new();
        let mut ctx = app.document_view.panel_ctx.clone();
        let log = &mut std::sync::Arc::make_mut(&mut ctx.command_log).log;
        let seq = log.record(LogRow {
            seq: 0,
            at: chrono::Local::now(),
            source: LogSource::Gui { window: None },
            command: "undo".to_string(),
            label: "Undo".to_string(),
            operands: serde_json::Map::new(),
            outcome: LogOutcome::Failed("Nothing to undo".to_string()),
            took: Duration::ZERO,
            document: None,
            project: None,
            undoable: false,
            view_only: false,
            undone_at: None,
        });
        assert_eq!(seq, Some(1));
        ctx
    }

    fn laid_out(ctx: &PanelContext, window: iced::Size) -> Vec<LaidOutText> {
        lay_out_texts(view(ctx), window)
            .into_iter()
            .map(|t| LaidOutText {
                text: t.text.replace(['\u{2068}', '\u{2069}'], ""),
                ..t
            })
            .collect()
    }

    fn bounds(texts: &[LaidOutText], label: &str) -> iced::Rectangle {
        crate::layout_probe::find(texts, label).bounds
    }

    /// Mockup row 161: search, Source chips, All / Failed / Undone and
    /// Follow newest share one row in the bottom dock, Follow newest last.
    #[test]
    fn the_filters_and_follow_newest_share_one_row_in_the_bottom_dock() {
        let ctx = context_with_one_row();
        let texts = laid_out(&ctx, SHORT_DOCK);
        let source = bounds(&texts, "Source");
        let all = bounds(&texts, "All");
        let follow = bounds(&texts, "Follow newest");
        let line = |b: iced::Rectangle| b.center_y();
        let tolerance = TS.body / 2.0;
        assert!(
            (line(all) - line(source)).abs() < tolerance,
            "{all:?} vs {source:?}"
        );
        assert!(
            (line(follow) - line(source)).abs() < tolerance,
            "{follow:?}"
        );
        let undone = bounds(&texts, "Undone 0");
        assert!(
            (line(undone) - line(source)).abs() < tolerance,
            "{undone:?}"
        );
        assert!(
            follow.x > undone.x + undone.width,
            "Follow newest comes after the last result segment"
        );
        assert!(follow.x + follow.width <= SHORT_DOCK.width, "{follow:?}");
    }

    /// A narrow side dock still shows Follow newest, on its own line.
    #[test]
    fn a_side_dock_keeps_follow_newest_inside_the_panel() {
        let ctx = context_with_one_row();
        let narrow = iced::Size::new(290.0, 400.0);
        let follow = bounds(&laid_out(&ctx, narrow), "Follow newest");
        assert!(follow.x + follow.width <= narrow.width, "{follow:?}");
    }

    /// Mockup row 161: in a side dock the filters and Follow newest do not
    /// fit one line, so Follow newest takes its own line under the filters;
    /// in the bottom dock it shares the filters' line.
    #[test]
    fn a_side_dock_puts_follow_newest_on_its_own_line() {
        let ctx = context_with_one_row();
        let texts = laid_out(&ctx, iced::Size::new(290.0, 400.0));
        let source = bounds(&texts, "Source");
        let undone = bounds(&texts, "Undone 0");
        let follow = bounds(&texts, "Follow newest");
        assert!(
            follow.y >= source.y + source.height && follow.y >= undone.y + undone.height,
            "under the filters: {follow:?} vs {source:?} {undone:?}"
        );
        let texts = laid_out(&ctx, SHORT_DOCK);
        let (source, follow) = (bounds(&texts, "Source"), bounds(&texts, "Follow newest"));
        assert!(
            (follow.center_y() - source.center_y()).abs() < TS.body / 2.0,
            "{follow:?} vs {source:?}"
        );
    }

    /// The search field gives up width first: the widest panel gives it
    /// the mock's width, a side dock its least, the bottom dock what lies
    /// between, and Follow newest stays in the panel.
    #[test]
    fn the_search_field_shrinks_so_follow_newest_stays_in_the_dock() {
        let ctx = context_with_one_row();
        let i18n: &I18n = &ctx.i18n;
        let placeholder = t!(i18n, ids::COMMAND_LOG_FILTER_PLACEHOLDER)
            .into_string()
            .replace(['\u{2068}', '\u{2069}'], "");
        let input_width = |width: f32| {
            let window = iced::Size::new(width, SHORT_DOCK.height);
            let inputs = crate::layout_probe::lay_out_inputs(view(&ctx), window);
            crate::layout_probe::find(&inputs, &placeholder)
                .bounds
                .width
        };
        let (wide, dock, side) = (input_width(1400.0), input_width(890.0), input_width(290.0));
        assert!(
            (wide - side - (layout::SEARCH_W - layout::SEARCH_MIN)).abs() < 0.5,
            "the mock's width at 1400 px, the least in a side dock: {wide} {side}"
        );
        assert!(side <= dock && dock <= wide, "{side} {dock} {wide}");
        let follow = bounds(&laid_out(&ctx, SHORT_DOCK), "Follow newest");
        assert!(follow.x + follow.width <= SHORT_DOCK.width, "{follow:?}");
    }

    /// Mockup row 161: in a short dock the detail pane keeps Replay and Jump
    /// to in view; the error, arguments, result, undo and CLI line scroll.
    #[test]
    fn a_short_dock_keeps_the_detail_actions_in_view() {
        let ctx = context_with_one_row();
        let texts = laid_out(&ctx, SHORT_DOCK);
        let replay = bounds(&texts, "Replay");
        assert!(
            replay.y + replay.height <= SHORT_DOCK.height,
            "Replay at {replay:?} is below the panel"
        );
        let jump = bounds(&texts, "Jump to object");
        assert!(jump.y + jump.height <= SHORT_DOCK.height, "{jump:?}");
    }

    /// A failed row's Replay hint stays pinned under Replay, so a short dock
    /// never shows Replay without the hint that qualifies it.
    #[test]
    fn a_short_dock_keeps_the_replay_hint_under_replay() {
        let ctx = context_with_one_row();
        let texts = laid_out(&ctx, SHORT_DOCK);
        let replay = bounds(&texts, "Replay");
        let hint = bounds(
            &texts,
            "Replay runs it again with the same arguments; it fails the same way until the cause is fixed.",
        );
        assert!(hint.y >= replay.y + replay.height, "{hint:?} vs {replay:?}");
        assert!(hint.y + hint.height <= SHORT_DOCK.height, "{hint:?}");
    }
}
