//! The agent edits a save commit lists in its message body (#2253), when the
//! project's "Summarise agent changes in the commit message" is on. Per
//! committed file: only the edits of that file since the last commit that
//! touched it. The subject and the author are not touched here. The rules and
//! their limits are in `docs/AGENT_JOURNALS.md`, "Commit message summary".

use std::fmt::Display;
use std::path::Path;

use chrono::{DateTime, Utc};

use super::append::Journal;
use super::entry::EditEntry;
use super::read::read_journal;

/// The most edits one body lists; the rest are counted in a last line.
pub const MAX_LISTED: usize = 20;

/// The longest any one free text of a line is, in characters.
const TEXT_CHARS: usize = 120;

const HEADER: &str = "Agent edits since the previous commit:";

/// Commands whose change lands in the project file, whatever sheet they name.
const PROJECT_WIDE: [&str; 1] = ["assign_net_class"];

/// `text` as one safe line fragment: control characters (a newline included)
/// become spaces and it is cut to length, so no field can start a line, a
/// trailer or a second paragraph.
fn clean(text: &str) -> String {
    let spaced: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(TEXT_CHARS)
        .collect();
    spaced.trim().to_string()
}

/// Is `entry` an edit of `file` (an absolute path)? A project-wide command, or
/// one that names no document, belongs to the project file's commit.
fn is_edit_of(entry: &EditEntry, file: &Path) -> bool {
    let is_project_file = file.extension().is_some_and(|ext| ext == "snxprj");
    let project_wide = PROJECT_WIDE.contains(&entry.command.as_str());
    match entry.path.as_deref().map(Path::new) {
        Some(path) if !project_wide => path == file,
        Some(path) => is_project_file && file.parent().is_some_and(|dir| path.starts_with(dir)),
        None => is_project_file,
    }
}

fn line(entry: &EditEntry, project_root: &Path) -> String {
    let who = match (&entry.client, entry.pid) {
        (Some(client), _) => clean(client),
        (None, Some(pid)) => format!("pid={pid}"),
        (None, None) => "unknown".to_string(),
    };
    let what = match (&entry.path, &entry.document) {
        (Some(path), _) => {
            let path = Path::new(path);
            let shown = path.strip_prefix(project_root).unwrap_or(path);
            clean(&shown.display().to_string())
        }
        (None, Some(document)) => clean(document),
        (None, None) => "project".to_string(),
    };
    let reason = entry
        .reason
        .as_deref()
        .map(|reason| format!(" ({})", clean(reason)))
        .unwrap_or_default();
    format!(
        "- {} client={who} command={} doc={what} outcome={}{reason}",
        clean(&entry.ts),
        clean(&entry.command),
        clean(&entry.outcome),
    )
}

/// The body for `file`, and how many lines had a time that could not be read
/// (skipped). `since` is the last commit of `file` in whole seconds; `None`
/// (never committed) lists every edit of it. `None` when nothing is listed.
fn body(
    entries: &[EditEntry],
    file: &Path,
    project_root: &Path,
    since: Option<i64>,
) -> (Option<String>, usize) {
    let mut unreadable = 0;
    let mut listed = Vec::new();
    for entry in entries.iter().filter(|entry| is_edit_of(entry, file)) {
        let Ok(at) = DateTime::parse_from_rfc3339(&entry.ts) else {
            unreadable += 1;
            continue;
        };
        if since.is_none_or(|since| at.timestamp() > since) {
            listed.push(entry);
        }
    }
    if listed.is_empty() {
        return (None, unreadable);
    }
    let hidden = listed.len().saturating_sub(MAX_LISTED);
    let mut lines = vec![HEADER.to_string()];
    lines.extend(
        listed[hidden..]
            .iter()
            .map(|entry| line(entry, project_root)),
    );
    if hidden > 0 {
        lines.push(format!("and {hidden} more earlier edit(s) not listed"));
    }
    (Some(lines.join("\n")), unreadable)
}

fn warn(text: String) {
    crate::diagnostics::log_warning(format!("[git] {text}"));
}

/// `message`, with the agent edits of `rel_path` since its last commit added
/// as the body. `last_commit` is the answer of the git adapter. The message
/// comes back unchanged when there are no such edits, and also when the journal
/// or the last commit cannot be read: the save stands, Messages says why. Runs
/// off the UI thread (it reads the journal).
pub fn message_with_agent_summary<E: Display>(
    project_root: &Path,
    rel_path: &Path,
    message: &str,
    last_commit: Result<Option<DateTime<Utc>>, E>,
) -> String {
    let since = match last_commit {
        Ok(at) => at.map(|at| at.timestamp()),
        Err(error) => {
            warn(format!(
                "The commit of {} was made without the agent edit summary: its last commit could not be read: {error}",
                rel_path.display()
            ));
            return message.to_string();
        }
    };
    let journal = project_root
        .join(".signex")
        .join(Journal::Edits.file_name());
    let report = match read_journal::<EditEntry>(&journal) {
        Ok(report) => report,
        Err(error) => {
            warn(format!(
                "The commit of {} was made without the agent edit summary: {error}",
                rel_path.display()
            ));
            return message.to_string();
        }
    };
    for problem in report.problems(&journal) {
        warn(problem);
    }
    let file = project_root.join(rel_path);
    let (body, unreadable) = body(&report.entries, &file, project_root, since);
    if unreadable > 0 {
        warn(format!(
            "{} has {unreadable} line(s) with a time that cannot be read; they were not listed",
            journal.display()
        ));
    }
    match body {
        Some(body) => format!("{message}\n\n{body}"),
        None => message.to_string(),
    }
}
