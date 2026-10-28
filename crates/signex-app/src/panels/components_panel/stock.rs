//! Bundled stock library: locate it next to the executable and mount it.
//!
//! The Stock section is not user-configured state, so nothing here reads or
//! writes `global_libraries.toml`. The app never writes to the stock
//! directory; only `open_library` (a read of the manifest) touches it.
//!
//! Install layouts (the installer packaging agrees on exactly these):
//! - Windows: `<exe dir>\library\Stock.snxlib`
//! - Linux: `<exe dir>/../share/signex/library/Stock.snxlib`
//! - macOS: `<exe dir>/../Resources/library/Stock.snxlib`
//! - dev checkout: `assets/stock-library/Stock.snxlib`

use std::path::{Path, PathBuf};

const STOCK_FILE: &str = "Stock.snxlib";

/// Packaging layout, split from `cfg!` so each one is testable anywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallLayout {
    Windows,
    Linux,
    MacOs,
}

impl InstallLayout {
    pub const fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Linux
        }
    }

    fn installed_path(self, exe_dir: &Path) -> PathBuf {
        match self {
            Self::Windows => exe_dir.join("library").join(STOCK_FILE),
            Self::Linux => exe_dir.join("../share/signex/library").join(STOCK_FILE),
            Self::MacOs => exe_dir.join("../Resources/library").join(STOCK_FILE),
        }
    }
}

fn dev_path(root: &Path) -> PathBuf {
    root.join("assets").join("stock-library").join(STOCK_FILE)
}

/// Pure resolver: installed copy first, then the dev checkout (the
/// compile-time workspace root, then any ancestor of the exe). `exists` is
/// injected so tests never touch the real filesystem.
pub fn resolve_stock_library(
    exe: &Path,
    layout: InstallLayout,
    workspace_root: &Path,
    exists: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let installed = exe.parent().map(|d| layout.installed_path(d));
    let dev_from_manifest = Some(dev_path(workspace_root));
    let dev_from_exe = exe.ancestors().skip(1).map(dev_path);
    installed
        .into_iter()
        .chain(dev_from_manifest)
        .chain(dev_from_exe)
        .find(|p| exists(p))
}

/// Resolve against the running executable and the real filesystem.
pub fn stock_library_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe = exe.canonicalize().unwrap_or(exe);
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    resolve_stock_library(&exe, InstallLayout::current(), &workspace_root, |p| {
        p.exists()
    })
}

/// Mount `path` (if any) onto `library_state`, read-only: the bundled
/// library ships with the app and must never be edited in place. Never fails the caller: a
/// missing or unmountable library warns and leaves the Stock section empty.
pub fn mount_at(library_state: &mut crate::library::LibraryState, path: Option<PathBuf>) {
    let Some(path) = path else {
        tracing::warn!(
            target: "signex::library",
            "stock library not found next to the executable; Stock section left empty"
        );
        return;
    };
    library_state.stock_library_root = Some(path.clone());
    match library_state.open_library_read_only(path.clone()) {
        Ok(()) => library_state.stock_library = Some(path),
        Err(e) => tracing::warn!(
            target: "signex::library",
            path = %path.display(),
            error = %e,
            "stock library mount failed; Stock section left empty"
        ),
    }
}

/// Bootstrap entry point, called next to the global-library mount.
pub fn mount_stock(library_state: &mut crate::library::LibraryState) {
    mount_at(library_state, stock_library_path());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn only(found: &'static [&'static str]) -> impl Fn(&Path) -> bool {
        move |p| found.iter().any(|f| p == Path::new(f))
    }

    #[test]
    fn windows_layout() {
        let got = resolve_stock_library(
            Path::new("C:/Signex/signex.exe"),
            InstallLayout::Windows,
            Path::new("/nowhere"),
            only(&["C:/Signex/library/Stock.snxlib"]),
        );
        assert_eq!(got, Some(PathBuf::from("C:/Signex/library/Stock.snxlib")));
    }

    #[test]
    fn linux_layout() {
        let got = resolve_stock_library(
            Path::new("/usr/bin/signex"),
            InstallLayout::Linux,
            Path::new("/nowhere"),
            only(&["/usr/bin/../share/signex/library/Stock.snxlib"]),
        );
        assert!(got.is_some());
    }

    #[test]
    fn macos_layout() {
        let got = resolve_stock_library(
            Path::new("/Applications/Signex.app/Contents/MacOS/signex"),
            InstallLayout::MacOs,
            Path::new("/nowhere"),
            only(&["/Applications/Signex.app/Contents/MacOS/../Resources/library/Stock.snxlib"]),
        );
        assert!(got.is_some());
    }

    #[test]
    fn dev_fallback_from_workspace_root() {
        let got = resolve_stock_library(
            Path::new("/usr/bin/signex"),
            InstallLayout::Linux,
            Path::new("/work/signex"),
            only(&["/work/signex/assets/stock-library/Stock.snxlib"]),
        );
        assert_eq!(
            got,
            Some(PathBuf::from(
                "/work/signex/assets/stock-library/Stock.snxlib"
            ))
        );
    }

    #[test]
    fn dev_fallback_walks_up_from_exe() {
        let got = resolve_stock_library(
            Path::new("/work/signex/target/debug/signex"),
            InstallLayout::Linux,
            Path::new("/nowhere"),
            only(&["/work/signex/assets/stock-library/Stock.snxlib"]),
        );
        assert!(got.is_some());
    }

    #[test]
    fn installed_copy_preferred_over_dev() {
        let got = resolve_stock_library(
            Path::new("/work/signex/target/debug/signex"),
            InstallLayout::Linux,
            Path::new("/work/signex"),
            only(&[
                "/work/signex/target/debug/../share/signex/library/Stock.snxlib",
                "/work/signex/assets/stock-library/Stock.snxlib",
            ]),
        );
        assert!(got.unwrap().to_string_lossy().contains("share/signex"));
    }

    #[test]
    fn nothing_found_is_none() {
        let got = resolve_stock_library(
            Path::new("/usr/bin/signex"),
            InstallLayout::Linux,
            Path::new("/nowhere"),
            |_| false,
        );
        assert_eq!(got, None);
    }

    #[test]
    fn missing_library_does_not_panic_and_mounts_nothing() {
        let mut lib = crate::library::LibraryState::default();
        mount_at(&mut lib, None);
        let tmp = tempfile::tempdir().unwrap();
        let gone = tmp.path().join("gone/Stock.snxlib");
        mount_at(&mut lib, Some(gone.clone()));
        // A failed mount still records the root so recovery stays blocked.
        assert_eq!(lib.stock_library_root, Some(gone));
        assert!(lib.stock_library.is_none());
        assert!(lib.open_libraries.is_empty());
    }

    #[test]
    fn bundled_dev_library_mounts_as_stock() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let path = dev_path(&root);
        let mut lib = crate::library::LibraryState::default();
        mount_at(&mut lib, Some(path.clone()));
        assert_eq!(lib.stock_library, Some(path.clone()));
        assert_eq!(
            lib.mount_source_for(&path, &[]),
            crate::library::state::ComponentsMountSource::Stock
        );
    }
}
