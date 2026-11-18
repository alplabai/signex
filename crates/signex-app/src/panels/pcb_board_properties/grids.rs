//! Grid Manager (#2353, #2374, #2386): one card per grid. The board's own
//! grid (the setup's step, counted from the board origin) comes first, then
//! the extra grids, highest priority first: the order is the priority. A
//! card shows its swatch, name, step (typed in place) and a ⋯ menu (Edit,
//! Move Up, Move Down, Delete; the board's own grid has Edit only), and
//! under them its origin and the Parts and Other pills. Fast Grids set the
//! board's step to one of the fast grid preferences (#1672).

use iced::widget::{Column, container};
use iced::{Element, Length, Widget as _};
use iced_aw::Wrap;
use signex_i18n::{Localized, ids, t};
use signex_types::measure::length_field_text;
use signex_types::pcb_options::BoardGrid;

use super::chrome::{Form, Shown};
use super::info::shown_unit;
use super::{BoardField, GridCard, PanelMsg, PcbBoardMsg, PcbBoardSummary, SP, cards, fields};

fn msg(m: PcbBoardMsg) -> Option<PanelMsg> {
    Some(PanelMsg::PcbBoard(m))
}

pub(super) fn grid_section<'a>(
    form: &Form<'a>,
    board: &'a PcbBoardSummary,
) -> Option<Element<'a, PanelMsg>> {
    let mut rows: Vec<Shown<'a>> = vec![global_card(form, board)];
    let count = board.options.grids.len();
    for (index, grid) in board.options.grids.iter().enumerate() {
        rows.push(grid_card(form, board, index, count, grid));
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

    let add = form.t(ids::PCB_BOARD_GRID_ADD);
    let press = (!board.read_only)
        .then_some(PcbBoardMsg::GridAdded)
        .and_then(msg);
    let button = container(form.action(add.clone(), press, None))
        .padding([SP.s, SP.l])
        .boxed();
    rows.push(form.row(&[&add], button));
    form.section("pcb_board_grids", ids::PCB_BOARD_SECTION_GRIDS, rows)
}

/// `(x, y)` in the panel's unit, as a card's origin line shows it.
fn origin_text(form: &Form<'_>, x: f64, y: f64) -> Localized {
    let unit = shown_unit(form);
    t!(
        form.i18n,
        ids::PCB_BOARD_POINT,
        "x" => length_field_text(x, unit),
        "y" => length_field_text(y, unit)
    )
}

/// The step field and the unit after it, beside a card's name.
fn step<'a>(form: &Form<'a>, field: Element<'a, PanelMsg>) -> Element<'a, PanelMsg> {
    let unit = t!(form.i18n, ids::PCB_BOARD_MEASURE, "value" => shown_unit(form).to_string());
    container(cards::line(vec![field, cards::muted(form, unit)]))
        .width(Length::FillPortion(2))
        .boxed()
}

/// A card's name, taking the room the step leaves.
fn name<'a>(form: &Form<'a>, label: impl Into<signex_widgets::Label<'a>>) -> Element<'a, PanelMsg> {
    container(cards::name(form, label))
        .width(Length::FillPortion(3))
        .boxed()
}

/// The Parts and Other pills: what the old Comp and Non Comp columns said.
fn pills<'a>(
    form: &Form<'a>,
    parts: (bool, Option<PanelMsg>),
    other: (bool, Option<PanelMsg>),
    why: Option<Localized>,
) -> Vec<Element<'a, PanelMsg>> {
    vec![
        cards::pill(
            form,
            form.t(ids::PCB_BOARD_GRID_PARTS),
            parts.0,
            parts.1,
            why.clone(),
        ),
        cards::pill(
            form,
            form.t(ids::PCB_BOARD_GRID_OTHER),
            other.0,
            other.1,
            why,
        ),
    ]
}

/// The board's own grid: always last in priority, never moved or deleted;
/// it applies to everything no extra grid claims.
fn global_card<'a>(form: &Form<'a>, board: &PcbBoardSummary) -> Shown<'a> {
    let label = form.t(ids::PCB_BOARD_GRID_GLOBAL);
    let card = GridCard::Global;
    let menu_msg = (!board.read_only)
        .then_some(PcbBoardMsg::GridMenuToggled(card))
        .and_then(msg);
    let header = cards::line(vec![
        cards::no_swatch(),
        name(form, label.clone()),
        step(form, step_field(form, board)),
        cards::more(form, menu_msg),
    ]);
    let fixed = Some(form.t(ids::PCB_BOARD_GRID_FIXED_TIP));
    let mut second = vec![
        container(cards::muted(form, origin_text(form, 0.0, 0.0)))
            .width(Length::Fill)
            .boxed(),
    ];
    second.extend(pills(form, (true, None), (true, None), fixed));
    let mut body = Column::new()
        .spacing(SP.xs)
        .push(header)
        .push(cards::line(second));
    if board.view.grid_menu == Some(card) {
        let edit = (
            form.t(ids::PCB_BOARD_EDIT),
            msg(PcbBoardMsg::GridEditToggled(card)),
        );
        body = body.push(cards::menu(form, vec![edit]));
    }
    if board.view.grid_editing == Some(card) {
        let step_label = form.t(ids::PCB_BOARD_GRID_STEP);
        body = body.push(cards::labelled(form, step_label, step_field(form, board)));
    }
    form.row(&[&label], form.card(body.boxed()))
}

/// An extra grid's card; `index` is its place among `count` extra grids,
/// highest priority first.
fn grid_card<'a>(
    form: &Form<'a>,
    board: &PcbBoardSummary,
    index: usize,
    count: usize,
    grid: &BoardGrid,
) -> Shown<'a> {
    let id = grid.uuid;
    let card = GridCard::Extra(id);
    let editable = !board.read_only;
    let live = |m: PcbBoardMsg| editable.then_some(m).and_then(msg);
    let swatch = if editable {
        let tip = form.t(ids::PCB_BOARD_SWATCH_TIP);
        fields::swatch(form, grid.color, PcbBoardMsg::GridColorStepped(id), tip)
    } else {
        cards::no_swatch()
    };
    let step_value = if editable {
        let label = form.t(ids::PCB_BOARD_GRID_STEP);
        fields::length(form, board, BoardField::GridStep(id), label, grid.step_mm)
    } else {
        form.value(fields::measure(form, grid.step_mm))
    };
    let header = cards::line(vec![
        swatch,
        name(form, signex_i18n::UserText::new(grid.name.clone())),
        step(form, step_value),
        cards::more(form, live(PcbBoardMsg::GridMenuToggled(card))),
    ]);
    let origin = board.options.origin;
    let at = origin_text(
        form,
        origin.user_x(grid.origin.x),
        origin.user_y(grid.origin.y),
    );
    let mut second = vec![
        container(cards::muted(form, at))
            .width(Length::Fill)
            .boxed(),
    ];
    second.extend(pills(
        form,
        (
            grid.components,
            live(PcbBoardMsg::GridComponentsToggled(id)),
        ),
        (
            grid.non_components,
            live(PcbBoardMsg::GridNonComponentsToggled(id)),
        ),
        None,
    ));
    let mut body = Column::new()
        .spacing(SP.xs)
        .push(header)
        .push(cards::line(second));
    if board.view.grid_menu == Some(card) {
        let actions = vec![
            (
                form.t(ids::PCB_BOARD_EDIT),
                msg(PcbBoardMsg::GridEditToggled(card)),
            ),
            (
                form.t(ids::PCB_BOARD_MOVE_UP),
                (index > 0)
                    .then_some(PcbBoardMsg::GridMoved(id, true))
                    .and_then(msg),
            ),
            (
                form.t(ids::PCB_BOARD_MOVE_DOWN),
                (index + 1 < count)
                    .then_some(PcbBoardMsg::GridMoved(id, false))
                    .and_then(msg),
            ),
            (
                form.t(ids::PCB_BOARD_DELETE),
                msg(PcbBoardMsg::GridDeleted(id)),
            ),
        ];
        body = body.push(cards::menu(form, actions));
    }
    if editable && board.view.grid_editing == Some(card) {
        body = body.push(editor(form, board, grid));
    }
    let title = form.t(ids::PCB_BOARD_SECTION_GRIDS);
    let key = t!(form.i18n, ids::PCB_BOARD_MEASURE, "value" => grid.name.clone());
    form.row(&[&title, &key], form.card(body.boxed()))
}

/// Edit: the grid's Name, then Origin X and Y as a pair.
fn editor<'a>(form: &Form<'a>, board: &PcbBoardSummary, grid: &BoardGrid) -> Element<'a, PanelMsg> {
    let id = grid.uuid;
    let name_label = form.t(ids::PCB_BOARD_GRID_NAME);
    let name = fields::text(
        form,
        board,
        BoardField::GridName(id),
        name_label.clone(),
        grid.name.clone(),
    );
    let origin = board.options.origin;
    let pair = |label, field, mm| {
        let label = form.t(label);
        let input = fields::length(form, board, field, label.clone(), mm);
        cards::labelled(form, label, input)
    };
    let x = pair(
        ids::PCB_BOARD_GRID_ORIGIN_X,
        BoardField::GridOriginX(id),
        origin.user_x(grid.origin.x),
    );
    let y = pair(
        ids::PCB_BOARD_GRID_ORIGIN_Y,
        BoardField::GridOriginY(id),
        origin.user_y(grid.origin.y),
    );
    Column::new()
        .spacing(SP.s)
        .push(cards::labelled(form, name_label, name))
        .push(cards::line(vec![x, y]))
        .boxed()
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

/// One chip per fast grid preference; a click sets the board's step to it,
/// and the chip of the step in use is lit.
fn fast_grids<'a>(form: &Form<'a>, board: &PcbBoardSummary) -> Element<'a, PanelMsg> {
    let unit = shown_unit(form);
    let usable = board.grid_mm.is_some() && !board.read_only;
    let mut chips = Wrap::new().spacing(SP.s).line_spacing(SP.xs);
    for (index, mm) in board.fast_grids_mm.iter().enumerate() {
        let mm = f64::from(*mm);
        let label = t!(
            form.i18n,
            ids::PCB_BOARD_MEASURE,
            "value" => length_field_text(mm, unit)
        );
        let in_use = board.grid_mm.is_some_and(|step| (step - mm).abs() < 1e-6);
        let press = usable.then_some(PanelMsg::PcbBoard(PcbBoardMsg::FastGridApplied(index)));
        let why = (!usable).then(|| form.t(ids::PCB_BOARD_GRID_NO_SETUP));
        chips = chips.push(form.chip(label, in_use, press, why));
    }
    chips.into()
}
