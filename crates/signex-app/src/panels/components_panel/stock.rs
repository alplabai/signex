//! Bundled stock library: locate it next to the executable and mount it.
//!
//! The Stock section is not user-configured state, so nothing here reads or
//! writes `global_libraries.toml`. The stock library mounts read-only and
//! the app never writes to it, except in the maintainer mode below.
//!
//! Maintainer mode (#1892): with `SIGNEX_EDIT_STOCK=1` set, a build run from
//! a source checkout mounts `assets/stock-library/Stock.snxlib` writable, so
//! the stock parts are edited in the normal editors and the saved files go
//! out by PR. An installed copy is never made writable: the next update
//! would overwrite the edits.
//!
//! Install layouts (the installer packaging agrees on exactly these):
//! - Windows: `<exe dir>\library\Stock.snxlib`
//! - Linux: `<exe dir>/../share/signex/library/Stock.snxlib`
//! - macOS: `<exe dir>/../Resources/library/Stock.snxlib`
//! - dev checkout: `assets/stock-library/Stock.snxlib`

use std::path::{Path, PathBuf};

const STOCK_FILE: &str = "Stock.snxlib";

/// Environment variable that asks for the maintainer mode; `1` turns it on.
pub const EDIT_STOCK_ENV: &str = "SIGNEX_EDIT_STOCK";

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

/// Whether the stock library at `path` mounts writable: the maintainer
/// asked for it and `path` is a source checkout's copy, never an
/// installed one.
pub fn is_stock_editable(path: &Path, requested: bool) -> bool {
    requested && path.ends_with(Path::new("assets").join("stock-library").join(STOCK_FILE))
}

/// Whether the maintainer mode was asked for in the environment.
fn edit_stock_requested() -> bool {
    std::env::var_os(EDIT_STOCK_ENV).is_some_and(|v| v == "1")
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

/// Locate the bundled stock library and register it with `library_state`
/// before any other mount can race it: its root is recorded (so recovery
/// flows and the read-only guard recognise it whether or not it opens) and a
/// mount is requested. Returns the path when the caller must spawn the
/// preparation. Cheap: a few `exists()` probes, no manifest or table reads.
/// A missing library warns and leaves the Stock section empty.
pub fn register_stock(library_state: &mut crate::library::LibraryState) -> Option<PathBuf> {
    let Some(path) = stock_library_path() else {
        tracing::warn!(
            target: "signex::library",
            "stock library not found next to the executable; Stock section left empty"
        );
        return None;
    };
    let editable = is_stock_editable(&path, edit_stock_requested());
    if editable {
        tracing::warn!(
            target: "signex::library",
            path = %path.display(),
            "{EDIT_STOCK_ENV}=1: the stock library mounts writable; saves change what ships"
        );
    }
    register_stock_at(library_state, path, editable)
}

fn register_stock_at(
    library_state: &mut crate::library::LibraryState,
    path: PathBuf,
    editable: bool,
) -> Option<PathBuf> {
    use crate::library::mount::{MountIntent, MountRequest};
    library_state.stock_library_root = Some(path.clone());
    library_state.stock_editable = editable;
    (library_state.request_mount(&path, MountIntent::Silent) == MountRequest::Spawn).then_some(path)
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
    fn registering_records_the_root_and_asks_for_one_mount() {
        let mut lib = crate::library::LibraryState::default();
        let path = PathBuf::from("/nowhere/Stock.snxlib");
        assert_eq!(
            register_stock_at(&mut lib, path.clone(), false),
            Some(path.clone())
        );
        assert_eq!(lib.stock_library_root, Some(path.clone()));
        assert!(lib.open_libraries.is_empty(), "nothing is read here");
        assert!(lib.is_read_only_target(&path), "guarded before it lands");
        assert_eq!(
            register_stock_at(&mut lib, path, false),
            None,
            "already requested"
        );
    }

    /// A writable mount of the stock file (a global entry, a project
    /// library) that lands first must still mount read-only.
    #[test]
    fn a_writable_prepare_of_the_stock_file_mounts_read_only() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let path = dev_path(&root);
        let mut lib = crate::library::LibraryState::default();
        register_stock_at(&mut lib, path.clone(), false);
        let prepared = crate::library::mount::prepare_mount(&path).unwrap();
        lib.mount_prepared(prepared).unwrap();
        assert!(lib.is_read_only(&path));
        assert_eq!(lib.stock_library, Some(path.clone()));
        assert_eq!(
            lib.mount_source_for(&path, &[]),
            crate::library::state::ComponentsMountSource::Stock
        );
    }

    /// The stock root ranks as Stock before it mounts, ahead of Project.
    #[test]
    fn a_stock_root_still_mounting_ranks_as_stock() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let path = dev_path(&root);
        let mut lib = crate::library::LibraryState::default();
        register_stock_at(&mut lib, path.clone(), false);
        assert!(lib.stock_library.is_none());
        assert_eq!(
            lib.mount_source_for(&path, std::slice::from_ref(&path)),
            crate::library::state::ComponentsMountSource::Stock
        );
    }

    #[test]
    fn a_synchronous_open_of_the_stock_file_is_read_only_too() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let path = dev_path(&root);
        let mut lib = crate::library::LibraryState::default();
        register_stock_at(&mut lib, path.clone(), false);
        lib.open_library(path.clone()).unwrap();
        assert!(lib.is_read_only(&path));
    }

    /// #1892: the maintainer mode needs both the request and a source
    /// checkout's copy; an installed copy never becomes writable.
    #[test]
    fn only_a_requested_source_checkout_copy_is_editable() {
        let dev = Path::new("/work/signex/assets/stock-library/Stock.snxlib");
        let installed = Path::new("C:/Signex/library/Stock.snxlib");
        assert!(is_stock_editable(dev, true));
        assert!(!is_stock_editable(dev, false));
        assert!(!is_stock_editable(installed, true));
        assert!(!is_stock_editable(installed, false));
    }

    /// #1892: in maintainer mode the stock library mounts writable, stays
    /// the Stock section, and its primitives open directly for editing.
    #[test]
    fn an_editable_stock_library_mounts_writable_and_opens_its_files() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let path = dev_path(&root);
        let mut lib = crate::library::LibraryState::default();
        register_stock_at(&mut lib, path.clone(), true);
        let prepared = crate::library::mount::prepare_mount(&path).unwrap();
        lib.mount_prepared(prepared).unwrap();
        assert!(!lib.is_read_only(&path));
        assert_eq!(lib.stock_library, Some(path.clone()));
        assert_eq!(
            lib.mount_source_for(&path, &[]),
            crate::library::state::ComponentsMountSource::Stock
        );
        let symbol = path.parent().unwrap().join("symbols").join("R.snxsym");
        assert_eq!(
            lib.stock_primitive_kind(&symbol, crate::app::CaseFold::host_default()),
            None,
            "no copy detour"
        );
        assert!(
            lib.is_read_only_target(&path),
            "recovery flows (git re-init, binding repair) still never run on it"
        );
    }
}
