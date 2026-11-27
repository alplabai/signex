//! A pour's Fill Mode with its hatch (#2579) and its Corners with the
//! Corner Size (#2578), in the Pour section of the area panel (mockup row
//! 99). Fill Mode offers Solid and Hatched and no None: the outline-only
//! state is Shelve. Hatch Width and Hatch Gap show while the pours are
//! hatched. Corner Size is greyed while Corners is Sharp.

use iced::Element;
use signex_i18n::{MessageId, UserText, ids};
use signex_types::zone_pour_shape::{PourCorners, PourFillMode};

use super::drawing::zone as dz;
use super::linked::{Segment, linked_row, segments, unit, with_unit};
use super::rows::{Look, labelled, mm_field, sunken_value, with_tip};
use super::zone::{greyed, veiled};
use super::{PcbPrimitiveMsg, PrimitiveField as F, ZoneFacts, format_number};
use crate::panels::PanelMsg;

/// Fill Mode, then Hatch Width and Hatch Gap while the pours are hatched.
pub(super) fn fill_rows<'a>(
    look: Look<'a>,
    f: &'a ZoneFacts,
) -> Vec<(MessageId, Element<'a, PanelMsg>)> {
    let common = &f.common;
    let mut rows = vec![(
        ids::PCB_PRIM_POUR_FILL_MODE,
        linked_row(
            look,
            common,
            dz::FILL_MODE,
            ids::PCB_PRIM_POUR_FILL_MODE,
            fill_mode(look, f),
        ),
    )];
    if f.fill_mode == Some(PourFillMode::Hatched) {
        for (label, field, value) in [
            (ids::PCB_PRIM_POUR_HATCH_WIDTH, F::HatchWidth, f.hatch_width),
            (ids::PCB_PRIM_POUR_HATCH_GAP, F::HatchGap, f.hatch_gap),
        ] {
            let value = with_unit(
                mm_field(common, field, value, look),
                unit(look, ids::PCB_PRIM_UNIT_MM),
            );
            rows.push((label, labelled(look, label, value)));
        }
    }
    rows
}

/// Fill Mode: Solid and Hatched, the one the pours have chosen; neither
/// while they differ. No None (mockup row 99).
pub(super) fn fill_mode<'a>(look: Look<'a>, f: &'a ZoneFacts) -> Element<'a, PanelMsg> {
    let cells = [
        (PourFillMode::Solid, ids::PCB_PRIM_POUR_FILL_SOLID),
        (PourFillMode::Hatched, ids::PCB_PRIM_POUR_FILL_HATCHED),
    ]
    .into_iter()
    .map(|(fill, id)| Segment {
        label: look.t(id),
        is_on: f.fill_mode == Some(fill),
        msg: fill_message(f, fill),
    })
    .collect();
    with_tip(look, segments(cells), ids::PCB_PRIM_POUR_HATCHED_TIP)
}

/// Corners (Sharp, Rounded, Chamfer) and Corner Size: a typed size while
/// the corners are Rounded or Chamfer (or differ), greyed at 0 while Sharp.
pub(super) fn corner_rows<'a>(
    look: Look<'a>,
    f: &'a ZoneFacts,
) -> [(MessageId, Element<'a, PanelMsg>); 2] {
    let common = &f.common;
    let cells = [
        (PourCorners::Sharp, ids::PCB_PRIM_POUR_CORNER_SHARP),
        (PourCorners::Rounded, ids::PCB_PRIM_POUR_CORNER_ROUNDED),
        (PourCorners::Chamfer, ids::PCB_PRIM_POUR_CORNER_CHAMFER),
    ]
    .into_iter()
    .map(|(corners, id)| Segment {
        label: look.t(id),
        is_on: f.corners == Some(corners),
        msg: corners_message(f, corners),
    })
    .collect();
    let shapes = linked_row(
        look,
        common,
        dz::CORNERS,
        ids::PCB_PRIM_POUR_CORNERS,
        with_tip(look, segments(cells), ids::PCB_PRIM_POUR_CORNERS_TIP),
    );
    let sharp = f.corners == Some(PourCorners::Sharp);
    let (label_look, value) = if sharp {
        (greyed(look), corner_size_value(look))
    } else {
        (look, mm_field(common, F::CornerSize, f.corner_size, look))
    };
    let size = labelled(
        label_look,
        ids::PCB_PRIM_POUR_CORNER_SIZE,
        with_unit(
            with_tip(look, value, ids::PCB_PRIM_POUR_CORNERS_TIP),
            unit(look, ids::PCB_PRIM_UNIT_MM),
        ),
    );
    [
        (ids::PCB_PRIM_POUR_CORNERS, shapes),
        (ids::PCB_PRIM_POUR_CORNER_SIZE, size),
    ]
}

/// Corner Size while Corners is Sharp, greyed and deaf to typing (mockup
/// row 99). It shows the size a sharp corner has: 0.
pub(super) fn corner_size_value<'a>(look: Look<'a>) -> Element<'a, PanelMsg> {
    veiled(
        look.tokens(),
        sunken_value(look, UserText::new(format_number(0.0))),
    )
}

/// What picking `fill` sends: nothing on a board that cannot be edited.
pub(super) fn fill_message(f: &ZoneFacts, fill: PourFillMode) -> Option<PcbPrimitiveMsg> {
    let common = &f.common;
    common
        .is_editable()
        .then(|| PcbPrimitiveMsg::PourFillPicked {
            target: common.target.clone(),
            fill,
        })
}

/// What picking `corners` sends: nothing on a board that cannot be edited.
pub(super) fn corners_message(f: &ZoneFacts, corners: PourCorners) -> Option<PcbPrimitiveMsg> {
    let common = &f.common;
    common
        .is_editable()
        .then(|| PcbPrimitiveMsg::PourCornersPicked {
            target: common.target.clone(),
            corners,
        })
}
