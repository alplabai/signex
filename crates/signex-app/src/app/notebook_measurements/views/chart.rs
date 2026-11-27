//! The timeline's chart: the readings over time, joined oldest to newest,
//! against the limit (dashed) and the computed value. A failing reading is
//! drawn in the theme's error colour, a stale one in its warning colour.

use iced::widget::canvas::{self, Frame, LineDash, Path, Stroke};
use iced::{Color, Point, Rectangle, Renderer, Theme, mouse};
use signex_notebook::measurement::{Limit, Timeline, Tolerance, Verdict};

use super::TS;
use crate::styles::HAIRLINE;

/// How a point is marked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mark {
    Normal,
    Fail,
    Stale,
}

/// The chart in fractions of its box: x from 0 (oldest) to 1 (newest), y
/// from 0 (top, the highest value) to 1 (bottom).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChartLayout {
    pub(crate) points: Vec<(f32, f32, Mark)>,
    pub(crate) limit: Option<f32>,
    pub(crate) computed: Option<f32>,
}

/// The value a limit draws its line at: the bound a reading must not pass
/// (the upper one of a range or a tolerance band).
pub(crate) fn limit_line(limit: Limit) -> f64 {
    match limit {
        Limit::Max { max } | Limit::Range { max, .. } => max,
        Limit::Min { min } => min,
        Limit::Nominal { nominal, tolerance } => {
            let band = match tolerance {
                Tolerance::Percent(percent) => nominal * percent / 100.0,
                Tolerance::Absolute(band) => band,
            };
            nominal + band.abs()
        }
    }
}

/// Lay out the dated readings of `timeline` that have a value. `None` when
/// there is none to draw.
pub(crate) fn layout(timeline: &Timeline) -> Option<ChartLayout> {
    let dated: Vec<_> = timeline
        .entries
        .iter()
        .rev()
        .filter_map(|e| Some((e.timestamp?.minutes_since_epoch(), e.value?, e)))
        .collect();
    let first = dated.first()?.0;
    let last = dated.last()?.0;
    let limit = timeline.limit.map(limit_line);
    let values = dated
        .iter()
        .map(|(_, v, _)| *v)
        .chain(limit)
        .chain(timeline.computed);
    let (mut low, mut high) = values.fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
        (lo.min(v), hi.max(v))
    });
    // A tenth of the span above and below, so no point sits on the edge.
    let pad = ((high - low) * 0.1).max(high.abs().max(1.0) * 1e-3);
    low -= pad;
    high += pad;
    let y = |v: f64| ((high - v) / (high - low)) as f32;
    let span = (last - first) as f64;
    let x = |at: i64| {
        if span > 0.0 {
            ((at - first) as f64 / span) as f32
        } else {
            0.5
        }
    };
    let points = dated
        .iter()
        .map(|(at, value, entry)| {
            let mark = match (entry.verdict, entry.is_stale()) {
                (Some(Verdict::Fail), _) => Mark::Fail,
                (_, true) => Mark::Stale,
                _ => Mark::Normal,
            };
            (x(*at), y(*value), mark)
        })
        .collect();
    Some(ChartLayout {
        points,
        limit: limit.map(y),
        computed: timeline.computed.map(y),
    })
}

/// The colours the chart draws with, from the theme's tokens.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ChartInk {
    pub(crate) line: Color,
    pub(crate) point: Color,
    pub(crate) fail: Color,
    pub(crate) stale: Color,
    pub(crate) limit: Color,
    pub(crate) computed: Color,
}

/// The chart as a canvas program.
pub(crate) struct TimelineChart {
    pub(crate) layout: ChartLayout,
    pub(crate) ink: ChartInk,
}

/// The limit line's dash, from the type scale.
const DASH: [f32; 2] = [TS.body / 3.0, TS.body / 4.0];
const POINT_RADIUS: f32 = TS.body / 4.0;
/// Room left at the sides so a point is not cut.
const INSET: f32 = TS.body / 2.0;
/// The trace is a little heavier than the limit and computed lines.
const TRACE_WIDTH: f32 = HAIRLINE * 1.5;

impl<M> canvas::Program<M> for TimelineChart {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let width = (bounds.width - 2.0 * INSET).max(1.0);
        let height = (bounds.height - 2.0 * INSET).max(1.0);
        let at = |x: f32, y: f32| Point::new(INSET + x * width, INSET + y * height);
        let across = |y: f32| Path::line(at(0.0, y), at(1.0, y));
        if let Some(y) = self.layout.limit {
            frame.stroke(
                &across(y),
                Stroke {
                    line_dash: LineDash {
                        segments: &DASH,
                        offset: 0,
                    },
                    ..Stroke::default()
                        .with_width(HAIRLINE)
                        .with_color(self.ink.limit)
                },
            );
        }
        if let Some(y) = self.layout.computed {
            let stroke = Stroke::default()
                .with_width(HAIRLINE)
                .with_color(self.ink.computed);
            frame.stroke(&across(y), stroke);
        }
        let trace = Path::new(|path| {
            for (i, (x, y, _)) in self.layout.points.iter().enumerate() {
                if i == 0 {
                    path.move_to(at(*x, *y));
                } else {
                    path.line_to(at(*x, *y));
                }
            }
        });
        let stroke = Stroke::default()
            .with_width(TRACE_WIDTH)
            .with_color(self.ink.line);
        frame.stroke(&trace, stroke);
        for (x, y, mark) in &self.layout.points {
            let colour = match mark {
                Mark::Normal => self.ink.point,
                Mark::Fail => self.ink.fail,
                Mark::Stale => self.ink.stale,
            };
            frame.fill(&Path::circle(at(*x, *y), POINT_RADIUS), colour);
        }
        vec![frame.into_geometry()]
    }
}
