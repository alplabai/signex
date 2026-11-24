//! The material library of the Layer Stack's Materials sub-tab (mockup rows
//! 105 and 177): cores, prepregs, copper foils and solder masks with their
//! Dk, Df and thickness. Built-in entries are the list below; a person's own
//! entries live in one user file in the config folder (`materials.snxmat`,
//! token `snxmat/1`). Picking an entry copies its values into the stack row
//! (`copy_into`), so a board opens the same without the library (decision
//! 2026-11-24, "the board stores a copy of the values it uses").

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::format_version::{FormatVersion, FormatVersionError, MissingToken, NewerMinor};
use crate::pcb_stack::{DEFAULT_COPPER_MM, DEFAULT_COPPER_WEIGHT_OZ, StackLayer, StackLayerKind};

/// The user library's file name, in the config folder.
pub const USER_MATERIALS_FILE: &str = "materials.snxmat";

/// The user library's format token.
pub const SNXMAT_FORMAT_V1: &str = "snxmat/1";

/// `.snxmat` version policy: a newer token is refused, so a library this
/// build cannot represent is never rewritten.
pub const SNXMAT_VERSION: FormatVersion = FormatVersion {
    current: SNXMAT_FORMAT_V1,
    readable: &[SNXMAT_FORMAT_V1],
    newer_minor: NewerMinor::Refused,
    missing: MissingToken::Refused,
};

/// Copper thickness per oz/ft² of weight, mm (1 oz is 35 µm).
const COPPER_MM_PER_OZ: f64 = DEFAULT_COPPER_MM / DEFAULT_COPPER_WEIGHT_OZ;

/// What a library entry is, and so which stack rows it fits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialKind {
    Core,
    Prepreg,
    Copper,
    SolderMask,
}

impl MaterialKind {
    pub const ALL: [Self; 4] = [Self::Core, Self::Prepreg, Self::Copper, Self::SolderMask];

    /// Whether an entry of this kind can be used for a row of `row`.
    pub fn fits(self, row: StackLayerKind) -> bool {
        match self {
            Self::Core | Self::Prepreg => row == StackLayerKind::Dielectric,
            Self::Copper => row.is_copper(),
            Self::SolderMask => row == StackLayerKind::SolderMask,
        }
    }
}

/// One library entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Material {
    pub name: String,
    pub kind: MaterialKind,
    /// Dk; 0 for copper.
    #[serde(default)]
    pub dielectric_constant: f64,
    /// Df; 0 for copper.
    #[serde(default)]
    pub loss_tangent: f64,
    pub thickness_mm: f64,
}

/// Why a library entry cannot be used or saved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MaterialFault {
    #[error("a material needs a name")]
    NoName,
    #[error("the material's values must be numbers that are not negative")]
    BadNumber,
    #[error("a material named \"{0}\" is already in the library")]
    NameTaken(String),
}

impl Material {
    /// What keeps the entry from being used or saved; `None` for a good one.
    pub fn fault(&self) -> Option<MaterialFault> {
        if self.name.trim().is_empty() {
            return Some(MaterialFault::NoName);
        }
        let numbers = [
            self.dielectric_constant,
            self.loss_tangent,
            self.thickness_mm,
        ];
        let valid = numbers.iter().all(|v| v.is_finite() && *v >= 0.0);
        (!valid).then_some(MaterialFault::BadNumber)
    }

    /// Copy the entry's values into `row`, which it must fit. A dielectric
    /// or mask row takes the name, thickness, Dk and Df; a copper row takes
    /// the thickness and the weight it makes, and keeps no material name.
    pub fn copy_into(&self, row: &mut StackLayer) {
        row.thickness_mm = self.thickness_mm;
        if row.kind.is_copper() {
            row.copper_weight_oz = self.thickness_mm / COPPER_MM_PER_OZ;
            return;
        }
        row.material = self.name.trim().to_string();
        row.dielectric_constant = self.dielectric_constant;
        row.loss_tangent = self.loss_tangent;
    }
}

/// `(name, kind, Dk, Df, thickness mm)`: Dk and Df at 1 GHz, prepreg
/// thickness as pressed. Generic classes only, no supplier grades.
const BUILT_IN: [(&str, MaterialKind, f64, f64, f64); 17] = {
    use MaterialKind::{Copper as CU, Core as CORE, Prepreg as PP, SolderMask as SM};
    [
        ("FR-4 core 0.10", CORE, 4.30, 0.018, 0.10),
        ("FR-4 core 0.20", CORE, 4.40, 0.018, 0.20),
        ("FR-4 core 0.71", CORE, 4.50, 0.018, 0.71),
        ("FR-4 core 1.00 (5 × 7628)", CORE, 4.50, 0.018, 1.00),
        ("FR-4 core 1.20 (6 × 7628)", CORE, 4.50, 0.018, 1.20),
        ("FR-4 core 1.40 (7 × 7628)", CORE, 4.50, 0.018, 1.40),
        ("High-Tg FR-4 core 0.20", CORE, 4.30, 0.016, 0.20),
        ("HF hydrocarbon-ceramic core", CORE, 3.48, 0.0037, 0.508),
        ("Prepreg 106", PP, 3.70, 0.020, 0.050),
        ("Prepreg 1080", PP, 3.90, 0.020, 0.079),
        ("Prepreg 2116", PP, 4.10, 0.019, 0.120),
        ("Prepreg 7628", PP, 4.40, 0.018, 0.190),
        ("Copper foil 0.5 oz", CU, 0.0, 0.0, 0.0175),
        ("Copper foil 1 oz", CU, 0.0, 0.0, 0.035),
        ("Copper foil 2 oz", CU, 0.0, 0.0, 0.070),
        ("LPI solder mask", SM, 3.50, 0.020, 0.015),
        ("Dry film solder mask", SM, 3.80, 0.025, 0.025),
    ]
};

/// The built-in entries, read-only, in library order.
pub fn builtin_materials() -> Vec<Material> {
    BUILT_IN
        .iter()
        .map(|&(name, kind, dk, df, mm)| Material {
            name: name.to_string(),
            kind,
            dielectric_constant: dk,
            loss_tangent: df,
            thickness_mm: mm,
        })
        .collect()
}

/// `entry` added to the user library `user`; refused for a bad entry or a
/// name the built-in or user entries already use.
pub fn add_user_material(
    user: &[Material],
    entry: Material,
) -> Result<Vec<Material>, MaterialFault> {
    if let Some(fault) = entry.fault() {
        return Err(fault);
    }
    let name = entry.name.trim().to_string();
    let taken = builtin_materials()
        .iter()
        .chain(user)
        .any(|m| m.name.trim() == name);
    if taken {
        return Err(MaterialFault::NameTaken(name));
    }
    let mut out = user.to_vec();
    out.push(Material { name, ..entry });
    Ok(out)
}

/// Why the user library could not be read or written.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum MaterialLibraryError {
    #[error(transparent)]
    Version(#[from] FormatVersionError),
    #[error("the material library is not valid: {0}")]
    Parse(String),
    #[error("material \"{name}\" in the library: {fault}")]
    Entry { name: String, fault: MaterialFault },
    #[error("{0}")]
    Io(String),
}

#[derive(Serialize)]
struct LibraryOut<'a> {
    format: &'static str,
    materials: &'a [Material],
}

#[derive(Deserialize)]
struct LibraryIn {
    format: Option<String>,
    #[serde(default)]
    materials: Vec<Material>,
}

/// The user library as its file holds it.
pub fn encode_user_materials(materials: &[Material]) -> Result<String, MaterialLibraryError> {
    if let Some(bad) = materials.iter().find(|m| m.fault().is_some()) {
        return Err(entry_error(bad));
    }
    let file = LibraryOut {
        format: SNXMAT_FORMAT_V1,
        materials,
    };
    toml::to_string(&file).map_err(|e| MaterialLibraryError::Parse(e.to_string()))
}

/// Read a user library file's text. Refused for a missing or newer token
/// and for an entry that could not be saved.
pub fn parse_user_materials(text: &str) -> Result<Vec<Material>, MaterialLibraryError> {
    let file: LibraryIn =
        toml::from_str(text).map_err(|e| MaterialLibraryError::Parse(e.to_string()))?;
    SNXMAT_VERSION.read(file.format.as_deref())?;
    if let Some(bad) = file.materials.iter().find(|m| m.fault().is_some()) {
        return Err(entry_error(bad));
    }
    Ok(file.materials)
}

fn entry_error(bad: &Material) -> MaterialLibraryError {
    MaterialLibraryError::Entry {
        name: bad.name.clone(),
        fault: bad.fault().unwrap_or(MaterialFault::BadNumber),
    }
}

/// Read the user library at `path`; a missing file is an empty library.
/// Blocking IO: the app calls it from a task.
pub fn read_user_materials(path: &Path) -> Result<Vec<Material>, MaterialLibraryError> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_user_materials(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(MaterialLibraryError::Io(e.to_string())),
    }
}

/// Write the user library to `path` atomically. Blocking IO: the app calls
/// it from a task.
pub fn write_user_materials(
    path: &Path,
    materials: &[Material],
) -> Result<(), MaterialLibraryError> {
    let text = encode_user_materials(materials)?;
    crate::atomic_io::atomic_write(path, text.as_bytes())
        .map_err(|e| MaterialLibraryError::Io(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, kind: MaterialKind) -> Material {
        Material {
            name: name.into(),
            kind,
            dielectric_constant: 3.66,
            loss_tangent: 0.0031,
            thickness_mm: 0.1016,
        }
    }

    #[test]
    fn the_built_in_library_has_every_kind_with_valid_unique_entries() {
        let built_in = builtin_materials();
        for kind in MaterialKind::ALL {
            assert!(built_in.iter().any(|m| m.kind == kind), "{kind:?}");
        }
        for (i, m) in built_in.iter().enumerate() {
            assert_eq!(m.fault(), None, "{}", m.name);
            assert!(
                built_in[..i].iter().all(|o| o.name != m.name),
                "{} twice",
                m.name
            );
        }
    }

    #[test]
    fn a_kind_fits_only_its_own_rows() {
        use StackLayerKind::*;
        assert!(MaterialKind::Core.fits(Dielectric) && MaterialKind::Prepreg.fits(Dielectric));
        assert!(MaterialKind::Copper.fits(Signal) && MaterialKind::Copper.fits(Plane));
        assert!(MaterialKind::SolderMask.fits(SolderMask));
        assert!(!MaterialKind::Core.fits(Signal));
        assert!(!MaterialKind::Copper.fits(Dielectric));
        assert!(!MaterialKind::SolderMask.fits(Overlay));
    }

    #[test]
    fn a_dielectric_row_takes_a_copy_of_every_value() {
        let mut row = StackLayer::new(StackLayerKind::Dielectric);
        entry("My core", MaterialKind::Core).copy_into(&mut row);
        assert_eq!(row.material, "My core");
        assert_eq!(row.thickness_mm, 0.1016);
        assert_eq!(row.dielectric_constant, 3.66);
        assert_eq!(row.loss_tangent, 0.0031);
    }

    #[test]
    fn a_copper_row_takes_thickness_and_weight_but_no_material() {
        let mut row = StackLayer::new(StackLayerKind::Signal);
        let before = row.material.clone();
        let foil = builtin_materials()
            .into_iter()
            .find(|m| m.name == "Copper foil 2 oz")
            .unwrap();
        foil.copy_into(&mut row);
        assert_eq!(row.thickness_mm, 0.070);
        assert!((row.copper_weight_oz - 2.0).abs() < 1e-9);
        assert_eq!(row.material, before);
        assert_eq!(row.loss_tangent, 0.0);
    }

    #[test]
    fn the_user_library_round_trips_under_its_token() {
        let user = vec![
            entry("My core", MaterialKind::Core),
            entry("My prepreg", MaterialKind::Prepreg),
        ];
        let text = encode_user_materials(&user).unwrap();
        assert!(
            text.starts_with(&format!("format = \"{SNXMAT_FORMAT_V1}\"")),
            "{text}"
        );
        assert_eq!(parse_user_materials(&text).unwrap(), user);
        let empty = encode_user_materials(&[]).unwrap();
        assert_eq!(parse_user_materials(&empty).unwrap(), []);
    }

    #[test]
    fn a_newer_or_missing_token_or_a_bad_entry_is_refused() {
        let text = encode_user_materials(&[entry("A", MaterialKind::Core)]).unwrap();
        let newer = text.replacen(SNXMAT_FORMAT_V1, "snxmat/2", 1);
        assert!(matches!(
            parse_user_materials(&newer),
            Err(MaterialLibraryError::Version(_))
        ));
        let missing = text.replacen(&format!("format = \"{SNXMAT_FORMAT_V1}\"\n"), "", 1);
        assert!(matches!(
            parse_user_materials(&missing),
            Err(MaterialLibraryError::Version(_))
        ));
        let negative = text.replacen("thickness_mm = 0.1016", "thickness_mm = -1.0", 1);
        assert!(matches!(
            parse_user_materials(&negative),
            Err(MaterialLibraryError::Entry { .. })
        ));
        let mut bad = entry("B", MaterialKind::Core);
        bad.dielectric_constant = f64::NAN;
        assert!(encode_user_materials(&[bad]).is_err());
    }

    #[test]
    fn a_name_in_use_or_a_blank_name_is_not_added() {
        let user = add_user_material(&[], entry(" Mine ", MaterialKind::Core)).unwrap();
        assert_eq!(user[0].name, "Mine", "trimmed");
        assert_eq!(
            add_user_material(&user, entry("Mine", MaterialKind::Prepreg)),
            Err(MaterialFault::NameTaken("Mine".into()))
        );
        assert_eq!(
            add_user_material(&user, entry("Prepreg 1080", MaterialKind::Prepreg)),
            Err(MaterialFault::NameTaken("Prepreg 1080".into()))
        );
        assert_eq!(
            add_user_material(&user, entry("  ", MaterialKind::Core)),
            Err(MaterialFault::NoName)
        );
    }

    #[test]
    fn the_file_is_written_atomically_and_a_missing_one_reads_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(USER_MATERIALS_FILE);
        assert_eq!(read_user_materials(&path).unwrap(), []);
        let user = vec![entry("Mine", MaterialKind::SolderMask)];
        write_user_materials(&path, &user).unwrap();
        assert_eq!(read_user_materials(&path).unwrap(), user);
    }
}
