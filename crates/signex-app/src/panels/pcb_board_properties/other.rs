//! Other (#2353, #2374): the units the panel and canvas show lengths in,
//! the designator display, the pour names and the sheet card (#2386). All
//! but the units are board options, each edit one undo step. Last, the link
//! to the Layer Stack, where the board materials live (#2523).

use iced::widget::Column;
use iced::{Element, Widget as _};
use signex_i18n::{MessageId, ids};
use signex_types::coord::Unit;
use signex_types::pcb_layer::CopperLayer;
use signex_types::pcb_options::{DesignatorDisplay, PolygonNaming};

use super::chrome::{Form, Segment, Shown};
use super::fields::{self, Choice};
use super::info::shown_unit;
use super::{BoardField, PanelMsg, PcbBoardMsg, PcbBoardSummary, SP, cards};

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

    let naming = form.t(ids::PCB_BOARD_POLYGON_NAMING);
    let schemes = PolygonNaming::ALL
        .into_iter()
        .map(|value| Choice {
            value,
            label: example_name(form, value),
        })
        .collect();
    let pick = fields::dropdown(
        schemes,
        Some(options.polygon_naming),
        naming.clone(),
        PcbBoardMsg::PolygonNamingPicked,
    );
    rows.push(form.row(&[&naming], form.field(&naming, pick)));

    rows.push(sheet_card(form, board));
    rows.push(materials(form));
    form.section("pcb_board_other", ids::PCB_BOARD_SECTION_OTHER, rows)
}

/// Board materials: solder mask, silkscreen, surface finish, copper weights
/// and materials live in the Layer Stack (#2523, #176); the row links there.
fn materials<'a>(form: &Form<'a>) -> Shown<'a> {
    let label = form.t(ids::PCB_BOARD_MATERIALS);
    let link_text = form.t(ids::PCB_BOARD_MATERIALS_LINK);
    let link = form.text_link(
        link_text.clone(),
        PanelMsg::PcbBoard(PcbBoardMsg::LayerStackOpened),
        form.t(ids::PCB_BOARD_MATERIALS_TIP),
    );
    form.row(&[&label, &link_text], form.field(&label, link))
}

/// A pour scheme as the name it gives the first pour of GND on the top
/// copper layer, which reads better than the pattern.
fn example_name(form: &Form<'_>, naming: PolygonNaming) -> String {
    let top = crate::pcb_canvas::layer_style::copper_name(form.i18n, CopperLayer::Top);
    naming.name(EXAMPLE_NET, 1, &top.into_string(), 1)
}

/// The net the pour-name examples use.
const EXAMPLE_NET: &str = "GND";

/// The Sheet card (#2386): Size From Board first; X and Y (the sheet's
/// bottom-left corner, from the board origin with Y up, #2384), then Width
/// and Height, as two pairs. With Size From Board on they are read-only and
/// muted (the sheet is the box around the board shape); off, they edit.
fn sheet_card<'a>(form: &Form<'a>, board: &PcbBoardSummary) -> Shown<'a> {
    let origin = board.options.origin;
    let sheet = &board.options.sheet;
    let caption = form.t(ids::PCB_BOARD_SHEET);
    let from_board = form.t(ids::PCB_BOARD_SHEET_FROM_LAYER);
    let toggle = (!board.read_only).then_some(PcbBoardMsg::SheetFromBoardToggled);
    let fixed = sheet.from_board || board.read_only;
    let cell = |label: MessageId, field: BoardField, mm: f64| {
        let label = form.t(label);
        let value = if fixed {
            cards::muted(form, fields::measure(form, mm))
        } else {
            fields::length(form, board, field, label.clone(), mm)
        };
        cards::labelled(form, label, value)
    };
    let corner = cards::line(vec![
        cell(
            ids::PCB_BOARD_GUIDE_X,
            BoardField::SheetX,
            origin.user_x(sheet.origin.x),
        ),
        cell(
            ids::PCB_BOARD_GUIDE_Y,
            BoardField::SheetY,
            origin.user_y(sheet.origin.y + sheet.height),
        ),
    ]);
    let size = cards::line(vec![
        cell(
            ids::PCB_BOARD_SHEET_WIDTH,
            BoardField::SheetWidth,
            sheet.width,
        ),
        cell(
            ids::PCB_BOARD_SHEET_HEIGHT,
            BoardField::SheetHeight,
            sheet.height,
        ),
    ]);
    let body = Column::new()
        .spacing(SP.s)
        .push(cards::muted(form, caption.clone()))
        .push(cards::line(vec![cards::pill(
            form,
            from_board.clone(),
            sheet.from_board,
            toggle.and_then(msg),
            None,
        )]))
        .push(corner)
        .push(size);
    form.row(&[&caption, &from_board], form.card(body.boxed()))
}
