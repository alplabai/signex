//! The rooms slice's handlers (#2457): Room Properties edits, Place ▸ Room
//! and Create Rooms from Sheets. Each edit is one labelled board undo step;
//! a refusal is said in Messages and changes nothing. The sheets the dialog
//! lists are read off the update thread.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use pcb_geom::ItemRef;
use signex_engine::pcb::BoardCommand;
use signex_engine::pcb::rooms::{SheetMembers, pull_moves};
use signex_i18n::{MessageId, ids, t};
use signex_types::coord::Unit;
use signex_types::measure::parse_length_field_mm;
use signex_types::pcb::room::{next_room_name, palette_colour};
use signex_types::pcb::{PcbBoard, Point, Room, RoomOutline, RoomSource};
use uuid::Uuid;

use signex_engine::pcb::rooms::part_box;
use signex_types::object_id::short_id;
use signex_types::schematic::InstancePath;

use super::{CreateRoomsDialog, CreateRoomsMsg, RoomField, RoomMsg};
use crate::app::command::{CommandOutcome, EditReads, OperandValue, translator};
use crate::app::pcb_tools::BoardPlaceTool;
use crate::app::project_sheets::{
    ProjectSheetPlan, plan_project_sheets_for, project_root_sheet_path,
};
use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{Message, Signex};
use crate::panels::pcb_room_properties::{RoomFacts, RoomPart, SheetChoice};
use crate::pcb_canvas::{RoomMark, RoomsOverlay};

impl Signex {
    /// The board in view's path and its room `id`.
    fn room_in_view(&self, id: Uuid) -> Option<(PathBuf, Room)> {
        let path = self.active_tab_path().filter(|_| self.has_active_pcb())?;
        let board = self.document_state.pcb_board(&path)?;
        let room = board.rooms.iter().find(|r| r.uuid == id)?.clone();
        Some((path, room))
    }

    /// Run `commands` on the board at `path` as one step labelled `label`;
    /// a refusal is said in Messages. Whether the board changed.
    pub(crate) fn run_room_commands(
        &mut self,
        path: &Path,
        commands: Vec<BoardCommand>,
        label: MessageId,
    ) -> bool {
        if self.document_state.board_engine(path).is_none() {
            return false;
        }
        match self.run_room_step(path, commands, label) {
            Ok(changed) => changed,
            Err(why) => {
                self.say_room_refused(why);
                false
            }
        }
    }

    /// [`Self::run_room_commands`] for a caller that answers the refusal
    /// itself: whether the board changed, or why it did not.
    pub(crate) fn run_room_step(
        &mut self,
        path: &Path,
        commands: Vec<BoardCommand>,
        label: MessageId,
    ) -> Result<bool, String> {
        let Some(engine) = self.document_state.board_engine(path) else {
            return Err("the board is not open".to_string());
        };
        let label = t!(self.ui_state.i18n, label).into_string();
        let step = CheckedStep {
            label: &label,
            expected_revision: engine.revision(),
        };
        let patches = self
            .execute_board_commands(path, commands, Some(step))
            .map_err(|error| error.to_string())?;
        self.refresh_pcb_highlight();
        Ok(!patches.is_empty())
    }

    fn say_room_refused(&self, why: String) {
        let line = t!(self.ui_state.i18n, ids::ROOMS_REFUSED, "why" => why);
        crate::diagnostics::log_warning(line.into_string());
    }

    pub(crate) fn handle_room(&mut self, msg: RoomMsg) -> Task<Message> {
        match msg {
            RoomMsg::Illustration(event) => self.ui_state.rooms.hover.update(event),
            RoomMsg::FieldEdited { room, field, text } => {
                self.ui_state.rooms.set_draft(room, field, text);
            }
            RoomMsg::FieldSubmitted { room, field } => self.submit_room_field(room, field),
            RoomMsg::LayerPicked { room, layer } => self.edit_room(room, |r| r.layer = layer),
            RoomMsg::ShapePicked { room, shape } => {
                self.edit_room(room, |r| r.outline = r.outline.as_shape(shape));
            }
            RoomMsg::SheetPicked { room, sheet } => self.edit_room(room, |r| {
                let channel = r
                    .source
                    .as_ref()
                    .filter(|s| Some(&s.sheet) == sheet.as_ref())
                    .and_then(|s| s.channel);
                r.source = sheet.map(|sheet| RoomSource { sheet, channel });
            }),
            RoomMsg::KeepToggled { room, keep } => {
                self.edit_room(room, |r| r.keep_parts_inside = keep);
            }
            RoomMsg::LockToggled { room } => self.edit_room(room, |r| r.locked = !r.locked),
            RoomMsg::PullRequested { room } => self.pull_room(room),
            RoomMsg::FitRequested { room } => {
                if let Some((path, _)) = self.room_in_view(room) {
                    let fit = BoardCommand::FitRoomToParts { room };
                    self.run_room_commands(&path, vec![fit], ids::ROOMS_UNDO_FIT);
                }
            }
        }
        // Properties shows the drafts and the hover from the summary.
        self.sync_pcb_board_summary();
        Task::none()
    }

    /// Change a copy of room `id` with `edit` and put it back as one step;
    /// nothing when the copy is unchanged.
    fn edit_room(&mut self, id: Uuid, edit: impl FnOnce(&mut Room)) {
        let Some((path, old)) = self.room_in_view(id) else {
            return;
        };
        let mut room = old.clone();
        edit(&mut room);
        if room == old {
            return;
        }
        let command = BoardCommand::EditRoom {
            room: Box::new(room),
        };
        self.run_room_commands(&path, vec![command], ids::ROOMS_UNDO_EDIT);
    }

    /// Enter in a field: its draft as one step. X and Y move the room and
    /// its parts; a number that does not read is said in Messages.
    fn submit_room_field(&mut self, id: Uuid, field: RoomField) {
        let Some(text) = self.ui_state.rooms.take_draft(id, field) else {
            return;
        };
        if field == RoomField::Name {
            let name = text.trim().to_string();
            if !name.is_empty() {
                self.edit_room(id, |r| r.name = name);
            }
            return;
        }
        let Some(mm) = parse_length_field_mm(&text, Unit::Mm) else {
            let line = t!(self.ui_state.i18n, ids::ROOMS_NUMBER_INVALID);
            crate::diagnostics::log_warning(line.into_string());
            return;
        };
        let Some((path, room)) = self.room_in_view(id) else {
            return;
        };
        let Some(b) = room.outline.bounds() else {
            return;
        };
        match field {
            RoomField::X | RoomField::Y => {
                let to = match field {
                    RoomField::X => Point::new(mm, b.y_mm),
                    _ => Point::new(b.x_mm, mm),
                };
                let command = BoardCommand::MoveRoom { room: id, to };
                self.run_room_commands(&path, vec![command], ids::ROOMS_UNDO_MOVE);
            }
            RoomField::Width => self.edit_room(id, |r| r.outline = r.outline.resized(mm, b.h_mm)),
            RoomField::Height => {
                self.edit_room(id, |r| r.outline = r.outline.resized(b.w_mm, mm));
            }
            RoomField::Margin => self.edit_room(id, |r| r.margin_mm = mm),
            RoomField::Name => {}
        }
    }

    /// Pack the room's parts outside it into it, or say they are all in.
    fn pull_room(&mut self, id: Uuid) {
        let Some((path, room)) = self.room_in_view(id) else {
            return;
        };
        let Some(board) = self.document_state.pcb_board(&path) else {
            return;
        };
        if pull_moves(&room, board).is_empty() {
            let line = t!(self.ui_state.i18n, ids::ROOMS_NOTHING_TO_PULL, "room" => room.name);
            crate::diagnostics::log_info(line.into_string());
            return;
        }
        let pull = BoardCommand::PullPartsIntoRoom { room: id };
        self.run_room_commands(&path, vec![pull], ids::ROOMS_UNDO_PULL);
    }

    /// A room drawn by hand on `board`: the next free `Room N` name and the
    /// palette colour after the rooms it has.
    fn new_drawn_room(&self, board: &PcbBoard, id: Uuid, outline: RoomOutline) -> Room {
        let prefix = t!(self.ui_state.i18n, ids::ROOMS_DEFAULT_NAME).into_string();
        Room::drawn(
            id,
            next_room_name(&board.rooms, &prefix),
            palette_colour(board.rooms.len()),
            outline,
        )
    }

    /// Place ▸ Room's second corner: the room from `a` to `b` as one step,
    /// placed by the operand handler the CLI runs (`start`, `end`), then
    /// selected so Properties shows it. `a` and `b` are model points; the
    /// operands take them in the board's frame (#2985). Whether it was
    /// placed, and what the step left to do; two corners on one line place
    /// nothing.
    pub(crate) fn place_drawn_room(&mut self, a: Point, b: Point) -> (bool, Task<Message>) {
        let Some(path) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return (false, Task::none());
        };
        let Some(board) = self.document_state.pcb_board(&path) else {
            return (false, Task::none());
        };
        if !RoomOutline::rect_between(a, b).has_area() {
            return (false, Task::none());
        }
        let origin = board.options.origin;
        let (a, b) = (origin.to_user(a), origin.to_user(b));
        let corners = vec![
            ("start", OperandValue::Point(a.x, a.y)),
            ("end", OperandValue::Point(b.x, b.y)),
        ];
        let (outcome, task) = translator::run(self, "place_board_room", &path, corners);
        let placed = match outcome {
            CommandOutcome::Answered(answer) => answer["room"]["id"]
                .as_str()
                .and_then(|id| Uuid::parse_str(id).ok()),
            CommandOutcome::Disabled { reason } => {
                crate::diagnostics::log_warning(reason.render(&self.ui_state.i18n));
                None
            }
            _ => None,
        };
        if let Some(id) = placed {
            self.select_pcb_items(&[ItemRef::Room(id)]);
        }
        (placed.is_some(), task)
    }

    /// The selected rooms, and the room Place ▸ Room is drawing from its
    /// first corner to the pointer.
    pub(crate) fn rooms_overlay(&self) -> RoomsOverlay {
        let selected = super::selected_rooms(&self.interaction_state.pcb_selection);
        let drawing = self.own_board_tool().and_then(|session| {
            if session.tool != BoardPlaceTool::Room {
                return None;
            }
            let first = *session.points.first()?;
            let outline = RoomOutline::rect_between(first, session.cursor?);
            let board = self.document_state.pcb_board(&session.path)?;
            Some(RoomMark::of_room(&self.new_drawn_room(
                board,
                Uuid::nil(),
                outline,
            )))
        });
        RoomsOverlay { selected, drawing }
    }

    pub(crate) fn handle_create_rooms(&mut self, msg: CreateRoomsMsg) -> Task<Message> {
        if let CreateRoomsMsg::Requested = msg {
            return self.read_room_sheets();
        }
        if let CreateRoomsMsg::SheetsRead { job, result } = msg {
            self.open_create_rooms(job, result);
            return Task::none();
        }
        if let CreateRoomsMsg::SheetsReadFor {
            waiting,
            board,
            request,
            result,
        } = msg
        {
            return self.finish_rooms_from_sheets(*waiting, board, request, result);
        }
        if matches!(msg, CreateRoomsMsg::Confirmed) {
            return self.create_rooms();
        }
        let rooms = &mut self.ui_state.rooms;
        if matches!(msg, CreateRoomsMsg::Cancelled) {
            rooms.create = None;
            rooms.pending = None;
            return Task::none();
        }
        let Some(dialog) = rooms.create.as_mut() else {
            return Task::none();
        };
        let options = &mut dialog.options;
        match msg {
            CreateRoomsMsg::RowToggled(name) => {
                if !dialog.unticked.remove(&name) {
                    dialog.unticked.insert(name);
                }
            }
            CreateRoomsMsg::ShapePicked(shape) => options.shape = shape,
            CreateRoomsMsg::MarginEdited(text) => {
                if let Some(mm) = parse_length_field_mm(&text, Unit::Mm).filter(|mm| *mm >= 0.0) {
                    options.margin_mm = mm;
                }
                dialog.margin_text = text;
            }
            CreateRoomsMsg::LayerPicked(layer) => options.layer = layer,
            CreateRoomsMsg::MovePartsToggled(on) => options.move_parts = on,
            CreateRoomsMsg::KeepRuleToggled(on) => options.keep_rule = on,
            CreateRoomsMsg::ReplaceToggled(on) => options.replace_same_name = on,
            CreateRoomsMsg::Requested
            | CreateRoomsMsg::SheetsRead { .. }
            | CreateRoomsMsg::SheetsReadFor { .. }
            | CreateRoomsMsg::Cancelled
            | CreateRoomsMsg::Confirmed => {}
        }
        Task::none()
    }

    /// Read the sheets of the board in view's project on a worker; the
    /// dialog opens when they come back.
    fn read_room_sheets(&mut self) -> Task<Message> {
        let Some(path) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return Task::none();
        };
        let Some((plan, project_dir, project_file)) = self.room_sheet_source(&path) else {
            let line = t!(self.ui_state.i18n, ids::ROOMS_NO_PROJECT);
            crate::diagnostics::log_warning(line.into_string());
            return Task::none();
        };
        let rooms = &mut self.ui_state.rooms;
        rooms.reads += 1;
        rooms.pending = Some((path, project_file));
        let job = rooms.reads;
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || sheet_members(plan, &project_dir))
                    .await
                    .unwrap_or_else(|e| Err(format!("the worker stopped: {e}")))
            },
            move |result| Message::from(CreateRoomsMsg::SheetsRead { job, result }),
        )
    }

    /// The sheets came back: open the dialog, or say why not.
    fn open_create_rooms(&mut self, job: u64, result: Result<Vec<SheetMembers>, String>) {
        let rooms = &mut self.ui_state.rooms;
        if job != rooms.reads {
            return;
        }
        let Some((board, project_file)) = rooms.pending.take() else {
            return;
        };
        match result {
            Ok(sheets) if sheets.is_empty() => {
                let line = t!(self.ui_state.i18n, ids::ROOMS_NO_SHEETS);
                crate::diagnostics::log_warning(line.into_string());
            }
            Ok(sheets) => {
                rooms.create = Some(Box::new(CreateRoomsDialog::new(
                    board,
                    project_file,
                    sheets,
                )));
            }
            Err(why) => {
                let line = t!(self.ui_state.i18n, ids::ROOMS_READ_FAILED, "why" => why);
                crate::diagnostics::log_warning(line.into_string());
            }
        }
    }

    /// Create: the ticked rooms and their part moves as one step on the
    /// board the dialog was opened on, made by the operand handler the CLI
    /// runs, on the sheets the dialog read, handed over as the handler's
    /// read (#2987). Rooms of a name already on the board that are kept are
    /// said in Messages. Returns what the step left to do.
    fn create_rooms(&mut self) -> Task<Message> {
        let Some(dialog) = self.ui_state.rooms.create.take() else {
            return Task::none();
        };
        let path = dialog.board.clone();
        let Some(operands) = self
            .document_state
            .pcb_board(&path)
            .and_then(|board| dialog.operands(board))
        else {
            return Task::none();
        };
        let reads = EditReads {
            room_sheets: Some(dialog.sheets),
            ..EditReads::default()
        };
        self.document_state.agent_edits.reads = Some(Arc::new(reads));
        let (outcome, task) = translator::run(self, "create_rooms_from_sheets", &path, operands);
        self.document_state.agent_edits.reads = None;
        match outcome {
            CommandOutcome::Answered(answer) => {
                let kept = answer["kept"].as_array().cloned().unwrap_or_default();
                for name in kept.iter().filter_map(|n| n.as_str()) {
                    let line = t!(self.ui_state.i18n, ids::ROOMS_KEPT, "room" => name.to_string());
                    crate::diagnostics::log_info(line.into_string());
                }
            }
            CommandOutcome::Disabled { reason } => {
                crate::diagnostics::log_warning(reason.render(&self.ui_state.i18n));
            }
            _ => {}
        }
        task
    }

    /// What reading the sheets of `board`'s project needs: the sheets to
    /// read, the project folder and the project file's name. `None` for a
    /// board in no open project with a root sheet.
    pub(super) fn room_sheet_source(
        &self,
        board: &Path,
    ) -> Option<(ProjectSheetPlan, PathBuf, String)> {
        let docs = &self.document_state;
        let project = docs
            .board_project(board, None)
            .or_else(|| docs.document_project(board))?;
        let root = project_root_sheet_path(project)?;
        let project_file = project
            .path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let plan = plan_project_sheets_for(docs, &root);
        Some((plan, project.dir().to_path_buf(), project_file))
    }
}

impl Signex {
    /// Room Properties' facts for the board at `path`: the one selected
    /// room, its member parts and how far each is outside, and the sheet
    /// placements From sheet offers. `None` unless exactly one room is
    /// selected on that board.
    pub(crate) fn room_facts(&self, path: &Path, board: &PcbBoard) -> Option<RoomFacts> {
        let selection = &self.interaction_state.pcb_selection;
        if selection.document() != Some(path) {
            return None;
        }
        let id = super::shown_room(selection)?;
        let room = board.rooms.iter().find(|r| r.uuid == id)?.clone();
        // Without the rule a part counts as outside only past the outline.
        let measured = if room.keep_parts_inside {
            room.clone()
        } else {
            Room {
                margin_mm: 0.0,
                ..room.clone()
            }
        };
        let parts = room
            .members(&board.footprints)
            .map(|fp| {
                let b = part_box(fp);
                RoomPart {
                    designator: fp.reference.clone(),
                    outside_nm: signex_drc::rooms::overhang_nm(&measured, fp).unwrap_or(0),
                    body: [b.min.x, b.min.y, b.max.x, b.max.y],
                }
            })
            .collect();
        let rooms = &self.ui_state.rooms;
        let drafts = if rooms.draft_room == Some(id) {
            rooms.drafts.clone()
        } else {
            Default::default()
        };
        Some(RoomFacts {
            sheets: sheet_choices(board, &room, &self.ui_state.i18n),
            room,
            parts,
            drafts,
            hover: rooms.hover.clone(),
        })
    }
}

/// From sheet's choices: none, then each sheet placement directly under
/// the root that a part of the board is linked under, named by the room
/// made for it or by its short id.
fn sheet_choices(board: &PcbBoard, room: &Room, i18n: &signex_i18n::I18n) -> Vec<SheetChoice> {
    let mut placements: Vec<InstancePath> = board
        .footprints
        .iter()
        .filter_map(|fp| {
            let first = *fp.schematic_link.as_ref()?.sheet.chain().first()?;
            Some(InstancePath::from_chain(vec![first]))
        })
        .chain(room.source.iter().map(|s| s.sheet.clone()))
        .collect();
    placements.sort();
    placements.dedup();
    let none = SheetChoice {
        sheet: None,
        label: t!(i18n, ids::ROOMS_SHEET_NONE).into_string(),
    };
    std::iter::once(none)
        .chain(placements.into_iter().map(|sheet| {
            let named = board
                .rooms
                .iter()
                .find(|r| r.source.as_ref().is_some_and(|s| s.sheet == sheet))
                .map(|r| r.name.clone());
            let label = named.unwrap_or_else(|| {
                sheet
                    .chain()
                    .last()
                    .map(|id| format!("/{}", short_id(id)))
                    .unwrap_or_else(|| sheet.to_string())
            });
            SheetChoice {
                sheet: Some(sheet),
                label,
            }
        }))
        .collect()
}

/// The sheets placed directly under the project's root, each with every
/// placement in design order; a sheet placed twice is one entry with two
/// placements.
pub(super) fn sheet_members(
    plan: ProjectSheetPlan,
    project_dir: &Path,
) -> Result<Vec<SheetMembers>, String> {
    let set = plan.assemble();
    let design = crate::app::design_read::DesignGraph::read(&set, project_dir)?;
    let mut sheets: Vec<SheetMembers> = Vec::new();
    for (placement, _, file) in design.placed() {
        if placement.chain().len() != 1 {
            continue;
        }
        let stem = |p: Option<&std::ffi::OsStr>| {
            p.map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        let (name, file) = (stem(file.file_stem()), stem(file.file_name()));
        match sheets.iter_mut().find(|s| s.file == file) {
            Some(sheet) => sheet.placements.push(placement),
            None => sheets.push(SheetMembers {
                name,
                file,
                placements: vec![placement],
            }),
        }
    }
    Ok(sheets)
}
