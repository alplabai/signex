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
use signex_bake::array::ArrayPreview;

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
        parts.extend(numbering_rows(pattern, tokens, i18n));
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

fn numbering_rows<'a>(
    pattern: &'a ArraySummary,
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
