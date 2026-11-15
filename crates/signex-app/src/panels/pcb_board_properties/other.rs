//! Other (#2353, #2374): the units the panel and canvas show lengths in,
//! the polygon naming scheme, the designator display and the sheet. All
//! but the units are board options, each edit one undo step.

use iced::Element;
use signex_i18n::{MessageId, ids};
use signex_types::coord::Unit;
use signex_types::pcb_options::{DesignatorDisplay, PolygonNaming};

use super::chrome::{Form, Segment, Shown};
use super::fields::{self, Choice};
use super::info::shown_unit;
use super::{BoardField, PanelMsg, PcbBoardMsg, PcbBoardSummary};

fn msg(m: PcbBoardMsg) -> Option<PanelMsg> {
    Some(PanelMsg::PcbBoard(m))
}

pub(super) fn section<'a>(
    form: &Form<'a>,
    board: &'a PcbBoardSummary,
) -> Option<Element<'a, PanelMsg>> {
    let mut rows: Vec<Shown<'a>> = Vec::new();
    let unit = shown_unit(form);
    let units = form.t(ids::PCB_BOARD_UNITS);
    let toggle = form.segmented(vec![
        Segment {
            label: form.t(ids::PCB_BOARD_UNIT_MM),
            active: unit == Unit::Mm,
            msg: Some(PanelMsg::SetUnit(Unit::Mm)),
        },
        Segment {
            label: form.t(ids::PCB_BOARD_UNIT_MILS),
            active: unit == Unit::Mil,
            msg: Some(PanelMsg::SetUnit(Unit::Mil)),
        },
    ]);
    rows.push(form.row(&[&units], form.field(&units, toggle)));

    let options = &board.options;
    let naming = form.t(ids::PCB_BOARD_POLYGON_NAMING);
    let schemes = PolygonNaming::ALL
        .into_iter()
        .map(|value| Choice {
            value,
            label: form.t(naming_label(value)).into_string(),
        })
        .collect();
    let pick = fields::dropdown(
        schemes,
        Some(options.polygon_naming),
        naming.clone(),
        PcbBoardMsg::PolygonNamingPicked,
    );
    rows.push(form.row(&[&naming], form.field(&naming, pick)));

    let designators = form.t(ids::PCB_BOARD_DESIGNATOR_DISPLAY);
    let display = |value, label: MessageId| Segment {
        label: form.t(label),
        active: options.designator_display == value,
        msg: msg(PcbBoardMsg::DesignatorDisplayPicked(value)),
    };
    let toggle = form.segmented(vec![
        display(
            DesignatorDisplay::Physical,
            ids::PCB_BOARD_DESIGNATOR_PHYSICAL,
        ),
        display(
            DesignatorDisplay::Logical,
            ids::PCB_BOARD_DESIGNATOR_LOGICAL,
        ),
    ]);
    rows.push(form.row(&[&designators], form.field(&designators, toggle)));

    sheet_rows(form, board, &mut rows);
    form.section("pcb_board_other", ids::PCB_BOARD_SECTION_OTHER, rows)
}

/// Sheet Settings: X, Y, Width and Height, greyed while Get Size From
/// Sheet Layer is on (the sheet is then the box around the board shape).
fn sheet_rows<'a>(form: &Form<'a>, board: &PcbBoardSummary, rows: &mut Vec<Shown<'a>>) {
    let sheet = &board.options.sheet;
    let caption = form.t(ids::PCB_BOARD_SHEET);
    rows.push(form.row(&[&caption], form.caption(caption.clone())));
    let fixed = sheet.from_board || board.read_only;
    let values = [
        (ids::PCB_BOARD_GUIDE_X, BoardField::SheetX, sheet.origin.x),
        (ids::PCB_BOARD_GUIDE_Y, BoardField::SheetY, sheet.origin.y),
        (
            ids::PCB_BOARD_SHEET_WIDTH,
            BoardField::SheetWidth,
            sheet.width,
        ),
        (
            ids::PCB_BOARD_SHEET_HEIGHT,
            BoardField::SheetHeight,
            sheet.height,
        ),
    ];
    for (label, field, mm) in values {
        let label = form.t(label);
        let value = if fixed {
            let shown = fields::measure(form, mm);
            form.unavailable(shown, form.t(ids::PCB_BOARD_SHEET_FROM_LAYER_TIP))
        } else {
            fields::length(form, board, field, label.clone(), mm)
        };
        rows.push(form.row(&[&caption, &label], form.field(&label, value)));
    }
    let from_layer = form.t(ids::PCB_BOARD_SHEET_FROM_LAYER);
    let toggle = (!board.read_only).then_some(PcbBoardMsg::SheetFromBoardToggled);
    let check = form.check(&from_layer, sheet.from_board, toggle.and_then(msg), None);
    rows.push(form.row(&[&caption, &from_layer], check));
}

fn naming_label(naming: PolygonNaming) -> MessageId {
    match naming {
        PolygonNaming::NetLayerPour => ids::PCB_BOARD_NAMING_NET_LAYER,
        PolygonNaming::LayerNetPour => ids::PCB_BOARD_NAMING_LAYER_NET,
        PolygonNaming::BoardLayerNetPour => ids::PCB_BOARD_NAMING_BOARD_LAYER_NET,
        PolygonNaming::NetBoardLayerPour => ids::PCB_BOARD_NAMING_NET_BOARD_LAYER,
    }
}
