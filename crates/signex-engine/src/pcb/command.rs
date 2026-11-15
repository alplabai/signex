use signex_net::board::BoardNetEdit;
use signex_types::pcb::{Footprint, PcbBoard, Point, SymbolLink};
use signex_types::pcb_options::BoardOptions;
use signex_types::pcb_sketch::BoardSketch;
use signex_types::property::PcbProperty;
use uuid::Uuid;

use super::batch;
use super::board_sketch;
use super::error::BoardCommandError;
use super::footprint::{self, FootprintFace};
use super::footprint_content::{self, FootprintText};
use super::items::{self, BoardItem, BoardItemId, RemovedItem};
use super::options;
use super::replace::{self, FootprintPad};

/// One edit of a board. Targets are absolute so the inverse is exact.
#[derive(Debug, Clone, PartialEq)]
pub enum BoardCommand {
    /// Place the footprint's origin at `to` (board mm).
    MoveFootprint { footprint: Uuid, to: Point },
    /// Set the footprint's rotation about its origin to `degrees`.
    Rotate { footprint: Uuid, degrees: f64 },
    /// Move the footprint to the other side of the board with
    /// `pcb_geom::flip_footprint` (ADR-0005 §8): sided layers of the
    /// footprint, pads, graphics and properties swap and the rotation is
    /// stored as `(-rotation) mod 360`. Local geometry is untouched. The
    /// inverse is a [`BoardCommand::RestoreFace`], so undo is exact. The
    /// caller enforces `Footprint::locked` for move, rotate and flip.
    Flip { footprint: Uuid },
    /// Put back the rotation and sided layers a flip replaced; the inverse
    /// of [`BoardCommand::Flip`]. Refused if the footprint's pad, graphic
    /// or property count changed since the face was taken.
    RestoreFace {
        footprint: Uuid,
        face: FootprintFace,
    },
    /// Replace the board outline polygon. Refused while the board sketch
    /// owns the outline: [`BoardCommand::SetBoardSketch`] writes it then.
    SetBoardOutline { outline: Vec<Point> },
    /// Replace the board sketch and its baked keepouts, cutouts, holes and
    /// V-scores (#2234), and the outline when `outline` is set: the only
    /// way any of them changes. Build it with
    /// [`BoardCommand::set_board_sketch`]. Its inverse is another one
    /// holding what it replaced.
    SetBoardSketch {
        sketch: Option<Box<BoardSketch>>,
        outline: Option<Vec<Point>>,
    },
    /// Remove the named items.
    DeleteItems { items: Vec<BoardItemId> },
    /// Put removed items back at their recorded list indices; the inverse
    /// of [`BoardCommand::DeleteItems`].
    RestoreItems { items: Vec<RemovedItem> },
    /// Append new items to the end of their lists. Refused when one is
    /// already on the board or named twice, when a position or angle is
    /// not a number, or when a pad uuid is already on the board. The inverse is a
    /// [`BoardCommand::DeleteItems`].
    AddItems { items: Vec<BoardItem> },
    /// Put each item over the board item with its id, at the same list
    /// index: a track's or via's edited fields (#2355). Refused whole for a
    /// footprint, an id named twice or gone, a number that is not finite, a
    /// width or size that is not positive, a via hole not inside its via, a
    /// track moved to a layer that is not copper, or a net not in the table.
    /// The inverse is another one holding the items it replaced.
    ReplaceItems { items: Vec<BoardItem> },
    /// Put each pad over the pad with its uuid on its footprint, at the same
    /// index: a pad's edited fields (#2355). Refused whole for a footprint or
    /// pad that is gone, a pad named twice, a number that is not finite, a
    /// size that is not positive, a hole larger than the pad, or a net not
    /// in the table under its name. The caller enforces `Footprint::locked`
    /// and `Pad::net_locked`. The inverse is another one holding the pads it
    /// replaced.
    ReplacePads { pads: Vec<FootprintPad> },
    /// Swap the footprint's pads, graphics and name for those of `with`, a
    /// library footprint in its top-side form. The board footprint keeps its
    /// uuid, position, rotation, side (a bottom one flips the template), lock,
    /// schematic link, designator and value, its library link when `with` has
    /// none, and its property placement and properties `with` lacks; the n-th
    /// new pad of a number takes the net and lock of the n-th old pad of it.
    /// Refused for a template not on a readable top-side layer, with a pad
    /// off the number line or a taken uuid, or a different count of a shared
    /// pad number. The
    /// inverse is a [`BoardCommand::RestoreFootprint`].
    ReplaceFootprint {
        footprint: Uuid,
        with: Box<Footprint>,
    },
    /// Put a whole footprint back over the one with its uuid; the inverse of
    /// [`BoardCommand::ReplaceFootprint`].
    RestoreFootprint { footprint: Box<Footprint> },
    /// Set the designator and value, and the Reference and Value property
    /// texts with them. The inverse is a [`BoardCommand::RestoreFootprintText`].
    SetFootprintText {
        footprint: Uuid,
        reference: String,
        value: String,
    },
    /// Put back the designator, value and property texts a text edit
    /// replaced. Refused if the footprint's property count changed since.
    RestoreFootprintText {
        footprint: Uuid,
        text: FootprintText,
    },
    /// Set or clear the footprint's schematic link (ADR-0005 §11).
    SetFootprintLink {
        footprint: Uuid,
        link: Option<SymbolLink>,
    },
    /// Lock or unlock the footprint. The inverse sets the state it replaced.
    /// The caller decides which edits a lock refuses; this one it never does,
    /// or a locked footprint could not be unlocked.
    SetFootprintLocked { footprint: Uuid, locked: bool },
    /// Replace the footprint's property at `index` with `property`: its
    /// value, visibility or placement. Refused when the key would change,
    /// when a number in it is not finite, and when it would change the text
    /// of the Reference or Value property, which follows the designator and
    /// value ([`BoardCommand::SetFootprintText`] sets those). The inverse is
    /// another one holding the property it replaced.
    SetFootprintProperty {
        footprint: Uuid,
        index: usize,
        property: PcbProperty,
    },
    /// Write a net table and pad nets (`signex_net::board`); its inverse is
    /// the edit [`BoardNetEdit::apply`] returns. Refused whole when a change
    /// names a footprint, pad or zone that is gone, or one twice.
    SetNets(BoardNetEdit),
    /// Set the board's snap grid step in mm, `PcbSetup::grid_size` (#2353).
    /// `0` is the PCB default step. Refused for a negative or non-finite
    /// step and on a board without a setup. The inverse is another one
    /// holding the old step.
    SetGridSize { grid_mm: f64 },
    /// Replace the board options (#2374): guides, extra snap grids, the
    /// sheet and the naming options. Refused for a number that is not
    /// finite, a grid step or sheet side that is not positive, or a guide or
    /// grid uuid named twice. The inverse is another one holding the old
    /// options.
    SetBoardOptions { options: Box<BoardOptions> },
}

/// What a command changed, for a consumer that redraws or re-derives only
/// what moved.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardPatch {
    pub kind: BoardPatchKind,
    /// The items the command touched. Empty for an outline change.
    pub items: Vec<BoardItemId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardPatchKind {
    FootprintMoved,
    FootprintRotated,
    FootprintFlipped,
    OutlineReplaced,
    ItemsDeleted,
    ItemsRestored,
    ItemsAdded,
    /// Tracks, vias, zones or texts replaced in place.
    ItemsReplaced,
    /// Pads replaced in place. The items are their footprints.
    PadsReplaced,
    FootprintReplaced,
    FootprintTextSet,
    FootprintLinkSet,
    FootprintLockSet,
    FootprintPropertySet,
    /// The net table, and pad and zone nets. The items are the footprints
    /// and zones written; a table-only change names none.
    NetsChanged,
    /// The board sketch and its baked geometry, the outline included when
    /// the command carried one. Names no items.
    BoardSketchReplaced,
    /// The setup's snap grid step. Names no items.
    GridChanged,
    /// The board options. Names no items.
    OptionsChanged,
}

/// The result of applying a command: the command that undoes it and the
/// patch it made.
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    pub inverse: BoardCommand,
    pub patch: BoardPatch,
}

impl BoardCommand {
    /// Apply the command to `board`. On error the board is unchanged.
    pub fn apply(self, board: &mut PcbBoard) -> Result<Applied, BoardCommandError> {
        match self {
            Self::MoveFootprint { footprint: id, to } => footprint::move_to(board, id, to),
            Self::Rotate {
                footprint: id,
                degrees,
            } => footprint::rotate_to(board, id, degrees),
            Self::Flip { footprint: id } => footprint::flip(board, id),
            Self::RestoreFace {
                footprint: id,
                face,
            } => footprint::restore_face(board, id, face),
            Self::SetBoardOutline { outline } => set_outline(board, outline),
            Self::SetBoardSketch { sketch, outline } => board_sketch::set(board, sketch, outline),
            Self::DeleteItems { items: ids } => items::delete(board, &ids),
            Self::RestoreItems { items: removed } => items::restore(board, removed),
            Self::AddItems { items: added } => items::add(board, added),
            Self::ReplaceItems { items: new } => replace::replace(board, new),
            Self::ReplacePads { pads } => replace::replace_pads(board, pads),
            Self::ReplaceFootprint {
                footprint: id,
                with,
            } => footprint_content::replace(board, id, *with),
            Self::RestoreFootprint { footprint: fp } => footprint_content::restore(board, *fp),
            Self::SetFootprintText {
                footprint: id,
                reference,
                value,
            } => footprint_content::set_text(board, id, reference, value),
            Self::RestoreFootprintText {
                footprint: id,
                text,
            } => footprint_content::restore_text(board, id, text),
            Self::SetFootprintLink {
                footprint: id,
                link,
            } => footprint_content::set_link(board, id, link),
            Self::SetFootprintLocked {
                footprint: id,
                locked,
            } => footprint::set_locked(board, id, locked),
            Self::SetFootprintProperty {
                footprint: id,
                index,
                property,
            } => footprint_content::set_property(board, id, index, property),
            Self::SetNets(edit) => batch::set_nets(board, edit),
            Self::SetGridSize { grid_mm } => set_grid_size(board, grid_mm),
            Self::SetBoardOptions { options: new } => options::set(board, *new),
        }
    }
}

fn set_outline(board: &mut PcbBoard, outline: Vec<Point>) -> Result<Applied, BoardCommandError> {
    if board.sketch.as_ref().is_some_and(BoardSketch::owns_outline) {
        return Err(BoardCommandError::OutlineOwnedBySketch);
    }
    if outline.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return Err(BoardCommandError::NonFinite { field: "outline" });
    }
    let previous = std::mem::replace(&mut board.outline, outline);
    Ok(Applied {
        inverse: BoardCommand::SetBoardOutline { outline: previous },
        patch: BoardPatch {
            kind: BoardPatchKind::OutlineReplaced,
            items: Vec::new(),
        },
    })
}

fn set_grid_size(board: &mut PcbBoard, grid_mm: f64) -> Result<Applied, BoardCommandError> {
    if !grid_mm.is_finite() {
        return Err(BoardCommandError::NonFinite { field: "grid_size" });
    }
    if grid_mm < 0.0 {
        return Err(BoardCommandError::NegativeGridStep(grid_mm));
    }
    let setup = board.setup.as_mut().ok_or(BoardCommandError::NoSetup)?;
    let previous = std::mem::replace(&mut setup.grid_size, grid_mm);
    Ok(Applied {
        inverse: BoardCommand::SetGridSize { grid_mm: previous },
        patch: BoardPatch {
            kind: BoardPatchKind::GridChanged,
            items: Vec::new(),
        },
    })
}
