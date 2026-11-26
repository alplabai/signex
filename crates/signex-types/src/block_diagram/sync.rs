//! Generate and regenerate a block diagram from the schematic hierarchy
//! (#2968, mockup row 171). The hierarchy is read elsewhere
//! (`signex_net::derive_block_diagram`) into a [`DerivedDiagram`]: the
//! blocks, ports and connections the schematic says there are. This module
//! merges that into a diagram: the content follows the schematic, the
//! person's layout, names, notes and hand-drawn blocks and connections stay.
//! The rules are in `docs/architecture/snxblk-format.md`, "Generate and
//! regenerate".

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::Serialize;
use uuid::Uuid;

use super::{
    Block, BlockDiagram, BlockDiagramError, BlockPort, BlockShape, Connection,
    DEFAULT_BLOCK_HEIGHT_MM, DEFAULT_BLOCK_WIDTH_MM, Endpoint, Interface, LinkState, PortDirection,
    PortSide, SheetLink,
};
use crate::document_id::DocumentId;
use crate::object_id::IdSource;
use crate::schematic::Point;

/// Distance between two ports on one side of a generated block, and from
/// the first port to the block's corner, mm.
pub const PORT_PITCH_MM: f64 = 5.0;

/// Space around and between generated blocks, mm.
const BLOCK_GAP_MM: f64 = 20.0;

/// What the schematic hierarchy says a block diagram holds.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DerivedDiagram {
    /// One block per sheet, in hierarchy walk order.
    pub blocks: Vec<DerivedBlock>,
    /// One connection per signal group and pair of blocks it joins.
    pub connections: Vec<DerivedConnection>,
}

/// The block one sheet stands for.
#[derive(Debug, Clone, PartialEq)]
pub struct DerivedBlock {
    /// The sheet's document id.
    pub sheet: DocumentId,
    /// The sheet's path relative to the project folder, `/`-separated.
    pub path: String,
    /// The name a new block gets: the sheet symbol's name, else the file
    /// stem.
    pub name: String,
    /// The sheet's depth in the hierarchy; a new block is laid out in the
    /// row of its depth.
    pub depth: usize,
    /// The power nets the sheet touches.
    pub rails: Vec<String>,
    /// The sheet's entries, in the order its sheet symbol lists them.
    pub ports: Vec<DerivedPort>,
}

/// A port a sheet entry stands for.
#[derive(Debug, Clone, PartialEq)]
pub struct DerivedPort {
    /// The entry's name.
    pub name: String,
    /// The entry's direction.
    pub direction: PortDirection,
}

/// A connection one signal group makes between two sheets.
#[derive(Debug, Clone, PartialEq)]
pub struct DerivedConnection {
    /// The net, bus or harness name the group goes by; stored as
    /// [`Connection::derived_from`] and how a regenerate finds it again.
    pub group: String,
    /// The interface the group carries.
    pub interface: Interface,
    /// The text drawn on the connection.
    pub label: String,
    /// Where the flow starts.
    pub from: DerivedEnd,
    /// Where the flow ends.
    pub to: DerivedEnd,
}

/// One end of a derived connection: a sheet, and the entry on it by name.
#[derive(Debug, Clone, PartialEq)]
pub struct DerivedEnd {
    /// The sheet's document id.
    pub sheet: DocumentId,
    /// The entry the group runs through; `None` ends on the block's edge.
    pub port: Option<String>,
}

/// One change a regenerate makes, as the command lists it before it is
/// applied.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum DiagramChange {
    /// A sheet with no block got one.
    BlockAdded {
        block: Uuid,
        name: String,
        sheet: DocumentId,
        path: String,
    },
    /// A linked sheet was renamed or moved; the block follows it.
    SheetMoved {
        block: Uuid,
        sheet: DocumentId,
        from: String,
        to: String,
    },
    /// A linked sheet left the project; the block stays, marked unlinked.
    SheetRemoved {
        block: Uuid,
        sheet: DocumentId,
        path: String,
    },
    /// The sheet of an unlinked block is back in the project.
    SheetReturned {
        block: Uuid,
        sheet: DocumentId,
        path: String,
    },
    /// A linked block's power nets changed.
    RailsChanged { block: Uuid, rails: Vec<String> },
    /// A sheet gained an entry.
    PortAdded {
        block: Uuid,
        port: Uuid,
        name: String,
    },
    /// A sheet lost an entry.
    PortRemoved {
        block: Uuid,
        port: Uuid,
        name: String,
    },
    /// An entry changed direction.
    PortChanged {
        block: Uuid,
        port: Uuid,
        name: String,
        direction: PortDirection,
    },
    /// A signal group joins two blocks it did not join before.
    ConnectionAdded {
        connection: Uuid,
        derived_from: String,
        from: Uuid,
        to: Uuid,
    },
    /// A generated connection's interface, label or ports changed.
    ConnectionChanged {
        connection: Uuid,
        derived_from: String,
        interface: Interface,
        label: String,
    },
    /// A generated connection's group no longer joins its two blocks.
    ConnectionRemoved {
        connection: Uuid,
        derived_from: String,
    },
}

/// A regenerated diagram and what changed, in the order it was done.
#[derive(Debug, Clone, PartialEq)]
pub struct Regenerated {
    /// The diagram after the changes.
    pub diagram: BlockDiagram,
    /// The changes; empty when the diagram already matched the schematic.
    pub changes: Vec<DiagramChange>,
}

/// A new diagram with id `id` holding what `derived` says: one block per
/// sheet, laid out in rows by depth, and every derived connection.
pub fn generate_block_diagram(
    derived: &DerivedDiagram,
    ids: &mut dyn IdSource,
    id: DocumentId,
) -> Result<BlockDiagram, BlockDiagramError> {
    regenerate_block_diagram(&BlockDiagram::with_id(id), derived, ids).map(|r| r.diagram)
}

/// Bring `diagram` in line with `derived`. A sheet with no block gets one,
/// laid out below the existing blocks; a linked block follows its sheet's
/// path, rails and entries and is marked unlinked when the sheet is gone;
/// generated connections are added, changed or removed to match. Positions,
/// sizes, names, colours, notes, groups, hand-drawn blocks and connections
/// stay. A hand-drawn connection on an entry the sheet lost ends on the
/// block's edge instead. `diagram` itself is not changed.
pub fn regenerate_block_diagram(
    diagram: &BlockDiagram,
    derived: &DerivedDiagram,
    ids: &mut dyn IdSource,
) -> Result<Regenerated, BlockDiagramError> {
    let mut next = diagram.clone();
    let mut changes = Vec::new();
    let top = next
        .blocks
        .iter()
        .map(|b| b.position.y + b.height_mm + BLOCK_GAP_MM)
        .fold(BLOCK_GAP_MM, f64::max);
    let mut added: Vec<(usize, usize)> = Vec::new();
    for want in &derived.blocks {
        match next.blocks.iter().position(|b| is_linked_to(b, want.sheet)) {
            Some(at) => follow_sheet(&mut next, at, want, ids, &mut changes),
            None => {
                let block = new_block(want, ids);
                changes.push(DiagramChange::BlockAdded {
                    block: block.id,
                    name: block.name.clone(),
                    sheet: want.sheet,
                    path: want.path.clone(),
                });
                added.push((next.blocks.len(), want.depth));
                next.blocks.push(block);
            }
        }
    }
    lay_out(&mut next.blocks, &added, top);
    mark_removed_sheets(&mut next, derived, &mut changes);
    sync_connections(&mut next, derived, ids, &mut changes);
    if let Some(fault) = next.fault() {
        return Err(fault.into());
    }
    Ok(Regenerated {
        diagram: next,
        changes,
    })
}

fn is_linked_to(block: &Block, sheet: DocumentId) -> bool {
    block.sheet.as_ref().is_some_and(|link| link.sheet == sheet)
}

/// A new block for `want`, its ports on the sides their direction gives.
fn new_block(want: &DerivedBlock, ids: &mut dyn IdSource) -> Block {
    let mut block = Block {
        id: ids.next_id(),
        name: block_name(want),
        function: String::new(),
        note: String::new(),
        shape: BlockShape::Rectangle,
        position: Point::new(0.0, 0.0),
        width_mm: DEFAULT_BLOCK_WIDTH_MM,
        height_mm: DEFAULT_BLOCK_HEIGHT_MM,
        color: None,
        group: None,
        rails: clean_names(&want.rails),
        specs: BTreeMap::new(),
        ports: Vec::new(),
        sheet: Some(SheetLink {
            sheet: want.sheet,
            path: want.path.clone(),
            state: LinkState::Linked,
        }),
    };
    for port in unique_ports(&want.ports) {
        push_port(&mut block, port, ids);
    }
    block
}

fn block_name(want: &DerivedBlock) -> String {
    let name = want.name.trim();
    if !name.is_empty() {
        return name.to_string();
    }
    let file = want.path.rsplit('/').next().unwrap_or(&want.path);
    let stem = file.rsplit_once('.').map_or(file, |(stem, _)| stem).trim();
    if stem.is_empty() {
        "Sheet".to_string()
    } else {
        stem.to_string()
    }
}

/// The side a port of `direction` sits on: inputs left, the rest right.
fn side_of(direction: PortDirection) -> PortSide {
    match direction {
        PortDirection::Input => PortSide::Left,
        PortDirection::Output | PortDirection::Bidirectional => PortSide::Right,
    }
}

/// Add `port` below the last port on its side, growing the block to fit.
fn push_port(block: &mut Block, port: &DerivedPort, ids: &mut dyn IdSource) -> Uuid {
    let side = side_of(port.direction);
    let offset = block
        .ports
        .iter()
        .filter(|p| p.side == side)
        .map(|p| p.offset_mm)
        .fold(0.0, f64::max)
        + PORT_PITCH_MM;
    block.height_mm = block.height_mm.max(offset + PORT_PITCH_MM);
    let id = ids.next_id();
    block.ports.push(BlockPort {
        id,
        name: port.name.trim().to_string(),
        side,
        offset_mm: offset,
        direction: port.direction,
    });
    id
}

/// The ports with a name, each name once, first one kept.
fn unique_ports(ports: &[DerivedPort]) -> Vec<&DerivedPort> {
    let mut seen = HashSet::new();
    ports
        .iter()
        .filter(|p| !p.name.trim().is_empty() && seen.insert(p.name.trim()))
        .collect()
}

fn clean_names(names: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for name in names.iter().map(|n| n.trim()) {
        if !name.is_empty() && !out.iter().any(|n| n == name) {
            out.push(name.to_string());
        }
    }
    out
}

/// Lay the new blocks `added` (index, depth) out from `top` down, one row
/// per depth, left to right in the order they were added.
fn lay_out(blocks: &mut [Block], added: &[(usize, usize)], top: f64) {
    let mut rows: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &(at, depth) in added {
        rows.entry(depth).or_default().push(at);
    }
    let mut y = top;
    for row in rows.values() {
        let mut x = BLOCK_GAP_MM;
        let mut height: f64 = 0.0;
        for &at in row {
            let block = &mut blocks[at];
            block.position = Point::new(x, y);
            x += block.width_mm + BLOCK_GAP_MM;
            height = height.max(block.height_mm);
        }
        y += height + BLOCK_GAP_MM;
    }
}

/// Bring the linked block at `at` in line with its sheet: link state, path,
/// rails and ports.
fn follow_sheet(
    diagram: &mut BlockDiagram,
    at: usize,
    want: &DerivedBlock,
    ids: &mut dyn IdSource,
    changes: &mut Vec<DiagramChange>,
) {
    let block = &mut diagram.blocks[at];
    let id = block.id;
    if let Some(link) = block.sheet.as_mut() {
        if link.state == LinkState::Unlinked {
            link.state = LinkState::Linked;
            changes.push(DiagramChange::SheetReturned {
                block: id,
                sheet: want.sheet,
                path: want.path.clone(),
            });
        }
        if link.path != want.path && !want.path.trim().is_empty() {
            let from = std::mem::replace(&mut link.path, want.path.clone());
            changes.push(DiagramChange::SheetMoved {
                block: id,
                sheet: want.sheet,
                from,
                to: want.path.clone(),
            });
        }
    }
    let rails = clean_names(&want.rails);
    if block.rails != rails {
        block.rails = rails.clone();
        changes.push(DiagramChange::RailsChanged { block: id, rails });
    }
    let removed = sync_ports(block, want, ids, changes);
    detach_from_ports(diagram, &removed);
}

/// Add the entries the block lacks, drop the ports the sheet lost and
/// follow a changed direction; answers the ports removed.
fn sync_ports(
    block: &mut Block,
    want: &DerivedBlock,
    ids: &mut dyn IdSource,
    changes: &mut Vec<DiagramChange>,
) -> Vec<Uuid> {
    let wanted = unique_ports(&want.ports);
    let id = block.id;
    let mut removed = Vec::new();
    block.ports.retain(|port| {
        let kept = wanted.iter().any(|w| w.name.trim() == port.name);
        if !kept {
            removed.push(port.id);
            changes.push(DiagramChange::PortRemoved {
                block: id,
                port: port.id,
                name: port.name.clone(),
            });
        }
        kept
    });
    for w in wanted {
        let name = w.name.trim();
        match block.ports.iter_mut().find(|p| p.name == name) {
            Some(port) if port.direction != w.direction => {
                port.direction = w.direction;
                changes.push(DiagramChange::PortChanged {
                    block: id,
                    port: port.id,
                    name: port.name.clone(),
                    direction: w.direction,
                });
            }
            Some(_) => {}
            None => {
                let port = push_port(block, w, ids);
                changes.push(DiagramChange::PortAdded {
                    block: id,
                    port,
                    name: name.to_string(),
                });
            }
        }
    }
    removed
}

/// Connections that ended on a removed port end on its block's edge.
fn detach_from_ports(diagram: &mut BlockDiagram, removed: &[Uuid]) {
    let detach = |end: &mut Endpoint| {
        if end.port.is_some_and(|p| removed.contains(&p)) {
            end.port = None;
        }
    };
    for link in &mut diagram.connections {
        detach(&mut link.from);
        detach(&mut link.to);
    }
}

/// Mark unlinked every linked block whose sheet the hierarchy no longer has.
fn mark_removed_sheets(
    diagram: &mut BlockDiagram,
    derived: &DerivedDiagram,
    changes: &mut Vec<DiagramChange>,
) {
    let present: HashSet<DocumentId> = derived.blocks.iter().map(|b| b.sheet).collect();
    for block in &mut diagram.blocks {
        let id = block.id;
        let Some(link) = block.sheet.as_mut() else {
            continue;
        };
        if link.state == LinkState::Linked && !present.contains(&link.sheet) {
            link.state = LinkState::Unlinked;
            changes.push(DiagramChange::SheetRemoved {
                block: id,
                sheet: link.sheet,
                path: link.path.clone(),
            });
        }
    }
}

/// Add, change and remove generated connections to match `derived`; a
/// hand-drawn connection (no `derived_from`) is never touched here.
fn sync_connections(
    diagram: &mut BlockDiagram,
    derived: &DerivedDiagram,
    ids: &mut dyn IdSource,
    changes: &mut Vec<DiagramChange>,
) {
    let linked: HashMap<DocumentId, Uuid> = diagram
        .blocks
        .iter()
        .filter_map(|b| {
            let link = b.sheet.as_ref()?;
            (link.state == LinkState::Linked).then_some((link.sheet, b.id))
        })
        .collect();
    let mut kept: HashSet<Uuid> = HashSet::new();
    for want in &derived.connections {
        let Some(from) = endpoint(diagram, &linked, &want.from) else {
            continue;
        };
        let Some(to) = endpoint(diagram, &linked, &want.to) else {
            continue;
        };
        if from == to {
            continue;
        }
        let interface = Interface::from_token(want.interface.token());
        let label = want.label.trim().to_string();
        let existing = diagram.connections.iter_mut().find(|c| {
            !kept.contains(&c.id)
                && c.derived_from.as_deref() == Some(want.group.as_str())
                && c.from.block == from.block
                && c.to.block == to.block
        });
        match existing {
            Some(link) => {
                kept.insert(link.id);
                if link.interface != interface
                    || link.label != label
                    || link.from != from
                    || link.to != to
                {
                    link.interface = interface.clone();
                    link.label = label.clone();
                    link.from = from;
                    link.to = to;
                    changes.push(DiagramChange::ConnectionChanged {
                        connection: link.id,
                        derived_from: want.group.clone(),
                        interface,
                        label,
                    });
                }
            }
            None => {
                let id = ids.next_id();
                kept.insert(id);
                diagram.connections.push(Connection {
                    id,
                    from,
                    to,
                    interface,
                    label,
                    derived_from: Some(want.group.clone()),
                });
                changes.push(DiagramChange::ConnectionAdded {
                    connection: id,
                    derived_from: want.group.clone(),
                    from: from.block,
                    to: to.block,
                });
            }
        }
    }
    diagram.connections.retain(|c| {
        let Some(group) = c.derived_from.as_ref() else {
            return true;
        };
        if kept.contains(&c.id) {
            return true;
        }
        changes.push(DiagramChange::ConnectionRemoved {
            connection: c.id,
            derived_from: group.clone(),
        });
        false
    });
}

/// The diagram endpoint `end` names: the linked block of its sheet and the
/// port of that name on it, if there is one.
fn endpoint(
    diagram: &BlockDiagram,
    linked: &HashMap<DocumentId, Uuid>,
    end: &DerivedEnd,
) -> Option<Endpoint> {
    let block = *linked.get(&end.sheet)?;
    let port = end.port.as_deref().map(str::trim).and_then(|name| {
        diagram
            .block(block)?
            .ports
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.id)
    });
    Some(Endpoint { block, port })
}
