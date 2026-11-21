use signex_net::board::BoardNetEdit;
use signex_types::pcb::{
    BoardGraphic, ComponentAttributes, Footprint, PcbBoard, Point, Room, RoomOutline, SymbolLink,
};
use signex_types::pcb_options::BoardOptions;
use signex_types::pcb_panel::BoardPanel;
use signex_types::pcb_sketch::BoardSketch;
use signex_types::pcb_stack::LayerStack;
use signex_types::property::PcbProperty;
use signex_types::sketch::board_attr::SketchBoardItems;
use uuid::Uuid;

use super::batch;
use super::board_sketch;
use super::board_sketch::items::SketchBakeSnapshot;
use super::error::BoardCommandError;
use super::footprint::{self, FootprintFace};
use super::footprint_content::{self, FootprintText};
use super::graphics;
use super::items::{self, BoardItem, BoardItemId, RemovedItem};
use super::lock;
use super::options;
use super::panel;
use super::replace::{self, FootprintPad, Values};
use super::rooms;
use super::stack::{self, StackSnapshot};

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
    /// [`BoardCommand::SetBoardSketch`] that also replaces the zones and
    /// drawings the sketch owns (uuid of an entity of the old or new sketch)
    /// with `items` and moves the anchored footprints, a locked one
    /// excepted (decision 2, 2026-11-20). Build it with
    /// [`BoardCommand::set_board_sketch_with_items`]. Refused for a number
    /// that is not finite or an anchor naming a footprint that is gone. The
    /// inverse is a [`BoardCommand::RestoreBoardSketchBake`].
    SetBoardSketchBake {
        sketch: Option<Box<BoardSketch>>,
        outline: Option<Vec<Point>>,
        items: Box<SketchBoardItems>,
    },
    /// Put back what a [`BoardCommand::SetBoardSketchBake`] replaced; the
    /// inverse of it and of itself.
    RestoreBoardSketchBake(Box<SketchBakeSnapshot>),
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
    /// The inverse is a [`BoardCommand::RestoreReplacedItems`].
    ReplaceItems { items: Vec<BoardItem> },
    /// Put back items a replace took off, at their list index; the inverse
    /// of [`BoardCommand::ReplaceItems`] and of itself. Only the ids are
    /// checked: the values were on the board before, odd ones included.
    RestoreReplacedItems { items: Vec<BoardItem> },
    /// Put each pad over the pad with its uuid on its footprint, at the same
    /// index: a pad's edited fields (#2355). Refused whole for a footprint or
    /// pad that is gone, a pad named twice, a number that is not finite, a
    /// size that is not positive, a hole larger than the pad, or a net not
    /// in the table under its name. The caller enforces `Footprint::locked`
    /// and `Pad::net_locked`. The inverse is a
    /// [`BoardCommand::RestoreReplacedPads`].
    ReplacePads { pads: Vec<FootprintPad> },
    /// Put back pads a replace took off; the inverse of
    /// [`BoardCommand::ReplacePads`] and of itself. Only the ids are checked.
    RestoreReplacedPads { pads: Vec<FootprintPad> },
    /// Swap the footprint's pads, graphics and name for those of `with`, a
    /// library footprint in its top-side form. The board footprint keeps its
    /// uuid, position, rotation, side (a bottom one flips the template), lock,
    /// schematic link, designator and value, its library link when `with` has
    /// none, its body opacity, locks, swapping and channel offset (`with`
    /// brings type, height and description), and its property placement and
    /// properties `with` lacks; the n-th
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
    /// Insert `property` at `index` of the footprint's properties: a new
    /// parameter (#2369). Refused for an empty key, the Reference or Value
    /// key, an index past the end and a number that is not finite; a key
    /// already on the footprint is the caller's check, so the inverse of a
    /// removal always applies. The inverse is a
    /// [`BoardCommand::RemoveFootprintProperty`].
    InsertFootprintProperty {
        footprint: Uuid,
        index: usize,
        property: PcbProperty,
    },
    /// Remove the footprint's property at `index`: a parameter (#2369).
    /// Refused for the Reference and Value texts. The inverse is a
    /// [`BoardCommand::InsertFootprintProperty`] holding what it removed.
    RemoveFootprintProperty { footprint: Uuid, index: usize },
    /// Replace the footprint's component settings (#2365): type, height,
    /// body opacity, locks, swapping, channel offset and description.
    /// Refused for a negative or non-finite height and an opacity outside
    /// 0..=1. The inverse is another one holding the settings it replaced.
    SetFootprintComponent {
        footprint: Uuid,
        component: ComponentAttributes,
    },
    /// Write a net table and pad nets (`signex_net::board`); its inverse is
    /// the edit [`BoardNetEdit::apply`] returns. Refused whole when a change
    /// names a footprint, pad or zone that is gone, or one twice.
    SetNets(BoardNetEdit),
    /// Set the board's snap grid step in mm, `PcbSetup::grid_size` (#2353).
    /// `0` is the PCB default step. Refused for a negative or non-finite
    /// step. The inverse is another one holding the old step; on a board
    /// without a setup it gives the board one with every other value unset
    /// (`0`, read as the default), and the inverse is [`Self::RemoveSetup`].
    SetGridSize { grid_mm: f64 },
    /// Take away the setup a [`Self::SetGridSize`] gave a board that had
    /// none. The inverse is that command again.
    RemoveSetup,
    /// Replace the board options (#2374): guides, extra snap grids, the
    /// sheet and the naming options. Refused for a number that is not
    /// finite, a grid step or sheet side that is not positive, or a guide or
    /// grid uuid named twice. The inverse is another one holding the old
    /// options.
    SetBoardOptions { options: Box<BoardOptions> },
    /// Replace the layer stack (#2401) and with it the copper layers of the
    /// layer list and the board thickness (the stack's total). Copper is
    /// named by place; a copper row names the board layer it was, or none for
    /// a new layer, and every item on a layer that moved place moves with it.
    /// Refused for a bad number, a copper count outside 2..=32, an outer
    /// copper row moved inward, and a copper layer left out with items or a
    /// keepout on it. The inverse is a [`BoardCommand::RestoreLayerStack`].
    SetLayerStack { stack: Box<LayerStack> },
    /// Put back the stack, layer list, thickness and copper names a stack
    /// edit replaced; the inverse of [`BoardCommand::SetLayerStack`] and of
    /// itself. Nothing is checked: the values were on the board before.
    RestoreLayerStack(Box<StackSnapshot>),
    /// Lock or unlock tracks and vias (#2407). Refused for an id named twice,
    /// gone, or not a track or via. The inverse sets the states it replaced.
    SetItemsLocked { items: Vec<(BoardItemId, bool)> },
    /// Append board drawings (#2410, a DXF import onto a layer). Refused for
    /// a number that is not finite. The inverse is a
    /// [`BoardCommand::RemoveGraphics`].
    AddGraphics { graphics: Vec<BoardGraphic> },
    /// Remove these drawings from the end of the board's drawings; the
    /// inverse of [`BoardCommand::AddGraphics`]. Refused unless the list
    /// still ends with exactly them.
    RemoveGraphics { graphics: Vec<BoardGraphic> },
    /// Set the board's panel (#2410), `None` to make the board on its own.
    /// Refused for a panel that breaks a rule of `BoardPanel::check`.
    /// Whether it lays out on the outline is the caller's check: the outline
    /// may change later and an undo must restore what was there. The inverse
    /// is another one holding the old panel.
    SetBoardPanel { panel: Option<Box<BoardPanel>> },
    /// Append a room (#2457). Refused for a uuid already used by a room, an
    /// empty name, an outline without area or with a number that is not
    /// finite, a colour that is not `#RRGGBB` and a negative margin. The
    /// inverse is a [`BoardCommand::RemoveRoom`].
    AddRoom { room: Box<Room> },
    /// Remove a room; its parts stay. The inverse is a
    /// [`BoardCommand::RestoreRoom`].
    RemoveRoom { room: Uuid },
    /// Put a removed room back at `index`; the inverse of
    /// [`BoardCommand::RemoveRoom`].
    RestoreRoom { room: Box<Room>, index: usize },
    /// Put `room` over the room with its uuid: name, source, layer, shape and
    /// outline, colour, Keep parts inside, margin and lock. Checked as
    /// [`BoardCommand::AddRoom`]; a locked room that stays locked keeps its
    /// outline. The inverse is another one holding the old room.
    EditRoom { room: Box<Room> },
    /// Move the room so its box starts at `to`, and its unlocked member
    /// parts by the same distance, as one change. Refused for a locked room.
    /// The inverse is a [`BoardCommand::PlaceRoom`].
    MoveRoom { room: Uuid, to: Point },
    /// Set a room's outline and put each named footprint at its origin: the
    /// absolute form a move, a fit and a pull undo to, and its own inverse.
    PlaceRoom {
        room: Uuid,
        outline: RoomOutline,
        footprints: Vec<(Uuid, Point)>,
    },
    /// Fit the room round its member parts' bodies plus its margin. Refused
    /// for a locked room and a room without members. The inverse is a
    /// [`BoardCommand::PlaceRoom`].
    FitRoomToParts { room: Uuid },
    /// Pack the unlocked member parts outside the room into it, in rows. The
    /// inverse is a [`BoardCommand::PlaceRoom`].
    PullPartsIntoRoom { room: Uuid },
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
    /// A property replaced, added or removed.
    FootprintPropertySet,
    /// The component settings (#2365).
    FootprintComponentSet,
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
    /// The layer stack, the copper of the layer list and any copper renamed
    /// with it. Names no items.
    LayerStackChanged,
    /// The lock of tracks and vias.
    ItemsLockSet,
    /// Board drawings added or removed. Names no items.
    GraphicsChanged,
    /// The panel (#2410). Names no items.
    PanelChanged,
    /// A room added, removed, edited or placed (#2457). The items are the
    /// footprints moved with it.
    RoomsChanged,
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
            Self::SetBoardSketchBake {
                sketch,
                outline,
                items,
            } => board_sketch::items::set(board, sketch, outline, *items),
            Self::RestoreBoardSketchBake(snapshot) => {
                board_sketch::items::restore(board, *snapshot)
            }
            Self::DeleteItems { items: ids } => items::delete(board, &ids),
            Self::RestoreItems { items: removed } => items::restore(board, removed),
            Self::AddItems { items: added } => items::add(board, added),
            Self::ReplaceItems { items: new } => replace::replace(board, new, Values::Checked),
            Self::RestoreReplacedItems { items: old } => {
                replace::replace(board, old, Values::Restored)
            }
            Self::ReplacePads { pads } => replace::replace_pads(board, pads, Values::Checked),
            Self::RestoreReplacedPads { pads } => {
                replace::replace_pads(board, pads, Values::Restored)
            }
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
            Self::InsertFootprintProperty {
                footprint: id,
                index,
                property,
            } => footprint_content::insert_property(board, id, index, property),
            Self::RemoveFootprintProperty {
                footprint: id,
                index,
            } => footprint_content::remove_property(board, id, index),
            Self::SetFootprintComponent {
                footprint: id,
                component,
            } => footprint::set_component(board, id, component),
            Self::SetNets(edit) => batch::set_nets(board, edit),
            Self::SetGridSize { grid_mm } => set_grid_size(board, grid_mm),
            Self::RemoveSetup => remove_setup(board),
            Self::SetBoardOptions { options: new } => options::set(board, *new),
            Self::SetLayerStack { stack: new } => stack::set(board, *new),
            Self::RestoreLayerStack(snapshot) => stack::restore(board, *snapshot),
            Self::SetItemsLocked { items: locks } => lock::set_locked(board, locks),
            Self::AddGraphics { graphics } => graphics::add(board, graphics),
            Self::RemoveGraphics { graphics } => graphics::remove(board, graphics),
            Self::SetBoardPanel { panel: new } => panel::set(board, new.map(|p| *p)),
            Self::AddRoom { room } => rooms::add(board, *room),
            Self::RemoveRoom { room } => rooms::remove(board, room),
            Self::RestoreRoom { room, index } => rooms::restore(board, *room, index),
            Self::EditRoom { room } => rooms::edit(board, *room),
            Self::MoveRoom { room, to } => rooms::move_room(board, room, to),
            Self::PlaceRoom {
                room,
                outline,
                footprints,
            } => rooms::place(board, room, outline, footprints),
            Self::FitRoomToParts { room } => rooms::fit(board, room),
            Self::PullPartsIntoRoom { room } => rooms::pull(board, room),
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
    let inverse = match board.setup.as_mut() {
        Some(setup) => BoardCommand::SetGridSize {
            grid_mm: std::mem::replace(&mut setup.grid_size, grid_mm),
        },
        None => {
            // Every reader takes an unset (0) value as its default.
            board.setup = Some(signex_types::pcb::PcbSetup {
                grid_size: grid_mm,
                trace_width: 0.0,
                via_diameter: 0.0,
                via_drill: 0.0,
                clearance: 0.0,
                track_min_width: 0.0,
                via_min_diameter: 0.0,
                via_min_drill: 0.0,
            });
            BoardCommand::RemoveSetup
        }
    };
    Ok(Applied {
        inverse,
        patch: BoardPatch {
            kind: BoardPatchKind::GridChanged,
            items: Vec::new(),
        },
    })
}

fn remove_setup(board: &mut PcbBoard) -> Result<Applied, BoardCommandError> {
    let setup = board.setup.take().ok_or(BoardCommandError::NoSetup)?;
    Ok(Applied {
        inverse: BoardCommand::SetGridSize {
            grid_mm: setup.grid_size,
        },
        patch: BoardPatch {
            kind: BoardPatchKind::GridChanged,
            items: Vec::new(),
        },
    })
}
