//! The sketch sections: Sketch solve (a DOF pill in the header; the DOF card
//! with the free motions, entity and constraint counts, the last solve's
//! iterations, residual and time, the redundant and conflicting
//! constraints; a board drops the constraint count and time rows the card
//! shows), Parameters and Solve warnings.
//! The nothing-selected body shows them, and so does Sketch mode under a
//! selection. A board sketch leads with the solve section, titled Sketch DOF,
//! right under its object header (mockup row 179).

use iced::widget::{Column, container, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{Localized, UserText, ids, t};
use signex_widgets::{raw, text, text_input};

use super::super::{FootprintModeKind, PanelMsg};
use super::body::{Ctx, caption, is_open, kv_row, pill, section};
use super::conflicts;
use crate::panels::form_look;
use crate::styles::{PillTone, Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

const SOLVE_KEY: &str = "fp_dof";
const PARAMETERS_KEY: &str = "fp_parameters";
const WARNINGS_KEY: &str = "fp_solve_warnings";
/// Warnings listed before the "+N more" line.
const MAX_WARNINGS: usize = 8;
/// Width of a parameter name, in captions.
const PARAMETER_NAME_EM: f32 = 10.0;

pub(super) fn view_sketch_sections<'a>(
    col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    let col = solve_section(col, cx);
    view_sketch_sections_after_solve(col, cx)
}

/// A board sketch in Sketch mode puts the Sketch DOF section right under
/// its object header (mockup row 179, m179-slot); a footprint keeps it with
/// the other sketch sections.
pub(super) fn leads_with_solve(cx: Ctx<'_, '_>) -> bool {
    cx.fp.board.is_some() && cx.fp.mode_kind == FootprintModeKind::Sketch
}

/// The Sketch solve section alone, for a body that leads with it.
pub(super) fn view_sketch_solve_section<'a>(
    col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    solve_section(col, cx)
}

/// The sketch sections after the solve: Parameters and Solve warnings.
pub(super) fn view_sketch_sections_after_solve<'a>(
    col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    let col = parameters_section(col, cx);
    warnings_section(col, cx)
}

/// The solve section's title: "Sketch solve" on a footprint; on a board the
/// mockup's "Sketch DOF", "Sketch: 2 DOF" or "Sketch DOF: 1 conflict"
/// (mockup row 179).
fn solve_title(cx: Ctx<'_, '_>) -> Localized {
    let i18n = cx.i18n;
    if cx.fp.board.is_none() {
        return t!(i18n, ids::EDITORS_FP_PROPS_SECTION_SOLVE);
    }
    match cx.fp.last_solve.as_ref() {
        Some(solve) if solve.over_constraint_count > 0 => t!(
            i18n, ids::SKETCH_DOF_SECTION_OVER, "count" => solve.over_constraint_count
        ),
        Some(solve) if solve.dof > 0 => {
            t!(i18n, ids::SKETCH_DOF_SECTION_UNDER, "dof" => solve.dof)
        }
        _ => t!(i18n, ids::SKETCH_DOF_SECTION),
    }
}

/// The "+N more" caption under a cut list.
pub(super) fn more_rows<'a>(cx: Ctx<'a, '_>, count: usize) -> Element<'a, PanelMsg> {
    caption(
        t!(cx.i18n, ids::EDITORS_FP_PROPS_MORE, "count" => count),
        cx.tokens,
    )
}

/// The sketch's solve state as a pill: the Sketch solve header and the
/// board's Bake panel show it.
pub(super) fn solve_pill<'a>(cx: Ctx<'a, '_>) -> Element<'a, PanelMsg> {
    let state = cx.fp.sketch_state;
    let dof = if state.tone == PillTone::Ok {
        t!(cx.i18n, ids::EDITORS_FP_PROPS_DOF_ZERO)
    } else {
        t!(cx.i18n, state.label)
    };
    pill(dof, state.tone, cx.tokens)
}

fn solve_section<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    let Ctx {
        fp, tokens, i18n, ..
    } = cx;
    let title = solve_title(cx);
    let header = section(
        SOLVE_KEY,
        &title,
        Some(solve_pill(cx)),
        cx.collapsed,
        tokens,
        i18n,
    );
    col = col.push(header);
    if !is_open(SOLVE_KEY, cx.collapsed) {
        return col;
    }
    // The DOF card and the free motions lead (mockup row 179).
    if let Some(solve) = fp.last_solve.as_ref() {
        col = super::dof_card::push_dof_card(col, cx, solve);
    }
    // On a board the card already shows the constraint count and the solve
    // time, so their rows go (mockup row 179).
    let card_counts = fp.board.is_some() && fp.last_solve.is_some();
    let kv = |id: signex_i18n::MessageId, value: String| kv_row(&t!(i18n, id), value, tokens, i18n);
    col = col.push(kv(
        ids::EDITORS_FP_PROPS_ENTITIES,
        i18n.count(fp.sketch_entity_count as i64),
    ));
    if !card_counts {
        col = col.push(kv(
            ids::EDITORS_FP_PROPS_CONSTRAINTS,
            i18n.count(fp.sketch_constraint_count as i64),
        ));
    }
    let Some(solve) = fp.last_solve.as_ref() else {
        col = col.push(caption(t!(i18n, ids::EDITORS_FP_PROPS_NOT_SOLVED), tokens));
        return conflict_rows(col, cx);
    };
    col = col.push(kv(
        ids::EDITORS_FP_PROPS_ITERATIONS,
        i18n.count(solve.iterations as i64),
    ));
    col = col.push(kv(
        ids::EDITORS_FP_PROPS_RESIDUAL,
        format!("{:.3e}", solve.final_residual_norm),
    ));
    if !card_counts {
        col = col.push(kv(
            ids::EDITORS_FP_PROPS_ELAPSED,
            t!(i18n, ids::EDITORS_FP_PROPS_ELAPSED_VALUE, "ms" => solve.elapsed_ms).into_string(),
        ));
    }
    col = col.push(kv(
        ids::EDITORS_FP_PROPS_OVER_CONSTRAINED,
        i18n.count(solve.over_constraint_count as i64),
    ));
    col = super::redundant::push_redundant_rows(col, solve, cx);
    conflict_rows(col, cx)
}

/// Conflicting constraints, worst first: the last solve's, then the last
/// rejected edit's. A click selects the row's entity.
fn conflict_rows<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    let fp = cx.fp;
    let rows = conflicts::conflict_rows(
        fp.last_solve
            .as_ref()
            .map_or(&[], |s| s.over_constraints.as_slice()),
        &fp.rejected_conflicts,
    );
    if rows.is_empty() {
        return col;
    }
    col = col.push(caption(
        t!(cx.i18n, ids::EDITORS_FP_PROPS_CONFLICTS),
        cx.tokens,
    ));
    for row in rows.iter().take(conflicts::MAX_ROWS) {
        col = col.push(conflicts::conflict_row(row, cx.i18n));
    }
    if rows.len() > conflicts::MAX_ROWS {
        col = col.push(more_rows(cx, rows.len() - conflicts::MAX_ROWS));
    }
    col
}

fn parameters_section<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    let Ctx {
        fp, tokens, i18n, ..
    } = cx;
    let count = pill(
        t!(i18n, ids::EDITORS_FP_PROPS_COUNT, "count" => fp.sketch_parameters.len()),
        PillTone::Neutral,
        tokens,
    );
    let title = t!(i18n, ids::EDITORS_FP_PROPS_SECTION_PARAMETERS);
    col = col.push(section(
        PARAMETERS_KEY,
        &title,
        Some(count),
        cx.collapsed,
        tokens,
        i18n,
    ));
    if !is_open(PARAMETERS_KEY, cx.collapsed) {
        return col;
    }
    if fp.sketch_parameters.is_empty() {
        return col.push(caption(
            t!(i18n, ids::EDITORS_FP_PROPS_PARAMETERS_NONE),
            tokens,
        ));
    }
    for (name, expr) in &fp.sketch_parameters {
        let edited = name.clone();
        let line = row![
            text(UserText::new(name.as_str()))
                .size(TS.body)
                .font(crate::fonts::iced_font_for_family(
                    crate::fonts::CHROME_MONO_FONT
                ))
                .color(ti(tokens.text))
                .width(Length::Fixed(TS.caption * PARAMETER_NAME_EM)),
            text_input(
                t!(i18n, ids::EDITORS_FP_PROPS_PARAMETER_PLACEHOLDER),
                expr.clone()
            )
            .size(TS.body)
            .padding(SP.xs)
            .style(form_look::text_input_style)
            .on_input(move |expr| PanelMsg::FpEditorEditParameter {
                name: edited.clone(),
                expr,
            }),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center);
        col = col.push(
            container(line)
                .padding([SP.xs, SP.m])
                .width(Length::Fill)
                .boxed(),
        );
    }
    col
}

fn warnings_section<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    let Ctx {
        fp, tokens, i18n, ..
    } = cx;
    let count = (!fp.solve_warnings.is_empty()).then(|| {
        pill(
            t!(i18n, ids::EDITORS_FP_PROPS_COUNT, "count" => fp.solve_warnings.len()),
            PillTone::Warn,
            tokens,
        )
    });
    let title = t!(i18n, ids::EDITORS_FP_PROPS_SECTION_WARNINGS);
    col = col.push(section(
        WARNINGS_KEY,
        &title,
        count,
        cx.collapsed,
        tokens,
        i18n,
    ));
    if !is_open(WARNINGS_KEY, cx.collapsed) {
        return col;
    }
    if fp.solve_warnings.is_empty() {
        return col.push(caption(
            t!(i18n, ids::EDITORS_FP_PROPS_WARNINGS_NONE),
            tokens,
        ));
    }
    for warning in fp.solve_warnings.iter().take(MAX_WARNINGS) {
        // raw: the solver's own note, not yet in the catalog.
        col = col.push(caption(raw(warning.clone()), tokens));
    }
    if fp.solve_warnings.len() > MAX_WARNINGS {
        col = col.push(more_rows(cx, fp.solve_warnings.len() - MAX_WARNINGS));
    }
    col
}
