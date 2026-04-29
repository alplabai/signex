//! Signex native file formats — `.snxsch` (schematic) and `.snxpcb` (PCB).
//!
//! Wire format: TOML envelope. Format-version-1 (`snxsch/1`,
//! `snxpcb/1`) uses pure TOML throughout — components, wires, tracks,
//! and other bulk lists serialise as TOML arrays of tables. A future
//! `snxsch/2` / `snxpcb/2` will move bulk numeric data into TSV
//! literal-string blocks (matching the `.snxlib` / `.snxsym` /
//! `.snxfpt` pattern from the v0.9 library refactor) for tighter git
//! diffs and smaller files. The format-version field at the top of
//! every file gates the codec selection.
//!
//! `.snxprj` (project) is unchanged and uses its own pre-existing
//! format. Stays as-is.
//!
//! These types are the canonical Signex schema. KiCad I/O — when it
//! returns via the `signex-kicad-import` companion repo (GPL-3.0) —
//! translates to/from these types at the file-format boundary; no
//! KiCad-shaped types live in this Apache codebase.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::pcb::PcbBoard;
use crate::schematic::SchematicSheet;

// ---------------------------------------------------------------------------
// Format version tokens
// ---------------------------------------------------------------------------

/// Current `.snxsch` format version. Bumping this is a wire-format
/// break: older Signex versions refuse to open the file.
pub const SNXSCH_FORMAT_V1: &str = "snxsch/1";

/// Current `.snxpcb` format version.
pub const SNXPCB_FORMAT_V1: &str = "snxpcb/1";

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum FormatError {
    #[error("toml serialisation failed: {0}")]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("toml deserialisation failed: {0}")]
    TomlDeserialize(#[from] toml::de::Error),
    #[error("unsupported format version: {found:?}; this build supports {expected:?}")]
    UnsupportedVersion { found: String, expected: String },
}

// ---------------------------------------------------------------------------
// .snxsch — schematic file
// ---------------------------------------------------------------------------

/// On-disk representation of a `.snxsch` file.
///
/// The `format` field is checked on load and rewritten on save.
/// The `sheet` field is the `SchematicSheet` payload; multi-sheet
/// designs sit in separate `.snxsch` files referenced via the
/// `child_sheets` table on each sheet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnxSchematic {
    pub format: String,
    pub sheet: SchematicSheet,
}

impl SnxSchematic {
    /// Wrap a `SchematicSheet` for serialisation as the current format version.
    pub fn new(sheet: SchematicSheet) -> Self {
        Self {
            format: SNXSCH_FORMAT_V1.to_string(),
            sheet,
        }
    }

    /// Serialise to a TOML string for writing to disk.
    pub fn write_string(&self) -> Result<String, FormatError> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// Parse a TOML string from disk.
    pub fn parse(input: &str) -> Result<Self, FormatError> {
        let value: SnxSchematic = toml::from_str(input)?;
        if value.format != SNXSCH_FORMAT_V1 {
            return Err(FormatError::UnsupportedVersion {
                found: value.format,
                expected: SNXSCH_FORMAT_V1.to_string(),
            });
        }
        Ok(value)
    }
}

// ---------------------------------------------------------------------------
// .snxpcb — PCB file
// ---------------------------------------------------------------------------

/// On-disk representation of a `.snxpcb` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnxPcb {
    pub format: String,
    pub board: PcbBoard,
}

impl SnxPcb {
    /// Wrap a `PcbBoard` for serialisation as the current format version.
    pub fn new(board: PcbBoard) -> Self {
        Self {
            format: SNXPCB_FORMAT_V1.to_string(),
            board,
        }
    }

    /// Serialise to a TOML string for writing to disk.
    pub fn write_string(&self) -> Result<String, FormatError> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// Parse a TOML string from disk.
    pub fn parse(input: &str) -> Result<Self, FormatError> {
        let value: SnxPcb = toml::from_str(input)?;
        if value.format != SNXPCB_FORMAT_V1 {
            return Err(FormatError::UnsupportedVersion {
                found: value.format,
                expected: SNXPCB_FORMAT_V1.to_string(),
            });
        }
        Ok(value)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn empty_sheet() -> SchematicSheet {
        SchematicSheet {
            uuid: Uuid::nil(),
            version: 1,
            generator: "signex-test".into(),
            generator_version: "0.9".into(),
            paper_size: "A4".into(),
            root_sheet_page: "1".into(),
            symbols: vec![],
            wires: vec![],
            junctions: vec![],
            labels: vec![],
            child_sheets: vec![],
            no_connects: vec![],
            text_notes: vec![],
            buses: vec![],
            bus_entries: vec![],
            drawings: vec![],
            no_erc_directives: vec![],
            title_block: Default::default(),
            lib_symbols: Default::default(),
        }
    }

    fn empty_board() -> PcbBoard {
        PcbBoard {
            uuid: Uuid::nil(),
            version: 1,
            generator: "signex-test".into(),
            thickness: 1.6,
            outline: vec![],
            layers: vec![],
            setup: None,
            nets: vec![],
            footprints: vec![],
            segments: vec![],
            vias: vec![],
            zones: vec![],
            graphics: vec![],
            texts: vec![],
        }
    }

    #[test]
    fn snxsch_round_trip_empty() {
        let snx = SnxSchematic::new(empty_sheet());
        let s = snx.write_string().expect("serialise");
        assert!(s.contains("format = \"snxsch/1\""));
        let back = SnxSchematic::parse(&s).expect("round-trip");
        assert_eq!(back.format, SNXSCH_FORMAT_V1);
    }

    #[test]
    fn snxpcb_round_trip_empty() {
        let snx = SnxPcb::new(empty_board());
        let s = snx.write_string().expect("serialise");
        assert!(s.contains("format = \"snxpcb/1\""));
        let back = SnxPcb::parse(&s).expect("round-trip");
        assert_eq!(back.format, SNXPCB_FORMAT_V1);
    }

    #[test]
    fn rejects_wrong_format_version() {
        // Hand-craft a TOML document with an unsupported version token.
        let bad = "format = \"snxsch/99\"\n\n[sheet]\nuuid = \"00000000-0000-0000-0000-000000000000\"\nversion = 1\ngenerator = \"\"\ngenerator_version = \"\"\npaper_size = \"\"\nroot_sheet_page = \"1\"\n\n[sheet.title_block]\n[sheet.lib_symbols]\n";
        let err = SnxSchematic::parse(bad).expect_err("must reject");
        match err {
            FormatError::UnsupportedVersion { found, expected } => {
                assert_eq!(found, "snxsch/99");
                assert_eq!(expected, SNXSCH_FORMAT_V1);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
