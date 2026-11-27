//! Why a block diagram edit, save or command did not run, as a catalog
//! message: the `block_*` commands answer with it, and the tab logs it in
//! Messages. The model's own errors map to one message each.

use signex_i18n::{I18n, Translatable, ids};
use signex_types::block_diagram::{BlockDiagramError, DiagramFault};
use uuid::Uuid;

/// One reason, with what it names.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Refusal {
    NoDocument,
    NotADiagram {
        name: String,
        kind: &'static str,
    },
    NotOpen {
        file: String,
    },
    ReadOnly {
        file: String,
        token: String,
    },
    Saving {
        file: String,
    },
    /// The diagram does not encode; nothing is written.
    Unwritable {
        file: String,
        reason: String,
    },
    NoProject,
    AlreadyOpen {
        file: String,
    },
    BadName {
        name: String,
    },
    Needs {
        operand: &'static str,
    },
    BadId {
        operand: &'static str,
        text: String,
    },
    PinPoint {
        operand: &'static str,
    },
    BadShape {
        text: String,
    },
    BadSide {
        text: String,
    },
    BadDirection {
        text: String,
    },
    BadClear {
        text: String,
    },
    BadSpec {
        text: String,
    },
    BadLength {
        text: String,
    },
    NoEnd {
        text: String,
    },
    Edit(BlockDiagramError),
}

impl From<BlockDiagramError> for Refusal {
    fn from(error: BlockDiagramError) -> Self {
        Self::Edit(error)
    }
}

fn id_arg(id: &Uuid) -> String {
    id.to_string()
}

impl Refusal {
    /// The message, for a command's answer or a notice.
    pub(crate) fn translatable(&self) -> Translatable {
        use Refusal as R;
        match self {
            R::NoDocument => Translatable::new(ids::BLOCK_DIAGRAM_REFUSED_NO_DOCUMENT),
            R::NotADiagram { name, kind } => {
                Translatable::new(ids::BLOCK_DIAGRAM_REFUSED_NOT_A_DIAGRAM)
                    .arg("name", name.as_str())
                    .arg("kind", *kind)
            }
            R::NotOpen { file } => {
                Translatable::new(ids::BLOCK_DIAGRAM_REFUSED_NOT_OPEN).arg("file", file.as_str())
            }
            R::ReadOnly { file, token } => Translatable::new(ids::BLOCK_DIAGRAM_REFUSED_READ_ONLY)
                .arg("file", file.as_str())
                .arg("token", token.as_str()),
            R::Saving { file } => {
                Translatable::new(ids::BLOCK_DIAGRAM_REFUSED_SAVING).arg("file", file.as_str())
            }
            R::Unwritable { file, reason } => {
                Translatable::new(ids::BLOCK_DIAGRAM_REFUSED_UNWRITABLE)
                    .arg("file", file.as_str())
                    .arg("reason", reason.as_str())
            }
            R::NoProject => Translatable::new(ids::BLOCK_DIAGRAM_REFUSED_NO_PROJECT),
            R::AlreadyOpen { file } => Translatable::new(ids::BLOCK_DIAGRAM_REFUSED_ALREADY_OPEN)
                .arg("file", file.as_str()),
            R::BadName { name } => {
                Translatable::new(ids::BLOCK_DIAGRAM_REFUSED_BAD_NAME).arg("name", name.as_str())
            }
            R::Needs { operand } => {
                Translatable::new(ids::BLOCK_DIAGRAM_REFUSED_NEEDS).arg("operand", *operand)
            }
            R::BadId { operand, text } => Translatable::new(ids::BLOCK_DIAGRAM_REFUSED_BAD_ID)
                .arg("operand", *operand)
                .arg("text", text.as_str()),
            R::PinPoint { operand } => {
                Translatable::new(ids::BLOCK_DIAGRAM_REFUSED_PIN_POINT).arg("operand", *operand)
            }
            R::BadShape { text } => {
                Translatable::new(ids::BLOCK_DIAGRAM_REFUSED_BAD_SHAPE).arg("text", text.as_str())
            }
            R::BadSide { text } => {
                Translatable::new(ids::BLOCK_DIAGRAM_REFUSED_BAD_SIDE).arg("text", text.as_str())
            }
            R::BadDirection { text } => Translatable::new(ids::BLOCK_DIAGRAM_REFUSED_BAD_DIRECTION)
                .arg("text", text.as_str()),
            R::BadClear { text } => {
                Translatable::new(ids::BLOCK_DIAGRAM_REFUSED_BAD_CLEAR).arg("text", text.as_str())
            }
            R::BadSpec { text } => {
                Translatable::new(ids::BLOCK_DIAGRAM_REFUSED_BAD_SPEC).arg("text", text.as_str())
            }
            R::BadLength { text } => {
                Translatable::new(ids::BLOCK_DIAGRAM_REFUSED_BAD_LENGTH).arg("text", text.as_str())
            }
            R::NoEnd { text } => {
                Translatable::new(ids::BLOCK_DIAGRAM_REFUSED_NO_END).arg("text", text.as_str())
            }
            R::Edit(error) => edit_error(error),
        }
    }

    /// The message in en-US, for the log.
    pub(crate) fn english(&self) -> String {
        self.translatable().render(&I18n::en_us())
    }
}

fn edit_error(error: &BlockDiagramError) -> Translatable {
    let one = |id, object: &Uuid| Translatable::new(id).arg("id", id_arg(object));
    match error {
        BlockDiagramError::UnknownBlock(id) => one(ids::BLOCK_DIAGRAM_NO_BLOCK, id),
        BlockDiagramError::UnknownPort(id) => one(ids::BLOCK_DIAGRAM_NO_PORT, id),
        BlockDiagramError::UnknownConnection(id) => one(ids::BLOCK_DIAGRAM_NO_CONNECTION, id),
        BlockDiagramError::UnknownGroup(id) => one(ids::BLOCK_DIAGRAM_NO_GROUP, id),
        BlockDiagramError::Fault(fault) => fault_message(fault),
    }
}

fn fault_message(fault: &DiagramFault) -> Translatable {
    let one = |id, object: &Uuid| Translatable::new(id).arg("id", id_arg(object));
    let two = |id, a: &Uuid, b: &Uuid| {
        Translatable::new(id)
            .arg("id", id_arg(a))
            .arg("other", id_arg(b))
    };
    match fault {
        DiagramFault::DuplicateId(id) => one(ids::BLOCK_DIAGRAM_FAULT_DUPLICATE_ID, id),
        DiagramFault::BlockWithoutName(id) => one(ids::BLOCK_DIAGRAM_FAULT_BLOCK_NAME, id),
        DiagramFault::BadBlockSize(id) => one(ids::BLOCK_DIAGRAM_FAULT_BLOCK_SIZE, id),
        DiagramFault::BadBlockPosition(id) => one(ids::BLOCK_DIAGRAM_FAULT_BLOCK_POSITION, id),
        DiagramFault::MissingGroup { block, group } => {
            two(ids::BLOCK_DIAGRAM_FAULT_MISSING_GROUP, block, group)
        }
        DiagramFault::SheetLinkWithoutPath(id) => one(ids::BLOCK_DIAGRAM_FAULT_SHEET_PATH, id),
        DiagramFault::PortWithoutName(id) => one(ids::BLOCK_DIAGRAM_FAULT_PORT_NAME, id),
        DiagramFault::BadPortOffset(id) => one(ids::BLOCK_DIAGRAM_FAULT_PORT_OFFSET, id),
        DiagramFault::GroupWithoutName(id) => one(ids::BLOCK_DIAGRAM_FAULT_GROUP_NAME, id),
        DiagramFault::MissingBlock { connection, block } => {
            two(ids::BLOCK_DIAGRAM_FAULT_MISSING_BLOCK, connection, block)
        }
        DiagramFault::MissingPort { connection, port } => {
            two(ids::BLOCK_DIAGRAM_FAULT_MISSING_PORT, connection, port)
        }
        DiagramFault::ClosedConnection(id) => one(ids::BLOCK_DIAGRAM_FAULT_CLOSED, id),
        DiagramFault::InterfaceWithoutName(id) => one(ids::BLOCK_DIAGRAM_FAULT_INTERFACE, id),
    }
}
