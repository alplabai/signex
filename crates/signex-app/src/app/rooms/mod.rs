//! Rooms on a board (#2457): Place ▸ Room, Room Properties and Design ▸
//! Rooms ▸ Create Rooms from Sheets. Every edit is one board command run as
//! one labelled undo step; the model, the commands and the planner are in
//! `signex_types::pcb::room` and `signex_engine::pcb::rooms`, the rule in
//! `signex_drc::rooms`. See `docs/PCB_ROOMS.md`. Every rooms command also
//! runs from its operands (`operands`, `auto_place`, #2935), and Arrange in
//! Room and Auto Place in Rooms reach those handlers from the GUI (`tool`).

mod answer;
mod auto_place;
mod operands;
mod state;
mod tool;
mod update;

#[cfg(test)]
mod operand_tests;
#[cfg(test)]
mod tests;

pub(crate) use auto_place::auto_place_by_operands;
pub(crate) use operands::{
    RoomsRequest, arrange_in_room_by_operands, create_rooms_by_operands, place_room_by_operands,
};
pub(crate) use state::{CreateRoomsDialog, RoomRow, RoomsUi};

use pcb_geom::ItemRef;
use signex_engine::pcb::rooms::PlanShape;
use signex_types::pcb::{RoomLayer, RoomShape};
use signex_types::schematic::InstancePath;
use signex_widgets::property_illustration::IllustrationMsg;
use uuid::Uuid;

use crate::app::Message;
use crate::pcb_canvas::PcbSelectionState;

/// A field of Room Properties that takes typed text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RoomField {
    Name,
    X,
    Y,
    Width,
    Height,
    Margin,
}

impl RoomField {
    /// The illustration field it lights (`None`: the name has no callout).
    pub(crate) fn callout(self) -> Option<&'static str> {
        Some(match self {
            Self::Name => return None,
            Self::X => "x",
            Self::Y => "y",
            Self::Width => "w",
            Self::Height => "h",
            Self::Margin => "margin",
        })
    }
}

/// What a person did in Room Properties, for the room `room` of the board
/// in view. Namespaced under `PcbMsg::Room`.
#[derive(Debug, Clone, PartialEq)]
pub enum RoomMsg {
    /// Text typed into a field; held as a draft until submitted.
    FieldEdited {
        room: Uuid,
        field: RoomField,
        text: String,
    },
    /// Enter in a field: the draft is applied as one undo step.
    FieldSubmitted {
        room: Uuid,
        field: RoomField,
    },
    LayerPicked {
        room: Uuid,
        layer: RoomLayer,
    },
    ShapePicked {
        room: Uuid,
        shape: RoomShape,
    },
    /// From sheet: a sheet placement of the board's parts, or none.
    SheetPicked {
        room: Uuid,
        sheet: Option<InstancePath>,
    },
    KeepToggled {
        room: Uuid,
        keep: bool,
    },
    LockToggled {
        room: Uuid,
    },
    PullRequested {
        room: Uuid,
    },
    FitRequested {
        room: Uuid,
    },
    /// The pointer moved over the illustration or a field row.
    Illustration(IllustrationMsg),
}

/// Create Rooms from Sheets and its dialog. Namespaced under
/// `PcbMsg::CreateRooms`.
#[derive(Debug, Clone, PartialEq)]
pub enum CreateRoomsMsg {
    /// `create_rooms_from_sheets` ran: the sheets of the board's project are
    /// read off the update thread, then the dialog opens.
    Requested,
    /// The read of read `job` came back: the sheets placed under the root,
    /// or why there are none.
    SheetsRead {
        job: u64,
        result: Result<Vec<signex_engine::pcb::rooms::SheetMembers>, String>,
    },
    /// A row's checkbox, by room name.
    RowToggled(String),
    ShapePicked(PlanShape),
    MarginEdited(String),
    LayerPicked(RoomLayer),
    MovePartsToggled(bool),
    KeepRuleToggled(bool),
    ReplaceToggled(bool),
    Cancelled,
    /// Create: the ticked rooms as one undo step.
    Confirmed,
    /// The sheets a `create_rooms_from_sheets` run with operands waited for
    /// came back: every room is made on `board` and the command answered.
    SheetsReadFor {
        waiting: Box<crate::app::agent::WaitingCommand>,
        board: std::path::PathBuf,
        request: Box<RoomsRequest>,
        result: Result<Vec<signex_engine::pcb::rooms::SheetMembers>, String>,
    },
}

/// Arrange in Room and Auto Place in Rooms ran on the board in view.
/// Namespaced under `PcbMsg::RoomTool`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomToolMsg {
    /// Arrange in Room, on the room Properties shows.
    ArrangeRequested,
    /// Auto Place in Rooms, on every room with the default weights.
    AutoPlaceRequested,
}

impl From<RoomToolMsg> for Message {
    fn from(msg: RoomToolMsg) -> Self {
        Message::Pcb(crate::app::PcbMsg::RoomTool(msg))
    }
}

impl From<RoomMsg> for Message {
    fn from(msg: RoomMsg) -> Self {
        Message::Pcb(crate::app::PcbMsg::Room(msg))
    }
}

impl From<CreateRoomsMsg> for Message {
    fn from(msg: CreateRoomsMsg) -> Self {
        Message::Pcb(crate::app::PcbMsg::CreateRooms(msg))
    }
}

/// The rooms `selection` holds.
pub(crate) fn selected_rooms(selection: &PcbSelectionState) -> Vec<Uuid> {
    selection
        .selection()
        .items()
        .filter_map(|item| match item {
            ItemRef::Room(id) => Some(id),
            _ => None,
        })
        .collect()
}

/// The one room `selection` holds when it holds nothing else: the room Room
/// Properties shows.
pub(crate) fn shown_room(selection: &PcbSelectionState) -> Option<Uuid> {
    let only = selection.selection();
    if only.len() != 1 {
        return None;
    }
    selected_rooms(selection).first().copied()
}
