//! The edits of a block diagram. Each one checks the diagram it would leave
//! with [`BlockDiagram::fault`] and changes nothing when that fails, so a
//! diagram edited only through these methods always saves. New ids come
//! from the caller's [`IdSource`] (ADR-0006).

use std::collections::BTreeMap;

use uuid::Uuid;

use super::{
    Block, BlockDiagram, BlockGroup, BlockPort, BlockShape, Connection, DEFAULT_BLOCK_HEIGHT_MM,
    DEFAULT_BLOCK_WIDTH_MM, DiagramFault, Endpoint, Interface, LinkState, PortDirection, PortSide,
    SheetLink,
};
use crate::document_id::DocumentId;
use crate::object_id::IdSource;
use crate::schematic::Point;
use crate::theme::Color;

/// Why an edit was refused. The diagram is unchanged.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BlockDiagramError {
    #[error("the diagram has no block {0}")]
    UnknownBlock(Uuid),
    #[error("the diagram has no port {0}")]
    UnknownPort(Uuid),
    #[error("the diagram has no connection {0}")]
    UnknownConnection(Uuid),
    #[error("the diagram has no group {0}")]
    UnknownGroup(Uuid),
    #[error(transparent)]
    Fault(#[from] DiagramFault),
}

/// The properties a new block is made with.
#[derive(Debug, Clone, PartialEq)]
pub struct BlockFields {
    /// See [`Block::name`].
    pub name: String,
    /// See [`Block::function`].
    pub function: String,
    /// See [`Block::note`].
    pub note: String,
    /// See [`Block::shape`].
    pub shape: BlockShape,
    /// See [`Block::position`].
    pub position: Point,
    /// See [`Block::width_mm`].
    pub width_mm: f64,
    /// See [`Block::height_mm`].
    pub height_mm: f64,
    /// See [`Block::color`].
    pub color: Option<Color>,
    /// See [`Block::group`].
    pub group: Option<Uuid>,
    /// See [`Block::rails`].
    pub rails: Vec<String>,
    /// See [`Block::specs`].
    pub specs: BTreeMap<String, String>,
}

impl BlockFields {
    /// A rectangle named `name` at `position`, default size, nothing else.
    pub fn named(name: &str, position: Point) -> Self {
        Self {
            name: name.to_string(),
            function: String::new(),
            note: String::new(),
            shape: BlockShape::Rectangle,
            position,
            width_mm: DEFAULT_BLOCK_WIDTH_MM,
            height_mm: DEFAULT_BLOCK_HEIGHT_MM,
            color: None,
            group: None,
            rails: Vec::new(),
            specs: BTreeMap::new(),
        }
    }
}

/// A change to a block's properties: `None` leaves one as it is. For the
/// optional properties, `Some(None)` clears it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BlockPatch {
    /// A new [`Block::name`], trimmed.
    pub name: Option<String>,
    /// A new [`Block::function`], trimmed.
    pub function: Option<String>,
    /// A new [`Block::note`].
    pub note: Option<String>,
    /// A new [`Block::shape`].
    pub shape: Option<BlockShape>,
    /// A new [`Block::position`].
    pub position: Option<Point>,
    /// A new [`Block::width_mm`].
    pub width_mm: Option<f64>,
    /// A new [`Block::height_mm`].
    pub height_mm: Option<f64>,
    /// A new [`Block::color`]; `Some(None)` returns to the theme's.
    pub color: Option<Option<Color>>,
    /// A new [`Block::group`]; `Some(None)` takes the block out of its group.
    pub group: Option<Option<Uuid>>,
    /// New [`Block::rails`], replacing the list, cleaned as in `add_block`.
    pub rails: Option<Vec<String>>,
    /// New [`Block::specs`], replacing the map, cleaned as in `add_block`.
    pub specs: Option<BTreeMap<String, String>>,
}

/// A removed block and the connections that went with it, so a caller can
/// report them.
#[derive(Debug, Clone, PartialEq)]
pub struct RemovedBlock {
    /// The block as it was.
    pub block: Block,
    /// The connections that ended on it, in diagram order.
    pub connections: Vec<Connection>,
}

impl BlockDiagram {
    /// Add a block made from `fields`; answers its id. Names, rails and
    /// spec names are trimmed, blank rails and specs dropped, a repeated
    /// rail kept once.
    pub fn add_block(
        &mut self,
        ids: &mut dyn IdSource,
        fields: BlockFields,
    ) -> Result<Uuid, BlockDiagramError> {
        let id = ids.next_id();
        let block = Block {
            id,
            name: fields.name.trim().to_string(),
            function: fields.function.trim().to_string(),
            note: fields.note,
            shape: fields.shape,
            position: fields.position,
            width_mm: fields.width_mm,
            height_mm: fields.height_mm,
            color: fields.color,
            group: fields.group,
            rails: clean_rails(fields.rails),
            specs: clean_specs(fields.specs),
            ports: Vec::new(),
            sheet: None,
        };
        self.commit(|next| next.blocks.push(block))?;
        Ok(id)
    }

    /// Change the properties `patch` names on block `id`.
    pub fn set_block(&mut self, id: Uuid, patch: BlockPatch) -> Result<(), BlockDiagramError> {
        self.block(id).ok_or(BlockDiagramError::UnknownBlock(id))?;
        self.commit(|next| {
            if let Some(block) = next.blocks.iter_mut().find(|b| b.id == id) {
                apply_patch(block, patch);
            }
        })
    }

    /// Remove block `id` and every connection that ends on it.
    pub fn remove_block(&mut self, id: Uuid) -> Result<RemovedBlock, BlockDiagramError> {
        let at = self
            .blocks
            .iter()
            .position(|b| b.id == id)
            .ok_or(BlockDiagramError::UnknownBlock(id))?;
        let (connections, kept): (Vec<Connection>, Vec<Connection>) =
            self.connections.drain(..).partition(|c| c.touches(id));
        self.connections = kept;
        Ok(RemovedBlock {
            block: self.blocks.remove(at),
            connections,
        })
    }

    /// Add a port named `name` to block `block`; answers its id.
    pub fn add_port(
        &mut self,
        ids: &mut dyn IdSource,
        block: Uuid,
        name: &str,
        side: PortSide,
        offset_mm: f64,
        direction: PortDirection,
    ) -> Result<Uuid, BlockDiagramError> {
        self.block(block)
            .ok_or(BlockDiagramError::UnknownBlock(block))?;
        let port = BlockPort {
            id: ids.next_id(),
            name: name.trim().to_string(),
            side,
            offset_mm,
            direction,
        };
        let id = port.id;
        self.commit(|next| {
            if let Some(owner) = next.blocks.iter_mut().find(|b| b.id == block) {
                owner.ports.push(port);
            }
        })?;
        Ok(id)
    }

    /// Remove port `port` and the connections that end on it; answers them.
    pub fn remove_port(&mut self, port: Uuid) -> Result<Vec<Connection>, BlockDiagramError> {
        let owner = self
            .blocks
            .iter_mut()
            .find(|b| b.port(port).is_some())
            .ok_or(BlockDiagramError::UnknownPort(port))?;
        owner.ports.retain(|p| p.id != port);
        let on_port = |end: &Endpoint| end.port == Some(port);
        let (removed, kept): (Vec<Connection>, Vec<Connection>) = self
            .connections
            .drain(..)
            .partition(|c| on_port(&c.from) || on_port(&c.to));
        self.connections = kept;
        Ok(removed)
    }

    /// Connect `from` to `to`, carrying `interface` and drawn with `label`;
    /// answers the connection's id. An [`Interface::Other`] that names a
    /// known interface is stored as that one, as a read of the file would.
    pub fn connect(
        &mut self,
        ids: &mut dyn IdSource,
        from: Endpoint,
        to: Endpoint,
        interface: Interface,
        label: &str,
    ) -> Result<Uuid, BlockDiagramError> {
        let link = Connection {
            id: ids.next_id(),
            from,
            to,
            interface: Interface::from_token(interface.token()),
            label: label.trim().to_string(),
            derived_from: None,
        };
        let id = link.id;
        self.commit(|next| next.connections.push(link))?;
        Ok(id)
    }

    /// Change the interface and the label of connection `id`; `None`
    /// leaves one as it is.
    pub fn set_connection(
        &mut self,
        id: Uuid,
        interface: Option<Interface>,
        label: Option<&str>,
    ) -> Result<(), BlockDiagramError> {
        self.connection(id)
            .ok_or(BlockDiagramError::UnknownConnection(id))?;
        self.commit(|next| {
            if let Some(link) = next.connections.iter_mut().find(|c| c.id == id) {
                if let Some(interface) = interface {
                    link.interface = Interface::from_token(interface.token());
                }
                if let Some(label) = label {
                    link.label = label.trim().to_string();
                }
            }
        })
    }

    /// Remove connection `id`; answers it.
    pub fn disconnect(&mut self, id: Uuid) -> Result<Connection, BlockDiagramError> {
        let at = self
            .connections
            .iter()
            .position(|c| c.id == id)
            .ok_or(BlockDiagramError::UnknownConnection(id))?;
        Ok(self.connections.remove(at))
    }

    /// Link block `block` to a sheet, replacing any link it had.
    pub fn link_sheet(&mut self, block: Uuid, link: SheetLink) -> Result<(), BlockDiagramError> {
        self.block(block)
            .ok_or(BlockDiagramError::UnknownBlock(block))?;
        self.commit(|next| {
            if let Some(owner) = next.blocks.iter_mut().find(|b| b.id == block) {
                owner.sheet = Some(link);
            }
        })
    }

    /// Drop block `block`'s sheet link; answers the link it had.
    pub fn unlink_sheet(&mut self, block: Uuid) -> Result<Option<SheetLink>, BlockDiagramError> {
        let owner = self
            .blocks
            .iter_mut()
            .find(|b| b.id == block)
            .ok_or(BlockDiagramError::UnknownBlock(block))?;
        Ok(owner.sheet.take())
    }

    /// Follow a sheet that was renamed or moved to `path`; answers the
    /// block that follows it, if any.
    pub fn follow_sheet_move(&mut self, sheet: DocumentId, path: &str) -> Option<Uuid> {
        let path = path.trim();
        if path.is_empty() {
            return None;
        }
        let (block, link) = self.sheet_link_mut(sheet)?;
        link.path = path.to_string();
        Some(block)
    }

    /// Mark the block linked to sheet `sheet` as unlinked, because the
    /// sheet left the project; answers the block, if any. The block stays.
    pub fn mark_sheet_removed(&mut self, sheet: DocumentId) -> Option<Uuid> {
        let (block, link) = self.sheet_link_mut(sheet)?;
        link.state = LinkState::Unlinked;
        Some(block)
    }

    /// Add a group named `name`; answers its id.
    pub fn add_group(
        &mut self,
        ids: &mut dyn IdSource,
        name: &str,
        color: Option<Color>,
    ) -> Result<Uuid, BlockDiagramError> {
        let group = BlockGroup {
            id: ids.next_id(),
            name: name.trim().to_string(),
            color,
        };
        let id = group.id;
        self.commit(|next| next.groups.push(group))?;
        Ok(id)
    }

    /// Rename or recolour group `id`; `None` leaves one as it is.
    pub fn set_group(
        &mut self,
        id: Uuid,
        name: Option<&str>,
        color: Option<Option<Color>>,
    ) -> Result<(), BlockDiagramError> {
        self.group(id).ok_or(BlockDiagramError::UnknownGroup(id))?;
        self.commit(|next| {
            if let Some(group) = next.groups.iter_mut().find(|g| g.id == id) {
                if let Some(name) = name {
                    group.name = name.trim().to_string();
                }
                if let Some(color) = color {
                    group.color = color;
                }
            }
        })
    }

    /// Remove group `id`; its blocks stay, in no group. Answers the group.
    pub fn remove_group(&mut self, id: Uuid) -> Result<BlockGroup, BlockDiagramError> {
        let at = self
            .groups
            .iter()
            .position(|g| g.id == id)
            .ok_or(BlockDiagramError::UnknownGroup(id))?;
        for block in self.blocks.iter_mut().filter(|b| b.group == Some(id)) {
            block.group = None;
        }
        Ok(self.groups.remove(at))
    }

    /// Apply `edit` to a copy and keep the copy only when it has no fault.
    fn commit(&mut self, edit: impl FnOnce(&mut Self)) -> Result<(), BlockDiagramError> {
        let mut next = self.clone();
        edit(&mut next);
        if let Some(fault) = next.fault() {
            return Err(fault.into());
        }
        *self = next;
        Ok(())
    }

    fn sheet_link_mut(&mut self, sheet: DocumentId) -> Option<(Uuid, &mut SheetLink)> {
        self.blocks.iter_mut().find_map(|b| {
            let id = b.id;
            b.sheet
                .as_mut()
                .filter(|link| link.sheet == sheet)
                .map(|link| (id, link))
        })
    }
}

fn apply_patch(block: &mut Block, patch: BlockPatch) {
    let BlockPatch {
        name,
        function,
        note,
        shape,
        position,
        width_mm,
        height_mm,
        color,
        group,
        rails,
        specs,
    } = patch;
    if let Some(name) = name {
        block.name = name.trim().to_string();
    }
    if let Some(function) = function {
        block.function = function.trim().to_string();
    }
    if let Some(note) = note {
        block.note = note;
    }
    block.shape = shape.unwrap_or(block.shape);
    block.position = position.unwrap_or(block.position);
    block.width_mm = width_mm.unwrap_or(block.width_mm);
    block.height_mm = height_mm.unwrap_or(block.height_mm);
    block.color = color.unwrap_or(block.color);
    block.group = group.unwrap_or(block.group);
    if let Some(rails) = rails {
        block.rails = clean_rails(rails);
    }
    if let Some(specs) = specs {
        block.specs = clean_specs(specs);
    }
}

fn clean_rails(rails: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for rail in rails {
        let rail = rail.trim();
        if !rail.is_empty() && !out.iter().any(|r| r == rail) {
            out.push(rail.to_string());
        }
    }
    out
}

fn clean_specs(specs: BTreeMap<String, String>) -> BTreeMap<String, String> {
    specs
        .into_iter()
        .map(|(name, value)| (name.trim().to_string(), value.trim().to_string()))
        .filter(|(name, _)| !name.is_empty())
        .collect()
}
