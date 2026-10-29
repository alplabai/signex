//! Showing a datasheet in the platform's own viewer.
//!
//! What is launched is decided here, not by the caller: a link must be
//! `http(s)`, and a file must be a PDF pinned inside the library's datasheet
//! folder. Every refusal is an advice line that names the part and never
//! the path, so a hostile path cannot flood or spoof the Messages panel.

use std::path::Path;

use iced::Task;

use super::{DatasheetLocation, OpenDatasheet};
use crate::app::contracts::Message;
use crate::library::{LibraryMessage, PlacedPartMsg};

/// Open `location` for the part `part`. Blocks on the launcher: call it
/// from a worker. The error is the advice line for Messages.
pub fn open(location: &DatasheetLocation, part: &str) -> Result<(), String> {
    match location {
        DatasheetLocation::Url(url) => open_url(url, part),
        DatasheetLocation::Pinned { path, assets_dir } => open_pinned(path, assets_dir, part),
    }
}

fn open_url(url: &str, part: &str) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err(format!(
            "Fix the datasheet link of {part}: it must start with https:// or http://."
        ));
    }
    webbrowser::open(url).map_err(|e| {
        format!("Set a default browser to open the datasheet of {part}: it did not launch ({e}).")
    })
}

/// Whether `path` is exactly one plain file name directly under `assets_dir`.
/// Decided on the text alone, before any filesystem call: the library root
/// is the user's own choice (a team library may sit on `\\\\server\\libs`), but
/// the document's name comes from the row and is untrusted, so a UNC,
/// verbatim or device path, a separator or a `..` in it is refused.
fn is_plain_name_under(path: &Path, assets_dir: &Path) -> bool {
    use std::path::Component;
    let Ok(rest) = path.strip_prefix(assets_dir) else {
        return false;
    };
    let mut parts = rest.components();
    let (Some(Component::Normal(name)), None) = (parts.next(), parts.next()) else {
        return false;
    };
    let name = name.to_string_lossy();
    !name.contains(['\\', '/', ':'])
}

/// Why a pinned file may not be opened, as the end of an advice line.
///
/// It must be a plain `.pdf` name that really sits inside the library's
/// datasheet folder, must not be a symlink (the launcher would follow it to
/// whatever it names), is a regular file, and carries the PDF header
/// `store_datasheet` demands. The extension is judged again on the resolved
/// path.
fn check_pinned(path: &Path, assets_dir: &Path) -> Result<(), &'static str> {
    use std::io::Read as _;
    if !is_plain_name_under(path, assets_dir) {
        return Err("its path is not allowed");
    }
    let has_pdf_extension = |p: &Path| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
    if !has_pdf_extension(path) {
        return Err("it is not a .pdf file");
    }
    let link = std::fs::symlink_metadata(path).map_err(|_| "its file is missing")?;
    if link.file_type().is_symlink() {
        return Err("it is a link, not the pinned file");
    }
    let real = path.canonicalize().map_err(|_| "its file is missing")?;
    let root = assets_dir
        .canonicalize()
        .map_err(|_| "the library's datasheet folder is missing")?;
    if !real.starts_with(&root) {
        return Err("it is not inside the library's datasheet folder");
    }
    // Unreachable while symlinks are refused (the name decides the target);
    // kept as defence in depth.
    if !has_pdf_extension(&real) {
        return Err("it is not a .pdf file");
    }
    if !real.is_file() {
        return Err("it is not a regular file");
    }
    let mut head = Vec::new();
    std::fs::File::open(&real)
        .and_then(|file| file.take(PDF_HEAD_BYTES).read_to_end(&mut head))
        .map_err(|_| "its file cannot be read")?;
    if !signex_library::is_pdf(&head) {
        return Err("it is not a PDF");
    }
    Ok(())
}

/// How much of the file is read to look for the PDF header.
const PDF_HEAD_BYTES: u64 = 1024;

fn open_pinned(path: &Path, assets_dir: &Path, part: &str) -> Result<(), String> {
    check_pinned(path, assets_dir)
        .map_err(|why| format!("Re-pin the datasheet of {part} in its library row: {why}."))?;
    // The checked path itself, not its canonical form: that is a `\\?\`
    // verbatim path on Windows, which `explorer` handles unreliably. With
    // symlinks refused and a plain hex name, both name the same file.
    launch(path).map_err(|e| {
        format!("Install a PDF viewer to open the datasheet of {part}: it did not launch ({e}).")
    })
}

#[cfg(target_os = "macos")]
fn launch(path: &Path) -> std::io::Result<()> {
    run_launcher(std::process::Command::new("open").arg(path))
}

#[cfg(target_os = "windows")]
fn launch(path: &Path) -> std::io::Result<()> {
    // `explorer` reports exit code 1 even when it opened the file, so its
    // status is not read; a launch that cannot even start is still an error.
    std::process::Command::new("explorer")
        .arg(path)
        .spawn()
        .map(|_| ())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn launch(path: &Path) -> std::io::Result<()> {
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

/// Open `location` for `part` off the update thread with `opener`. A
/// failure comes back as [`PlacedPartMsg::DatasheetOpenFailed`]; success says
/// nothing.
pub fn open_task(
    location: DatasheetLocation,
    part: String,
    opener: OpenDatasheet,
) -> Task<Message> {
    Task::future(async move {
        tokio::task::spawn_blocking(move || opener(&location, &part))
            .await
            .unwrap_or_else(|e| Err(format!("The datasheet launcher did not finish: {e}")))
            .err()
    })
    .then(|failure| match failure {
        Some(advice) => Task::done(Message::Library(LibraryMessage::PlacedPart(
            PlacedPartMsg::DatasheetOpenFailed(advice),
        ))),
        None => Task::none(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A library folder with its datasheet folder, and a file `name` in it.
    struct Lib {
        _tmp: tempfile::TempDir,
        assets: PathBuf,
    }

    fn lib() -> Lib {
        let tmp = tempfile::tempdir().unwrap();
        let assets = tmp.path().join("assets/datasheets");
        std::fs::create_dir_all(&assets).unwrap();
        Lib { _tmp: tmp, assets }
    }

    fn pinned(path: &Path, assets: &Path) -> DatasheetLocation {
        DatasheetLocation::Pinned {
            path: path.to_path_buf(),
            assets_dir: assets.to_path_buf(),
        }
    }

    fn refusal(location: &DatasheetLocation) -> String {
        open(location, "R1").unwrap_err()
    }

    fn assert_advice_without_path(advice: &str) {
        assert!(advice.starts_with("Re-pin the datasheet of R1"), "{advice}");
        assert!(advice.len() <= 157, "{advice}");
        assert!(
            !advice.contains('/') || !advice.contains("assets"),
            "{advice}"
        );
    }

    #[test]
    fn a_missing_pinned_file_is_refused() {
        let l = lib();
        let advice = refusal(&pinned(&l.assets.join("gone.pdf"), &l.assets));
        assert_advice_without_path(&advice);
        assert!(advice.contains("missing"), "{advice}");
    }

    #[test]
    fn a_pinned_file_that_is_not_a_pdf_is_refused() {
        let l = lib();
        let path = l.assets.join("fake.pdf");
        std::fs::write(&path, b"#!/bin/sh\necho owned\n").unwrap();
        let advice = refusal(&pinned(&path, &l.assets));
        assert_advice_without_path(&advice);
        assert!(advice.contains("not a PDF"), "{advice}");
    }

    #[test]
    fn a_pinned_file_without_a_pdf_extension_is_refused() {
        let l = lib();
        let path = l.assets.join("run.command");
        std::fs::write(&path, b"%PDF-1.7 pretend").unwrap();
        let advice = refusal(&pinned(&path, &l.assets));
        assert_advice_without_path(&advice);
        assert!(advice.contains(".pdf"), "{advice}");
    }

    #[test]
    fn a_pinned_file_outside_the_datasheet_folder_is_refused() {
        let l = lib();
        let outside = l.assets.parent().unwrap().join("elsewhere.pdf");
        std::fs::write(&outside, b"%PDF-1.7 x").unwrap();
        let advice = refusal(&pinned(&outside, &l.assets));
        assert_advice_without_path(&advice);
        assert!(advice.contains("not allowed"), "{advice}");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_out_of_the_datasheet_folder_is_refused() {
        let l = lib();
        let target = l.assets.parent().unwrap().join("secret.pdf");
        std::fs::write(&target, b"%PDF-1.7 secret").unwrap();
        let link = l.assets.join("link.pdf");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let advice = refusal(&pinned(&link, &l.assets));
        assert_advice_without_path(&advice);
        assert!(advice.contains("link"), "{advice}");
    }

    #[test]
    fn unc_and_verbatim_paths_are_refused_before_any_filesystem_call() {
        for bad in [
            r"\\attacker\share\a.pdf",
            r"\\?\C:\x\a.pdf",
            r"\\.\pipe\a.pdf",
            "//attacker/share/a.pdf",
        ] {
            let advice = refusal(&pinned(Path::new(bad), Path::new("/lib/assets/datasheets")));
            assert_advice_without_path(&advice);
            assert!(advice.contains("not allowed"), "{bad}: {advice}");
        }
    }

    /// The probe of the review: `<hash>.pdf` is a symlink to a script that
    /// starts with the PDF magic. Name, folder and header all pass on the link
    /// itself; the launcher would follow it to the script.
    #[cfg(unix)]
    #[test]
    fn a_pdf_named_symlink_to_a_script_inside_the_folder_is_refused() {
        let l = lib();
        let script = l.assets.join("run.command");
        std::fs::write(&script, b"%PDF-1.7\n#!/bin/sh\necho owned\n").unwrap();
        let link = l.assets.join(format!("{}.pdf", "a".repeat(64)));
        std::os::unix::fs::symlink(&script, &link).unwrap();
        let advice = refusal(&pinned(&link, &l.assets));
        assert_advice_without_path(&advice);
    }

    #[test]
    fn a_library_on_a_network_share_is_not_refused_for_its_root() {
        // Team libraries live on `//server/libs`: the share is the user's
        // choice. Only the document's own name is untrusted.
        let root = Path::new("//server/libs/main/assets/datasheets");
        let file = root.join(format!("{}.pdf", "b".repeat(64)));
        let advice = refusal(&pinned(&file, root));
        assert!(!advice.contains("not allowed"), "{advice}");
    }

    #[test]
    fn a_document_name_that_escapes_the_folder_is_refused() {
        let root = Path::new("/lib/assets/datasheets");
        for bad in [
            "/lib/assets/datasheets/../../etc/x.pdf",
            "/lib/assets/datasheets/sub/x.pdf",
            r"/lib/assets/datasheets/\\srv\share\x.pdf",
        ] {
            let advice = refusal(&pinned(Path::new(bad), root));
            assert!(advice.contains("not allowed"), "{bad}: {advice}");
        }
    }

    #[test]
    fn a_non_web_link_is_never_handed_to_the_browser() {
        for bad in ["file:///etc/passwd", "javascript:alert(1)"] {
            let advice = open(&DatasheetLocation::Url(bad.into()), "R1").unwrap_err();
            assert!(
                advice.starts_with("Fix the datasheet link of R1"),
                "{advice}"
            );
            assert!(!advice.contains(bad), "the link is not echoed: {advice}");
        }
    }

    #[test]
    fn a_good_pinned_pdf_passes_the_checks() {
        let l = lib();
        let path = l.assets.join("ok.pdf");
        std::fs::write(&path, b"%PDF-1.7 fine").unwrap();
        assert!(check_pinned(&path, &l.assets).is_ok());
    }
}
