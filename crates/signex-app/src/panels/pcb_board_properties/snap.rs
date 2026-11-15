//! Snap Options (#2353, #2374): grid, guide and axis snapping, which
//! layers' objects pull the pointer, which object points do, and how far.

use iced::widget::{container, row};
use iced::{Element, Length, Widget as _};
use signex_engine::pcb::snap::SnapObject;
use signex_i18n::{LocalNumber, Localized, MessageId, ids, t};

use super::chrome::{Form, Segment, Shown};
use super::{BoardField, PanelMsg, PcbBoardMsg, PcbBoardSummary, SP, fields};
use crate::pcb_canvas::{SnapDistance, SnapScope};

fn msg(m: PcbBoardMsg) -> Option<PanelMsg> {
    Some(PanelMsg::PcbBoard(m))
}

pub(super) fn section<'a>(
    form: &Form<'a>,
    board: &PcbBoardSummary,
) -> Option<Element<'a, PanelMsg>> {
    let snap = &board.snap;
    let mut rows: Vec<Shown<'a>> = Vec::new();

    let (grids, guides, axes) = (
        form.t(ids::PCB_BOARD_SNAP_GRIDS),
        form.t(ids::PCB_BOARD_SNAP_GUIDES),
        form.t(ids::PCB_BOARD_SNAP_AXES),
    );
    let targets = row![
        form.chip(
            grids.clone(),
            snap.to_grids,
            msg(PcbBoardMsg::SnapToGridsToggled),
            None
        ),
        form.chip(
            guides.clone(),
            snap.to_guides,
            msg(PcbBoardMsg::SnapToGuidesToggled),
            None
        ),
        form.chip(
            axes.clone(),
            snap.to_axes,
            msg(PcbBoardMsg::SnapToAxesToggled),
            None
        ),
    ]
    .spacing(SP.s);
    let targets = container(targets).padding([SP.xs, SP.l]).boxed();
    rows.push(form.row(&[&grids, &guides, &axes], targets));

    let snapping = form.t(ids::PCB_BOARD_SNAPPING);
    let scopes = SnapScope::ALL
        .into_iter()
        .map(|scope| Segment {
            label: form.t(scope_label(scope)),
            active: snap.scope == scope,
            msg: msg(PcbBoardMsg::SnapScopePicked(scope)),
        })
        .collect();
    let scopes = form.field(&snapping, form.segmented(scopes));
    rows.push(form.row(&[&snapping], scopes));

    let objects = form.t(ids::PCB_BOARD_SNAP_OBJECTS);
    rows.push(form.row(&[&objects], form.caption(objects.clone())));
    for object in SnapObject::ALL {
        let label = form.t(object_label(object));
        let check = if object.is_supported() {
            form.check(
                &label,
                snap.objects.contains(&object),
                msg(PcbBoardMsg::SnapObjectToggled(object)),
                None,
            )
        } else {
            let why = form.t(ids::PCB_BOARD_SNAP_OBJECT_UNAVAILABLE);
            form.check(&label, false, None, Some(why))
        };
        rows.push(form.row(&[&objects, &label], check));
    }

    let distance = form.t(ids::PCB_BOARD_SNAP_DISTANCE);
    rows.push(form.row(
        &[&distance],
        form.field(&distance, distance_pick(form, snap.distance)),
    ));

    let axis = form.t(ids::PCB_BOARD_AXIS_RANGE);
    let range = fields::length(
        form,
        board,
        BoardField::AxisRange,
        axis.clone(),
        snap.axis_range_mm,
    );
    rows.push(form.row(&[&axis], form.field(&axis, range)));

    form.section("pcb_board_snap", ids::PCB_BOARD_SECTION_SNAP, rows)
}

/// One Snap Distance choice as the dropdown lists it.
#[derive(Debug, Clone, PartialEq)]
struct DistanceChoice {
    distance: SnapDistance,
    label: String,
}

fn distance_pick<'a>(form: &Form<'a>, current: SnapDistance) -> Element<'a, PanelMsg> {
    use signex_widgets::dropdown_style::GraphiteDropdown as _;
    let choice = |distance: SnapDistance| DistanceChoice {
        distance,
        label: distance_label(form, distance).into_string(),
    };
    let options: Vec<DistanceChoice> = SnapDistance::ALL.into_iter().map(choice).collect();
    iced::widget::pick_list(Some(choice(current)), options, |c: &DistanceChoice| {
        c.label.clone()
    })
    .graphite()
    .on_select(|c: DistanceChoice| PanelMsg::PcbBoard(PcbBoardMsg::SnapDistancePicked(c.distance)))
    .padding([SP.xs, SP.s])
    .width(Length::Fill)
    .boxed()
}

fn distance_label(form: &Form<'_>, distance: SnapDistance) -> Localized {
    t!(
        form.i18n,
        ids::PCB_BOARD_SNAP_DISTANCE_VALUE,
        "factor" => LocalNumber(distance.factor())
    )
}

fn scope_label(scope: SnapScope) -> MessageId {
    match scope {
        SnapScope::AllLayers => ids::PCB_BOARD_SNAPPING_ALL,
        SnapScope::CurrentLayer => ids::PCB_BOARD_SNAPPING_CURRENT,
        SnapScope::Off => ids::PCB_BOARD_SNAPPING_OFF,
    }
}

fn object_label(object: SnapObject) -> MessageId {
    match object {
        SnapObject::TrackVertices => ids::PCB_BOARD_SNAP_TRACK_VERTICES,
        SnapObject::TrackLines => ids::PCB_BOARD_SNAP_TRACK_LINES,
        SnapObject::ArcCenters => ids::PCB_BOARD_SNAP_ARC_CENTERS,
        SnapObject::Intersections => ids::PCB_BOARD_SNAP_INTERSECTIONS,
        SnapObject::PadCenters => ids::PCB_BOARD_SNAP_PAD_CENTERS,
        SnapObject::PadVertices => ids::PCB_BOARD_SNAP_PAD_VERTICES,
        SnapObject::PadEdges => ids::PCB_BOARD_SNAP_PAD_EDGES,
        SnapObject::ViaCenters => ids::PCB_BOARD_SNAP_VIA_CENTERS,
        SnapObject::Regions => ids::PCB_BOARD_SNAP_REGIONS,
        SnapObject::BoardShape => ids::PCB_BOARD_SNAP_BOARD_SHAPE,
        SnapObject::FootprintOrigins => ids::PCB_BOARD_SNAP_FOOTPRINT_ORIGINS,
        SnapObject::Body3dPoints => ids::PCB_BOARD_SNAP_BODY_POINTS,
        SnapObject::Texts => ids::PCB_BOARD_SNAP_TEXTS,
    }
}
