use uuid::Uuid;

use super::items::BoardItemId;

/// Why a board command was refused. A refused command leaves the board as it
/// was: every check runs before the first write.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum BoardCommandError {
    #[error("no footprint {0} on the board")]
    FootprintNotFound(Uuid),
    #[error("no {0:?} on the board")]
    ItemNotFound(BoardItemId),
    #[error("{0:?} is named more than once")]
    DuplicateItem(BoardItemId),
    #[error("{0:?} is already on the board")]
    ItemAlreadyPresent(BoardItemId),
    #[error("{id:?} cannot be restored at index {index}: the list holds {len} items")]
    RestoreIndexOutOfRange {
        id: BoardItemId,
        index: usize,
        len: usize,
    },
    #[error("two restored items claim index {index} of the same list")]
    DuplicateRestoreIndex { index: usize },
    #[error("footprint {0} no longer has the pads, graphics and properties the face was taken from")]
    FaceMismatch(Uuid),
    #[error("{field} must be a finite number")]
    NonFinite { field: &'static str },
}
