//! A converter reports paths; Signex only follows ones inside the output
//! folder it chose. Shared by both import contracts.

use std::path::{Path, PathBuf};

/// Why a reported path was refused.
#[derive(Debug)]
pub enum ConfineError {
    /// It resolves outside the output folder, or the folder cannot be read.
    Outside,
    /// It cannot be resolved (it does not exist, say).
    Unreadable { joined: PathBuf, error: std::io::Error },
}

/// `reported`, resolved against `out_dir` when relative, refused unless it
/// lies inside `out_dir` once links are followed.
pub fn confine(reported: &Path, out_dir: &Path) -> Result<PathBuf, ConfineError> {
    let joined = out_dir.join(reported);
    let dir = out_dir.canonicalize().map_err(|_| ConfineError::Outside)?;
    let file = joined
        .canonicalize()
        .map_err(|error| ConfineError::Unreadable {
            joined: joined.clone(),
            error,
        })?;
    if file.starts_with(&dir) {
        Ok(joined)
    } else {
        Err(ConfineError::Outside)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inside_relative_and_absolute_are_kept_outside_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        std::fs::create_dir_all(out.join("lib")).unwrap();
        assert_eq!(confine(Path::new("lib"), &out).unwrap(), out.join("lib"));
        assert_eq!(confine(&out.join("lib"), &out).unwrap(), out.join("lib"));
        assert!(matches!(
            confine(dir.path(), &out),
            Err(ConfineError::Outside)
        ));
        assert!(matches!(
            confine(Path::new("../out/../"), &out),
            Err(ConfineError::Outside)
        ));
        assert!(matches!(
            confine(Path::new("missing"), &out),
            Err(ConfineError::Unreadable { .. })
        ));
    }
}
