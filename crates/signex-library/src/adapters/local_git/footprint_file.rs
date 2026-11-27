//! Reading a footprint with the `.snxfpt` envelope that holds it, so a
//! caller can reach the STEP models the file embeds (#3107).

use super::*;
use crate::primitive::FootprintFile;

impl LocalGitAdapter {
    /// The `.snxfpt` envelope of the footprint `uuid` (v0.18.4), its
    /// embedded models included.
    pub fn read_footprint_file(&self, uuid: Uuid) -> Result<FootprintFile, LibraryError> {
        let bytes = self.read_primitive_bytes(PrimitiveKind::Footprint, uuid)?;
        let path = self.primitive_path(PrimitiveKind::Footprint, uuid);
        FootprintFile::from_bytes(&bytes)
            .map_err(|e| LibraryError::Backend(format!("read {}: {e}", path.display())))
    }

    /// Read the first footprint of the `.snxfpt` envelope.
    ///
    /// Typed per kind rather than generic over `T: DeserializeOwned`: the
    /// generic form had to serialise the footprint to JSON and parse it back
    /// just to hand the caller the value it already had (#534).
    pub(super) fn read_footprint(&self, uuid: Uuid) -> Result<Footprint, LibraryError> {
        self.read_footprint_file(uuid)?
            .footprints
            .into_iter()
            .next()
            .ok_or_else(|| LibraryError::Backend("empty FootprintFile".into()))
    }

    /// `<library>/step/`: where libraries written before #3107 keep their
    /// STEP models as `<hash>.step`. Read as a fallback, never written.
    pub fn step_dir(&self) -> std::path::PathBuf {
        self.root_dir.join("step")
    }
}
