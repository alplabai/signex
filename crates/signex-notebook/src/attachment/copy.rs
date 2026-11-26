//! Bringing a file into the notebook (mockup row 199): a file dropped on
//! the page, picked from Figure or named on the command line is copied into
//! `notebook/attachments/<folder>/`, so the project holds everything its
//! notebook shows. The crate plans the copy and writes the call; the caller
//! copies the bytes.

use std::path::PathBuf;

use crate::reference::{Reference, quote};

use super::stale::DesignValues;
use super::{ATTACHMENTS_DIR, AttachmentKind};

/// One file to copy into the project.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AttachmentCopy {
    /// Where it is now, on the host.
    pub from: PathBuf,
    /// Where it goes, a path inside `notebook/`:
    /// `attachments/scope/ripple.csv`.
    pub to: String,
}

impl AttachmentCopy {
    /// The path the call names: inside `notebook/attachments/`.
    pub fn file(&self) -> &str {
        self.to
            .strip_prefix(&format!("{ATTACHMENTS_DIR}/"))
            .unwrap_or(&self.to)
    }
}

/// The folder of `notebook/attachments/` a file of `kind` goes to when
/// none is asked for: `scope`, `images`, `sim` or `files`.
pub fn folder_for(kind: AttachmentKind) -> &'static str {
    match kind {
        AttachmentKind::ScopeCsv => "scope",
        AttachmentKind::Image => "images",
        AttachmentKind::Simulation => "sim",
        AttachmentKind::Other => "files",
    }
}

/// Where the host file `from` is copied: `attachments/<folder>/<name>`,
/// the name made of letters, digits, `.`, `_` and `-` (anything else
/// becomes `-`), and numbered (`ripple-2.csv`) while `taken` says the path
/// is used. `None` when `from` names no file or `folder` is not one plain
/// folder name.
pub fn plan_copy(
    from: impl Into<PathBuf>,
    folder: &str,
    taken: impl Fn(&str) -> bool,
) -> Option<AttachmentCopy> {
    let from = from.into();
    let plain_folder = !folder.is_empty()
        && folder
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if !plain_folder {
        return None;
    }
    let name: String = from
        .file_name()?
        .to_string_lossy()
        .chars()
        .map(|c| match c {
            c if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' => c,
            _ => '-',
        })
        .collect();
    let name = name.trim_start_matches('.');
    if name.is_empty() {
        return None;
    }
    let (stem, extension) = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem, format!(".{ext}")),
        _ => (name, String::new()),
    };
    let to = (1u32..=9999)
        .map(|n| match n {
            1 => format!("{ATTACHMENTS_DIR}/{folder}/{stem}{extension}"),
            n => format!("{ATTACHMENTS_DIR}/{folder}/{stem}-{n}{extension}"),
        })
        .find(|path| !taken(path))?;
    Some(AttachmentCopy { from, to })
}

/// The `#attachment(...)` call for `file` (a path inside
/// `notebook/attachments/`), naming `targets` and storing the current value
/// of every part they name, as the editor inserts it.
pub fn attachment_call(
    file: &str,
    caption: &str,
    targets: &[Reference],
    values: &DesignValues,
) -> String {
    let mut args = vec![quote(file)];
    if !caption.trim().is_empty() {
        args.push(format!("caption: {}", quote(caption.trim())));
    }
    match targets {
        [] => {}
        [one] => args.push(format!("target: {}", one.to_code())),
        many => {
            let codes: Vec<String> = many.iter().map(Reference::to_code).collect();
            args.push(format!("target: ({})", codes.join(", ")));
        }
    }
    let mut parts: Vec<&str> = Vec::new();
    for part in targets.iter().filter_map(Reference::component) {
        if !parts.contains(&part) {
            parts.push(part);
        }
    }
    let stored: Vec<String> = parts
        .iter()
        .filter_map(|part| Some(format!("{}: {}", quote(part), quote(values.get(part)?))))
        .collect();
    if !stored.is_empty() {
        args.push(format!("taken-with: ({})", stored.join(", ")));
    }
    format!("#attachment({})", args.join(", "))
}
