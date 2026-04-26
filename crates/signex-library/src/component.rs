use serde::{Deserialize, Serialize};

use crate::embed::{PcbSide, SchematicSide, SharedSide};
use crate::identity::{ComponentId, InternalPn, Version};
use crate::lifecycle::LifecycleState;

/// One commit's worth of a component — see LIBRARY_PLAN §4.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Revision {
    pub version: Version,
    pub state: LifecycleState,
    pub created: chrono::DateTime<chrono::Utc>,
    pub author: String,
    pub message: String,
    pub schematic: SchematicSide,
    pub pcb: PcbSide,
    pub shared: SharedSide,
    /// SHA-256 of canonicalised JSON over (schematic, pcb, shared).
    pub content_hash: [u8; 32],
}

impl Revision {
    /// Recompute and store the content hash from current side data.
    pub fn refresh_content_hash(&mut self) {
        self.content_hash =
            crate::hash::hash_revision_content(&self.schematic, &self.pcb, &self.shared);
    }
}

/// A logical component with N revisions sorted by version.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Component {
    pub uuid: ComponentId,
    pub internal_pn: InternalPn,
    pub revisions: Vec<Revision>,
    /// The "Released" tip — typically the highest Released revision.
    pub head: Version,
}

impl Component {
    /// Find the head revision. Returns `None` if `head` doesn't reference a known revision.
    pub fn head_revision(&self) -> Option<&Revision> {
        self.revisions.iter().find(|r| r.version == self.head)
    }

    /// Highest Released revision, or `None` if no Released revision exists.
    pub fn highest_released(&self) -> Option<&Revision> {
        self.revisions
            .iter()
            .filter(|r| r.state == LifecycleState::Released)
            .max_by_key(|r| r.version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn fixture_revision(version: Version, state: LifecycleState) -> Revision {
        Revision {
            version,
            state,
            created: chrono::Utc::now(),
            author: "test@example.com".into(),
            message: "initial".into(),
            schematic: SchematicSide::default(),
            pcb: PcbSide::default(),
            shared: SharedSide {
                mpn: "TEST-001".into(),
                manufacturer: "Acme".into(),
                description: "test part".into(),
                ..Default::default()
            },
            content_hash: [0u8; 32],
        }
    }

    #[test]
    fn head_revision_resolves() {
        let c = Component {
            uuid: Uuid::now_v7(),
            internal_pn: InternalPn::new("R_TEST"),
            revisions: vec![
                fixture_revision(Version::new(1, 0), LifecycleState::Released),
                fixture_revision(Version::new(1, 1), LifecycleState::Released),
            ],
            head: Version::new(1, 1),
        };
        assert_eq!(c.head_revision().unwrap().version, Version::new(1, 1));
    }

    #[test]
    fn highest_released_skips_drafts() {
        let c = Component {
            uuid: Uuid::now_v7(),
            internal_pn: InternalPn::new("R_TEST"),
            revisions: vec![
                fixture_revision(Version::new(1, 0), LifecycleState::Released),
                fixture_revision(Version::new(1, 1), LifecycleState::Draft),
            ],
            head: Version::new(1, 0),
        };
        assert_eq!(
            c.highest_released().unwrap().version,
            Version::new(1, 0)
        );
    }

    #[test]
    fn component_round_trip() {
        let c = Component {
            uuid: Uuid::now_v7(),
            internal_pn: InternalPn::new("R0805_10k"),
            revisions: vec![fixture_revision(Version::new(1, 0), LifecycleState::Released)],
            head: Version::new(1, 0),
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: Component = serde_json::from_str(&json).unwrap();
        assert_eq!(c, back);
    }

    #[test]
    fn refresh_content_hash_populates() {
        let mut rev = fixture_revision(Version::new(1, 0), LifecycleState::Released);
        assert_eq!(rev.content_hash, [0u8; 32]);
        rev.refresh_content_hash();
        assert_ne!(rev.content_hash, [0u8; 32]);
    }
}
