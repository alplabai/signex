//! Selection Filter (#2353): one chip per object kind. It decides what a
//! click, a box and the hover pick on the board; the header menu shows the
//! same chips.

use iced::{Element, Widget as _};
use iced_aw::Wrap;
use pcb_geom::{PickFilter, PickKind};
use signex_i18n::{I18n, Localized, MessageId, ids, t};

use super::chrome::Form;
use super::{PanelMsg, PcbBoardMsg, PcbBoardSummary, SP};

pub(super) fn section<'a>(
    form: &Form<'a>,
    board: &PcbBoardSummary,
) -> Option<Element<'a, PanelMsg>> {
    let labels: Vec<Localized> = PickKind::ALL
        .into_iter()
        .map(|kind| form.t(kind_label(kind)))
        .collect();
    let refs: Vec<&Localized> = labels.iter().collect();
    let chips = form.row(&refs, chips(form, &board.filter));
    form.section(
        "pcb_board_filter",
        ids::PCB_BOARD_SECTION_FILTER,
        vec![chips],
    )
}

/// The `All - On` chip, then one chip per kind. A kind the board editor
/// cannot pick is inert, with a tooltip saying so.
pub(super) fn chips<'a>(form: &Form<'a>, filter: &PickFilter) -> Element<'a, PanelMsg> {
    let msg = |m: PcbBoardMsg| Some(PanelMsg::PcbBoard(m));
    let mut wrap = Wrap::new().spacing(SP.s).line_spacing(SP.s).push(form.chip(
        form.t(ids::PCB_BOARD_FILTER_ALL_ON),
        filter.is_all_on(),
        msg(PcbBoardMsg::FilterAllToggled),
        None,
    ));
    for kind in PickKind::ALL {
        let label = form.t(kind_label(kind));
        let chip = if kind.is_pickable() {
            form.chip(
                label,
                filter.is_on(kind),
                msg(PcbBoardMsg::FilterKindToggled(kind)),
                None,
            )
        } else {
            let why = form.t(ids::PCB_BOARD_FILTER_UNAVAILABLE);
            form.chip(label, false, None, Some(why))
        };
        wrap = wrap.push(chip);
    }
    iced::widget::container(wrap).padding([SP.xs, SP.l]).boxed()
}

/// "Components (and 12 more)": the first kind that is on and how many more.
pub(super) fn summary(i18n: &I18n, filter: &PickFilter) -> Localized {
    let mut on = filter.on_kinds();
    let Some(first) = on.next() else {
        return t!(i18n, ids::PCB_BOARD_FILTER_SUMMARY_NONE);
    };
    let first = t!(i18n, kind_label(first)).into_string();
    t!(
        i18n,
        ids::PCB_BOARD_FILTER_SUMMARY,
        "first" => first,
        "more" => on.count()
    )
}

pub(crate) fn kind_label(kind: PickKind) -> MessageId {
    match kind {
        PickKind::Components => ids::PCB_BOARD_KIND_COMPONENTS,
        PickKind::Bodies3d => ids::PCB_BOARD_KIND_BODIES,
        PickKind::Keepouts => ids::PCB_BOARD_KIND_KEEPOUTS,
        PickKind::Tracks => ids::PCB_BOARD_KIND_TRACKS,
        PickKind::Arcs => ids::PCB_BOARD_KIND_ARCS,
        PickKind::Pads => ids::PCB_BOARD_KIND_PADS,
        PickKind::Vias => ids::PCB_BOARD_KIND_VIAS,
        PickKind::Regions => ids::PCB_BOARD_KIND_REGIONS,
        PickKind::Polygons => ids::PCB_BOARD_KIND_POLYGONS,
        PickKind::Fills => ids::PCB_BOARD_KIND_FILLS,
        PickKind::Texts => ids::PCB_BOARD_KIND_TEXTS,
        PickKind::Rooms => ids::PCB_BOARD_KIND_ROOMS,
        PickKind::Other => ids::PCB_BOARD_KIND_OTHER,
    }
}
