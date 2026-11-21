//! Design reuse blocks (#2410): a placed and routed group of a board saved
//! once as a block of its project and placed again on another channel of the
//! same circuit, its copper with it. `signex_engine::pcb::reuse_block` cuts
//! the block out and lays it down; this slice keeps the project's blocks in
//! the Model, reads and writes their files off the update thread, and runs a
//! placement as one board undo step. Rules: `docs/PCB_TOOLS.md`, "Design
//! reuse blocks".

mod operands;
mod store;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use serde::Serialize;
use signex_engine::pcb::BoardItemId;
use signex_engine::pcb::reuse_block;
use signex_i18n::{command_ids, ids, t};
use signex_types::format::SnxPcb;
use signex_types::pcb::PcbBoard;
use uuid::Uuid;

pub(crate) use operands::{place_board_reuse_block, save_board_reuse_block};
pub use store::{BlocksOnDisk, ReuseBlockStore};

use crate::app::agent::Origin;
use crate::app::command::{CommandArgs, CommandOutcome, OperandValue, translator};
use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{Message, PcbMsg, Signex};
use crate::keymap::AppCommandId;

/// The design reuse slice's events, under `Message::Pcb`.
#[derive(Debug, Clone, PartialEq)]
pub enum ReuseBlockMsg {
    /// `save_board_reuse_block` ran without operands (the board menu, the
    /// palette): the selection is saved as a block named after its
    /// designators.
    SaveRequested,
    /// A board menu row: place the block `name` around the one footprint
    /// selected on the board in view.
    PlaceRequested { name: String },
    /// A block file was written, or why it was not.
    Written {
        saved: SavedBlock,
        result: Result<(), String>,
    },
    /// The block folder of the project in `folder` was read off the update
    /// thread; `rerun` is a command that waited for it.
    Read {
        folder: PathBuf,
        read: SharedRead,
        rerun: Option<Box<ReuseRerun>>,
    },
}

/// A read of a block folder, shared without a copy. Two reads are the same
/// when they are one read.
#[derive(Debug, Clone)]
pub struct SharedRead(pub(crate) Arc<BlocksOnDisk>);

impl PartialEq for SharedRead {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

/// A `place_board_reuse_block` from operands that waited for the project's
/// blocks to be read: run again, as the caller that sent it, when they land.
#[derive(Debug, Clone, PartialEq)]
pub struct ReuseRerun {
    pub(crate) ticket: u64,
    pub(crate) command: AppCommandId,
    pub(crate) args: CommandArgs,
    pub(crate) board: PathBuf,
    pub(crate) origin: Origin,
    pub(crate) serving: bool,
}

/// A saved block, as the command answers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SavedBlock {
    pub name: String,
    pub file: PathBuf,
    pub footprints: usize,
    pub tracks: usize,
    pub vias: usize,
}

/// A placed block, as the command answers it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct PlacedBlock {
    pub name: String,
    pub anchor: String,
    /// Each block designator and the component it placed.
    pub footprints: Vec<(String, String)>,
    pub tracks: usize,
    pub vias: usize,
}

impl Signex {
    pub(crate) fn handle_reuse_block(&mut self, msg: ReuseBlockMsg) -> Task<Message> {
        match msg {
            ReuseBlockMsg::SaveRequested => self.save_selection_as_block(),
            ReuseBlockMsg::PlaceRequested { name } => self.place_block_on_selection(&name),
            ReuseBlockMsg::Written { saved, result } => {
                self.end_project_write(&saved.file);
                self.report_block_written(&saved, result);
                self.start_waiting_block_write(&saved.file)
            }
            ReuseBlockMsg::Read {
                folder,
                read,
                rerun,
            } => self.land_block_read(&folder, &read.0, rerun),
        }
    }

    /// The folder of the project `board` belongs to: where its blocks live.
    pub(crate) fn reuse_block_folder(&self, board: &Path) -> Result<PathBuf, String> {
        let docs = &self.document_state;
        docs.document_project(board)
            .or_else(|| docs.project_for_path(board))
            .and_then(|project| project.path.parent().map(Path::to_path_buf))
            .ok_or_else(|| {
                "the board is in no open project, and reuse blocks are kept per project".to_string()
            })
    }

    /// Read the blocks of the project of the board in view, once: its board
    /// menu then lists them.
    pub(crate) fn read_reuse_blocks_in_view(&mut self) -> Task<Message> {
        let Some(board) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return Task::none();
        };
        let Ok(folder) = self.reuse_block_folder(&board) else {
            return Task::none();
        };
        self.document_state.reuse_blocks.retry_failed_read(&folder);
        if !self.document_state.reuse_blocks.needs_read(&folder) {
            return Task::none();
        }
        read_blocks_task(folder, None)
    }

    /// The names of the blocks of the project of the board `window` shows
    /// (`None`: the main window), for its board menu.
    pub(crate) fn reuse_block_names_in_window(
        &self,
        window: Option<iced::window::Id>,
    ) -> Vec<String> {
        let undocked = window.and_then(|id| self.overlay_owner(id));
        let board = match undocked.and_then(|id| self.ui_state.windows.get(&id)) {
            Some(crate::app::state::WindowKind::UndockedTab { path, .. }) => Some(path.clone()),
            _ => self.active_tab_path().filter(|_| self.has_active_pcb()),
        };
        board
            .and_then(|board| self.reuse_block_folder(&board).ok())
            .map(|folder| self.document_state.reuse_blocks.names(&folder))
            .unwrap_or_default()
    }

    /// Save `items` of `board` as the project's block `name`: kept in the
    /// Model at once, its file written by the returned task.
    pub(crate) fn save_reuse_block(
        &mut self,
        board: &Path,
        name: &str,
        items: &[BoardItemId],
    ) -> Result<(SavedBlock, Task<Message>), String> {
        let folder = self.reuse_block_folder(board)?;
        let pcb = self
            .document_state
            .pcb_board(board)
            .ok_or_else(|| "the board is not open".to_string())?;
        // What is saved is checked before its name: a menu save of a
        // selection with no footprint has no designator to name it by.
        let block = reuse_block::capture(pcb, items).map_err(|refusal| refusal.to_string())?;
        let name = store::checked_name(name)?;
        // The case check below needs the folder's listing: without it a
        // block of an earlier session is not seen.
        let blocks = &mut self.document_state.reuse_blocks;
        if blocks.needs_read(&folder) {
            return Err("the project's reuse blocks are not read yet".to_string());
        }
        if let Some(why) = blocks.answer_read_failure(&folder) {
            return Err(format!(
                "the project's reuse blocks could not be read: {why}"
            ));
        }
        // A case-blind file system keeps one file for names that differ only
        // in case, so such a name would overwrite the other block's file.
        if let Some(other) = blocks
            .names(&folder)
            .into_iter()
            .find(|other| *other != name && other.eq_ignore_ascii_case(&name))
        {
            return Err(format!(
                "the project has a reuse block named {other}; use that name or another"
            ));
        }
        let file = store::block_file(&folder, &name);
        self.refuse_while_project_moves(&file, "save the reuse block to")
            .map_err(|error| error.to_string())?;
        let text = SnxPcb::new(block.clone())
            .write_string()
            .map_err(|error| format!("the block could not be written out: {error}"))?;
        let saved = SavedBlock {
            file,
            footprints: block.footprints.len(),
            tracks: block.segments.len(),
            vias: block.vias.len(),
            name,
        };
        let blocks = &mut self.document_state.reuse_blocks;
        blocks.put(&folder, &saved.name, block);
        let task = match blocks.begin_write((saved.clone(), text)) {
            Some(write) => self.start_block_write(write),
            None => Task::none(),
        };
        Ok((saved, task))
    }

    /// The project's block `name` of `board`, as kept in the Model; refused
    /// when the project's blocks could not be read or it has none so named.
    fn block_to_place(&mut self, board: &Path, name: &str) -> Result<PcbBoard, String> {
        let folder = self.reuse_block_folder(board)?;
        let blocks = &mut self.document_state.reuse_blocks;
        if let Some(why) = blocks.answer_read_failure(&folder) {
            return Err(format!(
                "the project's reuse blocks could not be read: {why}"
            ));
        }
        blocks
            .block(&folder, name.trim())
            .cloned()
            .ok_or_else(|| format!("the project has no reuse block named {}", name.trim()))
    }

    /// Place the project's block `name` on `board` around `anchor`, turned
    /// by `degrees`, as one undo step. The block must be in the Model.
    pub(crate) fn place_reuse_block(
        &mut self,
        board: &Path,
        name: &str,
        anchor: Uuid,
        degrees: f64,
    ) -> Result<PlacedBlock, String> {
        let block = self.block_to_place(board, name)?;
        let engine = self
            .document_state
            .board_engine(board)
            .ok_or_else(|| "the board is not open".to_string())?;
        let revision = engine.revision();
        let anchor_name = engine
            .board()
            .footprints
            .iter()
            .find(|fp| fp.uuid == anchor)
            .map(signex_engine::pcb::placement::name)
            .unwrap_or_default();
        let mut new_id = Uuid::now_v7;
        let placement = reuse_block::place(&block, engine.board(), anchor, degrees, &mut new_id)
            .map_err(|refusal| refusal.to_string())?;
        let label = t!(
            self.ui_state.i18n,
            command_ids::CMD_MENU_PLACE_BOARD_REUSE_BLOCK
        )
        .into_string();
        if !placement.commands.is_empty() {
            let step = CheckedStep {
                label: &label,
                expected_revision: revision,
            };
            self.execute_board_commands(board, placement.commands, Some(step))
                .map_err(|error| error.to_string())?;
        }
        let placed = PlacedBlock {
            name: name.trim().to_string(),
            anchor: anchor_name,
            footprints: placement.footprints,
            tracks: placement.tracks,
            vias: placement.vias,
        };
        let line = t!(
            self.ui_state.i18n,
            ids::PCB_REUSE_BLOCK_PLACED,
            "name" => placed.name.as_str(),
            "anchor" => placed.anchor.as_str(),
            "footprints" => placed.footprints.len().to_string(),
            "tracks" => placed.tracks.to_string(),
            "vias" => placed.vias.to_string()
        );
        crate::diagnostics::log_info(line.into_string());
        self.sync_pcb_board_summary();
        Ok(placed)
    }

    /// `save_board_reuse_block` from the board menu: the selection, named
    /// after its designators, saved by the command's operand handler with
    /// the board, the name and the selected footprints, tracks and vias
    /// filled in (`command::translator`); a block is made of those alone.
    /// Before the project's blocks are read, they are read first and the
    /// save is asked again once they land, so the handler never defers it.
    fn save_selection_as_block(&mut self) -> Task<Message> {
        let Some(board) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return Task::none();
        };
        if let Ok(folder) = self.reuse_block_folder(&board)
            && self.retry_and_need_read(&folder)
        {
            let again = Message::Pcb(PcbMsg::ReuseBlock(ReuseBlockMsg::SaveRequested));
            return read_blocks_task(folder, None).chain(Task::done(again));
        }
        let items = self
            .with_pcb_selection(|state, _| {
                crate::app::handlers::pcb_placement::deletable_items(state.selection())
            })
            .unwrap_or_default();
        // What the selection cannot make a block of is refused in the menu's
        // own words, after the project check, as before the translator:
        // `ids` would refuse an empty or stale list in an agent's words.
        // shortcut: the handler captures the block again, a second board
        // clone per save; drop this check if `ids` ever keeps capture's
        // wording.
        let refusal = self
            .reuse_block_folder(&board)
            .ok()
            .and(self.document_state.pcb_board(&board))
            .and_then(|pcb| reuse_block::capture(pcb, &items).err());
        if let Some(refusal) = refusal {
            let why = refusal.to_string();
            self.refuse_reuse(command_ids::CMD_MENU_SAVE_BOARD_REUSE_BLOCK, &why);
            return Task::none();
        }
        let name = self.block_name_for(&board, &items);
        let ids: Vec<String> = items
            .iter()
            .filter_map(|item| match item {
                BoardItemId::Footprint(id) | BoardItemId::Segment(id) | BoardItemId::Via(id) => {
                    Some(id.to_string())
                }
                _ => None,
            })
            .collect();
        let operands = vec![
            ("name", OperandValue::Text(name)),
            ("ids", OperandValue::Text(ids.join(","))),
        ];
        let (outcome, task) = translator::run(self, "save_board_reuse_block", &board, operands);
        // Answered: the write landing says it. Deferred cannot come: the
        // blocks are read above and the handler reads nothing for the menu.
        if let CommandOutcome::Disabled { reason } = outcome {
            let why = reason.render(&self.ui_state.i18n);
            self.refuse_reuse(command_ids::CMD_MENU_SAVE_BOARD_REUSE_BLOCK, &why);
        }
        task
    }

    /// A save or place request starts: whether the project's blocks in
    /// `folder` are to be read first. A failed read a request was refused
    /// for is read again.
    pub(crate) fn retry_and_need_read(&mut self, folder: &Path) -> bool {
        let blocks = &mut self.document_state.reuse_blocks;
        blocks.retry_answered_failure(folder);
        blocks.needs_read(folder)
    }

    /// Start the block write `write`, registered as a project write: a
    /// project folder rename waits for it ([`Self::begin_project_write`]).
    fn start_block_write(&mut self, write: store::BlockWrite) -> Task<Message> {
        self.begin_project_write(&write.0.file);
        write_block_task(write)
    }

    /// The write of `file` landed: start the save that waited for it, unless
    /// the project folder is being renamed or was renamed since. The write
    /// would then recreate the old folder, so it is dropped and Messages
    /// says why.
    fn start_waiting_block_write(&mut self, file: &Path) -> Task<Message> {
        let Some(next) = self.document_state.reuse_blocks.end_write(file) else {
            return Task::none();
        };
        if let Err(refusal) = self.refuse_while_project_moves(file, "save the reuse block to") {
            self.document_state.reuse_blocks.drop_write(file);
            self.report_block_written(&next.0, Err(refusal.to_string()));
            return Task::none();
        }
        self.start_block_write(next)
    }

    /// The designators of the footprints in `items`, sorted and joined by
    /// `-`: the name a block saved from the menu takes.
    fn block_name_for(&self, board: &Path, items: &[BoardItemId]) -> String {
        let Some(pcb) = self.document_state.pcb_board(board) else {
            return String::new();
        };
        let mut names: Vec<&str> = pcb
            .footprints
            .iter()
            .filter(|fp| items.contains(&BoardItemId::Footprint(fp.uuid)))
            .map(|fp| fp.reference.trim())
            .filter(|name| !name.is_empty())
            .collect();
        names.sort_unstable();
        names.join("-").chars().take(64).collect()
    }

    /// A board menu row: the block around the one footprint selected, placed
    /// by the command's operand handler with the board, the name and the
    /// footprint's uuid filled in (`command::translator`). The block must be
    /// in the Model, as the menu lists it: a missing or unread one is refused
    /// here at once, so the handler never reads the blocks first.
    fn place_block_on_selection(&mut self, name: &str) -> Task<Message> {
        let Some(board) = self.active_tab_path().filter(|_| self.has_active_pcb()) else {
            return Task::none();
        };
        let footprints = self.pcb_selected_footprints();
        let anchor = match (footprints.len(), footprints.first()) {
            (1, Some(anchor)) => self.block_to_place(&board, name).map(|_| *anchor),
            _ => Err("select one component of the channel to place the block on".to_string()),
        };
        let anchor = match anchor {
            Ok(anchor) => anchor,
            Err(why) => {
                self.refuse_reuse(command_ids::CMD_MENU_PLACE_BOARD_REUSE_BLOCK, &why);
                return Task::none();
            }
        };
        let operands = vec![
            ("name", OperandValue::Text(name.to_string())),
            ("ids", OperandValue::Targets(vec![anchor])),
        ];
        let (outcome, task) = translator::run(self, "place_board_reuse_block", &board, operands);
        // Answered: `place_reuse_block` says it. Deferred cannot come: the
        // handler reads nothing for the menu (`agent.from_view`).
        if let CommandOutcome::Disabled { reason } = outcome {
            let why = reason.render(&self.ui_state.i18n);
            self.refuse_reuse(command_ids::CMD_MENU_PLACE_BOARD_REUSE_BLOCK, &why);
        }
        task
    }

    /// Say in Messages that the command `what` names did nothing, and why.
    pub(crate) fn refuse_reuse(&self, what: signex_i18n::MessageId, why: &str) {
        let what = t!(self.ui_state.i18n, what).into_string();
        let line = t!(
            self.ui_state.i18n,
            ids::PCB_REUSE_BLOCK_REFUSED,
            "what" => what.as_str(),
            "why" => why
        );
        crate::diagnostics::log_warning(line.into_string());
    }

    fn report_block_written(&self, saved: &SavedBlock, result: Result<(), String>) {
        let i18n = &self.ui_state.i18n;
        let file = saved.file.display().to_string();
        match result {
            Ok(()) => {
                let line = t!(
                    i18n,
                    ids::PCB_REUSE_BLOCK_SAVED,
                    "name" => saved.name.as_str(),
                    "file" => file.as_str(),
                    "footprints" => saved.footprints.to_string(),
                    "tracks" => saved.tracks.to_string(),
                    "vias" => saved.vias.to_string()
                );
                crate::diagnostics::log_info(line.into_string());
            }
            Err(why) => {
                let line = t!(
                    i18n,
                    ids::PCB_REUSE_BLOCK_NOT_WRITTEN,
                    "name" => saved.name.as_str(),
                    "file" => file.as_str(),
                    "why" => why.as_str()
                );
                crate::diagnostics::log_warning(line.into_string());
            }
        }
    }

    /// Take a folder read in, say what could not be read, and run the
    /// command that waited for it.
    fn land_block_read(
        &mut self,
        folder: &Path,
        read: &BlocksOnDisk,
        rerun: Option<Box<ReuseRerun>>,
    ) -> Task<Message> {
        for why in read.failures.iter().chain(&read.error) {
            let line = t!(self.ui_state.i18n, ids::PCB_REUSE_BLOCKS_UNREAD, "why" => why.as_str());
            crate::diagnostics::log_warning(line.into_string());
        }
        self.document_state.reuse_blocks.land(folder, read);
        match rerun {
            Some(rerun) => {
                let ReuseRerun {
                    ticket,
                    command,
                    args,
                    board,
                    origin,
                    serving,
                } = *rerun;
                self.rerun_after_read(ticket, &command, &args, &board, origin, serving)
            }
            None => Task::none(),
        }
    }
}

/// Read the block folder of the project in `folder` off the update thread.
pub(crate) fn read_blocks_task(folder: PathBuf, rerun: Option<Box<ReuseRerun>>) -> Task<Message> {
    let dir = folder.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || store::read_blocks(&dir))
                .await
                .unwrap_or_else(|error| BlocksOnDisk {
                    error: Some(format!("the read did not finish: {error}")),
                    ..BlocksOnDisk::default()
                })
        },
        move |read| {
            Message::Pcb(PcbMsg::ReuseBlock(ReuseBlockMsg::Read {
                folder: folder.clone(),
                read: SharedRead(Arc::new(read)),
                rerun: rerun.clone(),
            }))
        },
    )
}

/// Write a saved block's file off the update thread. One file's writes are
/// started one at a time ([`ReuseBlockStore::begin_write`]).
fn write_block_task((saved, text): store::BlockWrite) -> Task<Message> {
    let file = saved.file.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || store::write_block(&file, &text))
                .await
                .unwrap_or_else(|error| Err(format!("the write did not finish: {error}")))
        },
        move |result| {
            Message::Pcb(PcbMsg::ReuseBlock(ReuseBlockMsg::Written {
                saved: saved.clone(),
                result,
            }))
        },
    )
}
