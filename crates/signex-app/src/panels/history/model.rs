//! The History panel's repository view: the graph as read from git, what the
//! filter chips and search keep of it, and the panel's messages. Pure: the
//! reads run in `app::runtime::history_repo`.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use signex_library::repo_branches::SwitchFailure;
use signex_library::repo_graph::{
    RepoFileChange, RepoFileChangeKind, RepoRefKind, RepoRefTip, RepoSnapshot,
};
use signex_widgets::branch_switcher::BranchRow;
use signex_widgets::history_graph::{ChangedFile, FileStatus, GraphCommit, RefPill};

/// Which branches the graph lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HistoryBranchScope {
    #[default]
    All,
    Current,
}

/// The filter chips and the search text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HistoryFilter {
    pub scope: HistoryBranchScope,
    pub this_file_only: bool,
    pub search: String,
}

/// The repository as one load read it for the panel's file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HistoryRepoView {
    /// The repository's working directory.
    pub root: PathBuf,
    /// The panel's file, relative to `root`, forward slashes.
    pub tracked: String,
    /// Newest first, every branch.
    pub commits: Vec<GraphCommit>,
    pub on_head: HashSet<String>,
    pub touching: HashSet<String>,
    /// Commits that hold the file, so Restore has something to write.
    pub restorable: HashSet<String>,
    pub branches: Vec<BranchRow>,
    /// `None` on a detached HEAD.
    pub current_branch: Option<String>,
}

/// The pills of `sha`: branches with the current one first, HEAD, then tags.
fn pills_of(sha: &str, tips: &[RepoRefTip], head: Option<&str>) -> Vec<RefPill> {
    let here = || tips.iter().filter(move |tip| tip.sha == sha);
    let branch = |tip: &RepoRefTip, is_current: bool| RefPill::Branch {
        name: tip.name.clone(),
        is_current,
    };
    let current = here().filter(|t| t.kind == RepoRefKind::Branch { is_current: true });
    let others = here().filter(|t| t.kind == RepoRefKind::Branch { is_current: false });
    let mut pills: Vec<RefPill> = current.map(|t| branch(t, true)).collect();
    pills.extend(others.map(|t| branch(t, false)));
    if head == Some(sha) {
        pills.push(RefPill::Head);
    }
    pills.extend(
        here()
            .filter(|t| t.kind == RepoRefKind::Tag)
            .map(|t| RefPill::Tag {
                name: t.name.clone(),
            }),
    );
    pills
}

impl HistoryRepoView {
    /// Convert a snapshot at the library boundary.
    pub fn from_snapshot(root: PathBuf, tracked: String, snapshot: RepoSnapshot) -> Self {
        let head = snapshot.head_sha.as_deref();
        let mut view = Self {
            root,
            tracked,
            current_branch: snapshot.current_branch.clone(),
            ..Self::default()
        };
        for c in &snapshot.commits {
            if c.on_head {
                view.on_head.insert(c.sha.clone());
            }
            if c.touches_file {
                view.touching.insert(c.sha.clone());
            }
            if c.has_file {
                view.restorable.insert(c.sha.clone());
            }
            view.commits.push(GraphCommit {
                sha: c.sha.clone(),
                parents: c.parents.clone(),
                refs: pills_of(&c.sha, &snapshot.tips, head),
                author_name: c.author_name.clone(),
                author_email: c.author_email.clone(),
                time: c.time,
                subject: c.subject.clone(),
            });
        }
        view.branches = snapshot
            .branches
            .iter()
            .map(|b| BranchRow {
                name: b.name.clone(),
                is_remote: b.is_remote,
                is_current: b.is_current,
                ahead: b.ahead,
                behind: b.behind,
            })
            .collect();
        view
    }

    /// Local branches holding commits the current branch lacks.
    pub fn branches_ahead(&self) -> usize {
        self.branches
            .iter()
            .filter(|b| !b.is_remote && !b.is_current && b.ahead > 0)
            .count()
    }
}

/// Whether `commit` matches the search: subject or author, ignoring case,
/// or the start of its SHA.
fn matches_search(commit: &GraphCommit, needle: &str) -> bool {
    needle.is_empty()
        || search_key(&commit.subject).contains(needle)
        || search_key(&commit.author_name).contains(needle)
        || commit.sha.starts_with(needle)
}

/// A search key, never shown, so no language case rules apply.
fn search_key(text: &str) -> String {
    // ast-grep-ignore: no-case-in-view
    text.to_lowercase()
}

/// The kept ancestors `commit` reaches through commits the filter hid, first
/// parent first, so a filtered graph still joins up.
fn kept_parents(
    commit: &GraphCommit,
    by_sha: &HashMap<&str, &GraphCommit>,
    keep: &HashSet<&str>,
) -> Vec<String> {
    let mut found = Vec::new();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut stack: Vec<&str> = commit.parents.iter().rev().map(String::as_str).collect();
    while let Some(sha) = stack.pop() {
        if !seen.insert(sha) {
            continue;
        }
        if keep.contains(sha) {
            found.push(sha.to_owned());
        } else if let Some(hidden) = by_sha.get(sha) {
            stack.extend(hidden.parents.iter().rev().map(String::as_str));
        }
    }
    found
}

/// The rows the graph draws for `filter`.
pub fn visible_commits(view: &HistoryRepoView, filter: &HistoryFilter) -> Vec<GraphCommit> {
    let needle = search_key(filter.search.trim());
    let kept: Vec<&GraphCommit> = view
        .commits
        .iter()
        .filter(|c| filter.scope == HistoryBranchScope::All || view.on_head.contains(&c.sha))
        .filter(|c| !filter.this_file_only || view.touching.contains(&c.sha))
        .filter(|c| matches_search(c, &needle))
        .collect();
    let keep: HashSet<&str> = kept.iter().map(|c| c.sha.as_str()).collect();
    let by_sha: HashMap<&str, &GraphCommit> =
        view.commits.iter().map(|c| (c.sha.as_str(), c)).collect();
    kept.into_iter()
        .map(|c| GraphCommit {
            parents: kept_parents(c, &by_sha, &keep),
            ..c.clone()
        })
        .collect()
}

/// Detail-strip rows for a commit's changed files.
pub fn changed_files(changes: Vec<RepoFileChange>) -> Vec<ChangedFile> {
    changes
        .into_iter()
        .map(|change| ChangedFile {
            status: match change.kind {
                RepoFileChangeKind::Modified => FileStatus::Modified,
                RepoFileChangeKind::Added => FileStatus::Added,
                RepoFileChangeKind::Deleted => FileStatus::Deleted,
            },
            path: change.path,
        })
        .collect()
}

/// The sheet or board Show on board / sheet opens: the first one the commit
/// changed and left in place, other than the file the panel already follows.
pub fn document_to_show(files: &[ChangedFile], tracked: &str) -> Option<String> {
    files
        .iter()
        .filter(|f| f.status != FileStatus::Deleted && f.path != tracked)
        .find(|f| {
            std::path::Path::new(&f.path)
                .extension()
                .is_some_and(|ext| ext == "snxsch" || ext == "snxpcb")
        })
        .map(|f| f.path.clone())
}

/// Whether the find field's text can name a new local branch.
pub fn can_create_branch(branches: &[BranchRow], query: &str) -> bool {
    let name = query.trim();
    signex_library::repo_graph::is_valid_branch_name(name)
        && !branches.iter().any(|b| !b.is_remote && b.name == name)
}

/// What the name field under a commit creates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefPromptKind {
    Branch,
    Tag,
}

/// The open name field: what it creates, on which commit, and the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefPrompt {
    pub kind: RefPromptKind,
    pub sha: String,
    pub name: String,
}

/// Why the switcher cannot switch now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwitchBlock {
    /// The named document has unsaved edits.
    Unsaved(String),
    /// A save, commit, restore or another switch is running.
    Busy,
}

/// The panel's own view state, kept across reloads of the same file.
#[derive(Debug, Clone, Default)]
pub struct HistoryUi {
    pub filter: HistoryFilter,
    pub search_open: bool,
    pub selected: Option<String>,
    pub files: Vec<ChangedFile>,
    pub more_open: bool,
    pub prompt: Option<RefPrompt>,
    pub switcher_open: bool,
    pub switcher_query: String,
    pub switch_block: Option<SwitchBlock>,
    /// A save of the file makes a commit, so Commit… has something to do.
    pub commits_on_save: bool,
}

/// The filter chips and the search field.
#[derive(Debug, Clone)]
pub enum HistoryFilterMsg {
    ScopePicked(HistoryBranchScope),
    ThisFileToggled,
    SearchToggled,
    SearchEdited(String),
}

/// The commit rows and the detail strip.
#[derive(Debug, Clone)]
pub enum HistoryCommitMsg {
    Selected(String),
    /// Commit… on the working-tree row.
    CommitPressed,
    ShowDocumentPressed(String),
    MoreToggled,
    NewBranchPressed,
    TagPressed,
    CopyShaPressed,
    PromptEdited(String),
    PromptSubmitted,
    PromptCancelled,
}

/// The branch switcher.
#[derive(Debug, Clone)]
pub enum HistorySwitcherMsg {
    Toggled,
    Dismissed,
    QueryEdited(String),
    Picked { name: String, is_remote: bool },
    CreatePressed,
}

/// Every message of the History panel's repository view.
#[derive(Debug, Clone)]
pub enum HistoryPanelMsg {
    RefreshPressed,
    Filter(HistoryFilterMsg),
    Commit(HistoryCommitMsg),
    Switcher(HistorySwitcherMsg),
}

/// A finished repository job.
#[derive(Debug, Clone)]
pub enum HistoryRepoEvent {
    ViewLoaded {
        generation: u32,
        result: Result<Option<Arc<HistoryRepoView>>, String>,
    },
    FilesLoaded {
        generation: u32,
        sha: String,
        result: Result<Vec<ChangedFile>, String>,
    },
    RefCreated(Result<(), String>),
    SwitchFinished(Result<String, SwitchFailure>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn commit(sha: &str, parents: &[&str], subject: &str) -> GraphCommit {
        GraphCommit {
            sha: sha.into(),
            parents: parents.iter().map(|p| (*p).to_owned()).collect(),
            refs: Vec::new(),
            author_name: "j.berg".into(),
            author_email: "j@example.com".into(),
            time: Utc.timestamp_opt(1_795_000_000, 0).unwrap(),
            subject: subject.into(),
        }
    }

    /// `c` on a side branch; `a` and `b` on HEAD; only `a` and `c` touch the
    /// file.
    fn sample() -> HistoryRepoView {
        HistoryRepoView {
            commits: vec![
                commit("c", &["b"], "Side work"),
                commit("b", &["a"], "Add output cap"),
                commit("a", &[], "Initial import"),
            ],
            on_head: ["a", "b"].map(str::to_owned).into_iter().collect(),
            touching: ["a", "c"].map(str::to_owned).into_iter().collect(),
            ..HistoryRepoView::default()
        }
    }

    fn shas(rows: &[GraphCommit]) -> Vec<&str> {
        rows.iter().map(|c| c.sha.as_str()).collect()
    }

    #[test]
    fn the_current_branch_chip_hides_other_branches() {
        let filter = HistoryFilter {
            scope: HistoryBranchScope::Current,
            ..HistoryFilter::default()
        };
        assert_eq!(shas(&visible_commits(&sample(), &filter)), ["b", "a"]);
    }

    /// Hiding `b` must not cut the graph: `c` joins up to `a` through it.
    #[test]
    fn this_file_only_keeps_the_graph_joined() {
        let filter = HistoryFilter {
            this_file_only: true,
            ..HistoryFilter::default()
        };
        let rows = visible_commits(&sample(), &filter);
        assert_eq!(shas(&rows), ["c", "a"]);
        assert_eq!(rows[0].parents, ["a"]);
    }

    #[test]
    fn search_matches_subject_author_and_sha_start() {
        let by = |search: &str| {
            let filter = HistoryFilter {
                search: search.into(),
                ..HistoryFilter::default()
            };
            visible_commits(&sample(), &filter).len()
        };
        assert_eq!(by("OUTPUT"), 1);
        assert_eq!(by("j.berg"), 3);
        assert_eq!(by("c"), 2, "the sha c and the subject with 'cap'");
        assert_eq!(by("nothing here"), 0);
    }

    #[test]
    fn head_sits_after_the_current_branch_and_before_tags() {
        let tip = |name: &str, kind| RepoRefTip {
            sha: "a".into(),
            name: name.into(),
            kind,
        };
        let tips = [
            tip("v1.0", RepoRefKind::Tag),
            tip("side", RepoRefKind::Branch { is_current: false }),
            tip("main", RepoRefKind::Branch { is_current: true }),
        ];
        let names: Vec<String> = pills_of("a", &tips, Some("a"))
            .iter()
            .map(|p| match p {
                RefPill::Head => "HEAD".to_owned(),
                RefPill::Branch { name, .. } | RefPill::Tag { name } => name.clone(),
            })
            .collect();
        assert_eq!(names, ["main", "side", "HEAD", "v1.0"]);
    }

    #[test]
    fn only_a_new_valid_name_can_be_created() {
        let main = BranchRow {
            name: "main".into(),
            is_remote: false,
            is_current: true,
            ahead: 0,
            behind: 0,
        };
        let branches = [main];
        assert!(can_create_branch(&branches, " ripple-fix "));
        assert!(!can_create_branch(&branches, "main"));
        assert!(!can_create_branch(&branches, ""));
        assert!(!can_create_branch(&branches, "has space"));
    }

    #[test]
    fn show_on_board_skips_the_followed_file_and_deletions() {
        let file = |status, path: &str| ChangedFile {
            status,
            path: path.into(),
        };
        let files = [
            file(FileStatus::Modified, "power.snxsch"),
            file(FileStatus::Deleted, "old.snxpcb"),
            file(FileStatus::Added, "assets/Q2.step"),
            file(FileStatus::Modified, "dcdc-buck.snxpcb"),
        ];
        assert_eq!(
            document_to_show(&files, "power.snxsch").as_deref(),
            Some("dcdc-buck.snxpcb")
        );
        assert_eq!(document_to_show(&files[..1], "power.snxsch"), None);
    }
}
