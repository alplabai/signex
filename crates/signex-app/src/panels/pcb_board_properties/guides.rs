//! Guide Manager (#2374): the board's snap guides. A row's Enabled box,
//! X / Y fields and swatch edit the guide at once; clicking the row selects
//! it for Delete. Add puts a guide through the origin, Place arms one for
//! the next click on the board.

use iced::widget::{container, row};
use iced::{Element, Widget as _};
use signex_i18n::{MessageId, ids};
use signex_types::pcb_options::{BoardGuide, GuideKind};

use super::chrome::{Form, Shown};
use super::fields::{self, Choice};
use super::{BoardField, PanelMsg, PcbBoardMsg, PcbBoardSummary, SP};

pub(super) fn section<'a>(
    form: &Form<'a>,
    board: &'a PcbBoardSummary,
) -> Option<Element<'a, PanelMsg>> {
    let title = form.t(ids::PCB_BOARD_SECTION_GUIDES);
    let headings = [
        ids::PCB_BOARD_GUIDE_ENABLED,
        ids::PCB_BOARD_GRID_NAME,
        ids::PCB_BOARD_GUIDE_X,
        ids::PCB_BOARD_GUIDE_Y,
        ids::PCB_BOARD_GRID_COLOR,
    ];
    let header = form.table_row(headings.iter().map(|id| form.heading(*id)).collect(), true);
    let mut rows: Vec<Shown<'a>> = vec![form.row(&[&title], header)];
    let editable = !board.read_only;
    for guide in &board.options.guides {
        rows.push(form.row(&[&title], guide_row(form, board, guide, editable)));
    }
    if board.options.guides.is_empty() {
        let empty = form.t(ids::PCB_BOARD_GUIDES_EMPTY);
        rows.push(form.row(&[&empty], form.note(empty.clone())));
    }
    if editable {
        rows.push(form.row(&[&title], buttons(form, board)));
    }
    form.section("pcb_board_guides", ids::PCB_BOARD_SECTION_GUIDES, rows)
}

pub(super) fn kind_label(kind: GuideKind) -> MessageId {
    match kind {
        GuideKind::Vertical => ids::PCB_BOARD_GUIDE_VERTICAL,
        GuideKind::Horizontal => ids::PCB_BOARD_GUIDE_HORIZONTAL,
        GuideKind::Plus45 => ids::PCB_BOARD_GUIDE_PLUS45,
        GuideKind::Minus45 => ids::PCB_BOARD_GUIDE_MINUS45,
    }
}

fn guide_row<'a>(
    form: &Form<'a>,
    board: &PcbBoardSummary,
    guide: &BoardGuide,
    editable: bool,
) -> Element<'a, PanelMsg> {
    let id = guide.uuid;
    let mut enabled =
        signex_widgets::checkbox(guide.enabled).style(super::super::form_look::checkbox_style);
    if editable {
        enabled =
            enabled.on_toggle(move |_| PanelMsg::PcbBoard(PcbBoardMsg::GuideEnabledToggled(id)));
    }
    let (x, y) = (
        form.t(ids::PCB_BOARD_GUIDE_X),
        form.t(ids::PCB_BOARD_GUIDE_Y),
    );
    let coord = |field: BoardField, label, mm: f64| {
        if editable {
            fields::length(form, board, field, label, mm)
        } else {
            form.value(fields::measure(form, mm))
        }
    };
    let swatch = fields::swatch(
        form,
        guide.color,
        PcbBoardMsg::GuideColorStepped(id),
        form.t(ids::PCB_BOARD_SWATCH_TIP),
    );
    let cells = vec![
        enabled.boxed(),
        form.value(form.t(kind_label(guide.kind))),
        coord(BoardField::GuideX(id), x, guide.at.x),
        coord(BoardField::GuideY(id), y, guide.at.y),
        swatch,
    ];
    let line = form.table_row(cells, false);
    let picked = board.view.selected_guide == Some(id);
    fields::selectable(form, line, picked, PcbBoardMsg::GuideSelected(id))
}

/// Add and Place, each a dropdown of guide kinds, and Delete for the
/// selected guide.
fn buttons<'a>(form: &Form<'a>, board: &PcbBoardSummary) -> Element<'a, PanelMsg> {
    let kinds = || {
        GuideKind::ALL
            .into_iter()
            .map(|kind| Choice {
                value: kind,
                label: form.t(kind_label(kind)).into_string(),
            })
            .collect::<Vec<_>>()
    };
    let add = fields::dropdown(
        kinds(),
        None,
        form.t(ids::PCB_BOARD_ADD),
        PcbBoardMsg::GuideAdded,
    );
    let place = fields::dropdown(
        kinds(),
        None,
        form.t(ids::PCB_BOARD_PLACE),
        PcbBoardMsg::GuidePlaceArmed,
    );
    let picked = board
        .view
        .selected_guide
        .is_some_and(|id| board.options.guides.iter().any(|g| g.uuid == id));
    let delete = form.action(
        form.t(ids::PCB_BOARD_DELETE),
        picked.then_some(PanelMsg::PcbBoard(PcbBoardMsg::GuideDeleted)),
        (!picked).then(|| form.t(ids::PCB_BOARD_SELECT_ROW_FIRST)),
    );
    container(row![add, place, delete].spacing(SP.s))
        .padding([SP.s, SP.l])
        .boxed()
}
