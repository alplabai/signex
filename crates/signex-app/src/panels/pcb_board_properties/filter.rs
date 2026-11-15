//! Selection Filter (#2353): one chip per object kind. It decides what a
//! click, a box and the hover pick on the board; the header menu shows the
//! same chips. Under them, the saved presets (#2383).

use iced::widget::{Column, container, row};
use iced::{Alignment, Element, Length, Widget as _};
use iced_aw::Wrap;
use pcb_geom::{PickFilter, PickKind};
use signex_i18n::{Localized, MessageId, UserText, ids, t};
use signex_widgets::{Label, text_input};

use crate::active_bar::CUSTOM_FILTER_PRESET_LIMIT;
use crate::panels::form_look;
use crate::styles::TypeScale;

use super::chrome::Form;
use super::{PanelMsg, PcbBoardMsg, PcbBoardSummary, SP};

const TS: TypeScale = TypeScale::DEFAULT;

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
    let names: Vec<Localized> = [
        ids::PCB_BOARD_FILTER_PRESETS,
        ids::PCB_BOARD_FILTER_PRESET_ADD,
    ]
    .into_iter()
    .map(|id| form.t(id))
    .collect();
    let name_refs: Vec<&Localized> = names.iter().collect();
    let caption = form.row(
        &name_refs,
        form.caption(form.t(ids::PCB_BOARD_FILTER_PRESETS)),
    );
    let saved = form.row(&name_refs, presets(form, board));
    form.section(
        "pcb_board_filter",
        ids::PCB_BOARD_SECTION_FILTER,
        vec![chips, caption, saved],
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

/// The saved presets (#2383): a chip per preset that applies it, Add, and
/// for the preset last applied or added its name, Save Chips and Delete.
fn presets<'a>(form: &Form<'a>, board: &PcbBoardSummary) -> Element<'a, PanelMsg> {
    let msg = |m: PcbBoardMsg| Some(PanelMsg::PcbBoard(m));
    let mut wrap = Wrap::new().spacing(SP.s).line_spacing(SP.s);
    for (idx, preset) in board.presets.iter().enumerate() {
        let label: Label<'a> = if preset.name.trim().is_empty() {
            t!(form.i18n, ids::PANELS_FILTERS_UNNAMED, "number" => idx + 1).into()
        } else {
            UserText::new(preset.name.clone()).into()
        };
        let applied = PickKind::ALL
            .into_iter()
            .all(|kind| board.filter.is_on(kind) == preset.filters.contains(&kind));
        let chip = form.chip(
            label,
            applied,
            msg(PcbBoardMsg::FilterPresetApplied(idx)),
            None,
        );
        wrap = wrap.push(form.tip(chip, form.t(ids::PCB_BOARD_FILTER_PRESET_APPLY_TIP)));
    }
    let full = board.presets.len() >= CUSTOM_FILTER_PRESET_LIMIT;
    let add = if full {
        form.action(
            form.t(ids::PCB_BOARD_FILTER_PRESET_ADD),
            None,
            Some(t!(form.i18n, ids::PCB_BOARD_FILTER_PRESET_FULL, "limit" => CUSTOM_FILTER_PRESET_LIMIT)),
        )
    } else {
        form.action(
            form.t(ids::PCB_BOARD_FILTER_PRESET_ADD),
            msg(PcbBoardMsg::FilterPresetAdded),
            Some(form.t(ids::PCB_BOARD_FILTER_PRESET_ADD_TIP)),
        )
    };
    let mut column = Column::new().spacing(SP.s).push(wrap.push(add));
    let tab = board.preset_tab.min(board.presets.len().saturating_sub(1));
    if let Some(preset) = board.presets.get(tab) {
        let name = text_input(
            form.t(ids::PCB_BOARD_FILTER_PRESET_NAME),
            preset.name.clone(),
        )
        .size(TS.body)
        .padding([SP.xs, SP.s])
        .on_input(move |name| PanelMsg::PcbBoard(PcbBoardMsg::FilterPresetRenamed(tab, name)))
        .style(form_look::text_input_style)
        .width(Length::Fill);
        let edit = row![
            name,
            form.action(
                form.t(ids::PCB_BOARD_FILTER_PRESET_UPDATE),
                msg(PcbBoardMsg::FilterPresetUpdated(tab)),
                Some(form.t(ids::PCB_BOARD_FILTER_PRESET_UPDATE_TIP)),
            ),
            form.action(
                form.t(ids::PCB_BOARD_DELETE),
                msg(PcbBoardMsg::FilterPresetRemoved(tab)),
                None,
            ),
        ]
        .spacing(SP.s)
        .align_y(Alignment::Center);
        column = column.push(edit);
    }
    container(column).padding([SP.xs, SP.l]).boxed()
}
