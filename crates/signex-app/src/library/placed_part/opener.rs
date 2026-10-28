//! Showing a datasheet in the platform's own viewer.

use std::path::Path;

use iced::Task;

use super::{DatasheetLocation, OpenDatasheet};
use crate::app::contracts::Message;
use crate::library::{LibraryMessage, PlacedPartMsg};

/// Open `location`. Blocks on the launcher: call it from a worker. The
/// error is the advice line for Messages, the location last.
pub fn open(location: &DatasheetLocation) -> Result<(), String> {
    match location {
        DatasheetLocation::Url(url) => open_url(url),
        DatasheetLocation::File(path) => open_file(path),
    }
}

fn open_url(url: &str) -> Result<(), String> {
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err(format!(
            "Fix the datasheet link: it must start with https:// or http://, not: {url}"
        ));
    }
    webbrowser::open(url).map_err(|e| {
        format!("Set a default browser to read datasheets: it did not launch ({e}): {url}")
    })
}

fn open_file(path: &Path) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!(
            "Restore the datasheet file or link it again: it is missing: {}",
            path.display()
        ));
    }
    launch(path).map_err(|e| {
        format!(
            "Install a PDF viewer to read datasheets: it did not launch ({e}): {}",
            path.display()
        )
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

/// Open `location` off the update thread with `opener`. A failure comes
/// back as [`PlacedPartMsg::DatasheetOpenFailed`]; success says nothing.
pub fn open_task(location: DatasheetLocation, opener: OpenDatasheet) -> Task<Message> {
    Task::future(async move {
        tokio::task::spawn_blocking(move || opener(&location))
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

    #[test]
    fn a_missing_file_says_to_restore_it_before_any_launch() {
        let path = PathBuf::from("/definitely/not/here/ds.pdf");
        let advice = open(&DatasheetLocation::File(path.clone())).unwrap_err();
        assert!(advice.starts_with("Restore the datasheet file"), "{advice}");
        assert!(advice.ends_with(&path.display().to_string()), "{advice}");
    }

    #[test]
    fn a_non_web_link_is_never_handed_to_the_browser() {
        let advice = open(&DatasheetLocation::Url("file:///etc/passwd".into())).unwrap_err();
        assert!(advice.starts_with("Fix the datasheet link"), "{advice}");
    }
}
