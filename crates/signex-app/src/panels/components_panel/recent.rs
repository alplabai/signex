//! The parts placed from the Components panel this session (#2535),
//! most recent first. The table redesign (mockup row 52) no longer draws
//! the list; the log stays session-scoped and nothing is written to disk.

use signex_library::ComponentRow;

use crate::library::state::{ComponentsPanelRowKey, LibraryState};

/// Most parts the list keeps.
pub const MAX_RECENT: usize = 5;

/// One recently placed part and how often it was placed this session.
#[derive(Debug, Clone, PartialEq)]
pub struct RecentPart {
    pub key: ComponentsPanelRowKey,
    pub uses: u32,
}

/// The recently placed parts, most recent first, at most [`MAX_RECENT`].
#[derive(Debug, Clone, Default)]
pub struct RecentlyPlaced {
    parts: Vec<RecentPart>,
}

impl RecentlyPlaced {
    /// `key` was placed: it moves to the front with one more use.
    pub fn record(&mut self, key: ComponentsPanelRowKey) {
        let part = match self.parts.iter().position(|p| p.key == key) {
            Some(at) => {
                let mut part = self.parts.remove(at);
                part.uses = part.uses.saturating_add(1);
                part
            }
            None => RecentPart { key, uses: 1 },
        };
        self.parts.insert(0, part);
        self.parts.truncate(MAX_RECENT);
    }

    /// The parts, most recent first.
    pub fn parts(&self) -> &[RecentPart] {
        &self.parts
    }
}

/// The in-memory row `key` names.
pub(super) fn row_of<'a>(
    state: &'a LibraryState,
    key: &ComponentsPanelRowKey,
) -> Option<&'a ComponentRow> {
    state
        .library_at(&key.library_path)?
        .tables
        .get(&key.table)?
        .iter()
        .find(|r| r.row_id == key.row_id.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(n: u128) -> ComponentsPanelRowKey {
        ComponentsPanelRowKey {
            library_path: "lib.snxlib".into(),
            table: "t".into(),
            row_id: signex_library::RowId::from_uuid(uuid::Uuid::from_u128(n)),
        }
    }

    #[test]
    fn recent_parts_are_most_recent_first_capped_and_counted() {
        let mut recent = RecentlyPlaced::default();
        for n in 1..=6 {
            recent.record(key(n));
        }
        let order: Vec<_> = recent.parts().iter().map(|p| p.key.clone()).collect();
        assert_eq!(
            order,
            [key(6), key(5), key(4), key(3), key(2)],
            "at most five"
        );

        recent.record(key(4));
        recent.record(key(4));
        let first = &recent.parts()[0];
        assert_eq!(first.key, key(4), "placing again moves it to the front");
        assert_eq!(first.uses, 3);
        assert_eq!(recent.parts().len(), MAX_RECENT);
        assert!(recent.parts()[1..].iter().all(|p| p.uses == 1));
    }
}
