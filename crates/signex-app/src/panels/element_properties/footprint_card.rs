//! The data behind the FOOTPRINT card of a placed part's Properties: what
//! the card shows, and the cache of footprints read for it.
//!
//! `update` resolves the part's footprint and asks [`FootprintCards`] for
//! its status; a footprint not read yet is queued and read off the UI
//! thread, keyed by the footprint and the library data version it was
//! asked at, so a saved footprint is read again.

use std::collections::HashMap;
use std::sync::Arc;

use signex_library::primitive::{Footprint, PrimitiveRef};

use crate::library::editor::footprint::canvas::PreviewFootprint;

/// Most footprints the cache keeps; a fuller cache drops its finished reads.
const MAX_CACHED: usize = 32;

/// One read of a footprint: the footprint and the
/// `LibraryState::data_version` it was asked at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FootprintCardKey {
    pub footprint: PrimitiveRef,
    pub version: u64,
}

/// What the card states about a footprint, read once per key.
#[derive(Debug, Clone)]
pub struct FootprintCardFacts {
    pub name: String,
    /// Copper pads.
    pub pad_count: usize,
    pub pitch_mm: Option<f64>,
    /// A STEP model is attached; the procedural body every footprint has
    /// does not count.
    pub has_3d_body: bool,
    pub preview: PreviewFootprint,
}

impl FootprintCardFacts {
    pub fn of(footprint: &Footprint) -> Self {
        Self {
            name: footprint.name.clone(),
            pad_count: footprint.copper_pad_count(),
            pitch_mm: footprint.pad_pitch_mm(),
            has_3d_body: footprint.step_attachment.is_some(),
            preview: PreviewFootprint::build(footprint),
        }
    }

    /// "0.95 mm pitch, 5 pads"; the pitch only when it can be derived.
    pub fn summary(&self) -> String {
        let pads = match self.pad_count {
            1 => "1 pad".to_string(),
            n => format!("{n} pads"),
        };
        match self.pitch_mm {
            Some(pitch) => format!("{} mm pitch, {pads}", trimmed_mm(pitch)),
            None => pads,
        }
    }
}

/// `pitch` with at most three decimals and no trailing zeros.
fn trimmed_mm(pitch: f64) -> String {
    let fixed = format!("{pitch:.3}");
    fixed.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// Where the card's footprint stands.
#[derive(Debug, Clone)]
pub enum FootprintCardStatus {
    /// The read is queued or running.
    Loading,
    Ready(Arc<FootprintCardFacts>),
    Failed(String),
    /// The part names a footprint that no open library binds it to.
    Unlinked,
}

/// The card of the selected placed part.
#[derive(Debug, Clone)]
pub struct FootprintCardView {
    /// The footprint name the part holds.
    pub name: String,
    /// The library footprint behind it, when the part is linked to one.
    pub key: Option<FootprintCardKey>,
    /// Library data version the card was resolved at.
    pub version: u64,
    pub status: FootprintCardStatus,
}

/// Footprints read for the card, by key.
#[derive(Debug, Default)]
pub struct FootprintCards {
    entries: HashMap<FootprintCardKey, FootprintCardStatus>,
    queued: Vec<FootprintCardKey>,
}

impl FootprintCards {
    /// What the card for `key` shows now. The first ask queues a read; an
    /// ask at a newer version drops the reads of older versions.
    pub fn status(&mut self, key: FootprintCardKey) -> FootprintCardStatus {
        self.entries.retain(|k, _| k.version >= key.version);
        if let Some(status) = self.entries.get(&key) {
            return status.clone();
        }
        if self.entries.len() >= MAX_CACHED {
            self.entries
                .retain(|_, s| matches!(s, FootprintCardStatus::Loading));
        }
        self.entries.insert(key, FootprintCardStatus::Loading);
        self.queued.push(key);
        FootprintCardStatus::Loading
    }

    /// The reads to start, oldest first.
    pub fn take_queued(&mut self) -> Vec<FootprintCardKey> {
        std::mem::take(&mut self.queued)
    }

    /// Record a finished read. `None` when nothing waits for it any more (a
    /// newer version replaced it), so the result is dropped.
    pub fn finish(
        &mut self,
        key: FootprintCardKey,
        result: Result<Arc<FootprintCardFacts>, String>,
    ) -> Option<FootprintCardStatus> {
        let slot = self.entries.get_mut(&key)?;
        if !matches!(slot, FootprintCardStatus::Loading) {
            return None;
        }
        *slot = match result {
            Ok(facts) => FootprintCardStatus::Ready(facts),
            Err(why) => FootprintCardStatus::Failed(why),
        };
        Some(slot.clone())
    }
}

#[cfg(test)]
#[path = "footprint_card_tests.rs"]
mod tests;
