//! Snap Options (#2353, #2374): grid, guide and axis snapping, which
//! layers' objects pull the pointer, which object points do, and how far.
//! Closed by default, with the Snapping scope in its header; the objects
//! are a two-column checkbox grid (#2523, mockup row 93).

use iced::widget::{Column, Row, Space, container, responsive};
use iced::{Element, Length, Size, Widget as _};
use iced_aw::Wrap;
use signex_engine::pcb::snap::SnapObject;
use signex_i18n::{Localized, MessageId, ids};

use super::chrome::{Form, Head, Segment, Shown};
use super::{BoardField, PanelMsg, PcbBoardMsg, PcbBoardSummary, SP, fields};
use crate::pcb_canvas::SnapScope;
use crate::styles::TypeScale;

const TS: TypeScale = TypeScale::DEFAULT;
/// A checkbox column narrower than this many body sizes cuts its longest
/// label: below it the objects take one column.
const OBJECT_MIN_EM: f32 = 10.0;

fn msg(m: PcbBoardMsg) -> Option<PanelMsg> {
    Some(PanelMsg::PcbBoard(m))
}

pub(super) fn section<'a>(
    form: &Form<'a>,
    board: &'a PcbBoardSummary,
) -> Option<Element<'a, PanelMsg>> {
    let snap = &board.snap;
    let mut rows: Vec<Shown<'a>> = Vec::new();

    let (snap_to, grids, guides, axes) = (
        form.t(ids::PCB_BOARD_SNAP_TO),
        form.t(ids::PCB_BOARD_SNAP_GRIDS),
        form.t(ids::PCB_BOARD_SNAP_GUIDES),
        form.t(ids::PCB_BOARD_SNAP_AXES),
    );
    // The three wrap under each other in a narrow dock.
    let targets = Wrap::new()
        .spacing(SP.m)
        .line_spacing(SP.xs)
        .push(form.check_box(
            &grids,
            snap.to_grids,
            msg(PcbBoardMsg::SnapToGridsToggled),
            None,
        ))
        .push(form.check_box(
            &guides,
            snap.to_guides,
            msg(PcbBoardMsg::SnapToGuidesToggled),
            None,
        ))
        .push(form.check_box(
            &axes,
            snap.to_axes,
            msg(PcbBoardMsg::SnapToAxesToggled),
            None,
        ));
    rows.push(form.row(
        &[&snap_to, &grids, &guides, &axes],
        form.field(&snap_to, Element::from(targets)),
    ));

    let snapping = form.t(ids::PCB_BOARD_SNAPPING);
    let scopes = SnapScope::ALL
        .into_iter()
        .map(|scope| Segment {
            label: form.t(scope_label(scope)),
            active: snap.scope == scope,
            msg: msg(PcbBoardMsg::SnapScopePicked(scope)),
        })
        .collect();
    // Three two-word labels do not fit the field column: they would wrap
    // into each other. The choice gets the whole form width (#2262).
    let scopes = form.stacked(&snapping, form.segmented(scopes));
    rows.push(form.row(&[&snapping], scopes));

    let objects = form.t(ids::PCB_BOARD_SNAP_OBJECTS);
    rows.push(form.row(&[&objects], form.caption(objects.clone())));
    let mut labels = vec![objects.clone()];
    labels.extend(SnapObject::ALL.map(|object| form.t(object_label(object))));
    let refs: Vec<&Localized> = labels.iter().collect();
    let ctx = form.ctx;
    // Two columns where two fit, else one; laid out per width.
    let grid = responsive(move |size: Size| {
        let form = Form::new(ctx, &board.view.search, &board.view.illustration_hover);
        object_grid(&form, board, size.width)
    })
    .height(Length::Shrink);
    rows.push(form.row(&refs, grid.boxed()));

    // A length in the board's unit (#2879), as Axis Snap Range is.
    let distance = form.t(ids::PCB_BOARD_SNAP_DISTANCE);
    let reach = fields::length(
        form,
        board,
        BoardField::SnapDistance,
        distance.clone(),
        snap.distance_mm,
    );
    rows.push(form.row(&[&distance], form.field(&distance, reach)));

    let axis = form.t(ids::PCB_BOARD_AXIS_RANGE);
    let range = fields::length(
        form,
        board,
        BoardField::AxisRange,
        axis.clone(),
        snap.axis_range_mm,
    );
    rows.push(form.row(&[&axis], form.field(&axis, range)));

    let scope = form.header_caption(form.t(scope_label(snap.scope)));
    form.section_with(
        "pcb_board_snap",
        ids::PCB_BOARD_SECTION_SNAP,
        Head::closed().summary(scope),
        rows,
    )
}

/// The Objects for snapping checkboxes the search keeps, two to a row when
/// `width` holds two, else one. 3D Body Snap Points is greyed with why.
fn object_grid<'a>(form: &Form<'a>, board: &PcbBoardSummary, width: f32) -> Element<'a, PanelMsg> {
    let whole = form.matches(&form.t(ids::PCB_BOARD_SECTION_SNAP).into_string())
        || form.matches(&form.t(ids::PCB_BOARD_SNAP_OBJECTS).into_string());
    let columns = if width - 2.0 * SP.l >= 2.0 * TS.body * OBJECT_MIN_EM + SP.m {
        2
    } else {
        1
    };
    let mut kept: Vec<Element<'a, PanelMsg>> = Vec::new();
    for object in SnapObject::ALL {
        let label = form.t(object_label(object));
        if !whole && !form.matches(&label.to_string()) {
            continue;
        }
        let check = if object.is_supported() {
            form.check_box(
                &label,
                board.snap.objects.contains(&object),
                msg(PcbBoardMsg::SnapObjectToggled(object)),
                None,
            )
        } else {
            let why = form.t(ids::PCB_BOARD_SNAP_OBJECT_UNAVAILABLE);
            form.check_box(&label, false, None, Some(why))
        };
        kept.push(container(check).width(Length::FillPortion(1)).boxed());
    }
    let mut grid = Column::new().spacing(SP.xs).width(Length::Fill);
    let mut cells = kept.into_iter().peekable();
    while cells.peek().is_some() {
        let mut line: Row<Element<'a, PanelMsg>> = Row::new().spacing(SP.m).width(Length::Fill);
        for slot in 0..columns {
            line = match cells.next() {
                Some(cell) => line.push(cell),
                None if slot > 0 => line.push(Space::new().width(Length::FillPortion(1)).boxed()),
                None => line,
            };
        }
        grid = grid.push(line);
    }
    container(grid)
        .padding([SP.xs, SP.l])
        .width(Length::Fill)
        .boxed()
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
