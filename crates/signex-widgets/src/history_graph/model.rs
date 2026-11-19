//! Plain data the history graph draws: commits with their parents and ref
//! pills, and the changed files of the detail strip. No git types: the
//! caller converts its history rows at the boundary, as for
//! [`crate::history_pane::HistoryEntry`].

use chrono::{DateTime, Utc};

/// One commit row, newest first in the list the graph takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphCommit {
    /// Full commit SHA; rows show the first seven characters.
    pub sha: String,
    /// Parent SHAs, first parent first. More than one makes a merge.
    pub parents: Vec<String>,
    /// Branch, tag and HEAD pills sitting on this commit, in display order.
    pub refs: Vec<RefPill>,
    pub author_name: String,
    /// Hashed into the colour of the author's initials disc.
    pub author_email: String,
    /// Author timestamp.
    pub time: DateTime<Utc>,
    /// First line of the commit message.
    pub subject: String,
}

impl GraphCommit {
    /// A commit with more than one parent.
    pub fn is_merge(&self) -> bool {
        self.parents.len() > 1
    }

    /// The commit the working tree is checked out at.
    pub fn is_head(&self) -> bool {
        self.refs.iter().any(|r| matches!(r, RefPill::Head))
    }
}

/// A pill on a commit row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefPill {
    /// The checked-out commit.
    Head,
    /// A branch tip. The current branch is drawn in the accent; any other
    /// branch in the colour of the lane its tip sits in.
    Branch {
        name: String,
        is_current: bool,
    },
    Tag {
        name: String,
    },
}

/// How a file changed in a commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
}

/// One line of the detail strip's file list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub status: FileStatus,
    /// Path relative to the project folder.
    pub path: String,
}

/// Up to two initials for the author disc: the first letters of the first
/// and last words of the name ("j.berg" gives "JB"), or of the e-mail's
/// local part when the name has no letters. Empty when neither has any.
pub fn initials(name: &str, email: &str) -> String {
    let from = |s: &str| -> String {
        let words: Vec<&str> = s
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect();
        let first = words.first().and_then(|w| w.chars().next());
        let last = match words.as_slice() {
            [_, .., last] => last.chars().next(),
            _ => None,
        };
        first
            .into_iter()
            .chain(last)
            // Avatar initials, not translatable text: the letters come from a name.
            // ast-grep-ignore: no-case-in-view
            .flat_map(char::to_uppercase)
            .collect()
    };
    let by_name = from(name);
    if !by_name.is_empty() {
        return by_name;
    }
    from(email.split('@').next().unwrap_or_default())
}

/// A stable index into a palette of `len` colours for an author: FNV-1a
/// over the trimmed, lower-cased e-mail, so one author keeps one colour on
/// every machine and in every run. `len` must not be zero.
pub fn author_color_index(email: &str, len: usize) -> usize {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    // A hash key, never shown, so no language case rules apply.
    // ast-grep-ignore: no-case-in-view
    let hash = email
        .trim()
        .to_ascii_lowercase()
        .bytes()
        .fold(OFFSET, |h, b| (h ^ u64::from(b)).wrapping_mul(PRIME));
    (hash % len as u64) as usize
}
