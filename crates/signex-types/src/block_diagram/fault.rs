//! The one consistency check of a block diagram: every id unique, every
//! reference resolvable, every length a number the file can hold. The
//! writer refuses a diagram with a fault and the reader refuses a file with
//! one, so a bad diagram is never saved and never opened half-read.

use std::collections::HashSet;

use uuid::Uuid;

use super::{Block, BlockDiagram, Connection, Endpoint, Interface};

/// What makes a block diagram inconsistent. Each names the object.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DiagramFault {
    #[error("id {0} is used by more than one object")]
    DuplicateId(Uuid),
    #[error("block {0} has no name")]
    BlockWithoutName(Uuid),
    #[error("block {0} has a size that is not a number above zero")]
    BadBlockSize(Uuid),
    #[error("block {0} has a position that is not a number")]
    BadBlockPosition(Uuid),
    #[error("block {block} names group {group}, which the diagram does not have")]
    MissingGroup { block: Uuid, group: Uuid },
    #[error("block {0} links to a sheet with no path")]
    SheetLinkWithoutPath(Uuid),
    #[error("port {0} has no name")]
    PortWithoutName(Uuid),
    #[error("port {0} has an offset that is not a number at or above zero")]
    BadPortOffset(Uuid),
    #[error("group {0} has no name")]
    GroupWithoutName(Uuid),
    #[error("connection {connection} ends on block {block}, which the diagram does not have")]
    MissingBlock { connection: Uuid, block: Uuid },
    #[error("connection {connection} ends on port {port}, which its block does not have")]
    MissingPort { connection: Uuid, port: Uuid },
    #[error("connection {0} starts and ends at the same place")]
    ClosedConnection(Uuid),
    #[error("connection {0} has an interface with no name")]
    InterfaceWithoutName(Uuid),
}

impl BlockDiagram {
    /// The first fault of the diagram, in file order; `None` when it is
    /// consistent.
    pub fn fault(&self) -> Option<DiagramFault> {
        self.duplicate_id()
            .map(DiagramFault::DuplicateId)
            .or_else(|| self.groups.iter().find_map(group_fault))
            .or_else(|| self.blocks.iter().find_map(|b| self.block_fault(b)))
            .or_else(|| self.connections.iter().find_map(|c| self.link_fault(c)))
    }

    fn duplicate_id(&self) -> Option<Uuid> {
        let groups = self.groups.iter().map(|g| g.id);
        let blocks = self.blocks.iter().map(|b| b.id);
        let ports = self
            .blocks
            .iter()
            .flat_map(|b| b.ports.iter().map(|p| p.id));
        let links = self.connections.iter().map(|c| c.id);
        let mut seen = HashSet::new();
        groups
            .chain(blocks)
            .chain(ports)
            .chain(links)
            .find(|id| !seen.insert(*id))
    }

    fn block_fault(&self, block: &Block) -> Option<DiagramFault> {
        let id = block.id;
        if block.name.trim().is_empty() {
            return Some(DiagramFault::BlockWithoutName(id));
        }
        let sized = |v: f64| v.is_finite() && v > 0.0;
        if !sized(block.width_mm) || !sized(block.height_mm) {
            return Some(DiagramFault::BadBlockSize(id));
        }
        if !block.position.x.is_finite() || !block.position.y.is_finite() {
            return Some(DiagramFault::BadBlockPosition(id));
        }
        if let Some(group) = block.group.filter(|g| self.group(*g).is_none()) {
            return Some(DiagramFault::MissingGroup { block: id, group });
        }
        if block
            .sheet
            .as_ref()
            .is_some_and(|s| s.path.trim().is_empty())
        {
            return Some(DiagramFault::SheetLinkWithoutPath(id));
        }
        block.ports.iter().find_map(|port| {
            if port.name.trim().is_empty() {
                Some(DiagramFault::PortWithoutName(port.id))
            } else if !(port.offset_mm.is_finite() && port.offset_mm >= 0.0) {
                Some(DiagramFault::BadPortOffset(port.id))
            } else {
                None
            }
        })
    }

    fn link_fault(&self, link: &Connection) -> Option<DiagramFault> {
        let id = link.id;
        if link.from == link.to {
            return Some(DiagramFault::ClosedConnection(id));
        }
        if matches!(&link.interface, Interface::Other(name) if name.trim().is_empty()) {
            return Some(DiagramFault::InterfaceWithoutName(id));
        }
        [link.from, link.to]
            .into_iter()
            .find_map(|end| self.endpoint_fault(id, end))
    }

    fn endpoint_fault(&self, connection: Uuid, end: Endpoint) -> Option<DiagramFault> {
        let Some(block) = self.block(end.block) else {
            return Some(DiagramFault::MissingBlock {
                connection,
                block: end.block,
            });
        };
        let port = end.port.filter(|p| block.port(*p).is_none())?;
        Some(DiagramFault::MissingPort { connection, port })
    }
}

fn group_fault(group: &super::BlockGroup) -> Option<DiagramFault> {
    group
        .name
        .trim()
        .is_empty()
        .then_some(DiagramFault::GroupWithoutName(group.id))
}
