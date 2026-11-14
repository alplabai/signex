//! Other (#2353): the units the panel and canvas show lengths in. Polygon
//! naming, designator display and sheet settings are not in the board model
//! yet, so they show their defaults inert with a tooltip saying why.

use iced::Element;
use signex_i18n::ids;
use signex_types::coord::Unit;

use super::PanelMsg;
use super::chrome::{Form, Segment, Shown};
use super::info::shown_unit;

pub(super) fn section<'a>(form: &Form<'a>) -> Option<Element<'a, PanelMsg>> {
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

    let naming = form.t(ids::PCB_BOARD_POLYGON_NAMING);
    let value = form.unavailable(
        form.t(ids::PCB_BOARD_POLYGON_NAMING_DEFAULT),
        form.t(ids::PCB_BOARD_POLYGON_NAMING_UNAVAILABLE),
    );
    rows.push(form.row(&[&naming], form.field(&naming, value)));

    let designators = form.t(ids::PCB_BOARD_DESIGNATOR_DISPLAY);
    let value = form.unavailable(
        form.t(ids::PCB_BOARD_DESIGNATOR_PHYSICAL),
        form.t(ids::PCB_BOARD_DESIGNATOR_UNAVAILABLE),
    );
    rows.push(form.row(&[&designators], form.field(&designators, value)));

    let sheet = form.t(ids::PCB_BOARD_SHEET);
    rows.push(form.row(&[&sheet], form.caption(sheet.clone())));
    let why = || form.t(ids::PCB_BOARD_SHEET_UNAVAILABLE);
    for label in [
        ids::PCB_BOARD_GUIDE_X,
        ids::PCB_BOARD_GUIDE_Y,
        ids::PCB_BOARD_SHEET_WIDTH,
        ids::PCB_BOARD_SHEET_HEIGHT,
    ] {
        let label = form.t(label);
        let value = form.unavailable(form.t(ids::PCB_BOARD_UNSET), why());
        rows.push(form.row(&[&sheet, &label], form.field(&label, value)));
    }
    let from_layer = form.t(ids::PCB_BOARD_SHEET_FROM_LAYER);
    let check = form.check(&from_layer, true, None, Some(why()));
    rows.push(form.row(&[&sheet, &from_layer], check));

    form.section("pcb_board_other", ids::PCB_BOARD_SECTION_OTHER, rows)
}
