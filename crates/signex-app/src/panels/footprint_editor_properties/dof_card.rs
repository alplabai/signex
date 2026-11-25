//! The DOF card at the top of the Sketch solve section (mockup row 179): the
//! sketch's free degrees of freedom as a number on a toned card, its state,
//! the constraint count and the solve time, then one row per free motion
//! with Show, which selects what moves so the canvas brings it into view.

use iced::widget::{Column, column, container, row};
use iced::{Alignment, Background, Border, Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, ids, t};
use signex_sketch::solver::free_motion::MotionKind;
use signex_widgets::text;

use super::body::{Ctx, ghost};
use crate::panels::{FootprintSolveSummary, FreeMotionSummary, PanelMsg, SketchDof};
use crate::styles::{PillTone, Radii, Spacing, TypeScale, pill_colors};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// The number's size, in title sizes.
const NUMBER_SCALE: f32 = 1.4;
/// Free motions listed before the "+N more" line.
const MAX_MOTIONS: usize = 8;

/// What the card says: the number, its tone and its two lines.
#[derive(Debug, Clone)]
pub(super) struct DofCard {
    pub number: Localized,
    pub tone: PillTone,
    pub state: Localized,
    pub detail: Localized,
}

/// The card for the DOF state `dof` of `solve`, a sketch with `constraints`
/// constraints. An over-constrained sketch (a solve conflict or a rejected
/// edit) shows minus its conflict count and keeps its last good solve.
pub(super) fn dof_card(
    dof: SketchDof,
    solve: &FootprintSolveSummary,
    constraints: usize,
    i18n: &I18n,
) -> DofCard {
    let free = match dof {
        SketchDof::Over { conflicts } => {
            return DofCard {
                number: t!(i18n, ids::SKETCH_DOF_NUMBER_OVER, "count" => conflicts),
                tone: PillTone::Err,
                state: t!(i18n, ids::SKETCH_DOF_OVER, "count" => conflicts),
                detail: t!(i18n, ids::SKETCH_DOF_KEPT, "count" => constraints),
            };
        }
        SketchDof::Under { dof } => dof,
        SketchDof::Full => 0,
    };
    let detail = t!(
        i18n, ids::SKETCH_DOF_SOLVED,
        "count" => constraints, "ms" => solve.elapsed_ms
    );
    let (tone, state) = if free > 0 {
        (
            PillTone::Info,
            t!(i18n, ids::SKETCH_DOF_UNDER, "dof" => free),
        )
    } else {
        (PillTone::Ok, t!(i18n, ids::SKETCH_DOF_FULL))
    };
    DofCard {
        number: t!(i18n, ids::SKETCH_DOF_NUMBER, "dof" => free),
        tone,
        state,
        detail,
    }
}

/// What one free motion does, in words.
pub(super) fn motion_label(motion: &FreeMotionSummary, i18n: &I18n) -> Localized {
    let count = motion.entity_count;
    match motion.kind {
        MotionKind::Slide { dx, dy } => match slide_axis(dx, dy) {
            SlideAxis::X => t!(i18n, ids::SKETCH_DOF_SLIDE_X, "count" => count),
            SlideAxis::Y => t!(i18n, ids::SKETCH_DOF_SLIDE_Y, "count" => count),
            SlideAxis::Angle(angle) => t!(
                i18n, ids::SKETCH_DOF_SLIDE_ANGLE,
                "count" => count, "angle" => format!("{angle:.1}")
            ),
        },
        MotionKind::AnyDirection => t!(i18n, ids::SKETCH_DOF_ANY, "count" => count),
        MotionKind::Reshape => t!(i18n, ids::SKETCH_DOF_RESHAPE, "count" => count),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SlideAxis {
    X,
    Y,
    /// The direction in degrees from +X, in `[0, 180)`.
    Angle(f64),
}

/// A component this small (of a unit direction) is no component.
const AXIS_EPS: f64 = 1e-6;

fn slide_axis(dx: f64, dy: f64) -> SlideAxis {
    if dy.abs() < AXIS_EPS {
        return SlideAxis::X;
    }
    if dx.abs() < AXIS_EPS {
        return SlideAxis::Y;
    }
    SlideAxis::Angle(dy.atan2(dx).to_degrees().rem_euclid(180.0))
}

/// The card and the free-motion rows, under the section header.
pub(super) fn push_dof_card<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
    solve: &FootprintSolveSummary,
) -> Column<Element<'a, PanelMsg>> {
    let dof = card_dof(cx, solve);
    let card = dof_card(dof, solve, cx.fp.sketch_constraint_count, cx.i18n);
    let (fill, ink) = pill_colors(cx.tokens, card.tone);
    let number = text(card.number)
        .size(TS.title * NUMBER_SCALE)
        .font(crate::fonts::iced_font_for_family(
            crate::fonts::CHROME_MONO_FONT,
        ))
        .color(ink);
    let lines = column![
        text(card.state).size(TS.body).color(ink),
        text(card.detail).size(TS.caption).color(ink),
    ]
    .spacing(SP.xs);
    let face = row![number, lines].spacing(SP.m).align_y(Alignment::Center);
    let tile = container(face)
        .padding([SP.s, SP.m])
        .width(Length::Fill)
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                radius: Radii::GRAPHITE.card.into(),
                ..Border::default()
            },
            ..Default::default()
        });
    col = col.push(
        container(tile)
            .padding([SP.xs, SP.m])
            .width(Length::Fill)
            .boxed(),
    );
    for motion in solve.free_motions.iter().take(MAX_MOTIONS) {
        col = col.push(motion_row(motion, cx, ink));
    }
    if solve.free_motions.len() > MAX_MOTIONS {
        let more = solve.free_motions.len() - MAX_MOTIONS;
        col = col.push(super::sections::more_rows(cx, more));
    }
    col
}

/// The DOF state the chip shows, else one read from `solve` itself.
pub(super) fn card_dof(cx: Ctx<'_, '_>, solve: &FootprintSolveSummary) -> SketchDof {
    cx.fp.sketch_state.dof.unwrap_or_else(|| {
        SketchDof::of(
            solve.over_constraint_count,
            !cx.fp.rejected_conflicts.is_empty(),
            solve.dof,
        )
    })
}

/// "One point slides along X" and Show.
fn motion_row<'a>(
    motion: &FreeMotionSummary,
    cx: Ctx<'a, '_>,
    ink: iced::Color,
) -> Element<'a, PanelMsg> {
    let show = PanelMsg::FpEditorSelectSketchEntity {
        id: motion.focus_entity_id,
    };
    let line = row![
        text(motion_label(motion, cx.i18n))
            .size(TS.caption)
            .color(ink)
            .width(Length::Fill),
        ghost(t!(cx.i18n, ids::SKETCH_DOF_SHOW), Some(show), cx.tokens),
    ]
    .spacing(SP.s)
    .align_y(Alignment::Center);
    container(line)
        .padding([SP.xs, SP.m])
        .width(Length::Fill)
        .boxed()
}

#[cfg(test)]
#[path = "dof_card_tests.rs"]
mod tests;
