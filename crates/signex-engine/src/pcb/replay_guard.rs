//! Whether a recorded board edit may run again (#2840). A board command
//! carries absolute state, so running it on a target that changed since would
//! undo that change. The guard keeps each touched target as it stood before
//! and after the edit; the edit may run while every target is still one of
//! the two. Fields are not merged. Rules: `docs/COMMAND_LOG.md`, "Replay".

use signex_net::board::BoardNetEdit;
use signex_types::pcb::{
    ComponentAttributes, Footprint, LayerDef, NetDef, Pad, PadNet, PcbBoard, Point, Room,
    SymbolLink,
};
use signex_types::pcb_options::BoardOptions;
use signex_types::pcb_panel::BoardPanel;
use signex_types::pcb_sketch::BoardSketch;
use signex_types::pcb_stack::LayerStack;
use signex_types::property::PcbProperty;
use signex_types::sketch::board_attr::SketchBoardItems;
use uuid::Uuid;

use super::command::BoardCommand;
use super::items::{self, BoardItem, BoardItemId};

/// One part of a board a command writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Item(BoardItemId),
    Pad(Uuid, Uuid),
    Place(Uuid),
    Rotation(Uuid),
    Text(Uuid),
    Link(Uuid),
    Locked(Uuid),
    Property(Uuid, usize),
    Properties(Uuid),
    Component(Uuid),
    Footprint(Uuid),
    NetTable,
    PadNet(Uuid, Uuid),
    ZoneNet(Uuid),
    Outline,
    Sketch,
    Grid,
    Options,
    Panel,
    Stack,
    StackName,
    Room(Uuid),
}

/// What a [`Target`] holds.
#[derive(Debug, Clone, PartialEq)]
enum TargetState {
    Item(Box<BoardItem>),
    Pad(Box<Pad>),
    Place(Point),
    Rotation(f64),
    Text(String, String),
    Link(Option<Box<SymbolLink>>),
    Locked(bool),
    Property(Box<PcbProperty>),
    Properties(Vec<PcbProperty>),
    Component(Box<ComponentAttributes>),
    Footprint(Box<Footprint>),
    NetTable(Vec<NetDef>),
    PadNet(Option<PadNet>, bool),
    ZoneNet(String),
    Outline(Vec<Point>),
    Sketch(Option<Box<BoardSketch>>),
    Grid(Option<f64>),
    Options(Box<BoardOptions>),
    Panel(Option<Box<BoardPanel>>),
    Stack(Box<(Option<LayerStack>, Vec<LayerDef>, f64)>),
    StackName(Option<String>),
    Room(Box<Room>),
}

#[derive(Debug, Clone, PartialEq)]
struct GuardedTarget {
    target: Target,
    before: Option<TargetState>,
    after: Option<TargetState>,
}

/// The touched targets of a recorded board edit, before and after it.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardReplayGuard {
    targets: Vec<GuardedTarget>,
    /// A command in the edit has no target that can be checked.
    is_uncheckable: bool,
}

impl BoardReplayGuard {
    /// Read what `commands` are about to write on `board`.
    pub fn before(board: &PcbBoard, commands: &[BoardCommand]) -> Self {
        let mut guard = Self {
            targets: Vec::new(),
            is_uncheckable: false,
        };
        for command in commands {
            let Some(targets) = targets_of(command) else {
                guard.is_uncheckable = true;
                continue;
            };
            for target in targets {
                if !guard.targets.iter().any(|t| t.target == target) {
                    guard.targets.push(GuardedTarget {
                        target,
                        before: read(board, target),
                        after: None,
                    });
                }
            }
        }
        guard
    }

    /// Read the same targets once the commands have run.
    pub fn record_after(&mut self, board: &PcbBoard) {
        for guarded in &mut self.targets {
            guarded.after = read(board, guarded.target);
        }
    }

    /// `self` followed by `later`, run in one dispatch: a target both touch
    /// keeps its first `before` and its last `after`.
    pub fn merged(mut self, later: Self) -> Self {
        self.is_uncheckable |= later.is_uncheckable;
        for next in later.targets {
            match self.targets.iter_mut().find(|t| t.target == next.target) {
                Some(guarded) => guarded.after = next.after,
                None => self.targets.push(next),
            }
        }
        self
    }

    /// Whether every target on `board` is still as the edit found or left it.
    pub fn holds(&self, board: &PcbBoard) -> bool {
        !self.is_uncheckable
            && self.targets.iter().all(|guarded| {
                let live = read(board, guarded.target);
                live == guarded.before || live == guarded.after
            })
    }
}

/// What `command` writes. Empty for a command that carries no recorded state
/// and acts on what is there; `None` for one that cannot be checked: an
/// inverse, or one that adds objects.
fn targets_of(command: &BoardCommand) -> Option<Vec<Target>> {
    use BoardCommand as C;
    Some(match command {
        C::Flip { .. }
        | C::DeleteItems { .. }
        | C::RemoveRoom { .. }
        | C::RemoveSetup
        | C::FitRoomToParts { .. }
        | C::PullPartsIntoRoom { .. } => Vec::new(),
        C::MoveFootprint { footprint, .. } => vec![Target::Place(*footprint)],
        C::Rotate { footprint, .. } => vec![Target::Rotation(*footprint)],
        C::ReplaceItems { items } => items.iter().map(|i| Target::Item(i.id())).collect(),
        C::SetItemsLocked { items } => items.iter().map(|(id, _)| Target::Item(*id)).collect(),
        C::ReplacePads { pads } => pads
            .iter()
            .map(|p| Target::Pad(p.footprint, p.pad.uuid))
            .collect(),
        C::ReplaceFootprint { footprint, .. } => vec![Target::Footprint(*footprint)],
        C::SetFootprintText { footprint, .. } => vec![Target::Text(*footprint)],
        C::SetFootprintLink { footprint, .. } => vec![Target::Link(*footprint)],
        C::SetFootprintLocked { footprint, .. } => vec![Target::Locked(*footprint)],
        C::SetFootprintProperty {
            footprint, index, ..
        } => vec![Target::Property(*footprint, *index)],
        C::InsertFootprintProperty { footprint, .. }
        | C::RemoveFootprintProperty { footprint, .. } => vec![Target::Properties(*footprint)],
        C::SetFootprintComponent { footprint, .. } => vec![Target::Component(*footprint)],
        C::SetNets(edit) => net_targets(edit),
        C::SetBoardOutline { .. } => vec![Target::Outline],
        C::SetBoardSketch { outline, .. } => sketch_targets(outline.is_some()),
        C::SetBoardSketchBake { outline, items, .. } => bake_targets(outline.is_some(), items),
        C::SetGridSize { .. } => vec![Target::Grid],
        C::SetBoardOptions { .. } => vec![Target::Options],
        C::SetBoardPanel { .. } => vec![Target::Panel],
        C::SetLayerStack { .. } => vec![Target::Stack],
        C::SetStackupName { .. } => vec![Target::StackName],
        C::EditRoom { room } => vec![Target::Room(room.uuid)],
        C::MoveRoom { room, .. } => vec![Target::Room(*room)],
        C::RestoreFace { .. }
        | C::RestoreBoardSketchBake(_)
        | C::RestoreItems { .. }
        | C::AddItems { .. }
        | C::RestoreReplacedItems { .. }
        | C::RestoreReplacedPads { .. }
        | C::RestoreFootprint { .. }
        | C::RestoreFootprintText { .. }
        | C::RestoreLayerStack(_)
        | C::AddGraphics { .. }
        | C::RemoveGraphics { .. }
        | C::AddRoom { .. }
        | C::RestoreRoom { .. }
        | C::PlaceRoom { .. } => return None,
    })
}

fn net_targets(edit: &BoardNetEdit) -> Vec<Target> {
    let pads = edit.pads.iter().map(|c| Target::PadNet(c.footprint, c.pad));
    let zones = edit.zones.iter().map(|c| Target::ZoneNet(c.zone));
    std::iter::once(Target::NetTable)
        .chain(pads)
        .chain(zones)
        .collect()
}

fn sketch_targets(writes_outline: bool) -> Vec<Target> {
    let mut targets = vec![Target::Sketch];
    if writes_outline {
        targets.push(Target::Outline);
    }
    targets
}

/// The sketch, and the zones, drawings and footprints its bake writes.
fn bake_targets(writes_outline: bool, items: &SketchBoardItems) -> Vec<Target> {
    let zones = (items.zones.iter()).map(|z| Target::Item(BoardItemId::Zone(z.uuid)));
    let graphics = (items.graphics.iter()).map(|g| Target::Item(BoardItemId::Graphic(g.uuid)));
    let anchors = (items.anchors.iter()).map(|a| Target::Place(a.footprint));
    sketch_targets(writes_outline)
        .into_iter()
        .chain(zones)
        .chain(graphics)
        .chain(anchors)
        .collect()
}

fn read(board: &PcbBoard, target: Target) -> Option<TargetState> {
    use TargetState as S;
    let footprint = |id: Uuid| board.footprints.iter().find(|fp| fp.uuid == id);
    let pad = |fp: Uuid, id: Uuid| footprint(fp)?.pads.iter().find(|p| p.uuid == id);
    Some(match target {
        Target::Item(id) => S::Item(Box::new(read_item(board, id)?)),
        Target::Pad(fp, id) => S::Pad(Box::new(pad(fp, id)?.clone())),
        Target::Place(id) => S::Place(footprint(id)?.position),
        Target::Rotation(id) => S::Rotation(footprint(id)?.rotation),
        Target::Text(id) => {
            let fp = footprint(id)?;
            S::Text(fp.reference.clone(), fp.value.clone())
        }
        Target::Link(id) => S::Link(footprint(id)?.schematic_link.clone().map(Box::new)),
        Target::Locked(id) => S::Locked(footprint(id)?.locked),
        Target::Property(id, index) => {
            S::Property(Box::new(footprint(id)?.properties.get(index)?.clone()))
        }
        Target::Properties(id) => S::Properties(footprint(id)?.properties.clone()),
        Target::Component(id) => S::Component(footprint(id)?.component.clone()),
        Target::Footprint(id) => S::Footprint(Box::new(footprint(id)?.clone())),
        Target::NetTable => S::NetTable(board.nets.clone()),
        Target::PadNet(fp, id) => {
            let pad = pad(fp, id)?;
            S::PadNet(pad.net.clone(), pad.net_locked)
        }
        Target::ZoneNet(id) => {
            S::ZoneNet(board.zones.iter().find(|z| z.uuid == id)?.net_name.clone())
        }
        Target::Outline => S::Outline(board.outline.clone()),
        Target::Sketch => S::Sketch(board.sketch.clone().map(Box::new)),
        Target::Grid => S::Grid(board.setup.as_ref().map(|setup| setup.grid_size)),
        Target::Options => S::Options(Box::new(board.options.clone())),
        Target::Panel => S::Panel(board.options.panel.map(Box::new)),
        Target::Stack => S::Stack(Box::new((
            board.stackup.clone(),
            board.layers.clone(),
            board.thickness,
        ))),
        Target::StackName => S::StackName(board.stackup.as_ref().and_then(|s| s.name.clone())),
        Target::Room(id) => S::Room(Box::new(board.rooms.iter().find(|r| r.uuid == id)?.clone())),
    })
}

fn read_item(board: &PcbBoard, id: BoardItemId) -> Option<BoardItem> {
    let at = items::position(board, id)?;
    Some(match id {
        BoardItemId::Footprint(_) => BoardItem::Footprint(board.footprints[at].clone()),
        BoardItemId::Segment(_) => BoardItem::Segment(board.segments[at].clone()),
        BoardItemId::Via(_) => BoardItem::Via(board.vias[at].clone()),
        BoardItemId::Zone(_) => BoardItem::Zone(board.zones[at].clone()),
        BoardItemId::Text(_) => BoardItem::Text(board.texts[at].clone()),
        BoardItemId::Arc(_) => BoardItem::Arc(board.free.arcs[at].clone()),
        BoardItemId::FreePad(_) => BoardItem::FreePad(board.free.pads[at].clone()),
        BoardItemId::Dimension(_) => BoardItem::Dimension(board.free.dimensions[at].clone()),
        BoardItemId::Graphic(_) => BoardItem::Graphic(board.graphics[at].clone()),
    })
}
