//! Track fields: location, width, ends, layer, net, lock and mask settings.

use signex_types::format::pcb_layer_token;
use signex_types::pcb::{MaskExpansion, PcbBoard, Point, Segment};
use signex_types::pcb_layer::{CopperLayer, PcbLayer};
use uuid::Uuid;

use super::{PrimitiveEditRefusal, finite, finite_mask};
use crate::pcb::{BoardCommand, BoardItem};

/// One field of the selected tracks, set to an absolute value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrackEdit {
    /// Move the selection so its location (see [`track_location`]) has this x.
    LocationX(f64),
    LocationY(f64),
    Width(f64),
    StartX(f64),
    StartY(f64),
    EndX(f64),
    EndY(f64),
    Layer(CopperLayer),
    /// 0 is no net.
    Net(u32),
    /// Lock or unlock; the one edit a locked track takes (#2378).
    Locked(bool),
    PasteMask(MaskExpansion),
    SolderMask(MaskExpansion),
}

/// Where a selection of tracks is: the lower-left corner, as the screen shows
/// it (model y grows down), of the box around their ends (#2384). Moving the
/// location moves every track by the same step, so the shape of a selected
/// route is kept. `None` for no tracks.
pub fn track_location<'a>(tracks: impl IntoIterator<Item = &'a Segment>) -> Option<Point> {
    tracks
        .into_iter()
        .flat_map(|s| [s.start, s.end])
        .fold(None, |corner: Option<Point>, p| {
            Some(match corner {
                None => p,
                Some(c) => Point::new(c.x.min(p.x), c.y.max(p.y)),
            })
        })
}

/// The command setting `edit` on every track in `tracks`. Refused when a
/// track is locked and the edit is not the unlock.
pub fn track_edit_command(
    board: &PcbBoard,
    tracks: &[Uuid],
    edit: TrackEdit,
) -> Result<Option<BoardCommand>, PrimitiveEditRefusal> {
    let found: Vec<&Segment> = tracks
        .iter()
        .map(|id| board.segments.iter().find(|s| s.uuid == *id))
        .collect::<Option<_>>()
        .ok_or(PrimitiveEditRefusal::Gone)?;
    let corner = track_location(found.iter().copied()).ok_or(PrimitiveEditRefusal::Empty)?;
    let unlocking = matches!(edit, TrackEdit::Locked(_));
    if !unlocking && found.iter().any(|s| s.locked) {
        return Err(PrimitiveEditRefusal::ItemLocked);
    }
    let mut changed = Vec::new();
    for old in found {
        let mut new = old.clone();
        apply(&mut new, edit, corner)?;
        if new != *old {
            changed.push(BoardItem::Segment(new));
        }
    }
    Ok((!changed.is_empty()).then_some(BoardCommand::ReplaceItems { items: changed }))
}

fn apply(s: &mut Segment, edit: TrackEdit, corner: Point) -> Result<(), PrimitiveEditRefusal> {
    match edit {
        TrackEdit::LocationX(x) => {
            let dx = finite(x)? - corner.x;
            s.start.x += dx;
            s.end.x += dx;
        }
        TrackEdit::LocationY(y) => {
            let dy = finite(y)? - corner.y;
            s.start.y += dy;
            s.end.y += dy;
        }
        TrackEdit::Width(w) => s.width = finite(w)?,
        TrackEdit::StartX(x) => s.start.x = finite(x)?,
        TrackEdit::StartY(y) => s.start.y = finite(y)?,
        TrackEdit::EndX(x) => s.end.x = finite(x)?,
        TrackEdit::EndY(y) => s.end.y = finite(y)?,
        TrackEdit::Layer(layer) => s.layer = pcb_layer_token(PcbLayer::Copper(layer)),
        TrackEdit::Net(net) => s.net = net,
        TrackEdit::Locked(locked) => s.locked = locked,
        TrackEdit::PasteMask(mask) => s.attributes.paste_mask = finite_mask(mask)?,
        TrackEdit::SolderMask(mask) => s.attributes.solder_mask = finite_mask(mask)?,
    }
    Ok(())
}
