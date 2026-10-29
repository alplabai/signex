//! The net highlighted across every sheet of a project, and the member the
//! user last stepped to.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use signex_net::NetAnchor;
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use crate::app::state::ProjectId;

/// One highlighted net. It is named by what the user pointed at (`anchor`) and
/// the net's name, so a rebuilt index finds the same net again after an edit.
#[derive(Debug, Clone)]
pub struct NetHighlight {
    pub project: ProjectId,
    pub name: String,
    pub anchor: NetAnchor,
    /// The member last stepped to, as a position in the net's steps.
    pub cursor: Option<usize>,
    /// The elements of the net in each sheet placement: what stays bright
    /// there and nowhere else, since two placements of one file share uuids.
    pub elements: HashMap<(PathBuf, InstancePath), Arc<HashSet<Uuid>>>,
}

impl NetHighlight {
    /// Move the cursor one member forward or back over `len` members,
    /// wrapping at the ends; the first step back lands on the last member.
    pub fn step(&mut self, len: usize, forward: bool) -> Option<usize> {
        if len == 0 {
            self.cursor = None;
            return None;
        }
        let next = match (self.cursor.filter(|&c| c < len), forward) {
            (None, true) => 0,
            (None, false) => len - 1,
            (Some(c), true) => (c + 1) % len,
            (Some(c), false) => (c + len - 1) % len,
        };
        self.cursor = Some(next);
        Some(next)
    }

    /// What stays bright on the sheet at `path` shown through `placement`: its
    /// elements of the net in that placement, nothing at all when the net does
    /// not reach it (or the sheet is shown through none).
    pub fn bright_on(&self, path: &Path, placement: Option<&InstancePath>) -> Arc<HashSet<Uuid>> {
        let Some(placement) = placement else {
            return Arc::default();
        };
        self.elements
            .get(&(path.to_path_buf(), placement.clone()))
            .cloned()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn highlight() -> NetHighlight {
        NetHighlight {
            project: ProjectId::test_id(1),
            name: "VCC".into(),
            anchor: NetAnchor::new(
                signex_net::SheetKey::new("a.snxsch"),
                Uuid::nil(),
                InstancePath::root(),
            ),
            cursor: None,
            elements: HashMap::new(),
        }
    }

    #[test]
    fn stepping_forward_wraps_from_the_last_member_to_the_first() {
        let mut h = highlight();
        let seen: Vec<_> = (0..4).map(|_| h.step(3, true)).collect();
        assert_eq!(seen, [Some(0), Some(1), Some(2), Some(0)]);
    }

    #[test]
    fn the_first_step_back_lands_on_the_last_member_and_wraps() {
        let mut h = highlight();
        let seen: Vec<_> = (0..4).map(|_| h.step(3, false)).collect();
        assert_eq!(seen, [Some(2), Some(1), Some(0), Some(2)]);
    }

    #[test]
    fn a_cursor_past_a_shrunken_net_starts_over() {
        let mut h = highlight();
        h.cursor = Some(9);
        assert_eq!(h.step(3, true), Some(0));
        assert_eq!(h.step(0, true), None);
        assert_eq!(h.cursor, None);
    }

    #[test]
    fn a_sheet_the_net_does_not_reach_has_nothing_bright() {
        let mut h = highlight();
        let id = Uuid::from_u128(7);
        h.elements.insert(
            (PathBuf::from("a.snxsch"), InstancePath::root()),
            Arc::new(HashSet::from([id])),
        );
        let root = InstancePath::root();
        assert!(
            h.bright_on(Path::new("a.snxsch"), Some(&root))
                .contains(&id)
        );
        assert!(h.bright_on(Path::new("b.snxsch"), Some(&root)).is_empty());
        assert!(h.bright_on(Path::new("a.snxsch"), None).is_empty());
        let other = InstancePath::from_chain(vec![Uuid::from_u128(9)]);
        assert!(
            h.bright_on(Path::new("a.snxsch"), Some(&other)).is_empty(),
            "another placement of the same file shows nothing bright"
        );
    }
}
