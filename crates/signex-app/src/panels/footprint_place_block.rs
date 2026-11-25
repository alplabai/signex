//! Why the Library panel's Place is off for a footprint (mockup row 119).

use signex_i18n::{MessageId, ids};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FootprintPlaceBlock {
    NoBoard,
    SeveralBoards,
    ReadOnlyBoard,
    BoardInSketch,
    UnsavedEdits,
    NotInLibrary,
    /// A library address reads only the first footprint of a file.
    NotFirstInFile,
    UndockedEditor,
}

impl FootprintPlaceBlock {
    /// The tooltip that gives the reason.
    pub fn reason(self) -> MessageId {
        match self {
            Self::NoBoard => ids::LIBRARY_PANEL_PLACE_NO_BOARD,
            Self::SeveralBoards => ids::LIBRARY_PANEL_PLACE_SEVERAL_BOARDS,
            Self::ReadOnlyBoard => ids::LIBRARY_PANEL_PLACE_READ_ONLY_BOARD,
            Self::BoardInSketch => ids::LIBRARY_PANEL_PLACE_BOARD_IN_SKETCH,
            Self::UnsavedEdits => ids::LIBRARY_PANEL_PLACE_UNSAVED,
            Self::NotInLibrary => ids::LIBRARY_PANEL_PLACE_NOT_IN_LIBRARY,
            Self::NotFirstInFile => ids::LIBRARY_PANEL_PLACE_NOT_FIRST,
            Self::UndockedEditor => ids::LIBRARY_PANEL_PLACE_UNDOCKED,
        }
    }
}
