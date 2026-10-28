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

/// What the off-thread stock prepare hands back: the resolved path and the
/// outcome of preparing it as a read-only mount, or `None` when no stock
/// library was found (already warned).
pub type StockMount = Option<(PathBuf, crate::library::mount::PreparedMountCell)>;

/// Resolve and prepare the stock library off the UI thread: both the
/// path probe and the manifest/table reads touch the disk.
pub async fn prepare_stock_off_thread() -> StockMount {
    tokio::task::spawn_blocking(|| {
        let Some(path) = stock_library_path() else {
            tracing::warn!(
                target: "signex::library",
                "stock library not found next to the executable; Stock section left empty"
            );
            return None;
        };
        // A failure travels too: the path is recorded either way, so the
        // recovery flows stay blocked against a stock library that is there
        // but broken.
        let prepared = crate::library::mount::prepare_read_only_mount(&path);
        Some((
            path,
            crate::library::mount::PreparedMountCell::new(prepared),
        ))
    })
    .await
    .unwrap_or(None)
}

/// Bootstrap entry point: prepare the stock library in the background; it
/// lands as `LibraryMessage::StockMountFinished`.
pub fn spawn_stock_mount() -> iced::Task<crate::app::Message> {
    iced::Task::perform(prepare_stock_off_thread(), |mount| {
        crate::app::Message::Library(crate::library::LibraryMessage::StockMountFinished(mount))
    })
}

/// Mount a prepared stock library onto `library_state`, read-only. Cheap (no
/// disk IO). Returns the library id when it newly mounted; never fails the
/// caller — an unmountable library warns and leaves the Stock section empty.
pub fn finish_mount(
    library_state: &mut crate::library::LibraryState,
    mount: StockMount,
) -> Option<uuid::Uuid> {
    let (path, cell) = mount?;
    library_state.stock_library_root = Some(path.clone());
    let prepared = match cell.take()? {
        Ok(prepared) => prepared,
        Err(e) => {
            tracing::warn!(
                target: "signex::library",
                path = %path.display(),
                error = %e,
                "stock library mount failed; Stock section left empty"
            );
            return None;
        }
    };
    let id = prepared.entry.library_id;
    let newly = library_state.library_at(&path).is_none();
    match library_state.mount_prepared(prepared) {
        Ok(()) => {
            library_state.stock_library = Some(path);
            newly.then_some(id)
        }
        Err(e) => {
            tracing::warn!(
                target: "signex::library",
                path = %path.display(),
                error = %e,
                "stock library mount failed; Stock section left empty"
            );
            None
        }
    }
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
        assert!(finish_mount(&mut lib, None).is_none());
        let tmp = tempfile::tempdir().unwrap();
        let gone = tmp.path().join("gone/Stock.snxlib");
        let mut failed = Some((
            gone.clone(),
            crate::library::mount::PreparedMountCell::new(
                crate::library::mount::prepare_read_only_mount(&gone),
            ),
        ));
        assert!(finish_mount(&mut lib, failed.take()).is_none());
        // A failed mount still records the root so recovery stays blocked.
        assert_eq!(lib.stock_library_root, Some(gone));
        assert!(lib.stock_library.is_none());
        assert!(lib.open_libraries.is_empty());
    }

    #[test]
    fn bundled_dev_library_mounts_as_stock_read_only() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let path = dev_path(&root);
        let mut lib = crate::library::LibraryState::default();
        let prepared = crate::library::mount::prepare_read_only_mount(&path).unwrap();
        let mount = Some((
            path.clone(),
            crate::library::mount::PreparedMountCell::new(Ok(prepared)),
        ));
        assert!(finish_mount(&mut lib, mount).is_some());
        assert_eq!(lib.stock_library, Some(path.clone()));
        assert!(lib.is_read_only(&path));
        assert_eq!(
            lib.mount_source_for(&path, &[]),
            crate::library::state::ComponentsMountSource::Stock
        );
    }
}
