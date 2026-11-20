//! Guide Manager (#2374, #2386): one card per guide, as the Grid Manager's.
//! A card shows the guide's swatch, direction glyph and name, its enable
//! box and a ⋯ menu (Delete), and under them its position, typed in place:
//! X for a vertical guide, Y for a horizontal one, both for a diagonal.
//! "+ Add Guide" opens a dropdown of the four directions, which add a guide
//! through the board origin, and Place on Board, which makes them arm one
//! for the next click on the board instead.

use iced::widget::{Column, container};
use iced::{Element, Length, Widget as _};
use signex_i18n::{MessageId, ids, t};
use signex_types::pcb_options::{BoardGuide, GuideKind};

use super::chrome::{Form, Head, Shown};
use super::{BoardField, PanelMsg, PcbBoardMsg, PcbBoardSummary, SP, cards, fields};
use crate::styles::PillTone;

fn msg(m: PcbBoardMsg) -> Option<PanelMsg> {
    Some(PanelMsg::PcbBoard(m))
}

pub(super) fn section<'a>(
    form: &Form<'a>,
    board: &'a PcbBoardSummary,
) -> Option<Element<'a, PanelMsg>> {
    let title = form.t(ids::PCB_BOARD_SECTION_GUIDES);
    let mut rows: Vec<Shown<'a>> = Vec::new();
    for guide in &board.options.guides {
        let name = form.t(kind_label(guide.kind));
        rows.push(form.row(&[&title, &name], guide_card(form, board, guide)));
    }
    if board.options.guides.is_empty() {
        let empty = form.t(ids::PCB_BOARD_GUIDES_EMPTY);
        rows.push(form.row(&[&title, &empty], form.note(empty.clone())));
    }
    if !board.read_only {
        let add = form.t(ids::PCB_BOARD_GUIDE_ADD);
        rows.push(form.row(&[&title, &add], add_menu(form, board)));
    }
    // Closed by default, with the guide count in its header (#2523, mockup row 93).
    let count = t!(form.i18n, ids::PCB_BOARD_COUNT, "count" => board.options.guides.len());
    form.section_with(
        "pcb_board_guides",
        ids::PCB_BOARD_SECTION_GUIDES,
        Head::closed().summary(form.pill(count, PillTone::Neutral)),
        rows,
    )
}

pub(super) fn kind_label(kind: GuideKind) -> MessageId {
    match kind {
        GuideKind::Vertical => ids::PCB_BOARD_GUIDE_VERTICAL,
        GuideKind::Horizontal => ids::PCB_BOARD_GUIDE_HORIZONTAL,
        GuideKind::Plus45 => ids::PCB_BOARD_GUIDE_PLUS45,
        GuideKind::Minus45 => ids::PCB_BOARD_GUIDE_MINUS45,
    }
}

/// The glyph drawn before a guide's name: the way its line runs.
fn glyph(kind: GuideKind) -> MessageId {
    match kind {
        GuideKind::Vertical => ids::PCB_BOARD_GUIDE_GLYPH_VERTICAL,
        GuideKind::Horizontal => ids::PCB_BOARD_GUIDE_GLYPH_HORIZONTAL,
        GuideKind::Plus45 => ids::PCB_BOARD_GUIDE_GLYPH_PLUS45,
        GuideKind::Minus45 => ids::PCB_BOARD_GUIDE_GLYPH_MINUS45,
    }
}

fn guide_card<'a>(
    form: &Form<'a>,
    board: &PcbBoardSummary,
    guide: &BoardGuide,
) -> Element<'a, PanelMsg> {
    let id = guide.uuid;
    let editable = !board.read_only;
    let live = |m: PcbBoardMsg| editable.then_some(m).and_then(msg);
    let mut enabled =
        signex_widgets::checkbox(guide.enabled).style(super::super::form_look::checkbox_style);
    if editable {
        enabled =
            enabled.on_toggle(move |_| PanelMsg::PcbBoard(PcbBoardMsg::GuideEnabledToggled(id)));
    }
    let swatch = if editable {
        fields::swatch(
            form,
            guide.color,
            PcbBoardMsg::GuideColorStepped(id),
            form.t(ids::PCB_BOARD_SWATCH_TIP),
        )
    } else {
        cards::no_swatch()
    };
    let header = cards::line(vec![
        swatch,
        cards::muted(form, form.t(glyph(guide.kind))),
        cards::name(form, form.t(kind_label(guide.kind))),
        form.tip(enabled.boxed(), form.t(ids::PCB_BOARD_GUIDE_ENABLED)),
        cards::more(form, live(PcbBoardMsg::GuideMenuToggled(id))),
    ]);
    // From the board origin, Y up (#2384).
    let origin = board.options.origin;
    let coord = |field: BoardField, label: MessageId, mm: f64| {
        let label = form.t(label);
        let value = if editable {
            fields::length(form, board, field, label.clone(), mm)
        } else {
            form.value(fields::measure(form, mm))
        };
        cards::labelled(form, label, value)
    };
    let x = || {
        coord(
            BoardField::GuideX(id),
            ids::PCB_BOARD_GUIDE_X,
            origin.user_x(guide.at.x),
        )
    };
    let y = || {
        coord(
            BoardField::GuideY(id),
            ids::PCB_BOARD_GUIDE_Y,
            origin.user_y(guide.at.y),
        )
    };
    let position = match guide.kind {
        GuideKind::Vertical => vec![x()],
        GuideKind::Horizontal => vec![y()],
        GuideKind::Plus45 | GuideKind::Minus45 => vec![x(), y()],
    };
    let mut body = Column::new()
        .spacing(SP.xs)
        .push(header)
        .push(cards::line(position));
    if board.view.guide_menu == Some(id) {
        let delete = (
            form.t(ids::PCB_BOARD_DELETE),
            live(PcbBoardMsg::GuideDeleted(id)),
        );
        body = body.push(cards::menu(form, vec![delete]));
    }
    form.card(body.boxed())
}

/// "+ Add Guide" and, while open, its dropdown: a row per direction, then
/// Place on Board, which makes the rows arm a guide for the next click.
fn add_menu<'a>(form: &Form<'a>, board: &PcbBoardSummary) -> Element<'a, PanelMsg> {
    let view = &board.view;
    let toggle = form.action(
        form.t(ids::PCB_BOARD_GUIDE_ADD),
        msg(PcbBoardMsg::GuideAddMenuToggled),
        None,
    );
    let mut column = Column::new().spacing(SP.xs).push(toggle);
    if view.guide_add_open {
        let mut rows = Column::new().spacing(SP.xs);
        for kind in GuideKind::ALL {
            let pick = if view.guide_place {
                PcbBoardMsg::GuidePlaceArmed(kind)
            } else {
                PcbBoardMsg::GuideAdded(kind)
            };
            let label = form.t(kind_label(kind));
            let line = cards::line(vec![
                cards::muted(form, form.t(glyph(kind))),
                form.action(label, msg(pick), None),
            ]);
            rows = rows.push(line);
        }
        let place = form.t(ids::PCB_BOARD_GUIDE_PLACE_ON_BOARD);
        rows = rows.push(cards::pill(
            form,
            place,
            view.guide_place,
            msg(PcbBoardMsg::GuidePlaceModeToggled),
            Some(form.t(ids::PCB_BOARD_GUIDE_PLACE_HINT)),
        ));
        column = column.push(form.card(rows.boxed()));
    }
    container(column)
        .padding([SP.s, SP.l])
        .width(Length::Fill)
        .boxed()
}
