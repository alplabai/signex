//! What is typed in the Requirements section, and the last refusal, follow
//! their entry, not its place in the list (#2547): an add, a remove, a
//! renamed id, an undo or a redo of another entry leaves them where they are.

use std::cmp::Ordering;
use std::collections::HashMap;

use signex_types::project::RequirementEntry;

use super::RequirementsTabState;

/// The entry a draft or a refusal is for: its id and, among entries sharing
/// that id (the loader keeps them, after a hand edit or a merge), how many
/// come before it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntryKey {
    pub id: String,
    pub occurrence: usize,
}

impl EntryKey {
    /// The first (usually the only) entry with `id`.
    pub fn first(id: &str) -> Self {
        Self {
            id: id.to_string(),
            occurrence: 0,
        }
    }
}

/// The key of each entry of `entries`, by place; `None` for one without an
/// id.
pub(super) fn keys_of(entries: &[RequirementEntry]) -> Vec<Option<EntryKey>> {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    entries
        .iter()
        .map(|entry| {
            let id = entry.id()?;
            let count = seen.entry(id).or_default();
            let key = EntryKey {
                id: id.to_string(),
                occurrence: *count,
            };
            *count += 1;
            Some(key)
        })
        .collect()
}

/// The key of the entry at `index` of `entries`.
pub(super) fn key_at(entries: &[RequirementEntry], index: usize) -> Option<EntryKey> {
    keys_of(entries).get(index).cloned().flatten()
}

/// Where each entry of `before` is in `after`. One edit or history step adds,
/// removes or changes one entry; `removed` is the place an edit removed,
/// otherwise an insertion or removal is where the lists first differ.
fn places(
    before: &[RequirementEntry],
    after: &[RequirementEntry],
    removed: Option<usize>,
) -> Vec<Option<usize>> {
    let first_difference = || {
        before
            .iter()
            .zip(after)
            .position(|(was, now)| was != now)
            .unwrap_or(before.len().min(after.len()))
    };
    let all = 0..before.len();
    if after.len() == before.len() {
        all.map(Some).collect()
    } else if after.len() == before.len() + 1 {
        let at = first_difference();
        all.map(|i| Some(if i < at { i } else { i + 1 })).collect()
    } else if after.len() + 1 == before.len() {
        let at = removed.unwrap_or_else(first_difference);
        all.map(|i| match i.cmp(&at) {
            Ordering::Less => Some(i),
            Ordering::Equal => None,
            Ordering::Greater => Some(i - 1),
        })
        .collect()
    } else {
        vec![None; before.len()]
    }
}

impl RequirementsTabState {
    /// The place in `entries` of the entry the last refusal is shown at.
    pub(super) fn problem_shown_at(&self, entries: &[RequirementEntry]) -> Option<usize> {
        let (Some(key), _) = self.problem.as_ref()? else {
            return None;
        };
        keys_of(entries)
            .iter()
            .position(|place| place.as_ref() == Some(key))
    }

    /// An edit moved the list from `before` to `after`: every entry's drafts,
    /// refusal and card flip follow it, a renamed one's included; a removed
    /// entry's go with it.
    pub(super) fn follow_edit(
        &mut self,
        before: &[RequirementEntry],
        after: &[RequirementEntry],
        removed: Option<usize>,
    ) {
        self.follow(before, after, removed, false);
    }

    /// An undo or redo moved the list from `before` to `after`: the boxes of
    /// an entry it changed, added or removed show the stored values again;
    /// every other entry keeps its drafts.
    pub(in crate::app::project_tab) fn follow_step(
        &mut self,
        before: &[RequirementEntry],
        after: &[RequirementEntry],
    ) {
        self.follow(before, after, None, true);
        self.problem = None;
        self.problem_field = None;
    }

    fn follow(
        &mut self,
        before: &[RequirementEntry],
        after: &[RequirementEntry],
        removed: Option<usize>,
        only_unchanged: bool,
    ) {
        let (old_keys, new_keys) = (keys_of(before), keys_of(after));
        let places = places(before, after, removed);
        let moved = |key: &EntryKey| {
            let from = old_keys
                .iter()
                .position(|place| place.as_ref() == Some(key))?;
            let to = places[from]?;
            if only_unchanged && before[from] != after[to] {
                return None;
            }
            new_keys[to].clone()
        };
        self.drafts = std::mem::take(&mut self.drafts)
            .into_iter()
            .filter_map(|((key, field), text)| Some(((moved(&key)?, field), text)))
            .collect();
        self.toggled = std::mem::take(&mut self.toggled)
            .iter()
            .filter_map(moved)
            .collect();
        self.problem = self.problem.take().and_then(|(at, problem)| match at {
            None => Some((None, problem)),
            Some(key) => moved(&key).map(|key| (Some(key), problem)),
        });
        if self.problem.is_none() {
            self.problem_field = None;
        }
    }
}
