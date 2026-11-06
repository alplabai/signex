//! The native adapters: the only code that opens an OS file dialog, a
//! browser or a launcher process (`scripts/check_io_ports.sh`).

#![allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    reason = "the native adapter is the one place that may open dialogs, browsers and launchers"
)]

use std::path::{Path, PathBuf};

use super::FileDialog;

fn rfd_dialog(dialog: &FileDialog) -> rfd::AsyncFileDialog {
    let mut native = rfd::AsyncFileDialog::new().set_title(&dialog.title);
    for (name, extensions) in &dialog.filters {
        native = native.add_filter(name, extensions);
    }
    if let Some(dir) = &dialog.directory {
        native = native.set_directory(dir);
    }
    if let Some(name) = &dialog.file_name {
        native = native.set_file_name(name);
    }
    native
}

pub(super) async fn pick_file(dialog: &FileDialog) -> Option<PathBuf> {
    let file = rfd_dialog(dialog).pick_file().await?;
    Some(file.path().to_path_buf())
}

pub(super) async fn pick_files(dialog: &FileDialog) -> Option<Vec<PathBuf>> {
    let files = rfd_dialog(dialog).pick_files().await?;
    Some(files.iter().map(|file| file.path().to_path_buf()).collect())
}

pub(super) async fn pick_folder(dialog: &FileDialog) -> Option<PathBuf> {
    let folder = rfd_dialog(dialog).pick_folder().await?;
    Some(folder.path().to_path_buf())
}

pub(super) async fn save_file(dialog: &FileDialog) -> Option<PathBuf> {
    let file = rfd_dialog(dialog).save_file().await?;
    Some(file.path().to_path_buf())
}

pub(super) fn browse(url: &str) -> std::io::Result<()> {
    webbrowser::open(url)
}

#[cfg(target_os = "macos")]
pub(super) fn view(path: &Path) -> std::io::Result<()> {
    run_launcher(std::process::Command::new("open").arg(path))
}

#[cfg(target_os = "windows")]
pub(super) fn view(path: &Path) -> std::io::Result<()> {
    // `explorer` reports exit code 1 even when it opened the file, so its
    // status is not read; a launch that cannot even start is still an error.
    std::process::Command::new("explorer")
        .arg(path)
        .spawn()
        .map(|_| ())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub(super) fn view(path: &Path) -> std::io::Result<()> {
    run_launcher(std::process::Command::new("xdg-open").arg(path))
}

#[cfg(not(target_os = "windows"))]
fn run_launcher(command: &mut std::process::Command) -> std::io::Result<()> {
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("exit status {status}")))
    }
}

/// Show `path` in the file manager: Explorer and Finder select the file,
/// Linux opens the parent folder, as most file managers take no file to
/// select.
pub(super) fn reveal(path: &Path) -> std::io::Result<()> {
    use std::process::Command;

    let spawned = |launcher: &str, result: std::io::Result<std::process::Child>| {
        result
            .map(|_| ())
            .map_err(|e| std::io::Error::new(e.kind(), format!("{launcher} failed to spawn: {e}")))
    };

    #[cfg(target_os = "windows")]
    {
        // `explorer /select,<path>` opens the containing folder with the
        // file highlighted; a folder is just opened.
        let arg = if path.is_dir() {
            path.as_os_str().to_owned()
        } else {
            let mut s = std::ffi::OsString::from("/select,");
            s.push(path.as_os_str());
            s
        };
        spawned("explorer.exe", Command::new("explorer").arg(arg).spawn())
    }

    #[cfg(target_os = "macos")]
    {
        // `open -R <path>` reveals the file in Finder; plain `open <dir>`
        // opens a folder.
        let mut cmd = Command::new("open");
        if path.is_file() {
            cmd.arg("-R");
        }
        spawned("open", cmd.arg(path).spawn())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let target = if path.is_file() {
            path.parent().unwrap_or(path)
        } else {
            path
        };
        spawned("xdg-open", Command::new("xdg-open").arg(target).spawn())
    }
}
