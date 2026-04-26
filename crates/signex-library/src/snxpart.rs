//! `.snxpart` is one Revision serialised as JSON, plus the parent component's
//! `uuid` + `internal_pn`. Layout matches LIBRARY_PLAN §6 — `<uuid>-<version>.snxpart`.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::component::Revision;
use crate::identity::{ComponentId, InternalPn};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SnxPartFile {
    /// Format version — bump when `.snxpart` schema changes.
    pub schema: u32,
    pub uuid: ComponentId,
    pub internal_pn: InternalPn,
    pub revision: Revision,
}

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum SnxPartError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported schema {found}, expected {expected}")]
    UnsupportedSchema { found: u32, expected: u32 },
}

/// Write a `.snxpart` file as pretty-printed JSON.
pub fn write_snxpart(path: &Path, file: &SnxPartFile) -> Result<(), SnxPartError> {
    let bytes = serde_json::to_vec_pretty(file)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Read a `.snxpart` file, validating schema version.
pub fn read_snxpart(path: &Path) -> Result<SnxPartFile, SnxPartError> {
    let bytes = std::fs::read(path)?;
    let file: SnxPartFile = serde_json::from_slice(&bytes)?;
    if file.schema != SCHEMA_VERSION {
        return Err(SnxPartError::UnsupportedSchema {
            found: file.schema,
            expected: SCHEMA_VERSION,
        });
    }
    Ok(file)
}

/// Filename convention `<uuid>-<major>.<minor>.snxpart`.
pub fn snxpart_filename(uuid: ComponentId, version: crate::identity::Version) -> String {
    format!("{}-{}.snxpart", uuid, version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::Revision;
    use crate::embed::{PcbSide, SchematicSide, SharedSide};
    use crate::identity::{InternalPn, Version};
    use crate::lifecycle::LifecycleState;
    use uuid::Uuid;

    fn fixture_file() -> SnxPartFile {
        let mut rev = Revision {
            version: Version::new(1, 2),
            state: LifecycleState::Released,
            created: chrono::Utc::now(),
            author: "test@example.com".into(),
            message: "initial".into(),
            schematic: SchematicSide::default(),
            pcb: PcbSide::default(),
            shared: SharedSide {
                mpn: "TEST-001".into(),
                manufacturer: "Acme".into(),
                description: "test".into(),
                ..Default::default()
            },
            content_hash: [0u8; 32],
        };
        rev.refresh_content_hash();
        SnxPartFile {
            schema: SCHEMA_VERSION,
            uuid: Uuid::now_v7(),
            internal_pn: InternalPn::new("R_TEST"),
            revision: rev,
        }
    }

    #[test]
    fn write_then_read_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let f = fixture_file();
        let path = dir.path().join(snxpart_filename(f.uuid, f.revision.version));
        write_snxpart(&path, &f).unwrap();
        let back = read_snxpart(&path).unwrap();
        assert_eq!(f, back);
    }

    #[test]
    fn unsupported_schema_errors() {
        let dir = tempfile::tempdir().unwrap();
        let mut f = fixture_file();
        f.schema = 99;
        let path = dir.path().join("bad.snxpart");
        std::fs::write(&path, serde_json::to_vec(&f).unwrap()).unwrap();
        let err = read_snxpart(&path).unwrap_err();
        assert!(matches!(
            err,
            SnxPartError::UnsupportedSchema { found: 99, .. }
        ));
    }

    #[test]
    fn filename_matches_convention() {
        let uuid = Uuid::now_v7();
        let name = snxpart_filename(uuid, Version::new(1, 2));
        assert!(name.ends_with("-1.2.snxpart"));
        assert!(name.starts_with(&uuid.to_string()));
    }
}
