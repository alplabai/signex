//! Reading a `.snxblk` file, and the save that writes one. The layout is in
//! `docs/architecture/snxblk-format.md`; the writer is `writer`.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use uuid::Uuid;

use super::{
    Block, BlockDiagram, BlockGroup, BlockPort, Connection, DiagramFault, Endpoint, Interface,
    SheetLink, direction_from_token, link_state_from_token, shape_from_token, side_from_token,
};
use crate::document_id::DocumentId;
use crate::format::nm_to_mm;
use crate::format_version::{FormatVersion, FormatVersionError, MissingToken, NewerMinor};
use crate::schematic::Point;
use crate::theme::Color;

/// The block diagram's format token.
pub const SNXBLK_FORMAT_V1: &str = "snxblk/1";

/// `.snxblk` version policy. A newer minor opens read-only, as the other
/// document formats do; a newer major and a file with no token are refused.
pub const SNXBLK_VERSION: FormatVersion = FormatVersion {
    current: SNXBLK_FORMAT_V1,
    readable: &[SNXBLK_FORMAT_V1],
    newer_minor: NewerMinor::ReadOnly,
    missing: MissingToken::Refused,
};

/// Why a block diagram file could not be read or written. The caller adds
/// the file name.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum BlockDiagramFileError {
    #[error(transparent)]
    Version(#[from] FormatVersionError),
    #[error("the block diagram is not valid: {0}")]
    Parse(String),
    #[error("the block diagram is not consistent: {0}")]
    Fault(#[from] DiagramFault),
    #[error("{0} is not a length the file can hold")]
    Length(String),
    #[error("{0}")]
    Io(String),
}

/// A diagram read from a file, with what the reader learned about it.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadBlockDiagram {
    /// The diagram.
    pub diagram: BlockDiagram,
    /// The token the file carried. A newer minor than this build writes
    /// makes the document read-only: pass it to [`write_block_diagram`],
    /// which then refuses the save.
    pub token: String,
    /// The file had no document id, or the nil one, and the diagram was
    /// given a new one (`docs/DOCUMENT_IDS.md`, "Minted on open"). Not an
    /// edit: the next save writes it.
    pub minted_id: bool,
}

impl ReadBlockDiagram {
    /// Whether the file is from a newer minor, so every save is refused.
    pub fn is_read_only(&self) -> bool {
        SNXBLK_VERSION.is_newer_minor(&self.token)
    }
}

#[derive(Deserialize)]
struct FileIn {
    diagram_id: Option<Uuid>,
    #[serde(default)]
    groups: Vec<GroupIn>,
    #[serde(default)]
    blocks: Vec<BlockIn>,
    #[serde(default)]
    connections: Vec<ConnectionIn>,
}

#[derive(Deserialize)]
struct GroupIn {
    id: Uuid,
    name: String,
    color: Option<String>,
}

#[derive(Deserialize)]
struct BlockIn {
    id: Uuid,
    name: String,
    #[serde(default)]
    function: String,
    #[serde(default)]
    note: String,
    shape: Option<String>,
    x_nm: i64,
    y_nm: i64,
    width_nm: i64,
    height_nm: i64,
    color: Option<String>,
    group: Option<Uuid>,
    #[serde(default)]
    rails: Vec<String>,
    #[serde(default)]
    specs: BTreeMap<String, String>,
    sheet: Option<Uuid>,
    sheet_path: Option<String>,
    sheet_state: Option<String>,
    #[serde(default)]
    ports: Vec<PortIn>,
}

#[derive(Deserialize)]
struct PortIn {
    id: Uuid,
    name: String,
    side: Option<String>,
    offset_nm: i64,
    direction: Option<String>,
}

#[derive(Deserialize)]
struct ConnectionIn {
    id: Uuid,
    from_block: Uuid,
    from_port: Option<Uuid>,
    to_block: Uuid,
    to_port: Option<Uuid>,
    interface: Option<String>,
    #[serde(default)]
    label: String,
    derived_from: Option<String>,
}

/// Read a block diagram file's text. The token is checked before anything
/// else is read; a file with a fault is refused.
pub fn parse_block_diagram(text: &str) -> Result<ReadBlockDiagram, BlockDiagramFileError> {
    let table: toml::Table = toml::from_str(text).map_err(parse_error)?;
    let found = table.get("format").and_then(toml::Value::as_str);
    let token = SNXBLK_VERSION.read(found)?;
    let file: FileIn = toml::Value::Table(table).try_into().map_err(parse_error)?;
    let stored_id = file.diagram_id.and_then(DocumentId::from_uuid);
    let mut diagram = BlockDiagram::with_id(stored_id.unwrap_or_default());
    diagram.groups = file
        .groups
        .into_iter()
        .map(group)
        .collect::<Result<_, _>>()?;
    diagram.blocks = file
        .blocks
        .into_iter()
        .map(block)
        .collect::<Result<_, _>>()?;
    diagram.connections = file.connections.into_iter().map(connection).collect();
    if let Some(fault) = diagram.fault() {
        return Err(fault.into());
    }
    Ok(ReadBlockDiagram {
        diagram,
        token,
        minted_id: stored_id.is_none(),
    })
}

/// Read the block diagram at `path`. Blocking IO: the app calls it from a
/// task.
pub fn read_block_diagram(path: &Path) -> Result<ReadBlockDiagram, BlockDiagramFileError> {
    let text =
        std::fs::read_to_string(path).map_err(|e| BlockDiagramFileError::Io(e.to_string()))?;
    parse_block_diagram(&text)
}

/// Write `diagram` to `path` atomically. `source_token` is the token the
/// document was read with ([`SNXBLK_FORMAT_V1`] for a new one); a newer
/// minor refuses the save before anything is written. Blocking IO: the app
/// calls it from a task.
pub fn write_block_diagram(
    path: &Path,
    diagram: &BlockDiagram,
    source_token: &str,
) -> Result<(), BlockDiagramFileError> {
    SNXBLK_VERSION.refuse_save(source_token)?;
    let text = super::encode_block_diagram(diagram)?;
    crate::atomic_io::atomic_write(path, text.as_bytes())
        .map_err(|e| BlockDiagramFileError::Io(e.to_string()))
}

fn parse_error(error: toml::de::Error) -> BlockDiagramFileError {
    BlockDiagramFileError::Parse(error.to_string())
}

fn bad_value(what: &str, id: Uuid, detail: impl std::fmt::Display) -> BlockDiagramFileError {
    BlockDiagramFileError::Parse(format!("{what} of {id}: {detail}"))
}

fn color(text: Option<String>, id: Uuid) -> Result<Option<Color>, BlockDiagramFileError> {
    text.map(|hex| Color::parse_hex(&hex).ok_or_else(|| bad_value("color", id, hex)))
        .transpose()
}

fn group(raw: GroupIn) -> Result<BlockGroup, BlockDiagramFileError> {
    Ok(BlockGroup {
        color: color(raw.color, raw.id)?,
        id: raw.id,
        name: raw.name,
    })
}

fn block(raw: BlockIn) -> Result<Block, BlockDiagramFileError> {
    let id = raw.id;
    let shape = match raw.shape {
        Some(text) => shape_from_token(&text).map_err(|e| bad_value("shape", id, e))?,
        None => Default::default(),
    };
    let sheet = sheet_link(id, raw.sheet, raw.sheet_path, raw.sheet_state)?;
    Ok(Block {
        id,
        name: raw.name,
        function: raw.function,
        note: raw.note,
        shape,
        position: Point::new(nm_to_mm(raw.x_nm), nm_to_mm(raw.y_nm)),
        width_mm: nm_to_mm(raw.width_nm),
        height_mm: nm_to_mm(raw.height_nm),
        color: color(raw.color, id)?,
        group: raw.group,
        rails: raw.rails,
        specs: raw.specs,
        ports: raw.ports.into_iter().map(port).collect::<Result<_, _>>()?,
        sheet,
    })
}

fn sheet_link(
    block: Uuid,
    sheet: Option<Uuid>,
    path: Option<String>,
    state: Option<String>,
) -> Result<Option<SheetLink>, BlockDiagramFileError> {
    let Some(sheet) = sheet else {
        return Ok(None);
    };
    let sheet = DocumentId::from_uuid(sheet)
        .ok_or_else(|| bad_value("sheet", block, "the nil id names no sheet"))?;
    let state = match state {
        Some(text) => {
            link_state_from_token(&text).map_err(|e| bad_value("sheet_state", block, e))?
        }
        None => Default::default(),
    };
    Ok(Some(SheetLink {
        sheet,
        path: path.unwrap_or_default(),
        state,
    }))
}

fn port(raw: PortIn) -> Result<BlockPort, BlockDiagramFileError> {
    let id = raw.id;
    let side = match raw.side {
        Some(text) => side_from_token(&text).map_err(|e| bad_value("side", id, e))?,
        None => Default::default(),
    };
    let direction = match raw.direction {
        Some(text) => direction_from_token(&text).map_err(|e| bad_value("direction", id, e))?,
        None => Default::default(),
    };
    Ok(BlockPort {
        id,
        name: raw.name,
        side,
        offset_mm: nm_to_mm(raw.offset_nm),
        direction,
    })
}

fn connection(raw: ConnectionIn) -> Connection {
    Connection {
        id: raw.id,
        from: Endpoint {
            block: raw.from_block,
            port: raw.from_port,
        },
        to: Endpoint {
            block: raw.to_block,
            port: raw.to_port,
        },
        interface: raw
            .interface
            .map_or_else(Interface::default, |text| Interface::from_token(&text)),
        label: raw.label,
        derived_from: raw.derived_from,
    }
}
