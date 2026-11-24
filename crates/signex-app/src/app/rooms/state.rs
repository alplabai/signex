//! What the rooms slice keeps between messages (#2457): the Room Properties
//! drafts and illustration hover, and the open Create Rooms from Sheets
//! dialog. The plan the dialog shows is worked out from this state and the
//! board in `view`, the same way Create works it out.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use signex_engine::pcb::rooms::{RoomOptions, RoomPlan, SheetMembers, plan_rooms_from_sheets};
use signex_types::object_id::IdSource;
use signex_types::pcb::PcbBoard;
use signex_widgets::property_illustration::IllustrationHover;
use uuid::Uuid;

use super::RoomField;

/// The rooms slice of the UI state.
#[derive(Debug, Clone, Default)]
pub struct RoomsUi {
    /// Typed text not yet applied, by field, for the room `draft_room`.
    pub(crate) drafts: BTreeMap<RoomField, String>,
    pub(crate) draft_room: Option<Uuid>,
    /// Which callout or field the pointer is over.
    pub(crate) hover: IllustrationHover,
    /// Create Rooms from Sheets, while open. Boxed: a closed dialog costs
    /// one pointer (#2412).
    pub(crate) create: Option<Box<CreateRoomsDialog>>,
    /// Generation of the newest sheet read; an older answer is dropped.
    pub(crate) reads: u64,
    /// The board and project file the read in flight is for.
    pub(crate) pending: Option<(PathBuf, String)>,
}

impl RoomsUi {
    /// Keep `text` as the draft of `field` of `room`; drafts of another
    /// room are dropped.
    pub(crate) fn set_draft(&mut self, room: Uuid, field: RoomField, text: String) {
        if self.draft_room != Some(room) {
            self.drafts.clear();
            self.draft_room = Some(room);
        }
        self.drafts.insert(field, text);
    }

    pub(crate) fn take_draft(&mut self, room: Uuid, field: RoomField) -> Option<String> {
        if self.draft_room != Some(room) {
            return None;
        }
        self.drafts.remove(&field)
    }
}

/// One row of the dialog's list.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomRow {
    pub name: String,
    pub file: String,
    pub colour: String,
    pub parts: usize,
    pub ticked: bool,
    /// A channel of a sheet placed more than once.
    pub channel: bool,
}

/// Create Rooms from Sheets, while open.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateRoomsDialog {
    /// The board the rooms are made on.
    pub board: PathBuf,
    /// The project file, as the subtitle names it.
    pub project_file: String,
    pub sheets: Vec<SheetMembers>,
    /// Rooms left out, by name; every room starts ticked.
    pub unticked: BTreeSet<String>,
    pub options: RoomOptions,
    /// The margin as typed; the options hold the last one that read.
    pub margin_text: String,
}

/// Ids for a plan only drawn, never run: the preview and the counts.
struct PreviewIds(u128);

impl IdSource for PreviewIds {
    fn next_id(&mut self) -> Uuid {
        self.0 += 1;
        Uuid::from_u128(self.0)
    }
}

impl CreateRoomsDialog {
    pub(crate) fn new(board: PathBuf, project_file: String, sheets: Vec<SheetMembers>) -> Self {
        let options = RoomOptions::default();
        Self {
            board,
            project_file,
            sheets,
            unticked: BTreeSet::new(),
            margin_text: format!("{:.3}", options.margin_mm),
            options,
        }
    }

    /// The plan Create runs on `board` with `ids`: every room of the
    /// sheets, less the unticked ones and the parts only they move.
    pub(crate) fn plan_with(&self, board: &PcbBoard, ids: &mut dyn IdSource) -> RoomPlan {
        let mut plan = plan_rooms_from_sheets(board, &self.sheets, &self.options, ids);
        plan.rooms.retain(|p| !self.unticked.contains(&p.room.name));
        let kept: BTreeSet<Uuid> = plan
            .rooms
            .iter()
            .flat_map(|p| p.members.iter().copied())
            .collect();
        plan.moves.retain(|(fp, _)| kept.contains(fp));
        plan
    }

    /// [`Self::plan_with`] for drawing and counting.
    pub(crate) fn preview(&self, board: &PcbBoard) -> RoomPlan {
        self.plan_with(board, &mut PreviewIds(0))
    }

    /// The list's rows: sheets placed once, then channels, each with its
    /// colour and member count as the plan gives them.
    pub(crate) fn rows(&self, board: &PcbBoard) -> Vec<RoomRow> {
        let mut all = self.clone();
        all.unticked.clear();
        let plan = all.preview(board);
        plan.rooms
            .iter()
            .map(|p| RoomRow {
                name: p.room.name.clone(),
                file: p.file.clone(),
                colour: p.room.colour.clone(),
                parts: p.members.len(),
                ticked: !self.unticked.contains(&p.room.name),
                channel: p.room.source.as_ref().is_some_and(|s| s.channel.is_some()),
            })
            .collect()
    }

    /// How many sheets the list covers, and how many of its rooms are
    /// channels.
    pub(crate) fn counts(&self) -> (usize, usize) {
        let channels = self
            .sheets
            .iter()
            .filter(|s| s.placements.len() > 1)
            .map(|s| s.placements.len())
            .sum();
        (self.sheets.len(), channels)
    }
}
