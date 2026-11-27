//! The pattern form of a board sketch (mockup row 179e): the kind as its
//! title, Source, Count, Spacing X and Y with what an expression resolves
//! to, and what the bake writes ("Bakes into 4 holes"). A board item has no
//! pad number, so the footprint form's Numbering section is not shown.

use iced::widget::{Column, Row, container};
use iced::{Element, Length, Widget as _};
use signex_bake::array::ArrayPreview;
use signex_i18n::{I18n, Localized, ids, t};
use signex_types::sketch::{SketchHost, SketchRole};
use signex_types::theme::ThemeTokens;

use super::super::super::{
    ArrayKindSummary, ArrayParamField, ArraySummary, FootprintEditorPanelContext, PanelMsg,
};
use super::helpers::{SP, caption, danger_action, field_row, form, header, pair, value_box};
use super::pattern::{
    PatternKind, centre_row, count_sweep_row, is_plain_length, kind_of, mask_rows, param,
};

pub(super) fn view<'a>(
    fp: &'a FootprintEditorPanelContext,
    pattern: &'a ArraySummary,
    tokens: &'a ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let pill = pattern
        .preview
        .kept()
        .map(|n| t!(i18n, ids::EDITORS_ROLE_PATTERN_INSTANCES, "count" => n));
    let bakes = match bakes_into(fp.selected_sketch_role, &pattern.preview, i18n) {
        Some(line) => caption(line, tokens),
        None => Column::<Element<'a, PanelMsg>>::new().boxed(),
    };
    let mut parts = vec![header(title(pattern, i18n), pill, bakes, tokens)];
    if let Some(source) = source_label(fp, i18n) {
        parts.push(field_row(
            t!(i18n, ids::BOARD_PATTERN_SOURCE),
            value_box(source),
            tokens,
        ));
    }
    parts.extend(layout_rows(pattern, tokens, i18n));
    parts.extend(mask_rows(pattern, tokens, i18n));
    parts.push(danger_action(
        t!(i18n, ids::EDITORS_ROLE_PATTERN_DELETE),
        PanelMsg::FpEditorDeleteArray {
            array_id: pattern.array_id,
        },
        tokens,
    ));
    form(parts)
}

fn title(pattern: &ArraySummary, i18n: &I18n) -> Localized {
    let id = match kind_of(pattern) {
        PatternKind::Grid => ids::BOARD_PATTERN_TITLE_RECTANGULAR,
        PatternKind::Linear => ids::BOARD_PATTERN_TITLE_LINEAR,
        PatternKind::Polar => ids::BOARD_PATTERN_TITLE_CIRCULAR,
    };
    t!(i18n, id)
}

/// "Circle · Mounting hole": the source's kind and its bake target.
fn source_label(fp: &FootprintEditorPanelContext, i18n: &I18n) -> Option<Localized> {
    let entity = fp.selected_sketch_entity.as_ref()?;
    let kind = t!(i18n, ids::EDITORS_FP_PROPS_ENTITY_KIND, "kind" => entity.kind_label);
    let role = fp.selected_sketch_role;
    if role == SketchRole::Unassigned {
        return Some(kind);
    }
    let role = crate::library::editor::footprint::sketch_mode::roles::role_label(
        role,
        SketchHost::Board,
        i18n,
    );
    Some(t!(
        i18n,
        ids::BOARD_PATTERN_SOURCE_ROLE,
        "kind" => kind.into_string(),
        "role" => role
    ))
}

/// What a bake of the pattern writes, by the source's target. `None` when
/// the source has no role: a pattern of a profile point still repeats the
/// profile, so "nothing" could be wrong.
pub(super) fn bakes_into(
    role: SketchRole,
    preview: &ArrayPreview,
    i18n: &I18n,
) -> Option<Localized> {
    use SketchRole as R;
    let id = match role {
        R::MountingHole => ids::BOARD_PATTERN_BAKES_HOLES,
        R::Keepout => ids::BOARD_PATTERN_BAKES_KEEPOUTS,
        R::Cutout => ids::BOARD_PATTERN_BAKES_CUTOUTS,
        R::Slot => ids::BOARD_PATTERN_BAKES_SLOTS,
        R::BoardPour => ids::BOARD_PATTERN_BAKES_POURS,
        R::VScore => ids::BOARD_PATTERN_BAKES_V_SCORES,
        R::MechanicalLayer => ids::BOARD_PATTERN_BAKES_DRAWINGS,
        R::BoardOutline | R::ComponentAnchor => {
            return Some(t!(i18n, ids::BOARD_PATTERN_BAKES_REFUSED));
        }
        _ => return None,
    };
    Some(match preview.kept() {
        Some(count) => t!(i18n, id, "count" => count),
        None => t!(i18n, ids::BOARD_PATTERN_BAKES_UNRESOLVED),
    })
}

fn layout_rows<'a>(
    pattern: &'a ArraySummary,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Vec<Element<'a, PanelMsg>> {
    use ArrayParamField as F;
    let (step_x, step_y) = resolved_steps(pattern);
    match &pattern.kind {
        ArrayKindSummary::Linear {
            count_expr,
            dx_expr,
            dy_expr,
            ..
        } => vec![
            field_row(
                t!(i18n, ids::BOARD_PATTERN_COUNT),
                param(pattern, F::LinearCountExpr, "1", count_expr),
                tokens,
            ),
            spacing_row(
                pattern,
                Axis::X,
                F::LinearDxExpr,
                dx_expr,
                step_x,
                tokens,
                i18n,
            ),
            spacing_row(
                pattern,
                Axis::Y,
                F::LinearDyExpr,
                dy_expr,
                step_y,
                tokens,
                i18n,
            ),
        ],
        ArrayKindSummary::Grid {
            nx_expr,
            ny_expr,
            dx_expr,
            dy_expr,
            ..
        } => vec![
            field_row(
                t!(i18n, ids::BOARD_PATTERN_COUNT),
                pair(
                    param(pattern, F::GridNxExpr, "1", nx_expr),
                    param(pattern, F::GridNyExpr, "1", ny_expr),
                ),
                tokens,
            ),
            spacing_row(
                pattern,
                Axis::X,
                F::GridDxExpr,
                dx_expr,
                step_x,
                tokens,
                i18n,
            ),
            spacing_row(
                pattern,
                Axis::Y,
                F::GridDyExpr,
                dy_expr,
                step_y,
                tokens,
                i18n,
            ),
        ],
        ArrayKindSummary::Polar {
            count_expr,
            sweep_angle_expr,
            ..
        } => vec![
            count_sweep_row(pattern, count_expr, sweep_angle_expr, tokens, i18n),
            field_row(
                t!(i18n, ids::EDITORS_ROLE_PATTERN_CENTRE),
                centre_row(pattern, pattern.center_shown_mm, i18n),
                tokens,
            ),
        ],
    }
}

#[derive(Clone, Copy)]
enum Axis {
    X,
    Y,
}

/// A spacing field, with what it resolves to beside it when it names a
/// parameter (`board_w - 2*hole_inset`).
fn spacing_row<'a>(
    pattern: &ArraySummary,
    axis: Axis,
    field: ArrayParamField,
    expr: &str,
    resolved: Option<f64>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let label = match axis {
        Axis::X => ids::BOARD_PATTERN_SPACING_X,
        Axis::Y => ids::BOARD_PATTERN_SPACING_Y,
    };
    let input = param(pattern, field, "0", expr);
    let control = match resolved.filter(|_| !is_plain_length(expr)) {
        Some(value) => {
            let value = t!(
                i18n,
                ids::BOARD_PATTERN_RESOLVED,
                "value" => format!("{value:.2}")
            );
            Row::<Element<'a, PanelMsg>>::new()
                .spacing(SP.s)
                .align_y(iced::Alignment::Center)
                .push(container(input).width(Length::Fill).boxed())
                .push(caption(value, tokens))
                .boxed()
        }
        None => input,
    };
    field_row(t!(i18n, label), control, tokens)
}

/// The step between neighbouring instances along X and along Y in mm, from
/// the walk the bake runs: a row's `(dx, dy)`, a grid's column and row step.
fn resolved_steps(pattern: &ArraySummary) -> (Option<f64>, Option<f64>) {
    let ArrayPreview::Instances(all) = &pattern.preview else {
        return (None, None);
    };
    let at = |i: usize, j: usize| {
        all.iter()
            .find(|c| c.i == i && c.j == j)
            .map(|c| c.offset_mm)
    };
    match kind_of(pattern) {
        PatternKind::Linear => {
            let step = at(1, 0);
            (step.map(|[dx, _]| dx), step.map(|[_, dy]| dy))
        }
        PatternKind::Grid => (at(1, 0).map(|[dx, _]| dx), at(0, 1).map(|[_, dy]| dy)),
        PatternKind::Polar => (None, None),
    }
}
