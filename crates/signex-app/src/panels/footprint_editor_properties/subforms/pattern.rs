//! Pattern (array) role sub-form (mockup row 126): header with the instance count,
//! the numbered illustration, the kind, the Layout and Numbering sections
//! and Delete pattern. Every field edits the array through the shipped
//! `FpEditor*Array*` messages.

use iced::widget::{Column, Row, container, pick_list};
use iced::{Element, Length, Widget as _};
use signex_i18n::{I18n, Localized, UserText, ids, t};
use signex_types::theme::ThemeTokens;
use signex_widgets::dropdown_style::GraphiteDropdown as _;
use signex_widgets::{button, checkbox, text};

use super::super::super::{
    ArrayKindSummary, ArrayParamField, ArraySummary, CollapsedSections,
    FootprintEditorPanelContext, NumberingSchemeKindUi, PanelMsg, form_look,
};
use super::helpers::{
    SP, Segment, TS, caption, check, danger_action, example_input, field_row, form, gated,
    ghost_action, header, illustration, is_open, note, pair, section_header, segmented, value_box,
};
use super::pattern_drawing::drawing;
use signex_bake::array::{ArrayPreview, NumberMaskRefusal, instances_numbered, suppressed_numbers};
use signex_sketch::array::GridSweep;

const LAYOUT_SECTION: &str = "fp_role_pattern_layout";
const NUMBERING_SECTION: &str = "fp_role_pattern_numbering";

// Most per-instance checkboxes drawn (the shipped caps).
const MAX_GRID_CHECKBOX_DIM: u32 = 32;
const MAX_POLAR_CHECKBOX_COUNT: u32 = 64;

/// The kind of a pattern, for the kind segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PatternKind {
    Linear,
    Grid,
    Polar,
}

pub(super) fn kind_of(pattern: &ArraySummary) -> PatternKind {
    match pattern.kind {
        ArrayKindSummary::Linear { .. } => PatternKind::Linear,
        ArrayKindSummary::Grid { .. } => PatternKind::Grid,
        ArrayKindSummary::Polar { .. } => PatternKind::Polar,
    }
}

/// The kind segment: the pattern's kind, the other kinds shown and
/// disabled (a pattern keeps the kind it was made with).
pub(super) fn kind_segments(pattern: &ArraySummary, i18n: &I18n) -> Vec<Segment> {
    let current = kind_of(pattern);
    [
        (PatternKind::Linear, ids::EDITORS_ROLE_PATTERN_KIND_LINEAR),
        (PatternKind::Grid, ids::EDITORS_ROLE_PATTERN_KIND_GRID),
        (PatternKind::Polar, ids::EDITORS_ROLE_PATTERN_KIND_POLAR),
    ]
    .into_iter()
    .map(|(kind, name)| Segment {
        label: t!(i18n, name),
        active: kind == current,
        msg: None,
    })
    .collect()
}

/// Start row, start column and the letter skip apply to BGA numbering only;
/// they grey for the other schemes.
pub(super) fn bga_rows_enabled(pattern: &ArraySummary) -> bool {
    pattern.bga_config.is_some()
}

fn scheme_label(i18n: &I18n, scheme: NumberingSchemeKindUi) -> String {
    let id = match scheme {
        NumberingSchemeKindUi::LinearIncrement => ids::EDITORS_ROLE_PATTERN_SCHEME_LINEAR,
        NumberingSchemeKindUi::BgaRowCol => ids::EDITORS_ROLE_PATTERN_SCHEME_BGA,
        NumberingSchemeKindUi::Explicit => ids::EDITORS_ROLE_PATTERN_SCHEME_EXPLICIT,
    };
    t!(i18n, id).into_string()
}

/// The caption under the title: the source pad and the numbers the
/// pattern runs over.
fn range_caption(pattern: &ArraySummary, shown: &ArrayPreview, i18n: &I18n) -> Localized {
    let source = pattern.source_number.as_str();
    if let ArrayPreview::Instances(all) = shown {
        let mut kept = all.iter().filter(|c| !c.is_skipped);
        if let (Some(first), Some(last)) = (kept.next(), kept.next_back()) {
            return t!(
                i18n,
                ids::EDITORS_ROLE_PATTERN_RANGE,
                "source" => source,
                "first" => first.number.as_str(),
                "last" => last.number.as_str()
            );
        }
    }
    t!(i18n, ids::EDITORS_ROLE_PATTERN_SOURCE, "source" => source)
}

pub(super) fn view<'a>(
    fp: &'a FootprintEditorPanelContext,
    pattern: &'a ArraySummary,
    tokens: &'a ThemeTokens,
    collapsed: &'a CollapsedSections,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let shown = &pattern.preview;
    let pill = shown
        .kept()
        .map(|n| t!(i18n, ids::EDITORS_ROLE_PATTERN_INSTANCES, "count" => n));
    let mut parts = vec![
        header(
            t!(i18n, ids::EDITORS_ROLE_PATTERN_TITLE),
            pill,
            caption(range_caption(pattern, shown, i18n), tokens),
            tokens,
        ),
        illustration(drawing(pattern, shown, i18n), fp, tokens),
        container(segmented(kind_segments(pattern, i18n)))
            .padding([SP.xs, SP.m])
            .width(Length::Fill)
            .boxed(),
    ];
    let layout_open = is_open(LAYOUT_SECTION, collapsed, true);
    parts.push(section_header(
        LAYOUT_SECTION,
        t!(i18n, ids::EDITORS_ROLE_PATTERN_LAYOUT),
        layout_open,
        tokens,
        i18n,
    ));
    if layout_open {
        parts.extend(layout_rows(pattern, tokens, i18n));
    }
    let numbering_open = is_open(NUMBERING_SECTION, collapsed, true);
    parts.push(section_header(
        NUMBERING_SECTION,
        t!(i18n, ids::EDITORS_ROLE_PATTERN_NUMBERING),
        numbering_open,
        tokens,
        i18n,
    ));
    if numbering_open {
        let mask = fp
            .numeric_buffers
            .get(&crate::panels::number_mask_buffer_key(pattern.array_id));
        parts.extend(numbering_rows(pattern, mask, tokens, i18n));
    }
    parts.push(danger_action(
        t!(i18n, ids::EDITORS_ROLE_PATTERN_DELETE),
        PanelMsg::FpEditorDeleteArray {
            array_id: pattern.array_id,
        },
        tokens,
    ));
    form(parts)
}

/// One expression field of the array.
fn param<'a>(
    pattern: &ArraySummary,
    field: ArrayParamField,
    example: &'static str,
    value: &str,
) -> Element<'a, PanelMsg> {
    let array_id = pattern.array_id;
    example_input(example, value.to_owned(), move |value| {
        PanelMsg::FpEditorEditArrayParam {
            array_id,
            field,
            value,
        }
    })
}

fn layout_rows<'a>(
    pattern: &'a ArraySummary,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Vec<Element<'a, PanelMsg>> {
    use ArrayParamField as F;
    let mut rows = Vec::new();
    match &pattern.kind {
        ArrayKindSummary::Linear {
            count_expr,
            dx_expr,
            dy_expr,
        } => {
            rows.push(field_row(
                t!(i18n, ids::EDITORS_ROLE_PATTERN_COUNT_DX),
                pair(
                    param(pattern, F::LinearCountExpr, "1", count_expr),
                    param(pattern, F::LinearDxExpr, "0", dx_expr),
                ),
                tokens,
            ));
            rows.push(field_row(
                t!(i18n, ids::EDITORS_ROLE_PATTERN_DY),
                param(pattern, F::LinearDyExpr, "0", dy_expr),
                tokens,
            ));
            let exprs = [dx_expr.as_str(), dy_expr.as_str()];
            if let Some(spacing) = spacing_note(pattern, exprs, i18n) {
                rows.push(note(spacing, tokens));
            }
        }
        ArrayKindSummary::Grid {
            nx_expr,
            ny_expr,
            dx_expr,
            dy_expr,
            mask_expr,
            suppressed_instances,
            nx_value,
            ny_value,
        } => {
            rows.push(field_row(
                t!(i18n, ids::EDITORS_ROLE_PATTERN_NX_NY),
                pair(
                    param(pattern, F::GridNxExpr, "1", nx_expr),
                    param(pattern, F::GridNyExpr, "1", ny_expr),
                ),
                tokens,
            ));
            rows.push(field_row(
                t!(i18n, ids::EDITORS_ROLE_PATTERN_DX_DY),
                pair(
                    param(pattern, F::GridDxExpr, "0", dx_expr),
                    param(pattern, F::GridDyExpr, "0", dy_expr),
                ),
                tokens,
            ));
            let exprs = [dx_expr.as_str(), dy_expr.as_str()];
            if let Some(spacing) = spacing_note(pattern, exprs, i18n) {
                rows.push(note(spacing, tokens));
            }
            rows.push(field_row(
                t!(i18n, ids::EDITORS_PATTERN_MASK),
                param(pattern, F::MaskExpr, "i != 0 || j != 0", mask_expr),
                tokens,
            ));
            if let (Some(nx), Some(ny)) = (nx_value, ny_value) {
                rows.push(note(
                    t!(i18n, ids::EDITORS_ROLE_PATTERN_INSTANCE_GRID),
                    tokens,
                ));
                let nx = (*nx).min(MAX_GRID_CHECKBOX_DIM);
                let ny = (*ny).min(MAX_GRID_CHECKBOX_DIM);
                rows.push(instance_grid(pattern, nx, ny, |i, j| {
                    suppressed_instances.contains(&(i, j))
                }));
            }
        }
        ArrayKindSummary::Polar {
            count_expr,
            sweep_angle_expr,
            center_position_mm: _,
            mask_expr,
            suppressed_instances,
            count_value,
        } => {
            rows.push(field_row(
                t!(i18n, ids::EDITORS_ROLE_PATTERN_COUNT_SWEEP),
                pair(
                    param(pattern, F::PolarCountExpr, "1", count_expr),
                    param(pattern, F::PolarSweepAngleExpr, "360deg", sweep_angle_expr),
                ),
                tokens,
            ));
            rows.push(field_row(
                t!(i18n, ids::EDITORS_ROLE_PATTERN_CENTRE),
                centre_row(pattern, pattern.center_shown_mm, i18n),
                tokens,
            ));
            rows.push(field_row(
                t!(i18n, ids::EDITORS_PATTERN_MASK),
                param(pattern, F::MaskExpr, "i != 0", mask_expr),
                tokens,
            ));
            if let Some(count) = count_value {
                rows.push(note(
                    t!(i18n, ids::EDITORS_ROLE_PATTERN_INSTANCE_GRID),
                    tokens,
                ));
                let count = (*count).min(MAX_POLAR_CHECKBOX_COUNT);
                rows.push(instance_grid(pattern, count, 1, |i, _| {
                    suppressed_instances.contains(&i)
                }));
            }
        }
    }
    rows
}

/// "Spacing 53.00 × 33.00 mm" under spacing fields that name a parameter,
/// such as `board_w - 2*hole_inset` (mockup row 179e): the values the bake
/// walks, so an edit of `board_w` shows here. `None` for plain lengths and
/// while the pattern does not resolve.
pub(super) fn spacing_note(
    pattern: &ArraySummary,
    exprs: [&str; 2],
    i18n: &I18n,
) -> Option<Localized> {
    if exprs.into_iter().all(is_plain_length) {
        return None;
    }
    let (dx, dy) = walked_spacing(pattern)?;
    Some(t!(
        i18n, ids::SKETCH_PATTERN_SPACING_VALUE,
        "dx" => format!("{dx:.2}"), "dy" => format!("{dy:.2}")
    ))
}

/// A bare number with an optional `mm`: nothing to resolve.
fn is_plain_length(expr: &str) -> bool {
    let text = expr.trim().trim_start_matches('=').trim();
    let number = text.strip_suffix("mm").unwrap_or(text).trim();
    number.parse::<f64>().is_ok()
}

/// The step between neighbouring instances in mm, from the preview the bake
/// walks: `(dx, dy)` of a row, or a grid's column and row steps.
fn walked_spacing(pattern: &ArraySummary) -> Option<(f64, f64)> {
    let ArrayPreview::Instances(all) = &pattern.preview else {
        return None;
    };
    let at = |i: usize, j: usize| {
        all.iter()
            .find(|c| c.i == i && c.j == j)
            .map(|c| c.offset_mm)
    };
    match kind_of(pattern) {
        PatternKind::Linear => at(1, 0).map(|[dx, dy]| (dx, dy)),
        PatternKind::Grid => Some((at(1, 0)?[0], at(0, 1)?[1])),
        PatternKind::Polar => None,
    }
}

/// The centre position and the Re-pick button of a polar pattern.
fn centre_row<'a>(
    pattern: &ArraySummary,
    centre: Option<[f64; 2]>,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let value = match centre {
        Some([x, y]) => t!(
            i18n,
            ids::EDITORS_ROLE_PATTERN_CENTRE_VALUE,
            "x" => format!("{x:.3}"),
            "y" => format!("{y:.3}")
        ),
        None => t!(i18n, ids::EDITORS_ROLE_PATTERN_CENTRE_LOST),
    };
    let repick = PanelMsg::FpEditorBeginRepickPolarCenter {
        array_id: pattern.array_id,
    };
    let action = if pattern.repicking_polar_center {
        button(text(t!(i18n, ids::EDITORS_ROLE_PATTERN_REPICKING)).size(TS.caption))
            .padding([SP.xs, SP.m])
            .on_press(repick)
            .style(iced::widget::button::primary)
            .boxed()
    } else {
        ghost_action(t!(i18n, ids::EDITORS_ROLE_PATTERN_REPICK), repick)
    };
    Row::<Element<'a, PanelMsg>>::new()
        .spacing(SP.s)
        .align_y(iced::Alignment::Center)
        .push(container(value_box(value)).width(Length::Fill).boxed())
        .push(action)
        .boxed()
}

/// The shipped per-instance checkboxes, `nx` by `ny`: ticked keeps the
/// instance.
fn instance_grid<'a>(
    pattern: &ArraySummary,
    nx: u32,
    ny: u32,
    is_suppressed: impl Fn(u32, u32) -> bool,
) -> Element<'a, PanelMsg> {
    let array_id = pattern.array_id;
    let mut grid = Column::<Element<'a, PanelMsg>>::new()
        .spacing(SP.xs)
        .padding([0.0, SP.m]);
    for j in 0..ny {
        let mut cells = Row::<Element<'a, PanelMsg>>::new().spacing(SP.xs);
        for i in 0..nx {
            cells = cells.push(
                checkbox(!is_suppressed(i, j))
                    .size(TS.label)
                    .spacing(0)
                    .style(form_look::checkbox_style)
                    .on_toggle(move |value| PanelMsg::FpEditorToggleArrayInstance {
                        array_id,
                        i,
                        j,
                        value,
                    })
                    .boxed(),
            );
        }
        grid = grid.push(cells.boxed());
    }
    grid.boxed()
}

pub(super) fn sweep_label(i18n: &I18n, sweep: GridSweep) -> String {
    let id = match sweep {
        GridSweep::RowByRow => ids::EDITORS_ROLE_PATTERN_SWEEP_ROW,
        GridSweep::ColumnByColumn => ids::EDITORS_ROLE_PATTERN_SWEEP_COLUMN,
        GridSweep::DownThenUp => ids::EDITORS_ROLE_PATTERN_SWEEP_DIP,
    };
    t!(i18n, id).into_string()
}

/// Sweep (mockup row 126, decided 2026-11-22): the order a grid numbered
/// 1, 2, 3 takes its numbers; greyed under the other schemes.
fn sweep_row<'a>(
    pattern: &ArraySummary,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let array_id = pattern.array_id;
    let labels: Vec<(GridSweep, String)> = GridSweep::ALL
        .iter()
        .map(|&sweep| (sweep, sweep_label(i18n, sweep)))
        .collect();
    let list = pick_list(
        Some(pattern.sweep.unwrap_or_default()),
        GridSweep::ALL.to_vec(),
        move |sweep: &GridSweep| {
            labels
                .iter()
                .find(|(s, _)| s == sweep)
                .map(|(_, label)| label.clone())
                .unwrap_or_default()
        },
    )
    .graphite()
    .on_select(move |sweep| PanelMsg::FpEditorSetArraySweep { array_id, sweep })
    .padding([SP.xs, SP.m])
    .width(Length::Fill);
    let row = field_row(
        t!(i18n, ids::EDITORS_ROLE_PATTERN_SWEEP),
        list.boxed(),
        tokens,
    );
    gated(row, pattern.sweep.is_some(), tokens)
}

/// The instances on a pattern's suppressed list, as `(i, j)` (`j` is 0 on
/// a polar pattern).
fn suppressed_cells(pattern: &ArraySummary) -> Vec<(u32, u32)> {
    match &pattern.kind {
        ArrayKindSummary::Grid {
            suppressed_instances,
            ..
        } => suppressed_instances.clone(),
        ArrayKindSummary::Polar {
            suppressed_instances,
            ..
        } => suppressed_instances.iter().map(|&i| (i, 0)).collect(),
        ArrayKindSummary::Linear { .. } => Vec::new(),
    }
}

/// What the Numbering Mask shows: the typed text while it is refused or
/// skips what the pattern skips, else the skipped pads' numbers.
pub(super) fn shown_number_mask(
    pattern: &ArraySummary,
    typed: Option<&String>,
) -> (String, Option<NumberMaskRefusal>) {
    let mut suppressed = suppressed_cells(pattern);
    let stored = suppressed_numbers(&pattern.preview, &suppressed).join(", ");
    let Some(text) = typed else {
        return (stored, None);
    };
    match instances_numbered(&pattern.preview, text) {
        Err(refusal) => (text.clone(), Some(refusal)),
        Ok(mut cells) => {
            cells.sort_unstable();
            suppressed.sort_unstable();
            if cells == suppressed {
                (text.clone(), None)
            } else {
                (stored, None)
            }
        }
    }
}

/// Numbering Mask (mockup row 126): the pad numbers the pattern skips; a
/// linear pattern has no suppressed list and greys it.
fn number_mask_row<'a>(
    pattern: &ArraySummary,
    typed: Option<&String>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Element<'a, PanelMsg> {
    let array_id = pattern.array_id;
    let (value, refusal) = shown_number_mask(pattern, typed);
    let field = super::helpers::input(
        t!(i18n, ids::EDITORS_ROLE_PATTERN_NUMBER_MASK_HINT),
        value,
        move |value| PanelMsg::FpEditorEditArrayParam {
            array_id,
            field: ArrayParamField::NumberMask,
            value,
        },
    );
    let mut rows = Column::<Element<'a, PanelMsg>>::new()
        .width(Length::Fill)
        .push(field_row(
            t!(i18n, ids::EDITORS_PATTERN_MASK),
            field,
            tokens,
        ));
    if let Some(refusal) = refusal {
        let reason = match &refusal {
            NumberMaskRefusal::NotInPattern(number) => t!(
                i18n,
                ids::EDITORS_ROLE_PATTERN_NUMBER_MASK_UNKNOWN,
                "number" => number.as_str()
            ),
            NumberMaskRefusal::IsSource(number) => t!(
                i18n,
                ids::EDITORS_ROLE_PATTERN_NUMBER_MASK_SOURCE,
                "number" => number.as_str()
            ),
            NumberMaskRefusal::Unresolved => {
                t!(i18n, ids::EDITORS_ROLE_PATTERN_NUMBER_MASK_UNRESOLVED)
            }
        };
        let line = text(reason)
            .size(TS.caption)
            .color(crate::styles::ti(tokens.error));
        rows = rows.push(
            container(line)
                .padding([0.0, SP.m])
                .width(Length::Fill)
                .boxed(),
        );
    }
    let enabled = kind_of(pattern) != PatternKind::Linear;
    gated(rows.boxed(), enabled, tokens)
}

fn numbering_rows<'a>(
    pattern: &'a ArraySummary,
    mask: Option<&String>,
    tokens: &ThemeTokens,
    i18n: &I18n,
) -> Vec<Element<'a, PanelMsg>> {
    let array_id = pattern.array_id;
    let labels: Vec<(NumberingSchemeKindUi, String)> = NumberingSchemeKindUi::ALL
        .iter()
        .map(|&scheme| (scheme, scheme_label(i18n, scheme)))
        .collect();
    let scheme = pick_list(
        Some(pattern.numbering),
        NumberingSchemeKindUi::ALL.to_vec(),
        move |scheme: &NumberingSchemeKindUi| {
            labels
                .iter()
                .find(|(s, _)| s == scheme)
                .map(|(_, label)| label.clone())
                .unwrap_or_default()
        },
    )
    .graphite()
    .on_select(move |scheme| PanelMsg::FpEditorSetArrayNumberingScheme { array_id, scheme })
    .padding([SP.xs, SP.m])
    .width(Length::Fill);
    let mut rows = vec![field_row(
        t!(i18n, ids::EDITORS_ROLE_PATTERN_SCHEME),
        scheme.boxed(),
        tokens,
    )];
    if kind_of(pattern) == PatternKind::Grid {
        rows.push(sweep_row(pattern, tokens, i18n));
    }
    // Other schemes show the BGA defaults, greyed.
    let (skip, start_row, start_col) = pattern.bga_config.as_ref().map_or((true, 'A', 1), |b| {
        (b.skip_letters, b.start_row, b.start_col)
    });
    let enabled = bga_rows_enabled(pattern);
    let start = field_row(
        t!(i18n, ids::EDITORS_ROLE_PATTERN_START_ROW_COL),
        pair(
            super::helpers::input(
                UserText::new(String::new()),
                start_row.to_string(),
                move |value| PanelMsg::FpEditorSetBgaStartRow { array_id, value },
            ),
            super::helpers::input(
                UserText::new(String::new()),
                start_col.to_string(),
                move |value| PanelMsg::FpEditorSetBgaStartCol { array_id, value },
            ),
        ),
        tokens,
    );
    rows.push(gated(start, enabled, tokens));
    rows.push(number_mask_row(pattern, mask, tokens, i18n));
    let skip_row = field_row(
        UserText::new(String::new()),
        check(
            t!(i18n, ids::EDITORS_PATTERN_SKIP_LETTERS),
            skip,
            move |value| PanelMsg::FpEditorSetBgaSkipLetters { array_id, value },
            tokens,
        ),
        tokens,
    );
    rows.push(gated(skip_row, enabled, tokens));
    rows
}
