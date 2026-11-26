//! Which library converters Signex knows, and where each one is installed
//! (contract §1): the converter's environment variable when set, else its
//! binary name on `PATH`, absolute entries only.

use std::ffi::OsString;
use std::path::PathBuf;

use super::error::ImportError;

/// A converter Signex looks for. The one list: add a converter here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnownConverter {
    /// Binary name looked up on `PATH` (without the platform suffix).
    pub binary: &'static str,
    /// Environment variable naming the binary's path; beats `PATH`.
    pub env_override: &'static str,
    /// The command that installs it, shown when it is missing.
    pub install_command: &'static str,
}

/// Every converter Signex looks for, in the order the dialog lists them.
pub const KNOWN_CONVERTERS: &[KnownConverter] = &[KnownConverter {
    binary: crate::altium_import::BINARY,
    env_override: crate::altium_import::ENV_OVERRIDE,
    install_command: "cargo install --git https://github.com/alplabai/signex-altium-import \
                      signex-altium-import",
}];

/// Where a converter is, or why it was not found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Located {
    Found(PathBuf),
    /// The environment variable is set to something that is not a file.
    OverrideNotAFile(PathBuf),
    NotInstalled,
}

impl Located {
    /// The binary, or the refusal to show.
    pub fn into_result(self, converter: &KnownConverter) -> Result<PathBuf, ImportError> {
        match self {
            Self::Found(path) => Ok(path),
            Self::OverrideNotAFile(path) => Err(ImportError::OverrideNotAFile {
                env: converter.env_override.to_string(),
                path,
            }),
            Self::NotInstalled => Err(ImportError::NotInstalled {
                binary: converter.binary.to_string(),
                env: converter.env_override.to_string(),
                install: converter.install_command.to_string(),
            }),
        }
    }
}

/// `binary` (plus the platform's executable suffix) in an absolute `PATH`
/// entry. An empty or relative entry would resolve against the working
/// directory, so it is skipped.
pub fn find_on_path(binary: &str, path: Option<OsString>) -> Option<PathBuf> {
    let name = format!("{binary}{}", std::env::consts::EXE_SUFFIX);
    path.iter()
        .flat_map(std::env::split_paths)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(&name))
        .find(|p| p.is_file())
}

/// Locate `converter` given the override's value and `PATH`.
pub fn locate_in(
    converter: &KnownConverter,
    override_path: Option<OsString>,
    path: Option<OsString>,
) -> Located {
    if let Some(p) = override_path.filter(|p| !p.is_empty()).map(PathBuf::from) {
        return if p.is_file() {
            Located::Found(p)
        } else {
            Located::OverrideNotAFile(p)
        };
    }
    find_on_path(converter.binary, path).map_or(Located::NotInstalled, Located::Found)
}

/// Every known converter and where it is, read from this process's
/// environment. Blocking (it checks files); call from a worker.
pub fn discover() -> Vec<(KnownConverter, Located)> {
    KNOWN_CONVERTERS
        .iter()
        .map(|c| {
            let located = locate_in(
                c,
                std::env::var_os(c.env_override),
                std::env::var_os("PATH"),
            );
            (*c, located)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAKE: KnownConverter = KnownConverter {
        binary: "snx-fake-converter",
        env_override: "SNX_FAKE_CONVERTER",
        install_command: "install it",
    };

    #[test]
    fn the_override_wins_and_a_bad_one_is_named() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("conv");
        std::fs::write(&file, "").unwrap();
        assert_eq!(
            locate_in(&FAKE, Some(file.clone().into()), None),
            Located::Found(file)
        );
        let missing = dir.path().join("nope");
        let located = locate_in(&FAKE, Some(missing.clone().into()), None);
        assert_eq!(located, Located::OverrideNotAFile(missing));
    }

    #[test]
    fn path_lookup_uses_absolute_entries_only() {
        let dir = tempfile::tempdir().unwrap();
        let name = format!("{}{}", FAKE.binary, std::env::consts::EXE_SUFFIX);
        std::fs::write(dir.path().join(&name), "").unwrap();
        let found = locate_in(&FAKE, None, Some(dir.path().as_os_str().to_owned()));
        assert_eq!(found, Located::Found(dir.path().join(&name)));
        assert_eq!(
            locate_in(&FAKE, None, Some(OsString::from("relative/dir"))),
            Located::NotInstalled
        );
    }

    #[test]
    fn missing_converter_names_its_install_hint() {
        let err = Located::NotInstalled.into_result(&FAKE).unwrap_err();
        assert!(matches!(err, ImportError::NotInstalled { ref install, .. } if install == "install it"));
    }

    #[test]
    fn every_known_converter_has_a_binary_and_variable() {
        for c in KNOWN_CONVERTERS {
            assert!(!c.binary.is_empty() && !c.env_override.is_empty());
        }
    }
}
