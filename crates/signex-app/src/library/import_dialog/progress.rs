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

/// Phases the list keeps.
const MAX_PHASES: usize = 32;

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
            ProgressEvent::Phase { phase, total } => self.enter_phase(phase, *total),
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

    /// `phase` becomes the current one: a phase already listed moves to the
    /// end, started again, so a converter that repeats its phases does not
    /// grow the list.
    fn enter_phase(&mut self, phase: &str, total: Option<u64>) {
        self.phases.retain(|line| line.phase != phase);
        self.phases.push(PhaseLine {
            phase: phase.to_string(),
            done: 0,
            total,
        });
        // A converter may name any number of phases; only the latest are
        // shown, the oldest (all done) dropped from the front.
        let over = self.phases.len().saturating_sub(MAX_PHASES);
        self.phases.drain(..over);
    }

    /// The phase being worked on.
    pub fn current_phase(&self) -> Option<&PhaseLine> {
        self.phases.last()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phase(name: &str) -> ProgressEvent {
        ProgressEvent::Phase {
            phase: name.into(),
            total: Some(4),
        }
    }

    fn names(state: &ImportDialogState) -> Vec<&str> {
        state.phases.iter().map(|l| l.phase.as_str()).collect()
    }

    #[test]
    fn a_repeated_phase_becomes_current_instead_of_a_new_line() {
        let mut state = ImportDialogState::default();
        state.record_progress(&phase("read"));
        state.record_progress(&ProgressEvent::Progress {
            done: 3,
            total: None,
            item: None,
        });
        state.record_progress(&phase("write"));
        state.record_progress(&phase("read"));
        assert_eq!(names(&state), ["write", "read"]);
        let current = state.current_phase().unwrap();
        assert_eq!((current.phase.as_str(), current.done), ("read", 0));
    }

    #[test]
    fn the_phase_list_keeps_only_the_latest() {
        let mut state = ImportDialogState::default();
        for i in 0..MAX_PHASES + 8 {
            state.record_progress(&phase(&format!("p{i}")));
        }
        assert_eq!(state.phases.len(), MAX_PHASES);
        assert_eq!(state.phases[0].phase, "p8", "the oldest are dropped");
        let last = format!("p{}", MAX_PHASES + 7);
        assert_eq!(state.current_phase().unwrap().phase, last);
    }
}
