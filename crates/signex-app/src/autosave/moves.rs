//! Sidecar moves a document rename owes, and the bookkeeping around them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;

use super::AutosaveState;

/// Failed renames of one recovery copy after which autosave may write a
/// fresh copy to the new key instead of waiting for the rename (#1433).
pub const FAILED_MOVES_BEFORE_FRESH_COPY: u32 = 2;

/// One sidecar rename owed after a document moved from `old` to `new`.
#[derive(Debug, Clone, Serialize)]
pub struct SidecarMove {
    pub old: PathBuf,
    pub new: PathBuf,
    /// The copy is one this run wrote. Only such a copy is ever dropped
    /// (its document saved or closed since); any other may be an earlier
    /// run's crash copy and is always moved.
    pub this_run: bool,
    /// `new` is already held as "removing" by this move, so no autosave
    /// write lands there before the copy does.
    pub(super) held: bool,
    /// Renames that failed so far. At [`FAILED_MOVES_BEFORE_FRESH_COPY`] the
    /// hold on `new` is given up so a fresh copy can be written there.
    #[serde(skip)]
    pub(super) attempts: u32,
    /// The hold on `new` was given up after repeated failures: autosave
    /// writes the current content to `new` while this move stays owed, only
    /// to bring the old copy over (newest wins) or remove it.
    #[serde(skip)]
    pub(super) released: bool,
    /// Every target this move was journalled or repointed to, so dropping it
    /// clears only an entry that still names one of them.
    #[serde(skip)]
    pub(super) targets: Vec<PathBuf>,
    /// Set by a discarding quit, or a save of the new path, while the rename
    /// runs: the task removes the copy it just moved.
    #[serde(skip)]
    pub(super) discard: Arc<AtomicBool>,
}

impl SidecarMove {
    /// The hold on the new key was given up after repeated failures.
    #[cfg(test)]
    pub fn is_released(&self) -> bool {
        self.released
    }

    /// A discarding quit, or a save of the new path, arrived while the
    /// rename ran.
    pub fn is_discarded(&self) -> bool {
        self.discard.load(Ordering::SeqCst)
    }
}

impl AutosaveState {
    /// The document at `old` now lives at `new`: its bookkeeping follows and
    /// its sidecar is queued to move ([`Self::take_moves`]). Without it a
    /// crash before the next tick leaves the recovery copy under the old
    /// path, where nothing looks for it.
    pub fn rekey(&mut self, old: &Path, new: &Path) {
        // A write running for `old` lands there; it is recorded under `new`
        // and its copy moves once it has landed ([`Self::take_moves`]).
        for target in self.forwarded.values_mut().filter(|p| p.as_path() == old) {
            *target = new.to_path_buf();
        }
        if self.in_flight.contains_key(old) {
            self.forwarded.insert(old.to_path_buf(), new.to_path_buf());
        }
        let mut restoring = false;
        for current in self.restoring.values_mut().filter(|p| p.as_path() == old) {
            *current = new.to_path_buf();
            restoring = true;
        }
        // Decided positively: only a copy this run wrote may later be
        // dropped. A copy of a sheet that is not open, or one offered, kept
        // or being restored, is an earlier run's and always moves.
        let this_run = matches!(self.written.get(old), Some(Some(_)))
            && !restoring
            && !self.deferred.contains(old)
            && !self.offers.iter().any(|o| o.path == old);
        // A Restore reading the copy left the offer; until it reports, the
        // copy is kept like "Decide later" so nothing writes over it.
        if restoring {
            self.deferred.insert(new.to_path_buf());
        }
        // A move still owed to `old` now goes straight to `new`; the copy
        // is at its source, not at `old`. A released move may also have a
        // fresh copy written at `old`: that one needs its own move, so only
        // a held move makes a new one unnecessary.
        let mut repointed = false;
        for owed in self.moves.iter_mut().filter(|m| m.new == old) {
            owed.new = new.to_path_buf();
            owed.targets.push(new.to_path_buf());
            owed.this_run &= this_run;
            repointed |= !owed.released;
            if owed.held {
                Self::release(&mut self.removing, old);
                *self.removing.entry(new.to_path_buf()).or_insert(0) += 1;
            }
        }
        // Renamed back to where the copy sits: nothing left to move.
        let (removing, journal_clears) = (&mut self.removing, &mut self.journal_clears);
        self.moves.retain(|m| {
            let back_home = m.old == m.new;
            if back_home && m.held {
                Self::release(removing, &m.new);
            }
            if back_home {
                journal_clears.push((m.old.clone(), m.targets.clone()));
            }
            !back_home
        });
        if let Some(stamp) = self.written.remove(old) {
            self.written.insert(new.to_path_buf(), stamp);
        }
        if self.deferred.remove(old) {
            self.deferred.insert(new.to_path_buf());
        }
        if self.due_write.remove(old) {
            self.due_write.insert(new.to_path_buf());
        }
        for offer in self.offers.iter_mut().filter(|o| o.path == old) {
            offer.path = new.to_path_buf();
        }
        if !repointed {
            // Held from now on, so no fresh copy lands at `new` and is then
            // overwritten by the older one this move brings.
            self.begin_remove(new);
            self.moves.push(SidecarMove {
                old: old.to_path_buf(),
                new: new.to_path_buf(),
                this_run,
                held: true,
                attempts: 0,
                released: false,
                targets: vec![new.to_path_buf()],
                discard: Arc::default(),
            });
        }
    }

    pub(super) fn release(removing: &mut HashMap<PathBuf, u32>, path: &Path) {
        if let Some(count) = removing.get_mut(path) {
            *count -= 1;
            if *count == 0 {
                removing.remove(path);
            }
        }
    }

    /// A copy of this run for a document saved or closed since: nothing to
    /// recover, so it is removed instead of moved.
    fn is_obsolete(&self, owed: &SidecarMove) -> bool {
        owed.this_run && !self.tracks(&owed.new)
    }

    /// A fresh copy of `path` landed while a released move of this run's
    /// older copy into `path` is still owed (and not running): the older
    /// copy holds nothing the fresh one lacks, so the move is settled.
    /// Returns its old key to remove, once the fresh copy is safely on disk.
    /// `None` when that key is in use again (a document was renamed onto
    /// it): what sits there now is not this move's copy to delete, and the
    /// move is dropped so it never carries that copy over to `path` either.
    pub fn take_obsolete_released(&mut self, path: &Path) -> Option<PathBuf> {
        let i = self.moves.iter().position(|m| {
            m.released
                && m.this_run
                && m.new == path
                && !self
                    .running_moves
                    .iter()
                    .any(|r| r.old == m.old && r.new == m.new)
        })?;
        let settled = self.moves.remove(i);
        if settled.old != settled.new && !self.key_in_use(&settled.old) {
            return Some(settled.old);
        }
        self.journal_clears.push((settled.old, settled.targets));
        None
    }

    /// Drop each released move of this run whose old key a document took
    /// over (renamed onto it): the copy there is that document's now, and
    /// moving it would carry it to the wrong document. The released move's
    /// own document gets a fresh copy at `new` on the next tick while it
    /// holds unsaved edits: releasing the move marked `new` as due a write
    /// ([`Self::move_failed`]). Whatever sits at the old key is left there,
    /// never removed: if that document had no copy of its own and never
    /// writes one, the stale copy stays at its key and may be offered when
    /// it is next opened.
    fn drop_released_from_reused_keys(&mut self) {
        let reused: Vec<bool> = self
            .moves
            .iter()
            .map(|m| m.released && m.this_run && self.key_in_use(&m.old))
            .collect();
        let mut reused = reused.into_iter();
        let journal_clears = &mut self.journal_clears;
        self.moves.retain(|m| {
            let dropped = reused.next().unwrap_or(false);
            if dropped {
                journal_clears.push((m.old.clone(), m.targets.clone()));
            }
            !dropped
        });
    }

    /// Whether `path`'s key belongs to a document again: a copy tracked,
    /// offered, kept, checked or restored there, or a write, removal or
    /// rename into it owed or running.
    fn key_in_use(&self, path: &Path) -> bool {
        self.tracks(path)
            || self.in_flight.contains_key(path)
            || self.removing.contains_key(path)
            || self.checking.contains_key(path)
            || self.restoring.values().any(|p| p == path)
            || self.moves.iter().any(|m| m.new == path)
            || self.running_moves.iter().any(|m| m.new == path)
    }

    /// The sources whose journal entries must be cleared now.
    pub fn take_journal_clears(&mut self) -> Vec<(PathBuf, Vec<PathBuf>)> {
        std::mem::take(&mut self.journal_clears)
    }

    /// Whether a sidecar move is owed or running: the quit and the timer
    /// keep running them while autosave itself is off.
    pub fn has_owed_moves(&self) -> bool {
        !self.moves.is_empty()
    }

    /// The rename `moved` finished, either way: it no longer runs.
    fn end_running(&mut self, moved: &SidecarMove) {
        if let Some(i) = self
            .running_moves
            .iter()
            .position(|m| m.old == moved.old && m.new == moved.new)
        {
            self.running_moves.remove(i);
        }
    }

    /// The rename `moved` landed: release its hold on the new key.
    pub fn move_landed(&mut self, moved: &SidecarMove) {
        self.end_running(moved);
        if moved.held {
            Self::release(&mut self.removing, &moved.new);
        }
    }

    /// The rename of `failed.old`'s sidecar to `failed.new` failed (a file
    /// lock, say): the copy is still under the old key. It is retried on
    /// the next tick, with `new` held so no write lands there first until
    /// [`FAILED_MOVES_BEFORE_FRESH_COPY`] failures release it. Returns the
    /// old key to remove instead when the copy is obsolete.
    pub fn move_failed(&mut self, mut failed: SidecarMove) -> Option<PathBuf> {
        self.end_running(&failed);
        failed.attempts += 1;
        failed.released |= failed.attempts >= FAILED_MOVES_BEFORE_FRESH_COPY;
        // A move that keeps failing must not freeze recovery: give up the
        // hold on `new` so the next write puts the current content there.
        // The old copy stays owed a move, which brings it over only if it is
        // newer, and is never deleted.
        let was_held = failed.held;
        failed.held = !failed.released;
        if failed.released && was_held {
            Self::release(&mut self.removing, &failed.new);
        }
        // The document moved on again while this rename ran: go straight
        // to its newest path. A released move leaves the onward move alone:
        // a fresh copy written at `new` needs it.
        if let Some(i) = self.moves.iter().position(|m| m.old == failed.new) {
            if failed.released {
                let onward = &self.moves[i];
                failed.targets.extend(onward.targets.iter().cloned());
                failed.new = onward.new.clone();
                failed.this_run &= onward.this_run;
            } else {
                let onward = self.moves.remove(i);
                Self::release(&mut self.removing, &failed.new);
                failed.targets.extend(onward.targets);
                failed.new = onward.new;
                failed.this_run &= onward.this_run;
                failed.held = onward.held;
            }
        }
        // Renamed back to where the copy sits while it ran: nothing left to
        // move, and a kept `old -> old` move would later remove that copy.
        if failed.old == failed.new {
            if failed.held {
                Self::release(&mut self.removing, &failed.new);
            }
            self.journal_clears.push((failed.old, failed.targets));
            return None;
        }
        if self.is_obsolete(&failed) {
            if failed.held {
                Self::release(&mut self.removing, &failed.new);
            }
            return Some(failed.old);
        }
        if failed.released {
            self.mark_due_a_write(&failed.new);
        }
        self.moves.push(failed);
        None
    }

    /// The copy `written` records for `path` may still sit under an older
    /// key (its move was released, not landed): the next tick writes the
    /// current content at `path` even if the document has not changed
    /// since. Its stamp stays, so it still counts as this run's copy.
    fn mark_due_a_write(&mut self, path: &Path) {
        if self.written.contains_key(path) {
            self.due_write.insert(path.to_path_buf());
        }
    }

    /// The renames to start now, each with its new path held, and the old
    /// keys of obsolete copies to remove instead. A rename whose source is
    /// itself the target of a rename still running waits for it, and so does
    /// one whose target's copy is still being moved out.
    pub fn take_moves(&mut self) -> (Vec<SidecarMove>, Vec<PathBuf>) {
        self.drop_released_from_reused_keys();
        let (mut start, mut obsolete, mut wait) = (Vec::new(), Vec::new(), Vec::new());
        for mut owed in std::mem::take(&mut self.moves) {
            // The copy is still being written or moved into `old`, or the
            // copy at `new` is still being moved out.
            if self.removing.contains_key(&owed.old)
                || self.in_flight.contains_key(&owed.old)
                || self.running_moves.iter().any(|r| r.new == owed.old)
                || self.running_moves.iter().any(|r| r.old == owed.new)
            {
                if !owed.released && !std::mem::replace(&mut owed.held, true) {
                    self.begin_remove(&owed.new);
                }
                wait.push(owed);
            } else if self.is_obsolete(&owed) {
                if owed.held {
                    Self::release(&mut self.removing, &owed.new);
                }
                obsolete.push(owed.old);
            } else {
                start.push(owed);
            }
        }
        self.moves = wait;
        for owed in &mut start {
            if !owed.released && !std::mem::replace(&mut owed.held, true) {
                self.begin_remove(&owed.new);
            }
        }
        self.running_moves.extend(start.iter().cloned());
        (start, obsolete)
    }

    /// Whether `path`'s sidecar is not where it belongs yet: a rename into
    /// it is owed or running (or a removal of it runs). Reading it now
    /// would miss the copy.
    pub fn move_owed(&self, path: &Path) -> bool {
        self.removing.contains_key(path)
            || self.moves.iter().any(|m| m.new == path)
            || self.running_moves.iter().any(|m| m.new == path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autosave::DocStamp;

    fn tracked_at(paths: &[&Path]) -> AutosaveState {
        let mut state = AutosaveState::default();
        for path in paths {
            state.begin(path);
            state.finish(path, DocStamp::of_bytes(b"x"), true);
        }
        state
    }

    /// Fail the one owed move until its hold on the new key is given up.
    fn released(state: &mut AutosaveState) {
        for _ in 0..FAILED_MOVES_BEFORE_FRESH_COPY {
            let (mut start, obsolete) = state.take_moves();
            assert!(obsolete.is_empty());
            assert!(state.move_failed(start.remove(0)).is_none(), "kept owed");
        }
        assert!(state.moves.iter().all(SidecarMove::is_released));
    }

    fn fresh_copy_lands(state: &mut AutosaveState, path: &Path) {
        state.begin(path);
        state.finish(path, DocStamp::of_bytes(b"fresh"), true);
    }

    #[test]
    fn a_released_move_never_removes_a_copy_renamed_onto_its_old_key() {
        let (a, b, t) = (Path::new("a"), Path::new("b"), Path::new("t"));
        let mut state = tracked_at(&[a, b]);
        state.rekey(a, t);
        released(&mut state);
        // Document b is renamed to a: its copy is owed to key(a). t's fresh
        // copy lands before the next tick.
        state.rekey(b, a);
        fresh_copy_lands(&mut state, t);
        assert_eq!(
            state.take_obsolete_released(t),
            None,
            "key(a) is b's only copy"
        );
        assert!(state.written.contains_key(a), "b's copy stays tracked");
        assert_eq!(state.take_journal_clears(), [(a.into(), vec![t.into()])]);
        let (start, obsolete) = state.take_moves();
        assert!(obsolete.is_empty());
        let started: Vec<_> = start
            .iter()
            .map(|m| (m.old.clone(), m.new.clone()))
            .collect();
        assert_eq!(started, [(b.into(), a.into())], "a -> t is settled");
        state.move_landed(&start[0]);
        assert!(!state.has_owed_moves());
    }

    #[test]
    fn a_dropped_released_move_leaves_its_document_due_a_fresh_copy() {
        let (a, b, t) = (Path::new("a"), Path::new("b"), Path::new("t"));
        let mut state = tracked_at(&[a, b]);
        state.rekey(a, t);
        released(&mut state);
        state.rekey(b, a);
        let _ = state.take_moves(); // a -> t is dropped here
        assert!(
            !state.moves.iter().any(|m| m.new == t),
            "a -> t was dropped"
        );
        assert!(
            state.needs_write(t, DocStamp::of_bytes(b"x")),
            "t's only copy was left at key(a): the next tick writes t again"
        );
    }

    #[test]
    fn a_released_move_renamed_on_is_still_dropped_when_its_old_key_is_reused() {
        let (a, b, t, x) = (
            Path::new("a"),
            Path::new("b"),
            Path::new("t"),
            Path::new("x"),
        );
        let mut state = tracked_at(&[a, b]);
        state.rekey(a, t);
        released(&mut state);
        state.rekey(t, x);
        state.rekey(b, a);
        let (start, _) = state.take_moves();
        assert!(
            !state.moves.iter().any(|m| m.old == a),
            "b's copy at a is never carried to x"
        );
        assert!(!state.running_moves.iter().any(|m| m.old == a));
        // t -> x holds x until it lands; then x is due a fresh copy.
        for moved in &start {
            state.move_landed(moved);
        }
        assert!(
            state.needs_write(x, DocStamp::of_bytes(b"x")),
            "x is due a copy"
        );
    }

    #[test]
    fn a_released_move_renamed_on_then_saved_is_removed_not_moved() {
        let (a, t, x) = (Path::new("a"), Path::new("t"), Path::new("x"));
        let mut state = tracked_at(&[a]);
        state.rekey(a, t);
        released(&mut state);
        state.rekey(t, x);
        state.saved(x);
        let (start, obsolete) = state.take_moves();
        assert!(obsolete.contains(&a.to_path_buf()), "a's copy is obsolete");
        assert!(!start.iter().any(|m| m.old == a), "a -> x is not started");
    }

    #[test]
    fn a_move_into_a_key_whose_copy_is_moving_out_waits_for_it() {
        let (a, b, c) = (Path::new("a"), Path::new("b"), Path::new("c"));
        let mut state = tracked_at(&[a, b]);
        state.rekey(b, c);
        let (start, _) = state.take_moves();
        let running = start[0].clone();
        state.rekey(a, b);
        let (start, obsolete) = state.take_moves();
        assert!(obsolete.is_empty());
        assert!(start.is_empty(), "a -> b waits while b -> c runs");
        state.move_landed(&running);
        let (start, _) = state.take_moves();
        let started: Vec<_> = start
            .iter()
            .map(|m| (m.old.clone(), m.new.clone()))
            .collect();
        assert_eq!(started, [(a.into(), b.into())]);
    }

    #[test]
    fn a_tick_after_a_rename_onto_the_old_key_never_moves_that_copy_away() {
        let (a, b, t) = (Path::new("a"), Path::new("b"), Path::new("t"));
        let mut state = tracked_at(&[a, b]);
        state.rekey(a, t);
        released(&mut state);
        // Document b is renamed to a: its copy lands at key(a).
        state.rekey(b, a);
        let (start, obsolete) = state.take_moves();
        assert!(obsolete.is_empty());
        state.move_landed(&start[0]);
        // A tick before t's fresh copy lands.
        let (start, obsolete) = state.take_moves();
        assert!(obsolete.is_empty(), "b's copy at a is never removed");
        let started: Vec<_> = start
            .iter()
            .map(|m| (m.old.clone(), m.new.clone()))
            .collect();
        assert!(started.is_empty(), "b's copy is not carried to t");
        assert!(!state.has_owed_moves());
        assert!(state.written.contains_key(a), "b's copy stays tracked");
        assert!(
            state
                .take_journal_clears()
                .contains(&(a.into(), vec![t.into()]))
        );
    }

    #[test]
    fn a_released_move_renamed_back_while_it_runs_is_dropped() {
        let (a, t) = (Path::new("a"), Path::new("t"));
        let mut state = tracked_at(&[a]);
        state.rekey(a, t);
        released(&mut state);
        let (mut start, _) = state.take_moves();
        let running = start.remove(0);
        state.rekey(t, a);
        // The re-pointed retry would be a -> a: nothing left to move.
        assert!(state.move_failed(running).is_none());
        assert!(
            state.moves.iter().all(|m| m.old != m.new),
            "no a -> a move is kept"
        );
        assert!(
            state
                .take_journal_clears()
                .contains(&(a.into(), vec![t.into(), a.into()]))
        );
        let (start, obsolete) = state.take_moves();
        assert!(obsolete.is_empty());
        assert_eq!(start.len(), 1, "only t -> a runs");
        state.move_landed(&start[0]);
        fresh_copy_lands(&mut state, a);
        assert_eq!(
            state.take_obsolete_released(a),
            None,
            "the fresh copy at a is never removed"
        );
        assert!(state.written.contains_key(a));
        assert!(!state.has_owed_moves());
    }

    #[test]
    fn renaming_back_after_the_move_was_released_keeps_the_fresh_copy() {
        let (a, b) = (Path::new("a"), Path::new("b"));
        let mut state = tracked_at(&[a]);
        state.rekey(a, b);
        released(&mut state);
        state.rekey(b, a);
        let (start, obsolete) = state.take_moves();
        assert!(obsolete.is_empty());
        let started: Vec<_> = start
            .iter()
            .map(|m| (m.old.clone(), m.new.clone()))
            .collect();
        assert_eq!(started, [(b.into(), a.into())], "only b's copy moves home");
        state.move_landed(&start[0]);
        fresh_copy_lands(&mut state, a);
        assert_eq!(state.take_obsolete_released(a), None);
        assert!(state.written.contains_key(a));
        assert!(!state.has_owed_moves());
        assert!(state.removing.is_empty(), "no hold left behind");
    }
}
