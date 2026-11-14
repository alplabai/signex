//! Grid Manager and Guide Manager (#2353). A board holds one snap grid, its
//! step in the setup; the step and the visible grid are editable. More
//! grids, their colours and origins, and guides are not in the board model
//! yet, so those controls are inert with a tooltip saying why.

use iced::widget::container;
use iced::{Element, Widget as _};
use signex_i18n::{Localized, ids, t};
use signex_types::measure::length_field_text;

use super::chrome::{Form, Shown};
use super::info::shown_unit;
use super::{PanelMsg, PcbBoardMsg, PcbBoardSummary, SP};

pub(super) fn grid_section<'a>(
    form: &Form<'a>,
    board: &PcbBoardSummary,
) -> Option<Element<'a, PanelMsg>> {
    let more = || form.t(ids::PCB_BOARD_GRID_MORE_UNAVAILABLE);
    let unset = || form.unavailable(form.t(ids::PCB_BOARD_UNSET), more());
    let mut rows: Vec<Shown<'a>> = Vec::new();

    let headings = [
        ids::PCB_BOARD_GRID_PRIORITY,
        ids::PCB_BOARD_GRID_NAME,
        ids::PCB_BOARD_GRID_COLOR,
        ids::PCB_BOARD_GRID_ORIGIN,
        ids::PCB_BOARD_GRID_COMP,
        ids::PCB_BOARD_GRID_NON_COMP,
    ];
    let header = form.table_row(headings.iter().map(|id| form.heading(*id)).collect(), true);
    let name = form.t(ids::PCB_BOARD_GRID_GLOBAL);
    rows.push(form.row(&[&name], header));
    let applies = || {
        form.tip(
            signex_widgets::checkbox(true)
                .style(super::super::form_look::checkbox_style)
                .boxed(),
            more(),
        )
    };
    let global = form.table_row(
        vec![
            form.value(t!(form.i18n, ids::PCB_BOARD_COUNT, "count" => 1)),
            form.value(name.clone()),
            unset(),
            unset(),
            applies(),
            applies(),
        ],
        false,
    );
    rows.push(form.row(&[&name], global));

    let step = form.t(ids::PCB_BOARD_GRID_STEP);
    rows.push(form.row(&[&name, &step], form.field(&step, step_field(form, board))));

    let visible = form.t(ids::PCB_BOARD_GRID_VISIBLE);
    let check = form.check(
        &visible,
        form.ctx.grid_visible,
        Some(PanelMsg::ToggleGrid),
        None,
    );
    rows.push(form.row(&[&visible], check));

    let fast = form.t(ids::PCB_BOARD_GRID_FAST);
    let fast_value = form.unavailable(
        form.t(ids::PCB_BOARD_NOT_AVAILABLE),
        form.t(ids::PCB_BOARD_GRID_FAST_UNAVAILABLE),
    );
    rows.push(form.row(&[&fast], form.field(&fast, fast_value)));

    let buttons = inert_buttons(
        form,
        &[
            ids::PCB_BOARD_ADD,
            ids::PCB_BOARD_PROPERTIES,
            ids::PCB_BOARD_DELETE,
        ],
        more(),
    );
    rows.push(form.row(&[&name], buttons));
    form.section("pcb_board_grids", ids::PCB_BOARD_SECTION_GRIDS, rows)
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

pub(super) fn guide_section<'a>(form: &Form<'a>) -> Option<Element<'a, PanelMsg>> {
    let why = || form.t(ids::PCB_BOARD_SNAP_GUIDES_UNAVAILABLE);
    let headings = [
        ids::PCB_BOARD_GUIDE_ENABLED,
        ids::PCB_BOARD_GRID_NAME,
        ids::PCB_BOARD_GUIDE_X,
        ids::PCB_BOARD_GUIDE_Y,
        ids::PCB_BOARD_GRID_COLOR,
    ];
    let header = form.table_row(headings.iter().map(|id| form.heading(*id)).collect(), true);
    let empty = form.t(ids::PCB_BOARD_GUIDES_EMPTY);
    let title = form.t(ids::PCB_BOARD_SECTION_GUIDES);
    let rows = vec![
        form.row(&[&title], header),
        form.row(&[&empty], form.tip(form.note(empty.clone()), why())),
        form.row(
            &[&title],
            inert_buttons(
                form,
                &[
                    ids::PCB_BOARD_ADD,
                    ids::PCB_BOARD_PLACE,
                    ids::PCB_BOARD_DELETE,
                ],
                why(),
            ),
        ),
    ];
    form.section("pcb_board_guides", ids::PCB_BOARD_SECTION_GUIDES, rows)
}

/// A row of disabled buttons, each with `why` on hover.
fn inert_buttons<'a>(
    form: &Form<'a>,
    labels: &[signex_i18n::MessageId],
    why: Localized,
) -> Element<'a, PanelMsg> {
    let mut line: iced::widget::Row<Element<'a, PanelMsg>> = iced::widget::Row::new().spacing(SP.s);
    for label in labels {
        line = line.push(form.action(form.t(*label), None, Some(why.clone())));
    }
    container(line).padding([SP.s, SP.l]).boxed()
}
