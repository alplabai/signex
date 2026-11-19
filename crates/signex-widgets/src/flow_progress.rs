//! Flow progress: a left-to-right pipeline of run stages (Validate, Generate,
//! Package, Summary in an output run), each a card with a state marker, its
//! name and a detail line the caller formats (counts, "running…").
//!
//! Stateless: the caller owns the stages, the selected stage and the message
//! a click maps to. Stage state comes from [`StageCounts::state`] or is set by
//! the caller directly. Every length is an em of the style's text size.

use std::sync::OnceLock;

use crate::{Label, button, text};
use iced::alignment::Vertical;
use iced::widget::text::{Ellipsis, Wrapping};
use iced::widget::{Column, Row, Space, column, container, row, svg};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Theme, Widget};
use signex_types::theme::ThemeTokens;

use crate::theme_ext::to_color;

/// Marker disc side, in em of the body text.
pub const MARKER_EM: f32 = 20.0 / 12.0;
/// Glyph inside the marker, in em.
pub const GLYPH_EM: f32 = 13.0 / 12.0;
/// Marker ring weight, in em.
pub const RING_EM: f32 = 1.5 / 12.0;
/// Width of the arrow between two stages, in em.
pub const CONNECTOR_EM: f32 = 26.0 / 12.0;
/// Weight of the connector line, in em.
pub const CONNECTOR_LINE_EM: f32 = 2.0 / 12.0;
/// Card padding (vertical, horizontal), in em.
pub const CARD_PAD_EM: (f32, f32) = (6.0 / 12.0, 8.0 / 12.0);
/// Card corner radius, in em.
pub const CARD_RADIUS_EM: f32 = 5.0 / 12.0;
/// Gap between the marker and the stage name, in em.
pub const GAP_EM: f32 = 6.0 / 12.0;
/// Name size (the uppercase section step) relative to the body text.
pub const NAME_RATIO: f32 = 11.0 / 12.0;
/// Detail size (the caption step) relative to the body text.
pub const DETAIL_RATIO: f32 = 10.0 / 11.0;

/// Where one stage of a run is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageState {
    /// Not started (or not reached because an earlier stage failed).
    Pending,
    /// At least one step started and the stage has not finished.
    Running,
    /// Every step finished cleanly.
    Done,
    /// Every step finished, some with warnings.
    Warning,
    /// A step failed; the run stops here.
    Failed,
}

/// Step counts of one stage; [`StageCounts::state`] derives its state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StageCounts {
    /// Steps in the stage.
    pub total: u32,
    /// Steps that have finished (with or without warnings).
    pub finished: u32,
    /// Steps running now.
    pub running: u32,
    /// Warnings raised by the finished steps.
    pub warnings: u32,
    /// Errors raised; any error fails the stage.
    pub errors: u32,
}

impl StageCounts {
    /// The stage's state: an error fails it, unfinished work with something
    /// started is running, nothing started is pending, else done (with or
    /// without warnings). A stage with no steps is done once reached, so the
    /// caller decides pending for it by not calling this.
    pub fn state(&self) -> StageState {
        if self.errors > 0 {
            StageState::Failed
        } else if self.running > 0 || (self.finished > 0 && self.finished < self.total) {
            StageState::Running
        } else if self.finished == 0 && self.total > 0 {
            StageState::Pending
        } else if self.warnings > 0 {
            StageState::Warning
        } else {
            StageState::Done
        }
    }
}

/// Fraction of the run's steps that have finished, 0.0 to 1.0, for an
/// overall progress bar. An empty run reads as 0.
pub fn run_fraction(stages: &[StageCounts]) -> f32 {
    let total: u32 = stages.iter().map(|s| s.total).sum();
    if total == 0 {
        return 0.0;
    }
    let finished: u32 = stages.iter().map(|s| s.finished.min(s.total)).sum();
    finished as f32 / total as f32
}

/// How the arrow after a stage is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorState {
    /// The stage before it is still waiting, or failed.
    Idle,
    /// The stage before it is running.
    Active,
    /// The stage before it finished (done or with warnings).
    Passed,
}

impl ConnectorState {
    /// The arrow that follows a stage in `state`.
    pub fn after(state: StageState) -> Self {
        match state {
            StageState::Done | StageState::Warning => Self::Passed,
            StageState::Running => Self::Active,
            StageState::Pending | StageState::Failed => Self::Idle,
        }
    }
}

/// One stage card. `name` is shown as given (the caller upper-cases it for
/// display); `detail` carries the counts line.
#[derive(Debug, Clone)]
pub struct FlowStage<'a> {
    pub name: Label<'a>,
    pub detail: Label<'a>,
    pub state: StageState,
}

impl<'a> FlowStage<'a> {
    /// A stage whose state is derived from `counts`.
    pub fn from_counts(name: Label<'a>, detail: Label<'a>, counts: StageCounts) -> Self {
        Self {
            name,
            detail,
            state: counts.state(),
        }
    }
}

/// Colours and the text size the metrics derive from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlowProgressStyle {
    /// Body text size; every length is an em of it.
    pub text_size: f32,
    pub card: Color,
    pub border: Color,
    pub line_strong: Color,
    pub ink: Color,
    pub ink3: Color,
    pub ink4: Color,
    pub accent: Color,
    pub accent_text: Color,
    pub accent_soft: Color,
    pub ok: Color,
    pub warn: Color,
    pub warn_soft: Color,
    pub err: Color,
    pub err_soft: Color,
    /// Ring around the selected (opened) stage.
    pub selected: Color,
}

impl FlowProgressStyle {
    /// Colours from the theme; an extended token a theme leaves unset falls
    /// back to the nearest base token.
    pub fn from_tokens(tokens: &ThemeTokens, text_size: f32) -> Self {
        let ext = &tokens.extended;
        let or = |c: Option<signex_types::theme::Color>, base| to_color(&c.unwrap_or(base));
        Self {
            text_size,
            card: to_color(&tokens.panel_bg),
            border: to_color(&tokens.border),
            line_strong: or(ext.line_strong, tokens.border),
            ink: to_color(&tokens.text),
            ink3: or(ext.ink3, tokens.text_secondary),
            ink4: or(ext.ink4, tokens.text_secondary),
            accent: to_color(&tokens.accent),
            accent_text: or(ext.accent_text, tokens.accent),
            accent_soft: or(ext.accent_soft, tokens.selection),
            ok: to_color(&tokens.success),
            warn: to_color(&tokens.warning),
            warn_soft: or(ext.warn_soft, tokens.hover),
            err: to_color(&tokens.error),
            err_soft: or(ext.err_soft, tokens.hover),
            selected: or(ext.info, tokens.accent),
        }
    }

    fn em(&self, em: f32) -> f32 {
        self.text_size * em
    }

    /// Card fill, card edge, marker ring, marker fill and glyph ink.
    fn stage_colors(&self, state: StageState) -> StageColors {
        match state {
            StageState::Pending => StageColors {
                fill: None,
                edge: self.line_strong,
                ring: self.line_strong,
                disc: self.card,
                glyph: self.ink4,
                name: self.ink3,
                detail: self.ink4,
            },
            StageState::Running => StageColors {
                fill: Some(self.accent_soft),
                edge: self.accent,
                ring: self.accent,
                disc: self.card,
                glyph: self.accent_text,
                name: self.ink,
                detail: self.accent_text,
            },
            StageState::Done => StageColors {
                fill: Some(self.card),
                edge: self.border,
                ring: self.ok,
                disc: self.ok,
                glyph: self.card,
                name: self.ink3,
                detail: self.ink4,
            },
            StageState::Warning => StageColors {
                fill: Some(self.warn_soft),
                edge: mix(self.warn, self.border, 0.45),
                ring: self.warn,
                disc: self.warn,
                glyph: self.card,
                name: self.ink3,
                detail: self.warn,
            },
            StageState::Failed => StageColors {
                fill: Some(self.err_soft),
                edge: self.err,
                ring: self.err,
                disc: self.err,
                glyph: self.card,
                name: self.ink3,
                detail: self.err,
            },
        }
    }

    fn connector_color(&self, state: ConnectorState) -> Color {
        match state {
            ConnectorState::Idle => self.line_strong,
            ConnectorState::Active => self.accent,
            ConnectorState::Passed => self.ok,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct StageColors {
    fill: Option<Color>,
    edge: Color,
    ring: Color,
    disc: Color,
    glyph: Color,
    name: Color,
    detail: Color,
}

/// `a` weighted `t`, `b` weighted `1 - t`.
fn mix(a: Color, b: Color, t: f32) -> Color {
    let l = |x: f32, y: f32| x * t + y * (1.0 - t);
    Color::from_rgba(l(a.r, b.r), l(a.g, b.g), l(a.b, b.b), l(a.a, b.a))
}

const GLYPH_DONE: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M4 8.4l2.6 2.6L12 5.4" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>"#;
const GLYPH_WARN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M8 3.8v5.2" stroke="currentColor" stroke-width="1.9" stroke-linecap="round"/><circle cx="8" cy="11.9" r="1.1" fill="currentColor"/></svg>"#;
const GLYPH_FAIL: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><path d="M5.2 5.2l5.6 5.6M10.8 5.2l-5.6 5.6" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/></svg>"#;
const GLYPH_RUN: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="4.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-dasharray="18 40" stroke-linecap="round"/></svg>"#;
const GLYPH_PENDING: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="1.6" fill="currentColor"/></svg>"#;
const ARROW_HEAD: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 6 8"><path d="M0 0L6 4L0 8Z" fill="currentColor"/></svg>"#;

fn cached(cell: &'static OnceLock<svg::Handle>, bytes: &'static [u8]) -> svg::Handle {
    cell.get_or_init(|| svg::Handle::from_memory(bytes)).clone()
}

fn glyph_handle(state: StageState) -> svg::Handle {
    static DONE: OnceLock<svg::Handle> = OnceLock::new();
    static WARN: OnceLock<svg::Handle> = OnceLock::new();
    static FAIL: OnceLock<svg::Handle> = OnceLock::new();
    static RUN: OnceLock<svg::Handle> = OnceLock::new();
    static PENDING: OnceLock<svg::Handle> = OnceLock::new();
    match state {
        StageState::Done => cached(&DONE, GLYPH_DONE),
        StageState::Warning => cached(&WARN, GLYPH_WARN),
        StageState::Failed => cached(&FAIL, GLYPH_FAIL),
        StageState::Running => cached(&RUN, GLYPH_RUN),
        StageState::Pending => cached(&PENDING, GLYPH_PENDING),
    }
}

fn arrow_handle() -> svg::Handle {
    static ARROW: OnceLock<svg::Handle> = OnceLock::new();
    cached(&ARROW, ARROW_HEAD)
}

fn tinted<'a>(handle: svg::Handle, w: f32, h: f32, ink: Color) -> svg::Svg<'a> {
    svg(handle)
        .width(w)
        .height(h)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(ink),
            ..Default::default()
        })
}

/// The stage pipeline. Build with [`FlowProgress::new`], then
/// [`selected`](FlowProgress::selected) and
/// [`on_select`](FlowProgress::on_select); without a handler the cards are
/// not clickable.
pub struct FlowProgress<'a, M> {
    stages: Vec<FlowStage<'a>>,
    style: FlowProgressStyle,
    selected: Option<usize>,
    on_select: Option<Box<dyn Fn(usize) -> M + 'a>>,
}

impl<'a, M: Clone + 'a> FlowProgress<'a, M> {
    pub fn new(stages: Vec<FlowStage<'a>>, style: FlowProgressStyle) -> Self {
        Self {
            stages,
            style,
            selected: None,
            on_select: None,
        }
    }

    /// The stage drawn as opened.
    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self
    }

    /// Map a click on stage `index` to the caller's message.
    pub fn on_select(mut self, on_select: impl Fn(usize) -> M + 'a) -> Self {
        self.on_select = Some(Box::new(on_select));
        self
    }

    fn marker(&self, state: StageState) -> Element<'a, M> {
        let s = self.style;
        let c = s.stage_colors(state);
        let side = s.em(MARKER_EM);
        let glyph = s.em(GLYPH_EM);
        container(tinted(glyph_handle(state), glyph, glyph, c.glyph))
            .center(side)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(c.disc)),
                border: Border {
                    color: c.ring,
                    width: s.em(RING_EM),
                    radius: (side / 2.0).into(),
                    ..Border::default()
                },
                ..container::Style::default()
            })
            .boxed()
    }

    fn connector(&self, after: StageState) -> Element<'a, M> {
        let s = self.style;
        let ink = s.connector_color(ConnectorState::after(after));
        let weight = s.em(CONNECTOR_LINE_EM);
        let head_h = weight * 4.0;
        let line = container(Space::new().width(Length::Fill).height(weight))
            .width(Length::Fill)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(ink)),
                border: Border {
                    radius: (weight / 2.0).into(),
                    ..Border::default()
                },
                ..container::Style::default()
            });
        let arrow = row![line, tinted(arrow_handle(), head_h * 0.75, head_h, ink)]
            .align_y(Alignment::Center)
            .padding(Padding::from([0.0, weight]));
        // The card's top padding, then a marker-high band centres the arrow
        // on the markers.
        let top = s.em(CARD_PAD_EM.0);
        container(arrow)
            .width(s.em(CONNECTOR_EM))
            .height(top + s.em(MARKER_EM))
            .align_y(Vertical::Center)
            .padding(Padding {
                top,
                ..Padding::ZERO
            })
            .boxed()
    }

    fn card(&self, index: usize, stage: &FlowStage<'a>) -> Element<'a, M> {
        let s = self.style;
        let c = s.stage_colors(stage.state);
        let is_selected = self.selected == Some(index);
        let name = text(stage.name.clone())
            .size(s.text_size * NAME_RATIO)
            .color(c.name)
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End)
            .width(Length::Fill);
        let head = row![self.marker(stage.state), name]
            .spacing(s.em(GAP_EM))
            .align_y(Alignment::Center);
        let detail = text(stage.detail.clone())
            .size(s.text_size * DETAIL_RATIO)
            .color(c.detail)
            .wrapping(Wrapping::None)
            .ellipsis(Ellipsis::End)
            .width(Length::Fill);
        let body: Column<Element<'a, M>> = column![head, detail].spacing(s.em(GAP_EM) / 2.0);
        let message = self.on_select.as_ref().map(|f| f(index));
        let clickable = message.is_some();
        button(body)
            .width(Length::FillPortion(1))
            .padding(Padding::from([s.em(CARD_PAD_EM.0), s.em(CARD_PAD_EM.1)]))
            .on_press_maybe(message)
            .style(move |_: &Theme, status: button::Status| {
                let hovered = clickable
                    && matches!(status, button::Status::Hovered | button::Status::Pressed);
                let (edge, width) = if is_selected {
                    (s.selected, 2.0 * s.em(RING_EM))
                } else if hovered {
                    (s.line_strong, s.em(RING_EM))
                } else {
                    (c.edge, s.em(RING_EM) * 2.0 / 3.0)
                };
                button::Style {
                    background: c.fill.map(Background::Color),
                    text_color: c.name,
                    border: Border {
                        color: edge,
                        width,
                        radius: s.em(CARD_RADIUS_EM).into(),
                        ..Border::default()
                    },
                    ..button::Style::default()
                }
            })
            .boxed()
    }

    /// The pipeline as one row: card, arrow, card, …
    pub fn view(self) -> Element<'a, M> {
        let mut pipeline: Row<Element<'a, M>> = Row::new().align_y(Alignment::Start);
        for (i, stage) in self.stages.iter().enumerate() {
            if i > 0 {
                pipeline = pipeline.push(self.connector(self.stages[i - 1].state));
            }
            pipeline = pipeline.push(self.card(i, stage));
        }
        pipeline.width(Length::Fill).boxed()
    }
}

impl<'a, M: Clone + 'a> From<FlowProgress<'a, M>> for Element<'a, M> {
    fn from(flow: FlowProgress<'a, M>) -> Self {
        flow.view()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::theme::{ThemeId, theme_tokens};

    fn counts(total: u32, finished: u32, running: u32, warnings: u32, errors: u32) -> StageCounts {
        StageCounts {
            total,
            finished,
            running,
            warnings,
            errors,
        }
    }

    #[test]
    fn nothing_started_is_pending() {
        assert_eq!(counts(3, 0, 0, 0, 0).state(), StageState::Pending);
    }

    #[test]
    fn a_running_step_or_partial_finish_is_running() {
        assert_eq!(counts(3, 0, 1, 0, 0).state(), StageState::Running);
        assert_eq!(counts(3, 2, 0, 1, 0).state(), StageState::Running);
    }

    #[test]
    fn all_finished_is_done_or_warning() {
        assert_eq!(counts(3, 3, 0, 0, 0).state(), StageState::Done);
        assert_eq!(counts(3, 3, 0, 2, 0).state(), StageState::Warning);
    }

    #[test]
    fn any_error_fails_the_stage_even_mid_run() {
        assert_eq!(counts(3, 1, 1, 0, 1).state(), StageState::Failed);
        assert_eq!(counts(3, 3, 0, 4, 1).state(), StageState::Failed);
    }

    #[test]
    fn an_empty_stage_reads_as_done() {
        assert_eq!(counts(0, 0, 0, 0, 0).state(), StageState::Done);
    }

    #[test]
    fn run_fraction_counts_finished_steps_and_clamps_overcounts() {
        let run = [
            counts(3, 3, 0, 0, 0),
            counts(4, 1, 1, 0, 0),
            counts(1, 5, 0, 0, 0),
        ];
        assert_eq!(run_fraction(&run), 5.0 / 8.0);
        assert_eq!(run_fraction(&[]), 0.0);
        assert_eq!(run_fraction(&[counts(0, 0, 0, 0, 0)]), 0.0);
    }

    #[test]
    fn the_arrow_follows_the_stage_before_it() {
        assert_eq!(
            ConnectorState::after(StageState::Done),
            ConnectorState::Passed
        );
        assert_eq!(
            ConnectorState::after(StageState::Warning),
            ConnectorState::Passed
        );
        assert_eq!(
            ConnectorState::after(StageState::Running),
            ConnectorState::Active
        );
        assert_eq!(
            ConnectorState::after(StageState::Pending),
            ConnectorState::Idle
        );
        assert_eq!(
            ConnectorState::after(StageState::Failed),
            ConnectorState::Idle
        );
    }

    #[test]
    fn metrics_scale_with_the_text_size() {
        let tokens = theme_tokens(ThemeId::DEFAULT);
        let small = FlowProgressStyle::from_tokens(&tokens, 11.0);
        let large = FlowProgressStyle::from_tokens(&tokens, 13.0);
        assert!(large.em(MARKER_EM) > small.em(MARKER_EM));
        assert!((FlowProgressStyle::from_tokens(&tokens, 12.0).em(MARKER_EM) - 20.0).abs() < 1e-4);
    }

    #[test]
    fn every_state_has_distinct_colours_on_both_themes() {
        for id in ThemeId::BUILTINS {
            let s = FlowProgressStyle::from_tokens(&theme_tokens(*id), 12.0);
            let rings = [
                StageState::Pending,
                StageState::Running,
                StageState::Done,
                StageState::Warning,
                StageState::Failed,
            ]
            .map(|st| s.stage_colors(st).ring);
            for (i, a) in rings.iter().enumerate() {
                for b in &rings[i + 1..] {
                    assert_ne!(a, b, "{id:?}: two states share a marker ring");
                }
            }
        }
    }

    #[test]
    fn mix_weights_the_first_colour() {
        let m = mix(Color::WHITE, Color::BLACK, 0.25);
        assert!((m.r - 0.25).abs() < 1e-6);
        assert_eq!(m.a, 1.0);
    }

    #[derive(Debug, Clone, PartialEq)]
    enum Msg {
        StageOpened(usize),
    }

    #[test]
    fn builds_with_and_without_a_click_handler() {
        let style = FlowProgressStyle::from_tokens(&theme_tokens(ThemeId::DEFAULT), 12.0);
        let stages = || {
            [
                ("VALIDATE", "3 of 3", StageState::Done),
                ("GENERATE", "4 of 11", StageState::Running),
            ]
            .into_iter()
            .map(|(name, detail, state)| FlowStage {
                name: crate::raw(name),
                detail: crate::raw(detail),
                state,
            })
            .collect::<Vec<_>>()
        };
        let _: Element<'_, Msg> = FlowProgress::new(stages(), style).view();
        let _: Element<'_, Msg> = FlowProgress::new(stages(), style)
            .selected(Some(1))
            .on_select(Msg::StageOpened)
            .into();
    }
}
