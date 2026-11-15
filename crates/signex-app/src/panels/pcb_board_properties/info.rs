//! Board Information (#2353): sizes and counts read from the board. The
//! component counts select those components; the DRC count opens the
//! design rule check results.

use iced::{Element, Widget as _};
use signex_i18n::{LocalNumber, Localized, MessageId, ids, t};
use signex_types::coord::Unit;
use signex_types::measure::{format_length_mm, mm_per};
use signex_widgets::Label;

use super::chrome::{Form, Shown};
use super::{ComponentSide, PanelMsg, PcbBoardMsg, PcbBoardSummary};

pub(super) fn section<'a>(
    form: &Form<'a>,
    board: &PcbBoardSummary,
) -> Option<Element<'a, PanelMsg>> {
    let mut rows: Vec<Shown<'a>> = Vec::new();
    board_size(form, board, &mut rows);
    components(form, board, &mut rows);
    layers_and_nets(form, board, &mut rows);
    primitives(form, board, &mut rows);
    let reports = form.t(ids::PCB_BOARD_REPORTS);
    let button = form.action(
        reports.clone(),
        None,
        Some(form.t(ids::PCB_BOARD_REPORTS_UNAVAILABLE)),
    );
    let button = iced::widget::container(button)
        .padding([super::SP.s, super::SP.l])
        .boxed();
    rows.push(form.row(&[&reports], button));
    form.section("pcb_board_info", ids::PCB_BOARD_SECTION_INFO, rows)
}

/// A caption row over a group, found by the caption.
fn group<'a>(form: &Form<'a>, rows: &mut Vec<Shown<'a>>, caption: MessageId) {
    let caption = form.t(caption);
    rows.push(form.row(&[&caption], form.caption(caption.clone())));
}

/// Label | read-only value, found by its label and its group's caption.
fn value_row<'a>(
    form: &Form<'a>,
    rows: &mut Vec<Shown<'a>>,
    caption: MessageId,
    label: MessageId,
    value: impl Into<Label<'a>>,
) {
    let (caption, label) = (form.t(caption), form.t(label));
    let field = form.field(&label, form.value(value));
    rows.push(form.row(&[&caption, &label], field));
}

fn count(form: &Form<'_>, n: usize) -> Localized {
    t!(form.i18n, ids::PCB_BOARD_COUNT, "count" => n)
}

fn length(form: &Form<'_>, mm: f64) -> Localized {
    measure(form, format_length_mm(mm, shown_unit(form)))
}

fn area(form: &Form<'_>, mm2: f64) -> Localized {
    let unit = shown_unit(form);
    let per = mm_per(unit);
    measure(form, format!("{:.2} {unit}²", mm2 / (per * per)))
}

/// A length or area from Signex's own formatter, unit included.
fn measure(form: &Form<'_>, text: String) -> Localized {
    t!(form.i18n, ids::PCB_BOARD_MEASURE, "value" => text)
}

/// The panel's unit (`super::shown_unit`).
pub(super) fn shown_unit(form: &Form<'_>) -> Unit {
    super::shown_unit(form.ctx.unit)
}

fn board_size<'a>(form: &Form<'a>, board: &PcbBoardSummary, rows: &mut Vec<Shown<'a>>) {
    let caption = ids::PCB_BOARD_INFO_SIZE;
    group(form, rows, caption);
    match board.size_mm {
        Some((w, h)) => {
            value_row(
                form,
                rows,
                caption,
                ids::PCB_BOARD_INFO_HORIZONTAL,
                length(form, w),
            );
            value_row(
                form,
                rows,
                caption,
                ids::PCB_BOARD_INFO_VERTICAL,
                length(form, h),
            );
        }
        None => {
            let none = form.t(ids::PCB_PROPERTIES_NO_OUTLINE);
            value_row(
                form,
                rows,
                caption,
                ids::PCB_BOARD_INFO_HORIZONTAL,
                none.clone(),
            );
            value_row(form, rows, caption, ids::PCB_BOARD_INFO_VERTICAL, none);
        }
    }
    let stats = &board.stats;
    if let Some(a) = stats.area_mm2 {
        value_row(form, rows, caption, ids::PCB_BOARD_INFO_AREA, area(form, a));
    }
    value_row(
        form,
        rows,
        caption,
        ids::PCB_BOARD_INFO_COMPONENTS_AREA,
        area(form, stats.components_area_mm2),
    );
    if let Some(density) = stats.density() {
        let percent = t!(
            form.i18n,
            ids::PCB_BOARD_PERCENT,
            "value" => LocalNumber((density * 10_000.0).round() / 100.0)
        );
        value_row(form, rows, caption, ids::PCB_BOARD_INFO_DENSITY, percent);
    }
    value_row(
        form,
        rows,
        caption,
        ids::PCB_PROPERTIES_THICKNESS,
        length(form, board.thickness_mm),
    );
}

fn components<'a>(form: &Form<'a>, board: &PcbBoardSummary, rows: &mut Vec<Shown<'a>>) {
    let caption = ids::PCB_BOARD_INFO_COMPONENTS;
    group(form, rows, caption);
    let stats = &board.stats;
    for (label, n, side) in [
        (
            ids::PCB_BOARD_INFO_TOTAL,
            stats.components(),
            ComponentSide::All,
        ),
        (
            ids::PCB_BOARD_INFO_TOP,
            stats.components_top,
            ComponentSide::Top,
        ),
        (
            ids::PCB_BOARD_INFO_BOTTOM,
            stats.components_bottom,
            ComponentSide::Bottom,
        ),
    ] {
        let (cap, label) = (form.t(caption), form.t(label));
        let value = if n > 0 {
            form.link(
                count(form, n),
                PanelMsg::PcbBoard(PcbBoardMsg::ComponentsSelected(side)),
                form.t(ids::PCB_BOARD_INFO_SELECT_TIP),
            )
        } else {
            form.value(count(form, n))
        };
        rows.push(form.row(&[&cap, &label], form.field(&label, value)));
    }
}

fn layers_and_nets<'a>(form: &Form<'a>, board: &PcbBoardSummary, rows: &mut Vec<Shown<'a>>) {
    let layers = ids::PCB_BOARD_INFO_LAYERS;
    group(form, rows, layers);
    value_row(
        form,
        rows,
        layers,
        ids::PCB_BOARD_INFO_TOTAL,
        count(form, board.layers),
    );
    value_row(
        form,
        rows,
        layers,
        ids::PCB_BOARD_INFO_SIGNAL,
        count(form, board.copper_layers),
    );
    let nets = ids::PCB_BOARD_INFO_NETS;
    group(form, rows, nets);
    // The PCB panel's net count, so the two always agree.
    let total = form.ctx.pcb_nets.as_ref().map_or(0, |s| s.nets.len());
    value_row(
        form,
        rows,
        nets,
        ids::PCB_BOARD_INFO_TOTAL,
        count(form, total),
    );
    let unrouted = match board.unrouted_nets {
        Some(n) => count(form, n),
        None => form.t(ids::PCB_BOARD_INFO_NOT_COUNTED),
    };
    value_row(form, rows, nets, ids::PCB_BOARD_INFO_UNROUTED, unrouted);
    value_row(
        form,
        rows,
        nets,
        ids::PCB_BOARD_INFO_HIDDEN,
        count(form, board.hidden_nets),
    );
}

fn primitives<'a>(form: &Form<'a>, board: &PcbBoardSummary, rows: &mut Vec<Shown<'a>>) {
    let caption = ids::PCB_BOARD_INFO_PRIMITIVES;
    group(form, rows, caption);
    for label in [ids::PCB_BOARD_INFO_ARCS, ids::PCB_BOARD_INFO_FILLS] {
        let (cap, label) = (form.t(caption), form.t(label));
        let value = form.unavailable(
            form.t(ids::PCB_BOARD_NOT_AVAILABLE),
            form.t(ids::PCB_BOARD_INFO_NOT_HELD),
        );
        rows.push(form.row(&[&cap, &label], form.field(&label, value)));
    }
    let stats = &board.stats;
    for (label, n) in [
        (ids::PCB_BOARD_INFO_PADS, stats.pads),
        (ids::PCB_BOARD_INFO_STRINGS, stats.strings),
        (ids::PCB_BOARD_INFO_TRACKS, stats.tracks),
        (ids::PCB_BOARD_INFO_VIAS, stats.vias),
        (ids::PCB_BOARD_INFO_POLYGONS, stats.polygons),
        (ids::PCB_BOARD_INFO_HOLES, stats.pad_holes + stats.via_holes),
    ] {
        value_row(form, rows, caption, label, count(form, n));
    }
    let (cap, label) = (form.t(caption), form.t(ids::PCB_BOARD_INFO_DRC));
    let value = match drc_violations(form) {
        Some(n) => form.link(
            count(form, n),
            PanelMsg::PcbBoard(PcbBoardMsg::DrcResultsOpened),
            form.t(ids::PCB_BOARD_INFO_DRC_TIP),
        ),
        None => form.value(form.t(ids::PCB_BOARD_INFO_DRC_NOT_RUN)),
    };
    rows.push(form.row(&[&cap, &label], form.field(&label, value)));
}

/// The violations of the board's last design rule check; `None` before one.
pub(super) fn drc_violations(form: &Form<'_>) -> Option<usize> {
    let drc = form.ctx.pcb_drc.as_ref().filter(|d| d.ran)?;
    Some(drc.rows.len() + drc.hidden)
}
