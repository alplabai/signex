//! Grid Manager (#2353, #2374): the board's extra snap grids in priority
//! order above the Global Board Snap Grid (the setup's step, at the
//! origin). A row selects its grid for Properties and Delete; the Comp and
//! Non Comp boxes and the swatch edit it at once. Fast Grids set the global
//! step to one of the fast grid preferences (#1672).

use iced::widget::{Row, container};
use iced::{Element, Widget as _};
use signex_i18n::{Localized, ids, t};
use signex_types::measure::length_field_text;
use signex_types::pcb_options::BoardGrid;

use super::chrome::{Form, Shown};
use super::info::shown_unit;
use super::{BoardField, PanelMsg, PcbBoardMsg, PcbBoardSummary, SP, fields};

fn msg(m: PcbBoardMsg) -> Option<PanelMsg> {
    Some(PanelMsg::PcbBoard(m))
}

/// The Global Board Snap Grid's priority: below every extra grid.
const GLOBAL_PRIORITY: usize = 50;
/// The share of the table each column takes: Priority, Name, Color, Origin,
/// Comp, Non Comp. Name gets the room its text needs (#2386).
const COLUMNS: [u16; 6] = [2, 6, 2, 4, 2, 3];

pub(super) fn grid_section<'a>(
    form: &Form<'a>,
    board: &'a PcbBoardSummary,
) -> Option<Element<'a, PanelMsg>> {
    let mut rows: Vec<Shown<'a>> = Vec::new();
    let title = form.t(ids::PCB_BOARD_SECTION_GRIDS);
    let headings = [
        ids::PCB_BOARD_GRID_PRIORITY,
        ids::PCB_BOARD_GRID_NAME,
        ids::PCB_BOARD_GRID_COLOR,
        ids::PCB_BOARD_GRID_ORIGIN,
        ids::PCB_BOARD_GRID_COMP,
        ids::PCB_BOARD_GRID_NON_COMP,
    ];
    let cells = headings.iter().map(|id| form.heading(*id)).collect();
    let header = form.table_row_in(cells, &COLUMNS, true);
    rows.push(form.row(&[&title], header));
    let editable = !board.read_only;
    for (index, grid) in board.options.grids.iter().enumerate() {
        rows.push(grid_row(form, board, index, grid, editable));
    }
    let name = form.t(ids::PCB_BOARD_GRID_GLOBAL);
    let global = form.table_row_in(
        vec![
            form.value(t!(form.i18n, ids::PCB_BOARD_COUNT, "count" => GLOBAL_PRIORITY)),
            form.value(name.clone()),
            form.value(form.t(ids::PCB_BOARD_UNSET)),
            form.value(origin_text(form, 0.0, 0.0)),
            form.value(form.t(ids::PCB_BOARD_GRID_ALWAYS)),
            form.value(form.t(ids::PCB_BOARD_GRID_ALWAYS)),
        ],
        &COLUMNS,
        false,
    );
    rows.push(form.row(&[&name], global));

    let step = form.t(ids::PCB_BOARD_GRID_STEP);
    rows.push(form.row(&[&name, &step], form.field(&step, step_field(form, board))));
    if board.view.grid_editor_open
        && let Some(grid) = selected(board)
    {
        editor_rows(form, board, grid, &mut rows);
    }

    let visible = form.t(ids::PCB_BOARD_GRID_VISIBLE);
    let check = form.check(
        &visible,
        form.ctx.grid_visible,
        Some(PanelMsg::ToggleGrid),
        None,
    );
    rows.push(form.row(&[&visible], check));

    let fast = form.t(ids::PCB_BOARD_GRID_FAST);
    rows.push(form.row(&[&fast], form.field(&fast, fast_grids(form, board))));

    rows.push(form.row(&[&title], buttons(form, board)));
    form.section("pcb_board_grids", ids::PCB_BOARD_SECTION_GRIDS, rows)
}

fn selected(board: &PcbBoardSummary) -> Option<&BoardGrid> {
    let id = board.view.selected_grid?;
    board.options.grids.iter().find(|g| g.uuid == id)
}

fn origin_text(form: &Form<'_>, x: f64, y: f64) -> Localized {
    let unit = shown_unit(form);
    t!(
        form.i18n,
        ids::PCB_BOARD_POINT,
        "x" => length_field_text(x, unit),
        "y" => length_field_text(y, unit)
    )
}

/// One extra grid's row: priority, name, swatch, origin, Comp, Non Comp.
fn grid_row<'a>(
    form: &Form<'a>,
    board: &PcbBoardSummary,
    index: usize,
    grid: &'a BoardGrid,
    editable: bool,
) -> Shown<'a> {
    let id = grid.uuid;
    let check = |on: bool, toggled: PcbBoardMsg| {
        let mut b = signex_widgets::checkbox(on).style(super::super::form_look::checkbox_style);
        if editable {
            b = b.on_toggle(move |_| PanelMsg::PcbBoard(toggled.clone()));
        }
        b.boxed()
    };
    let swatch = if editable {
        let tip = form.t(ids::PCB_BOARD_SWATCH_TIP);
        fields::swatch(form, grid.color, PcbBoardMsg::GridColorStepped(id), tip)
    } else {
        form.value(form.t(ids::PCB_BOARD_UNSET))
    };
    let cells = vec![
        form.value(t!(form.i18n, ids::PCB_BOARD_COUNT, "count" => index + 1)),
        form.value(signex_i18n::UserText::new(grid.name.clone())),
        swatch,
        form.value(origin_text(
            form,
            board.options.origin.user_x(grid.origin.x),
            board.options.origin.user_y(grid.origin.y),
        )),
        check(grid.components, PcbBoardMsg::GridComponentsToggled(id)),
        check(
            grid.non_components,
            PcbBoardMsg::GridNonComponentsToggled(id),
        ),
    ];
    let line = form.table_row_in(cells, &COLUMNS, false);
    let picked = board.view.selected_grid == Some(id);
    let line = fields::selectable(form, line, picked, PcbBoardMsg::GridSelected(id));
    form.row(&[&form.t(ids::PCB_BOARD_SECTION_GRIDS)], line)
}

/// The selected grid's Name, Step and Origin fields (Properties).
fn editor_rows<'a>(
    form: &Form<'a>,
    board: &PcbBoardSummary,
    grid: &BoardGrid,
    rows: &mut Vec<Shown<'a>>,
) {
    let id = grid.uuid;
    let name = form.t(ids::PCB_BOARD_GRID_NAME);
    let field = fields::text(
        form,
        board,
        BoardField::GridName(id),
        name.clone(),
        grid.name.clone(),
    );
    rows.push(form.row(&[&name], form.field(&name, field)));
    let lengths = [
        (
            ids::PCB_BOARD_GRID_STEP,
            BoardField::GridStep(id),
            grid.step_mm,
        ),
        (
            ids::PCB_BOARD_GRID_ORIGIN_X,
            BoardField::GridOriginX(id),
            board.options.origin.user_x(grid.origin.x),
        ),
        (
            ids::PCB_BOARD_GRID_ORIGIN_Y,
            BoardField::GridOriginY(id),
            board.options.origin.user_y(grid.origin.y),
        ),
    ];
    for (label, field, mm) in lengths {
        let label = form.t(label);
        let input = fields::length(form, board, field, label.clone(), mm);
        rows.push(form.row(&[&label], form.field(&label, input)));
    }
}

/// The snap grid step: a field in the panel's unit that Enter applies as
/// one undo step, or the step with a tooltip when the board cannot take one.
fn step_field<'a>(form: &Form<'a>, board: &PcbBoardSummary) -> Element<'a, PanelMsg> {
    let unit = shown_unit(form);
    let Some(grid_mm) = board.grid_mm.filter(|_| !board.read_only) else {
        let shown = match board.grid_mm {
            Some(mm) => {
                t!(form.i18n, ids::PCB_BOARD_MEASURE, "value" => length_field_text(mm, unit))
            }
            None => form.t(ids::PCB_BOARD_UNSET),
        };
        let why = if board.read_only {
            form.t(ids::PCB_FOOTPRINT_READ_ONLY)
        } else {
            form.t(ids::PCB_BOARD_GRID_NO_SETUP)
        };
        return form.unavailable(shown, why);
    };
    let text = board
        .view
        .grid_draft
        .clone()
        .unwrap_or_else(|| length_field_text(grid_mm, unit));
    form.input(
        form.t(ids::PCB_BOARD_GRID_STEP),
        text,
        |s| PanelMsg::PcbBoard(PcbBoardMsg::GridStepTyped(s)),
        PanelMsg::PcbBoard(PcbBoardMsg::GridStepSubmitted),
    )
}

/// One button per fast grid preference; each sets the global step to it.
fn fast_grids<'a>(form: &Form<'a>, board: &PcbBoardSummary) -> Element<'a, PanelMsg> {
    let unit = shown_unit(form);
    let usable = board.grid_mm.is_some() && !board.read_only;
    let mut line: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.s);
    for (index, mm) in board.fast_grids_mm.iter().enumerate() {
        let label = t!(
            form.i18n,
            ids::PCB_BOARD_MEASURE,
            "value" => length_field_text(f64::from(*mm), unit)
        );
        let press = usable.then_some(PanelMsg::PcbBoard(PcbBoardMsg::FastGridApplied(index)));
        let why = (!usable).then(|| form.t(ids::PCB_BOARD_GRID_NO_SETUP));
        line = line.push(form.action(label, press, why));
    }
    line.boxed()
}

/// Add, Properties and Delete. Properties and Delete act on the selected
/// grid; with none selected they say so.
fn buttons<'a>(form: &Form<'a>, board: &PcbBoardSummary) -> Element<'a, PanelMsg> {
    let picked = selected(board).is_some() && !board.read_only;
    let why = || (!picked).then(|| form.t(ids::PCB_BOARD_SELECT_ROW_FIRST));
    let line = iced::widget::row![
        form.action(
            form.t(ids::PCB_BOARD_ADD),
            (!board.read_only)
                .then_some(PcbBoardMsg::GridAdded)
                .and_then(msg),
            None,
        ),
        form.action(
            form.t(ids::PCB_BOARD_PROPERTIES),
            picked
                .then_some(PcbBoardMsg::GridPropertiesToggled)
                .and_then(msg),
            why(),
        ),
        form.action(
            form.t(ids::PCB_BOARD_DELETE),
            picked.then_some(PcbBoardMsg::GridDeleted).and_then(msg),
            why(),
        ),
    ]
    .spacing(SP.s);
    container(line).padding([SP.s, SP.l]).boxed()
}
