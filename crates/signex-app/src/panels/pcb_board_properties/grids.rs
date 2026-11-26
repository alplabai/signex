//! Grid Manager (#2353, #2374, #2386, #2523): Visible Grid as a switch,
//! then one card per grid. The board's own grid (the setup's step, counted
//! from the board origin) comes first, then the extra grids, highest
//! priority first: the order is the priority. A card shows its swatch, name,
//! step (typed in place) and a ⋯ menu (Edit, Move Up, Move Down, Delete; the
//! board's own grid has Edit only), and under them its origin and the Parts
//! and Other pills (the board's own grid: a lock and "applies to all"). Fast
//! Grids set the board's step to one of the fast grid preferences (#1672).
//! Each card, and Visible Grid, lights up with its part of the illustration.

use iced::widget::text::Wrapping;
use iced::widget::{Column, container, svg};
use iced::{Element, Length, Theme, Widget as _};
use iced_aw::Wrap;
use signex_i18n::{Localized, ids, t};
use signex_types::measure::length_field_text;
use signex_types::pcb_options::BoardGrid;
use signex_widgets::{ToggleSwitchStyle, button, text, toggle_switch};

use super::super::form_look;
use super::chrome::{Form, Head, Shown};
use super::drawing::{SNAP_STEP, VISIBLE_GRID, grid_field};
use super::info::shown_unit;
use super::{BoardField, GridCard, PanelMsg, PcbBoardMsg, PcbBoardSummary, SP, cards, fields};
use crate::styles::tokens::resolve;
use crate::styles::{PillTone, TypeScale, ti};

const TS: TypeScale = TypeScale::DEFAULT;

const GLYPH_LOCK: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><rect x="3.5" y="7" width="9" height="6.5" rx="1.2" fill="none" stroke="currentColor" stroke-width="1.4"/><path d="M5.5 7V5.2a2.5 2.5 0 0 1 5 0V7" fill="none" stroke="currentColor" stroke-width="1.4"/></svg>"#;

fn msg(m: PcbBoardMsg) -> Option<PanelMsg> {
    Some(PanelMsg::PcbBoard(m))
}

pub(super) fn section<'a>(
    form: &Form<'a>,
    board: &'a PcbBoardSummary,
) -> Option<Element<'a, PanelMsg>> {
    let mut rows: Vec<Shown<'a>> = vec![visible_grid(form)];
    rows.push(global_card(form, board));
    let count = board.options.grids.len();
    for (index, grid) in board.options.grids.iter().enumerate() {
        rows.push(grid_card(form, board, index, count, grid));
    }

    let fast = form.t(ids::PCB_BOARD_GRID_FAST);
    rows.push(form.row(&[&fast], form.field(&fast, fast_grids(form, board))));

    let add = form.t(ids::PCB_BOARD_GRID_ADD);
    let press = (!board.read_only)
        .then_some(PcbBoardMsg::GridAdded)
        .and_then(msg);
    rows.push(form.row(&[&add], add_row(form, add.clone(), press)));
    let grids = t!(form.i18n, ids::PCB_BOARD_COUNT, "count" => count + 1);
    form.section_with(
        "pcb_board_grids",
        ids::PCB_BOARD_SECTION_GRIDS,
        Head::open().summary(form.pill(grids, PillTone::Neutral)),
        rows,
    )
}

/// Visible Grid: the switch of the canvas grid; it also shows the grid
/// lines in the illustration.
fn visible_grid<'a>(form: &Form<'a>) -> Shown<'a> {
    let label = form.t(ids::PCB_BOARD_GRID_VISIBLE);
    let tokens = form.tokens();
    let r = resolve(tokens);
    let style = ToggleSwitchStyle::new(ti(tokens.accent), r.line_strong, r.primary_ink);
    let switch = toggle_switch(form.ctx.grid_visible, style, Some(PanelMsg::ToggleGrid));
    let row = form.field_lit(&label, switch, form.is_hot(&VISIBLE_GRID));
    form.row(&[&label], form.hover_zone(VISIBLE_GRID, row))
}

/// "+ Add Grid": an accent link row the width of the section.
fn add_row<'a>(
    form: &Form<'a>,
    label: Localized,
    press: Option<PanelMsg>,
) -> Element<'a, PanelMsg> {
    let ink = if press.is_some() {
        form.accent()
    } else {
        form.faint()
    };
    let row = button(
        text(label)
            .size(TS.body)
            .color(ink)
            .wrapping(Wrapping::None),
    )
    .padding([SP.s, SP.s])
    .width(Length::Fill)
    .on_press_maybe(press)
    .style(form_look::ghost_button);
    container(row)
        .padding([0.0, SP.l])
        .width(Length::Fill)
        .boxed()
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

/// 'Origin' and the point in the mono face, as a card's second line starts.
fn origin_line<'a>(form: &Form<'a>, at: Localized) -> Vec<Element<'a, PanelMsg>> {
    vec![
        cards::muted(form, form.t(ids::PCB_BOARD_GRID_ORIGIN)),
        text(at)
            .size(TS.caption)
            .font(super::chrome::mono())
            .color(form.muted())
            .wrapping(Wrapping::None)
            .boxed(),
    ]
}

/// The step field and the unit after it, beside a card's name.
fn step<'a>(form: &Form<'a>, field: Element<'a, PanelMsg>) -> Element<'a, PanelMsg> {
    let unit = t!(form.i18n, ids::PCB_BOARD_MEASURE, "value" => shown_unit(form).to_string());
    container(cards::line(vec![field, cards::muted(form, unit)]))
        .width(Length::FillPortion(2))
        .boxed()
}

/// A card's name in bold, taking the room the step leaves.
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
) -> Vec<Element<'a, PanelMsg>> {
    vec![
        cards::pill(
            form,
            form.t(ids::PCB_BOARD_GRID_PARTS),
            parts.0,
            parts.1,
            None,
        ),
        cards::pill(
            form,
            form.t(ids::PCB_BOARD_GRID_OTHER),
            other.0,
            other.1,
            None,
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
    let faint = form.faint();
    let lock = svg(crate::icons::static_glyph(GLYPH_LOCK))
        .width(TS.caption)
        .height(TS.caption)
        .style(move |_: &Theme, _| svg::Style {
            color: Some(faint),
            ..Default::default()
        });
    let mut second = vec![lock.boxed()];
    second.extend(origin_line(form, origin_text(form, 0.0, 0.0)));
    second.push(iced::widget::Space::new().width(Length::Fill).boxed());
    second.push(form.tip(
        cards::muted(form, form.t(ids::PCB_BOARD_GRID_APPLIES_ALL)),
        form.t(ids::PCB_BOARD_GRID_FIXED_TIP),
    ));
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
    let shown = form.card_lit(body.boxed(), form.is_hot(&SNAP_STEP));
    form.row(&[&label], form.hover_zone(SNAP_STEP, shown))
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
    let mut second = origin_line(form, at);
    second.push(iced::widget::Space::new().width(Length::Fill).boxed());
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
    let field = grid_field(id);
    let shown = form.card_lit(body.boxed(), form.is_hot(&field));
    form.row(&[&title, &key], form.hover_zone(field, shown))
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
    Element::from(chips)
}
