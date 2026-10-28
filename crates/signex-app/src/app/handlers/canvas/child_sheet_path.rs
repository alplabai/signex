//! Where a new child sheet may be written, from the text the user typed in
//! the Create Sheet dialog. Pure path rules (no file access) so they test on
//! every OS. Design: docs/HIERARCHY.md, "Creating the child sheet".

use std::path::{Component, Path, PathBuf};

use crate::app::state::CaseFold;
use crate::app::state::scope::path_key_with;

/// Extension every sheet file keeps.
const SHEET_EXTENSION: &str = "snxsch";

/// `path` with `.` and `..` folded lexically. A `..` that would climb above
/// the start stays, so the caller's containment check sees the escape.
fn fold_dots(path: &Path) -> PathBuf {
    let mut out: Vec<Component<'_>> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match out.last() {
                Some(Component::Normal(_)) => {
                    out.pop();
                }
                Some(Component::ParentDir) | None => out.push(component),
                Some(Component::RootDir | Component::Prefix(_) | Component::CurDir) => {}
            },
            other => out.push(other),
        }
    }
    out.iter().map(|c| c.as_os_str()).collect()
}

/// The file the dialog's `text` names: relative to `parent_dir` (the folder of
/// the sheet carrying the symbol) unless absolute. Refused when it is empty,
/// not a `.snxsch`, or does not resolve inside `project_dir`.
pub(crate) fn resolve_new_sheet_path(
    parent_dir: &Path,
    project_dir: &Path,
    text: &str,
    fold: CaseFold,
) -> Result<PathBuf, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Enter a file name".to_owned());
    }
    let typed = Path::new(text);
    let named_sheet = typed
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(SHEET_EXTENSION))
        && typed.file_stem().is_some_and(|stem| !stem.is_empty());
    if !named_sheet {
        return Err(format!("The file name must end in .{SHEET_EXTENSION}"));
    }
    let joined = if typed.is_absolute() {
        typed.to_path_buf()
    } else {
        parent_dir.join(typed)
    };
    let resolved = fold_dots(&joined);
    if !is_inside(&resolved, project_dir, fold) {
        return Err("The sheet must be inside the project folder".to_owned());
    }
    Ok(resolved)
}

/// Whether `path` is `dir` or below it, compared under the project's case
/// rules (so a pick that differs only in case still counts as inside).
fn is_inside(path: &Path, dir: &Path, fold: CaseFold) -> bool {
    let key = path_key_with(&fold_dots(path), fold);
    let dir_key = path_key_with(&fold_dots(dir), fold);
    let dir_key = dir_key.trim_end_matches('/');
    key == dir_key || key.starts_with(&format!("{dir_key}/"))
}

/// Whether writing `path` would land outside `project_dir` through a symlink:
/// the nearest ancestor of `path` that exists, with links resolved, must stay
/// inside the project folder with links resolved. Reads the disk.
pub(crate) fn escapes_through_symlink(project_dir: &Path, path: &Path) -> bool {
    let Ok(root) = project_dir.canonicalize() else {
        return false;
    };
    let nearest = path.ancestors().find(|a| a.exists());
    nearest
        .and_then(|a| a.canonicalize().ok())
        .is_some_and(|real| !real.starts_with(&root))
}

/// `to` as a reference from the folder `from_dir`, with `/` separators:
/// `sheet1.snxsch`, `power/reg.snxsch`, `../shared/reg.snxsch`.
pub(crate) fn reference_from(from_dir: &Path, to: &Path) -> String {
    let from = fold_dots(from_dir);
    let to = fold_dots(to);
    let from: Vec<_> = from.components().collect();
    let to: Vec<_> = to.components().collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = from[common..].iter().map(|_| "..".to_owned()).collect();
    parts.extend(
        to[common..]
            .iter()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()),
    );
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn a_plain_name_lands_beside_the_parent() {
        let got = resolve_new_sheet_path(
            &p("/proj/top"),
            &p("/proj"),
            "sheet1.snxsch",
            CaseFold::PRESERVE,
        );
        assert_eq!(got, Ok(p("/proj/top/sheet1.snxsch")));
    }

    #[test]
    fn a_sub_folder_path_is_allowed_inside_the_project() {
        let got = resolve_new_sheet_path(
            &p("/proj"),
            &p("/proj"),
            "power/regulator.snxsch",
            CaseFold::PRESERVE,
        );
        assert_eq!(got, Ok(p("/proj/power/regulator.snxsch")));
        let up = resolve_new_sheet_path(
            &p("/proj/top"),
            &p("/proj"),
            "../shared/x.snxsch",
            CaseFold::PRESERVE,
        );
        assert_eq!(up, Ok(p("/proj/shared/x.snxsch")));
    }

    #[test]
    fn a_path_that_leaves_the_project_is_refused() {
        for text in [
            "../../outside.snxsch",
            "/elsewhere/x.snxsch",
            "../../../etc/x.snxsch",
        ] {
            let got =
                resolve_new_sheet_path(&p("/proj/top"), &p("/proj"), text, CaseFold::PRESERVE);
            assert!(got.is_err(), "{text}: {got:?}");
        }
        // A sibling folder that only shares a prefix is outside too.
        let got = resolve_new_sheet_path(
            &p("/proj"),
            &p("/proj"),
            "../proj2/x.snxsch",
            CaseFold::PRESERVE,
        );
        assert!(got.is_err(), "{got:?}");
    }

    #[test]
    fn the_name_must_be_a_snxsch() {
        for text in ["", "  ", "x", "x.txt", ".snxsch", "dir/"] {
            let got = resolve_new_sheet_path(&p("/proj"), &p("/proj"), text, CaseFold::PRESERVE);
            assert!(got.is_err(), "{text:?}: {got:?}");
        }
        assert!(
            resolve_new_sheet_path(&p("/proj"), &p("/proj"), "X.SNXSCH", CaseFold::PRESERVE)
                .is_ok()
        );
    }

    #[test]
    fn a_pick_that_differs_only_in_case_is_inside_under_a_folding_project() {
        let text = "/PROJ/Top/x.snxsch";
        assert!(resolve_new_sheet_path(&p("/proj"), &p("/proj"), text, CaseFold::FOLD).is_ok());
        assert!(
            resolve_new_sheet_path(&p("/proj"), &p("/proj"), text, CaseFold::PRESERVE).is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_sub_folder_that_leaves_the_project_is_caught() {
        let dir = tempfile::tempdir().expect("tempdir");
        let project = dir.path().join("proj");
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&project).expect("project");
        std::fs::create_dir_all(&outside).expect("outside");
        std::os::unix::fs::symlink(&outside, project.join("link")).expect("symlink");
        let through = project.join("link").join("new").join("x.snxsch");
        assert!(escapes_through_symlink(&project, &through));
        assert!(!escapes_through_symlink(
            &project,
            &project.join("fresh").join("x.snxsch")
        ));
    }

    #[test]
    fn the_reference_is_relative_to_the_parent_folder() {
        assert_eq!(
            reference_from(&p("/proj"), &p("/proj/a.snxsch")),
            "a.snxsch"
        );
        assert_eq!(
            reference_from(&p("/proj"), &p("/proj/power/r.snxsch")),
            "power/r.snxsch"
        );
        assert_eq!(
            reference_from(&p("/proj/top"), &p("/proj/shared/r.snxsch")),
            "../shared/r.snxsch"
        );
    }
}
