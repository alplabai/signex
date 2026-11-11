//! The selection surfaces on screen (the main window's canvas and each
//! undocked document window) and what a selection on one stands for.

use std::path::{Path, PathBuf};

use pcb_geom::ItemRef;
use signex_net::cross_probe::{self, PartOccurrence};
use signex_types::pcb::PcbBoard;
use signex_types::schematic::{InstancePath, SelectedItem, SelectedKind};

use crate::app::state::WindowKind;
use crate::app::state::cross_select::{CrossSelection, SelectSurface, SurfaceStamp};
use crate::app::{Signex, TabKind};

/// A window (`None`: the main one) and the document it shows.
pub(super) type Shown = (Option<iced::window::Id>, PathBuf);

impl Signex {
    /// The sheet the main window shows, when its active tab is a sheet.
    fn main_sheet(&self) -> Option<PathBuf> {
        let docs = &self.document_state;
        let tab = docs.tabs.get(docs.active_tab)?;
        let shown = matches!(tab.kind, TabKind::Schematic)
            && docs.active_path.as_deref() == Some(tab.path.as_path())
            && docs.engines.contains_key(&tab.path);
        shown.then(|| tab.path.clone())
    }

    /// The board the main window shows, when its active tab is a board.
    fn main_board(&self) -> Option<PathBuf> {
        self.active_pcb()?;
        self.active_tab_path()
    }

    /// Undocked document windows and the document each shows.
    fn undocked_documents(&self) -> Vec<(iced::window::Id, PathBuf)> {
        self.ui_state
            .windows
            .iter()
            .filter(|(id, _)| self.ui_state.main_window_id != Some(**id))
            .filter_map(|(id, kind)| match kind {
                WindowKind::UndockedTab { path, .. } => Some((*id, path.clone())),
                _ => None,
            })
            .collect()
    }

    /// Every sheet on screen: the main window's first, then each window's.
    pub(super) fn sheets_shown(&self) -> Vec<Shown> {
        let windows = self
            .undocked_documents()
            .into_iter()
            .filter(|(id, path)| {
                self.interaction_state.canvases.contains_key(id)
                    && self.document_state.engines.contains_key(path)
            })
            .map(|(id, path)| (Some(id), path));
        self.main_sheet()
            .map(|path| (None, path))
            .into_iter()
            .chain(windows)
            .collect()
    }

    /// Every board on screen: the main window's first, then each window's.
    pub(super) fn boards_shown(&self) -> Vec<Shown> {
        let windows = self
            .undocked_documents()
            .into_iter()
            .filter(|(_, path)| self.document_state.pcb_board(path).is_some())
            .map(|(id, path)| (Some(id), path));
        self.main_board()
            .map(|path| (None, path))
            .into_iter()
            .chain(windows)
            .collect()
    }

    /// The selection of the sheet canvas of `window` (`None`: the main one).
    pub(super) fn sheet_selection(&self, window: Option<iced::window::Id>) -> &[SelectedItem] {
        match window {
            None => &self.interaction_state.canvas.selected,
            Some(id) => self
                .interaction_state
                .canvases
                .get(&id)
                .map(|slot| slot.selected.as_slice())
                .unwrap_or_default(),
        }
    }

    /// The selection of the board canvas of `window` showing `path`; empty
    /// when its state belongs to another board.
    pub(super) fn board_selection(
        &self,
        window: Option<iced::window::Id>,
        path: &Path,
    ) -> Vec<ItemRef> {
        let state = match window {
            None => Some(&self.interaction_state.pcb_selection),
            Some(id) => self.interaction_state.pcb_selections.get(&id),
        };
        state
            .filter(|s| s.document() == Some(path))
            .map(|s| s.selection().items().collect())
            .unwrap_or_default()
    }

    /// Each surface on screen with a fingerprint of its selection.
    pub(super) fn selection_stamps(&self) -> Vec<(SelectSurface, SurfaceStamp)> {
        let sheets = self.sheets_shown().into_iter().map(|(window, path)| {
            let items = self.sheet_selection(window).iter().copied();
            (
                SelectSurface::Schematic(window),
                SurfaceStamp::new(path, items),
            )
        });
        let boards = self.boards_shown().into_iter().map(|(window, path)| {
            let items = self.board_selection(window, &path);
            (SelectSurface::Board(window), SurfaceStamp::new(path, items))
        });
        sheets.chain(boards).collect()
    }

    /// The placement the sheet at `path` is shown through; the root chain
    /// when no hierarchy says otherwise.
    fn probe_placement(&self, path: &Path) -> InstancePath {
        self.shown_placement(path)
            .unwrap_or_else(InstancePath::root)
    }

    /// What `items` on the sheet at `path` stand for on the board: its
    /// symbols as parts, its wires and labels as nets.
    pub(super) fn cross_selection_from_sheet(
        &self,
        path: &Path,
        items: &[SelectedItem],
    ) -> CrossSelection {
        let sheet = self.probe_placement(path);
        let parts = items
            .iter()
            .filter(|item| item.kind == SelectedKind::Symbol)
            .map(|item| PartOccurrence {
                sheet: sheet.clone(),
                symbol: item.uuid,
            })
            .collect();
        CrossSelection {
            parts,
            nets: self.net_names_of_selection(path, items),
        }
    }

    /// What `items` on the board at `path` stand for in the schematic.
    pub(super) fn cross_selection_from_board(
        &self,
        path: &Path,
        items: &[ItemRef],
    ) -> CrossSelection {
        self.document_state
            .pcb_board(path)
            .map(|board| cross_selection_of_board_items(board, items))
            .unwrap_or_default()
    }

    /// What the selection on `surface` stands for on the other side.
    pub(super) fn cross_selection_of(&self, surface: SelectSurface) -> CrossSelection {
        match surface {
            SelectSurface::Schematic(window) => self
                .sheets_shown()
                .into_iter()
                .find(|(shown, _)| *shown == window)
                .map(|(_, path)| {
                    self.cross_selection_from_sheet(&path, self.sheet_selection(window))
                })
                .unwrap_or_default(),
            SelectSurface::Board(window) => self
                .boards_shown()
                .into_iter()
                .find(|(shown, _)| *shown == window)
                .map(|(_, path)| {
                    let items = self.board_selection(window, &path);
                    self.cross_selection_from_board(&path, &items)
                })
                .unwrap_or_default(),
        }
    }

    /// The items on the sheet at `path` that `what` names: the symbols of
    /// its parts in the placement shown, and the members of its nets.
    pub(super) fn sheet_items_for(&self, path: &Path, what: &CrossSelection) -> Vec<SelectedItem> {
        let Some(engine) = self.document_state.engines.get(path) else {
            return Vec::new();
        };
        // An unknown placement (no hierarchy yet) matches by unit alone.
        let shown = self.shown_placement(path);
        let mut items: Vec<SelectedItem> = engine
            .document()
            .symbols
            .iter()
            .filter(|symbol| {
                what.parts.iter().any(|p| {
                    p.symbol == symbol.uuid && shown.as_ref().is_none_or(|s| *s == p.sheet)
                })
            })
            .map(|symbol| SelectedItem::new(symbol.uuid, SelectedKind::Symbol))
            .collect();
        items.extend(self.net_items_named(path, &what.nets));
        items
    }
}

/// A footprint stands for its part; a pad for its net (its part when it has
/// none); a track or via for its net.
fn cross_selection_of_board_items(board: &PcbBoard, items: &[ItemRef]) -> CrossSelection {
    let mut footprints = Vec::new();
    let mut nets = std::collections::BTreeSet::new();
    for item in items {
        match *item {
            ItemRef::Footprint(fp) => footprints.push(fp),
            ItemRef::Pad { footprint, pad } => match cross_probe::pad_net(board, footprint, pad) {
                Some(net) => {
                    nets.insert(net);
                }
                None => footprints.push(footprint),
            },
            ItemRef::Track(id) => {
                nets.extend(
                    board
                        .segments
                        .iter()
                        .filter(|s| s.uuid == id)
                        .map(|s| s.net),
                );
            }
            ItemRef::Via(id) => {
                nets.extend(board.vias.iter().filter(|v| v.uuid == id).map(|v| v.net));
            }
        }
    }
    CrossSelection {
        parts: cross_probe::parts_for_footprints(board, footprints),
        nets: cross_probe::net_names(board, &nets),
    }
}

/// The items on `board` that `what` names: the footprints of its parts and
/// the pads, tracks and vias of its nets.
pub(super) fn board_items_for(board: &PcbBoard, what: &CrossSelection) -> Vec<ItemRef> {
    let mut items: Vec<ItemRef> = cross_probe::footprints_for_parts(board, &what.parts)
        .into_iter()
        .map(ItemRef::Footprint)
        .collect();
    let nets = cross_probe::nets_named(board, &what.nets);
    let members = cross_probe::board_net_members(board, &nets);
    items.extend(
        members
            .pads
            .into_iter()
            .map(|(footprint, pad)| ItemRef::Pad { footprint, pad }),
    );
    items.extend(members.tracks.into_iter().map(ItemRef::Track));
    items.extend(members.vias.into_iter().map(ItemRef::Via));
    items
}
