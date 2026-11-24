//! Per-kind object counts the filter panels show beside each kind. They are
//! counted when the panel context is built, once per document revision,
//! never in `view`.

use std::collections::HashMap;
use std::hash::Hash;
use std::iter::{once, repeat_n};
use std::path::PathBuf;

use pcb_geom::PickKind;
use signex_types::pcb::PcbBoard;

use crate::active_bar::SelectionFilter;

/// How many objects of each kind a document holds. A kind it lacks is 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindCounts<K: Hash + Eq> {
    counts: HashMap<K, usize>,
}

impl<K: Hash + Eq> Default for KindCounts<K> {
    fn default() -> Self {
        Self {
            counts: HashMap::new(),
        }
    }
}

impl<K: Hash + Eq + Copy> KindCounts<K> {
    /// One count per kind `kinds` names, each occurrence one object.
    pub fn of(kinds: impl IntoIterator<Item = K>) -> Self {
        let mut counts = HashMap::new();
        for kind in kinds {
            *counts.entry(kind).or_insert(0) += 1;
        }
        Self { counts }
    }

    pub fn get(&self, kind: K) -> usize {
        self.counts.get(&kind).copied().unwrap_or(0)
    }
}

/// The editor the Filter panel follows: the active tab's.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FilterEditor {
    /// No schematic or board tab is active.
    #[default]
    None,
    Schematic,
    Board,
}

/// What the Filter and SCH Filter panels read besides the filters
/// themselves: the active editor, its per-kind counts and the preset name
/// being typed.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SelectionFilterFacts {
    pub editor: FilterEditor,
    /// Objects on the active sheet; empty unless `editor` is `Schematic`.
    pub schematic: KindCounts<SelectionFilter>,
    /// Objects on the active board; empty unless `editor` is `Board`.
    pub board: KindCounts<PickKind>,
    /// The document and revision the counts were taken from: a rebuild at
    /// the same key keeps them.
    pub key: Option<(PathBuf, u64)>,
    /// SCH Filter's inline rename: the preset and the name typed so far.
    pub rename: Option<(usize, String)>,
}

/// The board objects of each kind, by the kinds [`PickKind::of`] gives the
/// items the selectable index holds: a footprint, its pads and its shown
/// texts, tracks, vias, free pads and arcs, dimensions (Other), board
/// texts, drawn areas and drawings.
pub fn board_counts(board: &PcbBoard) -> KindCounts<PickKind> {
    let footprints = board.footprints.iter().flat_map(|footprint| {
        once(PickKind::Components)
            .chain(repeat_n(PickKind::Pads, footprint.pads.len()))
            .chain(repeat_n(
                PickKind::Texts,
                pcb_geom::footprint_texts(footprint).len(),
            ))
    });
    let kinds = footprints
        .chain(board.segments.iter().map(|_| PickKind::Tracks))
        .chain(board.vias.iter().map(|_| PickKind::Vias))
        .chain(board.free.pads.iter().map(|_| PickKind::Pads))
        .chain(board.free.arcs.iter().map(|_| PickKind::Arcs))
        .chain(board.free.dimensions.iter().map(|_| PickKind::Other))
        .chain(board.texts.iter().map(|_| PickKind::Texts))
        .chain(board.zones.iter().filter_map(PickKind::of_zone))
        .chain(
            board
                .graphics
                .iter()
                .filter_map(|graphic| graphic.kind().map(PickKind::of_graphic)),
        );
    KindCounts::of(kinds)
}
