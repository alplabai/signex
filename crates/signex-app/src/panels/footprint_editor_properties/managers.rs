//! The Grid Manager: a compact table of the editor's grids (Name / Step /
//! Display), "+ Add" in the header, Properties… and Delete acting on the
//! active grid. The first column picks the active grid; its step is
//! editable in place, the other rows read greyed.

use iced::widget::{Column, Row, Space, container, row};
use iced::{Element, Length, Widget as _};
use signex_i18n::{UserText, ids, t};
use signex_widgets::{checkbox, raw, text, text_input};

use super::super::PanelMsg;
use super::body::{Ctx, caption, fields, ghost, is_open, rule, section};
use super::illustration::linked_field;
use crate::library::editor::footprint::{GridDef, GridDisplay};
use crate::panels::form_look;
use crate::styles::tokens::resolve;
use crate::styles::{Spacing, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

const KEY: &str = "fp_grid_manager";
/// Width of the active column and of the Step column, in captions.
const CHECK_EM: f32 = 2.0;
const STEP_EM: f32 = 6.0;

fn display_label(display: GridDisplay) -> signex_i18n::MessageId {
    match display {
        GridDisplay::Lines => ids::EDITORS_FP_PROPS_GRID_LINES,
        GridDisplay::Dots => ids::EDITORS_FP_PROPS_GRID_DOTS,
        GridDisplay::Hidden => ids::EDITORS_FP_PROPS_GRID_HIDDEN,
    }
}

/// One table line: the active column, then Name, Step and Display.
fn table_row<'a>(
    check: Element<'a, PanelMsg>,
    name: Element<'a, PanelMsg>,
    step: Element<'a, PanelMsg>,
    display: Element<'a, PanelMsg>,
) -> Element<'a, PanelMsg> {
    container(
        row![
            container(check).width(Length::Fixed(TS.caption * CHECK_EM)),
            container(name).width(Length::Fill),
            container(step)
                .width(Length::Fixed(TS.caption * STEP_EM))
                .align_x(iced::alignment::Horizontal::Right),
            container(display).width(Length::Fill),
        ]
        .spacing(SP.s)
        .align_y(iced::Alignment::Center),
    )
    .padding([SP.xs, SP.m])
    .width(Length::Fill)
    .boxed()
}

fn grid_row<'a>(cx: Ctx<'a, '_>, idx: usize, grid: &'a GridDef) -> Element<'a, PanelMsg> {
    let tokens = cx.tokens;
    let is_active = idx == cx.fp.active_grid_idx;
    let ink = if is_active {
        ti(tokens.text)
    } else {
        resolve(tokens).ink4
    };
    let check = checkbox(is_active)
        .on_toggle(move |_| PanelMsg::FpEditorGridSetActive(idx))
        .size(TS.body)
        .spacing(0)
        .style(form_look::checkbox_style)
        .boxed();
    let name = text(UserText::new(grid.name.as_str()))
        .size(TS.body)
        .color(ink)
        .boxed();
    let step = format!("{:.3}", grid.step_mm);
    let step = if is_active {
        let input = text_input(UserText::new(String::new()), step)
            .on_input(PanelMsg::FpEditorSetSnapGridStep)
            .size(TS.body)
            .padding(SP.xs)
            .style(form_look::text_input_style)
            .boxed();
        linked_field(input, fields::GRID_STEP, &cx.fp.illustration_hover, tokens)
    } else {
        // raw: the step is a measured number, not language.
        text(raw(step)).size(TS.body).color(ink).boxed()
    };
    let display = text(t!(cx.i18n, display_label(grid.fine_display)))
        .size(TS.body)
        .color(ink)
        .boxed();
    table_row(check, name, step, display)
}

/// The Grid Manager section.
pub(super) fn view_grid_manager<'a>(
    mut col: Column<Element<'a, PanelMsg>>,
    cx: Ctx<'a, '_>,
) -> Column<Element<'a, PanelMsg>> {
    let Ctx {
        fp, tokens, i18n, ..
    } = cx;
    let add = ghost(
        t!(i18n, ids::EDITORS_FP_PROPS_GRID_ADD),
        Some(PanelMsg::FpEditorGridManagerAdd),
        tokens,
    );
    let title = t!(i18n, ids::EDITORS_FP_PROPS_SECTION_GRIDS);
    col = col.push(section(KEY, &title, Some(add), cx.collapsed, tokens, i18n));
    if !is_open(KEY, cx.collapsed) {
        return col;
    }
    if fp.grids.is_empty() {
        return col.push(caption(t!(i18n, ids::EDITORS_FP_PROPS_GRID_NONE), tokens));
    }
    let head = |id: signex_i18n::MessageId| {
        text(t!(i18n, id))
            .size(TS.caption)
            .color(resolve(tokens).ink4)
            .boxed()
    };
    col = col.push(table_row(
        Space::new().boxed(),
        head(ids::EDITORS_FP_PROPS_GRID_NAME),
        head(ids::EDITORS_FP_PROPS_GRID_STEP),
        head(ids::EDITORS_FP_PROPS_GRID_DISPLAY),
    ));
    col = col.push(rule(tokens));
    for (idx, grid) in fp.grids.iter().enumerate() {
        col = col.push(grid_row(cx, idx, grid));
    }
    // Delete keeps one grid: the editor always snaps to an active one.
    let delete = (fp.grids.len() > 1).then_some(PanelMsg::FpEditorGridManagerDelete);
    let actions = Row::new()
        .spacing(SP.s)
        .push(Space::new().width(Length::Fill).boxed())
        .push(ghost(
            t!(i18n, ids::EDITORS_FP_PROPS_GRID_PROPERTIES),
            Some(PanelMsg::FpEditorGridManagerProperties),
            tokens,
        ))
        .push(ghost(
            t!(i18n, ids::EDITORS_FP_PROPS_DELETE),
            delete,
            tokens,
        ));
    col.push(
        container(actions)
            .padding([SP.xs, SP.m])
            .width(Length::Fill)
            .boxed(),
    )
}
