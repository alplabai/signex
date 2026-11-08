//! Path confinement (#1923): a path a command names is resolved first
//! (symlinks followed, `..` gone) and must then sit inside an allowed root.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

/// `path` with every symlink and `..` resolved. The tail that does not exist
/// yet (a file about to be created) is appended after the resolved part and
/// may not hold `..`; a dangling link is refused, not followed blind.
pub fn resolve(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!("{} is not an absolute path", path.display()));
    }
    let unresolved = |why: String| format!("{} cannot be resolved: {why}", path.display());
    let mut missing: Vec<OsString> = Vec::new();
    let mut head = path.to_path_buf();
    // `symlink_metadata`, not `exists`: a dangling link exists as a link.
    while let Err(error) = fs::symlink_metadata(&head) {
        if error.kind() != std::io::ErrorKind::NotFound {
            return Err(unresolved(error.to_string()));
        }
        let (Some(name), Some(parent)) = (head.file_name(), head.parent()) else {
            return Err(unresolved(
                "it climbs out of a folder that is not there".into(),
            ));
        };
        missing.push(name.to_owned());
        head = parent.to_path_buf();
    }
    let mut resolved = canonical(&head).map_err(|error| unresolved(error.to_string()))?;
    resolved.extend(missing.iter().rev());
    Ok(resolved)
}

/// [`resolve`], then the check that the result stays inside one of `roots`.
pub fn confine(path: &Path, roots: &[PathBuf]) -> Result<PathBuf, String> {
    let resolved = resolve(path)?;
    if roots.iter().any(|root| resolved.starts_with(root)) {
        Ok(resolved)
    } else {
        Err(format!(
            "{} is outside the project and its libraries; allow its folder with --allow-path",
            resolved.display()
        ))
    }
}

/// `fs::canonicalize` without the `\\?\` prefix Windows adds to a plain drive
/// path, so keys the app builds from it match the ones it builds itself.
pub fn canonical(path: &Path) -> std::io::Result<PathBuf> {
    let resolved = fs::canonicalize(path)?;
    #[cfg(windows)]
    if let Some(plain) = resolved
        .to_str()
        .and_then(|text| text.strip_prefix(r"\\?\"))
        && plain.as_bytes().get(1) == Some(&b':')
    {
        return Ok(PathBuf::from(plain));
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = canonical(dir.path()).expect("canonical");
        (dir, root)
    }

    #[test]
    fn a_path_inside_the_root_passes_even_when_it_does_not_exist_yet() {
        let (_guard, root) = root();
        let existing = root.join("a.snxsch");
        fs::write(&existing, "x").unwrap();
        let roots = [root.clone()];
        assert_eq!(confine(&existing, &roots).unwrap(), existing);
        let new = root.join("sub").join("new.snxsch");
        assert_eq!(confine(&new, &roots).unwrap(), new);
    }

    #[test]
    fn a_path_outside_every_root_is_refused() {
        let (_guard, root) = root();
        let (_other_guard, other) = self::root();
        let err = confine(&other.join("x.snxsch"), &[root]).unwrap_err();
        assert!(err.contains("--allow-path"), "{err}");
    }

    #[test]
    fn a_dot_dot_out_of_the_root_is_refused_whether_or_not_it_exists() {
        let (_guard, root) = root();
        let (_other_guard, other) = self::root();
        let roots = [root.clone()];
        let sibling = other.file_name().unwrap().to_owned();
        let up = root.join("..").join(&sibling).join("x");
        assert!(confine(&up, &roots).is_err());
        // The folder is not there. Unix cannot resolve `..` against it and says
        // so; Windows folds `..` into the path text before it looks, so the path
        // resolves one folder up and is refused as outside. Either way it is out.
        let missing = root.join("nothing").join("..").join("..").join("x");
        let err = confine(&missing, &roots).unwrap_err();
        assert!(
            err.contains("cannot be resolved") || err.contains("outside the project"),
            "{err}"
        );
    }

    #[test]
    fn a_relative_path_is_refused() {
        let err = resolve(Path::new("relative/x.snxsch")).unwrap_err();
        assert!(err.contains("not an absolute path"), "{err}");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_out_of_the_root_is_followed_before_the_check() {
        let (_guard, root) = root();
        let (_other_guard, other) = self::root();
        fs::write(other.join("secret.snxsch"), "x").unwrap();
        std::os::unix::fs::symlink(&other, root.join("link")).unwrap();
        let roots = [root.clone()];
        let err = confine(&root.join("link").join("secret.snxsch"), &roots).unwrap_err();
        assert!(err.contains("outside"), "{err}");
        // A file about to be created under the link lands outside too.
        assert!(confine(&root.join("link").join("new.snxsch"), &roots).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn a_dangling_symlink_is_refused_not_written_through() {
        let (_guard, root) = root();
        let (_other_guard, other) = self::root();
        std::os::unix::fs::symlink(other.join("gone"), root.join("dangling")).unwrap();
        let err = confine(&root.join("dangling"), std::slice::from_ref(&root)).unwrap_err();
        assert!(err.contains("cannot be resolved"), "{err}");
    }
}
