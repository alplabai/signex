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
    #[error("no pad {pad} on footprint {footprint}")]
    PadNotFound { footprint: Uuid, pad: Uuid },
    #[error("pad {pad} of footprint {footprint} is named more than once")]
    DuplicatePadChange { footprint: Uuid, pad: Uuid },
    #[error(
        "pad {pad} of footprint {footprint} would still be on net {name} ({number}), \
         which the edit removes from the net table"
    )]
    PadOnRemovedNet {
        footprint: Uuid,
        pad: Uuid,
        number: u32,
        name: String,
    },
    #[error("uuid {0} of a new pad is already used on the board")]
    PadAlreadyPresent(Uuid),
    #[error("{0:?} takes a uuid the board or another item already uses")]
    UuidAlreadyUsed(BoardItemId),
    #[error("footprint template {0} is not on a top-side layer this build reads")]
    TemplateNotTopSide(Uuid),
    #[error(
        "footprint {footprint} has {old} pads numbered {number:?} and the new footprint {new}; \
         a net or lock on a pad past the new count would be lost"
    )]
    PadCountMismatch {
        footprint: Uuid,
        number: String,
        old: usize,
        new: usize,
    },
    #[error("{id:?} cannot be restored at index {index}: the list holds {len} items")]
    RestoreIndexOutOfRange {
        id: BoardItemId,
        index: usize,
        len: usize,
    },
    #[error("two restored items claim index {index} of the same list")]
    DuplicateRestoreIndex { index: usize },
    #[error(
        "footprint {0} no longer has the pads, graphics and properties the face was taken from"
    )]
    FaceMismatch(Uuid),
    #[error("footprint {0} no longer has the properties the text was taken from")]
    TextMismatch(Uuid),
    #[error("footprint {footprint} has no property at index {index}")]
    PropertyNotFound { footprint: Uuid, index: usize },
    #[error("property {index} of footprint {footprint} would change its key")]
    PropertyKeyChanged { footprint: Uuid, index: usize },
    #[error(
        "the {key} text of footprint {footprint} shows its designator or value; \
         set those with a text edit"
    )]
    PropertyTextOwned { footprint: Uuid, key: String },
    #[error("{0:?} cannot be replaced in place; footprints have their own edits")]
    NotReplaceable(BoardItemId),
    #[error("{field} must be greater than zero")]
    NotPositive { field: &'static str },
    #[error("{0} is not a copper layer")]
    NotCopperLayer(String),
    #[error("net {0} is not in the board's net table")]
    UnknownNet(u32),
    #[error("the hole ({hole} mm) must fit inside the copper around it ({diameter} mm)")]
    HoleTooLarge { hole: f64, diameter: f64 },
    #[error("{field} must be a finite number")]
    NonFinite { field: &'static str },
    #[error("the board sketch holds a number that is not finite at {0}")]
    NonFiniteSketch(String),
    #[error("the grid step must not be negative, not {0} mm")]
    NegativeGridStep(f64),
    #[error("guide or grid {0} is named more than once")]
    DuplicateOption(Uuid),
    #[error("the board has no setup to hold a grid step")]
    NoSetup,
    #[error("the board outline belongs to the board sketch; edit the sketch instead")]
    OutlineOwnedBySketch,
    #[error(
        "the board changed since revision {expected} (it is at {current}); read it again and retry"
    )]
    StaleRevision { expected: u64, current: u64 },
    #[error("a failed command could not be rolled back: {0}")]
    RollbackFailed(String),
}
