//! Step 3, import: the phases the progress stream reported, the bar, and
//! when a pending Cancel takes effect. Pure; the handler feeds it each
//! event with the one before it.
//!
//! Cancel stops after the current item (decided 2026-11-26): `item` names
//! the source item being worked on, and a converter repeats its last
//! `progress` while one item takes long (contract §3.1). So an item is in
//! flight while the last event is a `progress` naming one, and it is done
//! when an event that differs from it arrives.

use super::ImportDialogState;
use crate::library_import::ProgressEvent;

/// One phase as the list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseLine {
    /// The converter's phase name (`read`, `convert`, `write`, `models`).
    pub phase: String,
    pub done: u64,
    pub total: Option<u64>,
}

impl PhaseLine {
    /// Whole percent done, when the total is known.
    pub fn percent(&self) -> Option<u64> {
        let total = self.total.filter(|&t| t > 0)?;
        Some((self.done.min(total) * 100) / total)
    }
}

/// Whether the last event says an item is being worked on.
pub fn item_in_flight(last: Option<&ProgressEvent>) -> bool {
    matches!(last, Some(ProgressEvent::Progress { item: Some(_), .. }))
}

/// Whether `next` ends the item `previous` was working on: any phase or
/// progress event other than a repeat of it.
pub fn ends_item(previous: Option<&ProgressEvent>, next: &ProgressEvent) -> bool {
    !matches!(next, ProgressEvent::Other(_)) && previous != Some(next)
}

impl ImportDialogState {
    /// Fold `event` into the phase list.
    pub fn record_progress(&mut self, event: &ProgressEvent) {
        match event {
            ProgressEvent::Phase { phase, total } => self.phases.push(PhaseLine {
                phase: phase.clone(),
                done: 0,
                total: *total,
            }),
            ProgressEvent::Progress { done, total, .. } => match self.phases.last_mut() {
                Some(line) => {
                    line.done = *done;
                    if total.is_some() {
                        line.total = *total;
                    }
                }
                None => self.phases.push(PhaseLine {
                    phase: String::new(),
                    done: *done,
                    total: *total,
                }),
            },
            ProgressEvent::Other(_) => {}
        }
    }

    /// The phase being worked on.
    pub fn current_phase(&self) -> Option<&PhaseLine> {
        self.phases.last()
    }
}
