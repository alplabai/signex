//! The kinds of file File ▸ Fabrication Outputs writes for a project
//! (#2489), stored in the `.snxprj` as the optional `fab_outputs` object.
//! Absent while the ticks are the defaults, so a project that never changed
//! them re-saves byte-identical. Rules: `docs/FABRICATION_OUTPUTS.md`.

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// Which kinds of file Generate writes. A key a file leaves out takes its
/// default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FabOutputKinds {
    pub gerber: bool,
    pub drill: bool,
    pub centroid: bool,
    pub test_netlist: bool,
    pub ipc2581: bool,
    pub odb: bool,
    /// The ODB++ job as its folder tree instead of one archive.
    pub odb_folder: bool,
    /// Both sides' assembly drawings as one PDF.
    pub assembly_pdf: bool,
    /// Each side's assembly drawing as an SVG file.
    pub assembly_svg: bool,
    /// The board and its parts as a STEP model.
    pub step: bool,
}

impl Default for FabOutputKinds {
    /// The fabrication files, the IPC-2581 file, the ODB++ archive and the
    /// assembly PDF; the SVG drawings and the STEP model when asked.
    fn default() -> Self {
        Self {
            gerber: true,
            drill: true,
            centroid: true,
            test_netlist: true,
            ipc2581: true,
            odb: true,
            odb_folder: false,
            assembly_pdf: true,
            assembly_svg: false,
            step: false,
        }
    }
}

impl FabOutputKinds {
    /// Whether a kind the fabrication writer makes is chosen (everything but
    /// the assembly drawings and the STEP model).
    pub fn has_fab_files(&self) -> bool {
        self.gerber || self.drill || self.centroid || self.test_netlist || self.ipc2581 || self.odb
    }

    /// What a project stores for these kinds: nothing for the defaults.
    pub fn stored(self) -> Option<Self> {
        (self != Self::default()).then_some(self)
    }
}

/// Read a stored `fab_outputs`. A value this build cannot read never makes
/// the project unopenable: it reads as absent (the defaults) and a warning
/// says so; the next save of a changed tick replaces it.
pub(super) fn lenient_fab_outputs<'de, D>(
    deserializer: D,
) -> Result<Option<FabOutputKinds>, D::Error>
where
    D: Deserializer<'de>,
{
    let Some(value) = Option::<Value>::deserialize(deserializer)? else {
        return Ok(None);
    };
    match serde_json::from_value::<FabOutputKinds>(value) {
        Ok(kinds) => Ok(Some(kinds)),
        Err(why) => {
            tracing::warn!(
                target: "signex::project",
                "The project's fabrication output settings are not readable ({why}); the \
                 defaults apply."
            );
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use serde_json::{Value, json};

    use super::FabOutputKinds;
    use crate::project::{encode_project, parse_project, write_project};

    fn write_raw(dir: &Path, body: &Value) -> PathBuf {
        let path = dir.join("Proj.snxprj");
        std::fs::write(&path, body.to_string()).unwrap();
        path
    }

    fn bare() -> Value {
        json!({"name": "Proj", "dir": "", "schematic_root": null, "pcb_file": null})
    }

    #[test]
    fn a_project_without_the_key_re_saves_byte_identical() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_raw(dir.path(), &bare());
        let data = parse_project(&path).unwrap();
        assert_eq!(data.fab_outputs, None);
        write_project(&path, &data).unwrap();
        let first = std::fs::read(&path).unwrap();
        let again = parse_project(&path).unwrap();
        assert_eq!(encode_project(&again).unwrap(), first);
        let saved: Value = serde_json::from_slice(&first).unwrap();
        assert!(saved.get("fab_outputs").is_none(), "{saved}");
    }

    #[test]
    fn stored_kinds_are_read_back_equal() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_raw(dir.path(), &bare());
        let mut data = parse_project(&path).unwrap();
        let kinds = FabOutputKinds {
            gerber: false,
            odb_folder: true,
            assembly_svg: true,
            step: true,
            ..FabOutputKinds::default()
        };
        data.fab_outputs = kinds.stored();
        write_project(&path, &data).unwrap();
        assert_eq!(parse_project(&path).unwrap().fab_outputs, Some(kinds));
    }

    #[test]
    fn the_defaults_store_nothing() {
        assert_eq!(FabOutputKinds::default().stored(), None);
        let step = FabOutputKinds {
            step: true,
            ..FabOutputKinds::default()
        };
        assert_eq!(step.stored(), Some(step));
    }

    #[test]
    fn a_missing_kind_takes_its_default_and_an_unreadable_value_opens_the_project() {
        let dir = tempfile::tempdir().unwrap();
        let mut body = bare();
        body["fab_outputs"] = json!({"step": true});
        let path = write_raw(dir.path(), &body);
        let kinds = parse_project(&path).unwrap().fab_outputs.expect("read");
        assert!(kinds.step && kinds.gerber && !kinds.assembly_svg);

        body["fab_outputs"] = json!({"step": "yes"});
        let path = write_raw(dir.path(), &body);
        assert_eq!(parse_project(&path).unwrap().fab_outputs, None);
    }
}
