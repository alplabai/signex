//! `signex-cli agent-log <project> [--access]` (#2253): print a project's agent
//! journal as a table. Read-only: it opens the journal without following a
//! link, skips a partial last line and lines of a newer version, and says so
//! on standard error; it never writes anything. See `docs/AGENT_JOURNALS.md`.

use std::io::Write;
use std::path::{Path, PathBuf};

use signex_app::headless::agent_journal::{AccessEntry, EditEntry, Journal, read_journal};

/// Which journal to print.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    Edits,
    Access,
}

/// The folder whose `.signex/` holds the journals: the project's folder, given
/// as the `.snxprj` or as the folder itself.
fn journal_dir(project: &Path) -> Result<PathBuf, String> {
    if project.is_dir() {
        return Ok(project.to_path_buf());
    }
    project
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .or_else(|| Some(PathBuf::from(".")))
        .ok_or_else(|| format!("{} has no folder", project.display()))
}

/// Print the journal to `out`; what the reader skipped goes to `problems`.
pub fn run(
    project: &Path,
    which: Which,
    out: &mut impl Write,
    problems: &mut impl Write,
) -> Result<(), String> {
    let dir = journal_dir(project)?;
    let journal = match which {
        Which::Edits => Journal::Edits,
        Which::Access => Journal::Access,
    };
    let path = dir.join(".signex").join(journal.file_name());
    let (table, skipped) = match which {
        Which::Edits => {
            let report = read_journal::<EditEntry>(&path)?;
            (edits_table(&report.entries), report.problems(&path))
        }
        Which::Access => {
            let report = read_journal::<AccessEntry>(&path)?;
            (access_table(&report.entries), report.problems(&path))
        }
    };
    for line in skipped {
        writeln!(problems, "signex-cli: {line}").map_err(|error| error.to_string())?;
    }
    out.write_all(table.as_bytes())
        .map_err(|error| format!("writing to standard output: {error}"))
}

fn who(pid: Option<u32>, client: Option<&String>) -> String {
    match (client, pid) {
        (Some(client), Some(pid)) => format!("{client} ({pid})"),
        (Some(client), None) => client.clone(),
        (None, Some(pid)) => format!("pid {pid}"),
        (None, None) => "?".to_string(),
    }
}

fn file_name(path: Option<&String>) -> String {
    path.and_then(|path| Path::new(path).file_name())
        .map_or_else(
            || "-".to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
}

fn revisions(entry: &EditEntry) -> String {
    let show = |revision: Option<u64>| revision.map_or_else(|| "-".to_string(), |r| r.to_string());
    format!(
        "{} > {}",
        show(entry.revision_before),
        show(entry.revision_after)
    )
}

/// The edit journal as a table, oldest first.
pub fn edits_table(entries: &[EditEntry]) -> String {
    let mut rows = vec![row([
        "TIME", "AGENT", "COMMAND", "DOCUMENT", "REVISION", "GROUP", "OUTCOME",
    ])];
    for entry in entries {
        let outcome = match (&entry.reason, entry.truncated) {
            (Some(reason), _) => format!("{}: {reason}", entry.outcome),
            (None, true) => format!("{} (truncated)", entry.outcome),
            (None, false) => entry.outcome.clone(),
        };
        rows.push(row([
            entry.ts.as_str(),
            who(entry.pid, entry.client.as_ref()).as_str(),
            entry.command.as_str(),
            file_name(entry.path.as_ref()).as_str(),
            revisions(entry).as_str(),
            entry
                .group
                .map_or_else(|| "-".to_string(), |g| g.to_string())
                .as_str(),
            outcome.as_str(),
        ]));
    }
    aligned(&rows)
}

/// The access journal as a table, oldest first.
pub fn access_table(entries: &[AccessEntry]) -> String {
    let mut rows = vec![row(["TIME", "EVENT", "MODE", "AGENT", "DETAIL"])];
    for entry in entries {
        let mut detail = Vec::new();
        if !entry.capabilities.is_empty() {
            detail.push(entry.capabilities.join(","));
        }
        if let Some(reason) = &entry.reason {
            detail.push(reason.clone());
        }
        rows.push(row([
            entry.ts.as_str(),
            entry.event.as_str(),
            entry.mode.as_str(),
            who(entry.pid, entry.client.as_ref()).as_str(),
            detail.join("; ").as_str(),
        ]));
    }
    aligned(&rows)
}

fn row<const N: usize>(cells: [&str; N]) -> Vec<String> {
    cells.iter().map(|cell| single_line(cell)).collect()
}

/// A cell on one line: a newline or a control character would break the table
/// (and, for a terminal, could do worse), so each becomes a `?`.
fn single_line(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

/// Rows with each column padded to its widest cell; the last column is not.
fn aligned(rows: &[Vec<String>]) -> String {
    let columns = rows.first().map_or(0, Vec::len);
    let widths: Vec<usize> = (0..columns)
        .map(|column| {
            rows.iter()
                .map(|row| row[column].chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    let mut out = String::new();
    for row in rows {
        for (column, cell) in row.iter().enumerate() {
            if column + 1 == columns {
                out.push_str(cell);
            } else {
                out.push_str(&format!("{cell:<width$}  ", width = widths[column]));
            }
        }
        out.push('\n');
    }
    out
}
