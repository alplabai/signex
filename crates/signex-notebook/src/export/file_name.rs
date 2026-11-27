//! Where a notebook PDF goes by default (DECISIONS, Caner 2026-11-25, mockup
//! row 202): `<project>-<document>-rev<R>.pdf` in `Project Outputs/notebook/`.

use super::title::TitleBlock;

/// The folders, under the project folder, a notebook PDF is written to.
pub const OUTPUT_FOLDERS: [&str; 2] = ["Project Outputs", "notebook"];

/// The default file name: `buck-3v3-DN-0042-revB.pdf` for project
/// `buck-3v3`, document `DN-0042`, revision `B`. With no document number the
/// notebook file's stem stands in (`design`); with no revision the `-rev`
/// part is left out. A character a file name cannot hold becomes `_`.
pub fn default_file_name(project: &str, notebook_file: &str, title: &TitleBlock) -> String {
    let name = notebook_file.rsplit('/').next().unwrap_or(notebook_file);
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    let document = title.doc.as_deref().unwrap_or(stem);
    let mut parts: Vec<String> = [project, document]
        .into_iter()
        .map(file_safe)
        .filter(|part| !part.is_empty())
        .collect();
    if let Some(rev) = title
        .rev
        .as_deref()
        .map(file_safe)
        .filter(|r| !r.is_empty())
    {
        parts.push(format!("rev{rev}"));
    }
    if parts.is_empty() {
        parts.push("notebook".to_string());
    }
    format!("{}.pdf", parts.join("-"))
}

/// `text` with every character a file name on Windows, macOS or Linux
/// cannot hold replaced by `_`, and surrounding spaces and dots dropped.
fn file_safe(text: &str) -> String {
    let replaced: String = text
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    replaced.trim_matches([' ', '.']).to_string()
}
