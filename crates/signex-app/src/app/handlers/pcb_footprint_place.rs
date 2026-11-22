//! Place in the Library panel's Footprints kind (mockup row 119, #2828): the
//! footprint goes on the cursor of the open board through the same Place
//! carry as a library part (`pcb_part_place`). The editor holds no unsaved
//! edits here, so its copy is the saved one and `update` reads no disk.

use iced::Task;
use signex_engine::pcb::part_place::PartToPlace;
use signex_i18n::{ids, t};
use signex_library::Footprint;
use signex_library::part_footprint::resolve_named_footprint;
use signex_library::primitive::PrimitiveRef;
use signex_types::designator::placement_prefix;
use uuid::Uuid;

use crate::app::state::WindowKind;
use crate::app::{Message, Signex, TabKind};
use crate::panels::FootprintPlaceBlock as Block;

impl Signex {
    /// Why Place cannot put row `idx` of the open file on a board.
    pub(crate) fn footprint_place_block(&self, idx: usize) -> Option<Block> {
        self.footprint_place_source(idx)
            .and(self.footprint_place_board())
            .err()
    }

    /// The library address and saved copy of row `idx`. A library address
    /// reads the first footprint of the file named by its uuid, so only that
    /// one can keep its link to the library.
    fn footprint_place_source(&self, idx: usize) -> Result<(PrimitiveRef, &Footprint), Block> {
        if self.interaction_state.acting_window.is_some() {
            return Err(Block::UndockedEditor);
        }
        let path = self
            .active_footprint_editor_path()
            .ok_or(Block::NotInLibrary)?;
        let editor = self
            .document_state
            .footprint_editors
            .get(&path)
            .ok_or(Block::NotInLibrary)?;
        if editor.is_dirty() {
            return Err(Block::UnsavedEdits);
        }
        let footprint = editor.file.footprints.get(idx).ok_or(Block::NotInLibrary)?;
        if idx != 0 {
            return Err(Block::NotFirstInFile);
        }
        let file_id = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| Uuid::parse_str(stem).ok());
        if file_id != Some(footprint.uuid) {
            return Err(Block::NotInLibrary);
        }
        let library = self
            .library
            .containing_library_with(&path, self.identity_fold_for(&path))
            .ok_or(Block::NotInLibrary)?;
        Ok((
            PrimitiveRef::new(library.library_id, footprint.uuid),
            footprint,
        ))
    }

    /// The tab of the one open board that can take a footprint.
    fn footprint_place_board(&self) -> Result<usize, Block> {
        let mut boards = self
            .document_state
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, tab)| matches!(tab.kind, TabKind::Pcb));
        let (idx, tab) = boards.next().ok_or(Block::NoBoard)?;
        if boards.next().is_some() {
            return Err(Block::SeveralBoards);
        }
        let board = self
            .document_state
            .pcb_board(&tab.path)
            .ok_or(Block::NoBoard)?;
        if board.unknown.newer_minor_source.is_some() {
            return Err(Block::ReadOnlyBoard);
        }
        let in_sketch = self
            .interaction_state
            .board_sketch
            .as_ref()
            .is_some_and(|session| session.board_path == tab.path);
        if in_sketch {
            return Err(Block::BoardInSketch);
        }
        Ok(idx)
    }

    /// Place pressed on row `idx`: show the board and put the footprint on
    /// its cursor. A footprint the board cannot hold is refused with why.
    pub(crate) fn place_library_footprint(&mut self, idx: usize) -> Task<Message> {
        let target = self
            .footprint_place_source(idx)
            .and_then(|source| Ok((source, self.footprint_place_board()?)));
        let ((address, footprint), tab) = match target {
            Ok(target) => target,
            Err(block) => {
                let why = t!(self.ui_state.i18n, block.reason()).into_string();
                crate::diagnostics::log_warning(why);
                return Task::none();
            }
        };
        let name = footprint.name.clone();
        let part = footprint_part(&address, footprint);
        let part = match part {
            Ok(part) => part,
            Err(reason) => {
                let line = t!(
                    self.ui_state.i18n,
                    ids::PCB_PLACE_PART_REFUSED,
                    "part" => name.as_str(),
                    "reason" => reason.as_str()
                )
                .into_string();
                self.report_place_failure(crate::app::state::untranslated(line));
                return Task::none();
            }
        };
        self.library.picker = None;
        // A read still in flight from an earlier pick must not replace this.
        self.interaction_state.placement_epoch += 1;
        self.carry_footprint_to_board(tab, &name, part)
    }

    /// Show the board of `tab`, in its own window when undocked, with `part`
    /// on the cursor.
    fn carry_footprint_to_board(
        &mut self,
        tab: usize,
        name: &str,
        part: PartToPlace,
    ) -> Task<Message> {
        let Some(board) = self.document_state.tabs.get(tab).map(|t| t.path.clone()) else {
            return Task::none();
        };
        let undocked = self.ui_state.windows.iter().find_map(|(id, kind)| {
            matches!(kind, WindowKind::UndockedTab { path, .. } if *path == board).then_some(*id)
        });
        if let Some(window) = undocked {
            let name = name.to_owned();
            return self.run_in_window(window, false, move |app| {
                app.put_part_on_cursor(&board, &name, part);
                Task::none()
            });
        }
        if tab != self.document_state.active_tab {
            self.park_active_schematic_session();
            self.document_state.active_tab = tab;
            self.sync_active_tab();
        }
        self.put_part_on_cursor(&board, name, part);
        Task::none()
    }
}

/// The library footprint as a part for the board: numbered from its own
/// default designator, with no value since no component row stands behind it.
fn footprint_part(address: &PrimitiveRef, footprint: &Footprint) -> Result<PartToPlace, String> {
    let resolved = resolve_named_footprint(address, |_, _| Some(footprint.clone()))
        .map_err(|why| why.to_string())?;
    let prefix = placement_prefix(&footprint.default_designator);
    PartToPlace::new(resolved.footprint, prefix, "").map_err(|why| why.to_string())
}

#[cfg(test)]
#[path = "pcb_footprint_place_tests.rs"]
mod tests;
