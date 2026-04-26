//! `.snxprt` is one Component binding record + revision list serialised as
//! JSON. Per `v0.9-library-refactor-plan.md` §3.1 the file extension changes
//! from the legacy `.snxpart` to the canonical 6-letter `.snxprt`, and the
//! schema_version bumps from `1` to `2`.
//!
//! Filename convention is `<uuid>.snxprt` — one file per component (revisions
//! live inside it, since revisions reference reusable primitives now and the
//! per-revision file no longer carries body geometry).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::component::{Component, Revision};
use crate::identity::ComponentId;

/// Current schema version. Bumps with the v0.9 library refactor.
pub const SCHEMA_VERSION: u32 = 2;

/// File header — describes payload schema, plus the component identity for
/// quick triage without parsing the full record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SnxPartFile {
    /// Format version — bump when `.snxprt` schema changes.
    pub schema_version: u32,
    /// The full component record — uuid, class, category, revisions, head.
    pub component: Component,
}

#[derive(Debug, thiserror::Error)]
pub enum SnxPartError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error(
        "unsupported schema {found}, expected {expected}. \
         If this file was authored by an older v0.9 build, re-save it from \
         the Library editor to migrate, or run the in-tree converter binary"
    )]
    UnsupportedSchema { found: u32, expected: u32 },
}

/// Filename convention — `<uuid>.snxprt`, one file per component.
pub fn snxpart_filename(uuid: ComponentId) -> String {
    format!("{uuid}.snxprt")
}

/// Convenience: pull the head revision out of a freshly-read file.
impl SnxPartFile {
    pub fn head_revision(&self) -> Option<&Revision> {
        self.component.head_revision()
    }
}

/// Write a `.snxprt` file as pretty-printed JSON.
pub fn write_snxpart(path: &Path, file: &SnxPartFile) -> Result<(), SnxPartError> {
    let bytes = serde_json::to_vec_pretty(file)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

/// Read a `.snxprt` file, validating schema version. Schema `1` is rejected
/// with a friendly migration hint via [`SnxPartError::UnsupportedSchema`].
pub fn read_snxpart(path: &Path) -> Result<SnxPartFile, SnxPartError> {
    let bytes = std::fs::read(path)?;
    let file: SnxPartFile = serde_json::from_slice(&bytes)?;
    if file.schema_version != SCHEMA_VERSION {
        return Err(SnxPartError::UnsupportedSchema {
            found: file.schema_version,
            expected: SCHEMA_VERSION,
        });
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{Component, DatasheetRef, PlmReserved};
    use crate::identity::{ComponentClass, InternalPn, Version};
    use crate::lifecycle::LifecycleState;
    use crate::manufacturer::ManufacturerPart;
    use crate::param::ParamMap;
    use crate::primitive::PrimitiveRef;
    use std::path::PathBuf;
    use uuid::Uuid;

    fn fixture_component() -> Component {
        let lib = Uuid::new_v4();
        let mut rev = Revision {
            version: Version::new(1, 2),
            state: LifecycleState::Released,
            created: chrono::Utc::now(),
            author: "test@example.com".into(),
            message: "initial".into(),
            symbol_ref: PrimitiveRef::new(lib, Uuid::new_v4()),
            footprint_ref: Some(PrimitiveRef::new(lib, Uuid::new_v4())),
            sim_ref: None,
            pin_map_overrides: Vec::new(),
            primary_mpn: ManufacturerPart::draft("Acme", "TEST-001"),
            alternates: Vec::new(),
            supply: Vec::new(),
            datasheet: DatasheetRef::url("https://example.com/ds.pdf"),
            parameters: ParamMap::new(),
            plm: PlmReserved::default(),
            content_hash: [0u8; 32],
        };
        rev.refresh_content_hash();
        Component {
            uuid: Uuid::now_v7(),
            internal_pn: InternalPn::new("R_TEST"),
            class: ComponentClass::new("resistor"),
            category: PathBuf::from("Passives/Resistors"),
            family: None,
            head: rev.version,
            revisions: vec![rev],
        }
    }

    #[test]
    fn write_then_read_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let f = SnxPartFile {
            schema_version: SCHEMA_VERSION,
            component: fixture_component(),
        };
        let path = dir.path().join(snxpart_filename(f.component.uuid));
        write_snxpart(&path, &f).unwrap();
        let back = read_snxpart(&path).unwrap();
        assert_eq!(f, back);
    }

    #[test]
    fn unsupported_schema_one_errors_with_migration_hint() {
        let dir = tempfile::tempdir().unwrap();
        let mut f = SnxPartFile {
            schema_version: 1, // legacy v0.9 shape
            component: fixture_component(),
        };
        f.schema_version = 1;
        let path = dir.path().join("legacy.snxprt");
        std::fs::write(&path, serde_json::to_vec(&f).unwrap()).unwrap();
        let err = read_snxpart(&path).unwrap_err();
        match err {
            SnxPartError::UnsupportedSchema { found, expected } => {
                assert_eq!(found, 1);
                assert_eq!(expected, SCHEMA_VERSION);
            }
            other => panic!("expected UnsupportedSchema, got {other:?}"),
        }
    }

    #[test]
    fn filename_uses_snxprt_extension() {
        let uuid = Uuid::now_v7();
        let name = snxpart_filename(uuid);
        assert!(name.ends_with(".snxprt"));
        assert!(name.starts_with(&uuid.to_string()));
    }

    #[test]
    fn schema_version_constant_is_two() {
        assert_eq!(SCHEMA_VERSION, 2);
    }
}
