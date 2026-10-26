//! Child-sheet references for ownership checks, resolved from the open
//! projects' own sheet lists rather than only from the loaded engines.
//!
//! A sibling's dirty child sheet whose parent tab was closed has no parent
//! engine to point at it; reading the unloaded parents from disk keeps that
//! child attributable to its project (#741).
//!
//! The disk walk blocks, so it never runs inside `update`: the caller takes a
//! [`SheetRefInputs`] snapshot, resolves it on a worker thread and gets the
//! [`SheetRefs`] back as a message.
//!
//! Closing project A splits A's dirty documents by one strict rule: `mine` is
//! what A's own ownership walk reaches (plus A's `.snxprj`) AND no other open
//! project's walk reaches; every other dirty document associated with A is
//! `kept`. If any open project has a sheet that cannot be read, ownership is
//! unknown and Discard All is off.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::link_policy::link_leads_to_refused;
use super::reference_policy::{
    Share, is_path_refused, is_walk_refused, names_windows_device, share_of, trust_real_shares,
};
use super::scope::{CaseFold, KeyMap, ReachSets, path_key_with};
use super::sheet_read::{WalkLimits, read_child_refs};
use super::volume_case::{IgnoresCase, VolumeCase};
use super::{DocumentState, ProjectId, scope};
use crate::app::project_sheets::{normalize_clamped, ownership_reference};
use crate::library::state::EditorAddress;

/// How many file names a single Messages-panel or dialog line spells out
/// before it falls back to a count.
const MAX_NAMES_LISTED: usize = 5;

/// A sheet that exists but could not be read or parsed, so its children are
/// unknown.
#[derive(Debug, Clone)]
pub struct UnreadableSheet {
    pub(crate) path: PathBuf,
    /// Why it was not followed, e.g. `is not a regular file`.
    pub(crate) reason: String,
    /// Label with the whole reference text, not just its file name. A refused
    /// network or device reference needs the host or share spelled out, and
    /// only a Unix path keeps `\\host\share\x` as one component.
    pub(crate) whole_path: bool,
}

impl UnreadableSheet {
    /// `name (reason)` for the Messages panel and the dialog.
    fn label(&self) -> String {
        let name = if self.whole_path {
            self.path.to_string_lossy().into_owned()
        } else {
            file_label(&self.path)
        };
        match self.reason.as_str() {
            "" => name,
            reason => format!("{name} ({reason})"),
        }
    }
}

/// `a, b, c (+N more)`: at most [`MAX_NAMES_LISTED`] names, then a count, so a
/// line stays short however many files there are.
pub(crate) fn bounded_names(names: &[String]) -> String {
    let shown = names
        .iter()
        .take(MAX_NAMES_LISTED)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    match names.len().saturating_sub(MAX_NAMES_LISTED) {
        0 => shown,
        more => format!("{shown} (+{more} more)"),
    }
}

/// What the off-thread resolve learned: the saved files' child references,
/// the sheets that could not be read, and how paths map to file identity.
#[derive(Debug, Clone, Default)]
pub struct SheetRefs {
    /// Child references as saved on disk, per walked sheet (normalised path).
    /// An open engine's references are merged in at use time
    /// ([`DocumentState::merged_refs`]); they never replace these.
    pub(crate) disk: HashMap<PathBuf, Vec<String>>,
    pub(crate) unreadable: Vec<UnreadableSheet>,
    /// The open projects this was resolved for. A project opened afterwards
    /// has disk-only children nobody read, so a consumer re-resolves when
    /// [`DocumentState::refs_are_current`] says no.
    pub(crate) resolved_for: Vec<ProjectId>,
    /// Real (file identity) paths of everything the resolve touched.
    pub(crate) keys: KeyMap,
    /// Keys of the sheets the walk visited and read.
    pub(crate) walked: HashSet<String>,
    /// The walk hit its sheet cap, so `walked` is knowingly incomplete. Not
    /// a reason to resolve again (that would hit the same cap); Discard All
    /// is already off through `unreadable`.
    pub(crate) truncated: bool,
}

impl SheetRefs {
    /// True when exactly the projects in `open` were open at resolve time.
    pub(crate) fn is_current_for(&self, open: &[ProjectId]) -> bool {
        let resolved: HashSet<ProjectId> = self.resolved_for.iter().copied().collect();
        let now: HashSet<ProjectId> = open.iter().copied().collect();
        resolved == now
    }

    /// Why Discard All is off, or `None` when every sheet could be read. A
    /// sheet that cannot be read hides its children, so which documents are
    /// the closing project's alone is unknown and nothing may be discarded.
    pub fn discard_blocked_reason(&self) -> Option<String> {
        if self.unreadable.is_empty() {
            return None;
        }
        let names: Vec<String> = self.unreadable.iter().map(UnreadableSheet::label).collect();
        Some(format!(
            "Discard All is disabled: {} sheet(s) could not be read ({}), so ownership is unknown",
            names.len(),
            bounded_names(&names),
        ))
    }
}

/// Everything the disk walk needs, owned so it can move to a worker thread.
#[derive(Debug, Clone)]
pub(crate) struct SheetRefInputs {
    /// Child references of every loaded engine, followed IN ADDITION to the
    /// saved file's, so unsaved edits never hide a reference the file keeps.
    loaded: HashMap<PathBuf, Vec<String>>,
    /// Every sheet each open project lists.
    roots: Vec<PathBuf>,
    /// The projects the roots came from.
    projects: Vec<ProjectId>,
    /// Other paths ownership compares later (dirty documents, project files,
    /// library files), so they are keyed by file identity too.
    extra: Vec<PathBuf>,
    /// UNC shares the open projects sit on: a reference on one of them is
    /// followed, any other network or device name is refused.
    trusted: Vec<Share>,
    /// The open projects' directories as spelled. Their canonical spelling can
    /// name another share (mapped drive, DFS, symlink), which is then the
    /// project's own too.
    project_dirs: Vec<PathBuf>,
}

/// The real spelling of `path`, remembered in `keys`. Disk IO.
///
/// An existing path is canonicalised, so a case variant on a case-insensitive
/// volume and a path through a symlinked directory collapse onto the one real
/// file. A path that does not exist gets its nearest existing ancestor
/// canonicalised and the rest appended; if even that fails the normalised
/// path is kept.
///
/// The missing tail has no real spelling to collapse onto, so when its
/// ancestor sits on a case-insensitive volume the tail is lowercased: `N` and
/// `n` are then one never-saved file (#771). A case-sensitive volume keeps
/// the tail verbatim, so two names that differ only by case stay two files.
fn real_path(path: &Path, volume: &mut VolumeCase) -> PathBuf {
    let mut tail: Vec<&std::ffi::OsStr> = Vec::new();
    let mut current = path;
    loop {
        // An ancestor that maps to a device (`NUL` in `NUL/x.snxsch`) must
        // never reach CreateFile either (#999): fold the path lexically from
        // here, exactly as when nothing canonicalises.
        if names_windows_device(current) {
            let folded = PathBuf::from(current.to_string_lossy().to_lowercase());
            return join_tail(folded, tail, true);
        }
        if let Ok(real) = std::fs::canonicalize(current) {
            let fold = !tail.is_empty() && volume.ignores_case(&real);
            return join_tail(real, tail, fold);
        }
        match (current.parent(), current.file_name()) {
            (Some(parent), Some(name)) if !parent.as_os_str().is_empty() => {
                tail.push(name);
                current = parent;
            }
            // Nothing on the path canonicalises (an offline share, an
            // unplugged drive): there is no volume to ask, so fold the WHOLE
            // path, prefix included. That is `VolumeCase`'s undecided answer
            // ("ignores case") and the safe side for identity: over-merging
            // two spellings keeps a sheet open, under-merging could let Discard
            // All drop a sheet another project shares.
            _ => {
                let folded = PathBuf::from(current.to_string_lossy().to_lowercase());
                return join_tail(folded, tail, true);
            }
        }
    }
}

/// `base` followed by `tail` (collected leaf-first), lowercased when `fold`.
fn join_tail(mut base: PathBuf, tail: Vec<&std::ffi::OsStr>, fold: bool) -> PathBuf {
    for name in tail.into_iter().rev() {
        if fold {
            base.push(name.to_string_lossy().to_lowercase());
        } else {
            base.push(name);
        }
    }
    base
}

/// Record and return the ownership key of `path`.
///
/// A path the walk would refuse (device, untrusted network name) is keyed
/// lexically and never canonicalised, so no library or dirty path touches the
/// network or a device just to be compared.
fn identity_key(
    keys: &mut KeyMap,
    volume: &mut VolumeCase,
    trusted: &[Share],
    path: &Path,
) -> String {
    if is_path_refused(path, trusted) || link_leads_to_refused(&normalize_clamped(path), trusted) {
        return keys.key(path);
    }
    let lexical = keys.lexical_key(path);
    let real = keys
        .real
        .entry(lexical)
        .or_insert_with(|| real_path(&normalize_clamped(path), volume));
    path_key_with(real, keys.fold)
}

/// The network / device references the walk refused, each listed once and the
/// list bounded: a hostile sheet can name a million distinct `\\h\s\<n>`
/// children, and neither the list nor the duplicate check may grow with them.
struct Refusals {
    seen: HashSet<PathBuf>,
    /// Entries listed in `unreadable`; once it reaches `cap` the rest only count.
    listed: usize,
    hidden: usize,
    cap: usize,
}

impl Refusals {
    fn new(cap: usize) -> Self {
        Self {
            seen: HashSet::new(),
            listed: 0,
            hidden: 0,
            cap,
        }
    }

    /// Record `reference` as unreadable, once.
    fn refuse(&mut self, unreadable: &mut Vec<UnreadableSheet>, reference: &str) {
        let path = PathBuf::from(reference.trim());
        if !self.seen.insert(path.clone()) {
            return;
        }
        if self.listed >= self.cap {
            self.hidden += 1;
            return;
        }
        self.listed += 1;
        unreadable.push(UnreadableSheet {
            path,
            reason: "is a network or device path, not followed".to_string(),
            whole_path: true,
        });
    }

    /// The single `(+N more refused)` entry for what the cap hid.
    fn finish(self, unreadable: &mut Vec<UnreadableSheet>) {
        if self.hidden > 0 {
            unreadable.push(UnreadableSheet {
                path: PathBuf::from(format!("(+{} more refused)", self.hidden)),
                reason: String::new(),
                whole_path: true,
            });
        }
    }
}

fn push_unique(into: &mut Vec<String>, from: impl IntoIterator<Item = String>) {
    for item in from {
        if !into.contains(&item) {
            into.push(item);
        }
    }
}

impl SheetRefInputs {
    /// Walk the open projects' sheets, reading every one from disk.
    ///
    /// A sheet an engine holds follows the UNION of the engine's references
    /// and the saved file's. Every reference is followed (see
    /// [`ownership_reference`]): ownership never refuses an escape, only
    /// opening a file does. Each sheet is visited once per file identity and
    /// `..` is clamped at the filesystem root, so a `../` loop or chain
    /// terminates.
    ///
    /// Blocking file IO: call from a worker thread.
    pub(crate) fn resolve(self, ignores_case: IgnoresCase) -> SheetRefs {
        self.resolve_within(ignores_case, WalkLimits::default())
    }

    /// [`resolve`](Self::resolve) with explicit bounds.
    ///
    /// The references and files are untrusted (see [`super::sheet_read`]): a
    /// network, device or verbatim reference is never canonicalised or read,
    /// a file that is not a regular file or is oversized is not read, and a
    /// walk past `limits.max_sheets` stops. Each is reported as unreadable,
    /// which switches Discard All off.
    pub(crate) fn resolve_within(self, ignores_case: IgnoresCase, limits: WalkLimits) -> SheetRefs {
        let Self {
            loaded,
            roots,
            projects,
            extra,
            mut trusted,
            project_dirs,
        } = self;
        // Only a directory the classifier already allows is canonicalised.
        trust_real_shares(&mut trusted, &project_dirs, |dir| {
            std::fs::canonicalize(dir).ok()
        });
        // Identity keys never fold case: this map spans EVERY open project,
        // and one project's volume rule must not decide whether two real
        // files of another are one. A file that exists is keyed by its
        // canonical spelling; only a never-saved tail goes through
        // `VolumeCase`, per directory (#771). Only the host-derived
        // separator rule applies.
        let mut keys = KeyMap::lexical(CaseFold::from_probe(false));
        let mut volume = VolumeCase::new(ignores_case);
        let mut loaded_by_key: HashMap<String, Vec<String>> = HashMap::new();
        for (path, refs) in &loaded {
            let key = identity_key(&mut keys, &mut volume, &trusted, path);
            push_unique(loaded_by_key.entry(key).or_default(), refs.iter().cloned());
        }
        for path in &extra {
            identity_key(&mut keys, &mut volume, &trusted, path);
        }
        let mut disk: HashMap<PathBuf, Vec<String>> = HashMap::new();
        let mut unreadable = Vec::new();
        let mut refusals = Refusals::new(limits.max_sheets);
        let mut walked: HashSet<String> = HashSet::new();
        let mut pending: Vec<PathBuf> = Vec::new();
        let mut truncated = false;
        for root in &roots {
            if is_path_refused(root, &trusted) {
                refusals.refuse(&mut unreadable, &root.to_string_lossy());
                // Never canonicalised (that is the IO refused), so the key the
                // staleness check computes is the one recorded by `identity_key`
                // if anything else keyed this path, the lexical one if not.
                walked.insert(keys.key(root));
            } else {
                pending.push(normalize_clamped(root));
            }
        }
        while let Some(path) = pending.pop() {
            let key = identity_key(&mut keys, &mut volume, &trusted, &path);
            if !walked.insert(key.clone()) {
                continue;
            }
            if walked.len() > limits.max_sheets {
                unreadable.push(UnreadableSheet {
                    path: PathBuf::from("(rest of the walk)"),
                    reason: format!("stopped after {} sheets", limits.max_sheets),
                    whole_path: false,
                });
                truncated = true;
                break;
            }
            // A link to a device or untrusted share is as refused as its spelling
            // would be (#1020); it is never opened.
            if link_leads_to_refused(&path, &trusted) {
                refusals.refuse(&mut unreadable, &path.to_string_lossy());
                continue;
            }
            let saved = read_child_refs(&path, limits.max_sheet_bytes).unwrap_or_else(|reason| {
                unreadable.push(UnreadableSheet {
                    path: path.clone(),
                    reason,
                    whole_path: false,
                });
                Vec::new()
            });
            let mut children = saved.clone();
            if let Some(engine) = loaded_by_key.get(&key) {
                push_unique(&mut children, engine.iter().cloned());
            }
            disk.insert(path.clone(), saved);
            let dir = keys.child_dir(&path);
            for child in &children {
                let Some(next) = ownership_reference(&dir, child) else {
                    continue;
                };
                if is_walk_refused(child, &next, &trusted) {
                    refusals.refuse(&mut unreadable, child);
                    walked.insert(keys.key(&next));
                } else {
                    pending.push(next);
                }
            }
        }
        refusals.finish(&mut unreadable);
        SheetRefs {
            disk,
            unreadable,
            resolved_for: projects,
            keys,
            walked,
            truncated,
        }
    }
}

/// Why a resolved [`SheetRefs`] no longer describes the workspace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StaleRefs {
    /// A project was opened or closed since the resolve.
    ProjectsChanged,
    /// A document became dirty that the resolve never keyed.
    NewDirtyDocument,
    /// A sheet now references a file the resolve never read (an engine edit).
    ReferencesChanged,
}

impl StaleRefs {
    /// Log wording for one cause.
    pub(crate) const fn describe(self) -> &'static str {
        match self {
            Self::ProjectsChanged => "the open projects changed",
            Self::NewDirtyDocument => "a document with unsaved changes was added",
            Self::ReferencesChanged => "a sheet's child references were edited",
        }
    }

    /// `a; b` over `causes`.
    pub(crate) fn describe_all(causes: &[Self]) -> String {
        causes
            .iter()
            .map(|c| c.describe())
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// A closing project's dirty documents, split so none falls through: each
/// one is either in `mine` (offered to Save All / Discard All) or in `kept`
/// (never discarded: closing leaves it open and says so). Component Preview
/// rows follow the same split in `mine_rows` / `kept_rows`.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct CloseScan {
    pub(crate) mine: Vec<PathBuf>,
    pub(crate) kept: Vec<PathBuf>,
    pub(crate) mine_rows: Vec<EditorAddress>,
    pub(crate) kept_rows: Vec<EditorAddress>,
}

impl CloseScan {
    /// Messages-panel line naming the closing project's kept documents,
    /// bounded.
    pub(crate) fn warning(&self, project_name: &str) -> Option<String> {
        let mut names: Vec<String> = self.kept.iter().map(|p| file_label(p)).collect();
        names.extend(self.kept_rows.iter().map(|r| file_label(&r.library_path)));
        if names.is_empty() {
            return None;
        }
        Some(format!(
            "Close Project '{project_name}': kept {} unsaved document(s) open, not provably this project's alone: {}",
            names.len(),
            bounded_names(&names),
        ))
    }
}

/// File name of `path` for display, the whole path when it has none.
pub(crate) fn file_label(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

impl DocumentState {
    /// Snapshot what [`SheetRefInputs::resolve`] needs; cheap, no IO.
    pub(crate) fn sheet_ref_inputs(&self) -> SheetRefInputs {
        let mut extra: Vec<PathBuf> = self.dirty_paths.iter().cloned().collect();
        for project in &self.projects {
            extra.push(project.path.clone());
            extra.extend(
                project
                    .data
                    .libraries
                    .iter()
                    .map(|entry| project.data.resolve_library_path(entry)),
            );
        }
        SheetRefInputs {
            loaded: self.child_sheet_refs(),
            roots: self
                .projects
                .iter()
                .flat_map(|p| {
                    let dir = p.dir();
                    p.data.sheets.iter().map(move |s| dir.join(&s.filename))
                })
                .collect(),
            projects: self.projects.iter().map(|p| p.id).collect(),
            extra,
            trusted: self
                .projects
                .iter()
                .filter_map(|p| share_of(&p.path))
                .collect(),
            project_dirs: self
                .projects
                .iter()
                .map(|p| p.dir().to_path_buf())
                .collect(),
        }
    }

    /// Every sheet's child references right now: the saved files' (from the
    /// resolve) UNIONED with the open engines' current ones. Engines are
    /// re-read here, so an edit made after the resolve started is seen. No IO.
    pub(crate) fn merged_refs(&self, sheet_refs: &SheetRefs) -> HashMap<PathBuf, Vec<String>> {
        let keys = &sheet_refs.keys;
        let engines = self.child_sheet_refs();
        let mut by_key: HashMap<String, (PathBuf, Vec<String>)> = HashMap::new();
        for (path, refs) in sheet_refs.disk.iter().chain(engines.iter()) {
            let entry = by_key
                .entry(keys.key(path))
                .or_insert_with(|| (path.clone(), Vec::new()));
            push_unique(&mut entry.1, refs.iter().cloned());
        }
        by_key.into_values().collect()
    }

    /// What every open project's ownership walk reaches, for `sheet_refs`.
    pub(crate) fn reach_sets(&self, sheet_refs: &SheetRefs) -> ReachSets {
        ReachSets::with_keys(
            &self.projects,
            &self.merged_refs(sheet_refs),
            &sheet_refs.keys,
        )
    }

    /// Ownership walks over the open engines' references alone: no disk, so
    /// safe inside `update`. A walk that meets a sheet no engine holds is
    /// flagged (`ReachSets::other_walk_incomplete`) rather than guessed.
    /// Keys follow `fold`, the caller's probed rule, never the host family
    /// (#1034).
    pub(crate) fn lexical_reach_sets(&self, fold: CaseFold) -> ReachSets {
        ReachSets::with_keys(
            &self.projects,
            &self.child_sheet_refs(),
            &KeyMap::lexical(fold),
        )
    }

    /// True when `sheet_refs` still describes the workspace: the same projects
    /// are open, every sheet the walk reaches now (engine edits included) was
    /// read from disk, and every dirty document was keyed by file identity.
    /// When false the caller resolves again rather than act on stale evidence.
    pub(crate) fn refs_are_current(&self, sheet_refs: &SheetRefs) -> bool {
        self.stale_causes(sheet_refs).is_empty()
    }

    /// Everything that makes `sheet_refs` stale, none when it is current.
    /// Reported together so the log says what actually changed (#771).
    pub(crate) fn stale_causes(&self, sheet_refs: &SheetRefs) -> Vec<StaleRefs> {
        let open: Vec<ProjectId> = self.projects.iter().map(|p| p.id).collect();
        let mut causes = Vec::new();
        if !sheet_refs.is_current_for(&open) {
            causes.push(StaleRefs::ProjectsChanged);
        }
        if !self
            .dirty_paths
            .iter()
            .all(|path| sheet_refs.keys.knows(path))
        {
            causes.push(StaleRefs::NewDirtyDocument);
        }
        if !sheet_refs.truncated
            && !self
                .reach_sets(sheet_refs)
                .all_keys()
                .all(|key| sheet_refs.walked.contains(key))
        {
            causes.push(StaleRefs::ReferencesChanged);
        }
        causes
    }

    /// Split the dirty documents of `projects[closing]` into [`CloseScan`]'s
    /// lists. `rows` are the dirty Component Preview editors to place.
    ///
    /// A dirty path is associated with the project when it sits in the
    /// project directory, backs one of its tabs, is reached by its walk, or is
    /// its `.snxprj`. It is `mine` only when the project's own walk reaches it
    /// (or it is the project's `.snxprj`) AND no other open project's walk
    /// does; everything else associated is `kept`. Location, tabs and library
    /// mounts are never evidence of ownership.
    ///
    /// A row is `mine` only when its library file is one of the project's own
    /// library entries, no other open project lists the same library, and it
    /// does not sit inside another open project's directory.
    pub(crate) fn close_scan(
        &self,
        sheet_refs: &SheetRefs,
        closing: usize,
        rows: &[EditorAddress],
    ) -> CloseScan {
        let Some(project) = self.projects.get(closing) else {
            return CloseScan::default();
        };
        let keys = &sheet_refs.keys;
        let reach = self.reach_sets(sheet_refs);
        let project_file = keys.key(&project.path);
        let mut scan = CloseScan::default();
        for path in &self.dirty_paths {
            let reached = reach.reached_by(closing, path);
            let is_project_file = keys.key(path) == project_file;
            let backs_tab = self
                .tabs
                .iter()
                .any(|t| t.path == *path && t.project_id == Some(project.id));
            let associated =
                path.starts_with(project.dir()) || backs_tab || reached || is_project_file;
            if !associated {
                continue;
            }
            if (reached || is_project_file) && !reach.reached_by_other(closing, path) {
                scan.mine.push(path.clone());
            } else {
                scan.kept.push(path.clone());
            }
        }
        for row in rows {
            let library = keys.key(&row.library_path);
            let own = self.library_keys(closing, keys).contains(&library);
            let listed_elsewhere = (0..self.projects.len())
                .filter(|i| *i != closing)
                .any(|i| self.library_keys(i, keys).contains(&library));
            let inside_other = self
                .projects
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != closing)
                .any(|(_, p)| scope::path_is_under(&row.library_path, p.dir(), p.case_fold));
            if own && !listed_elsewhere && !inside_other {
                scan.mine_rows.push(row.clone());
            } else if own || row.library_path.starts_with(project.dir()) {
                scan.kept_rows.push(row.clone());
            }
        }
        scan.mine.sort();
        scan.kept.sort();
        scan
    }

    /// File-identity keys of the library files `projects[idx]` mounts.
    fn library_keys(&self, idx: usize, keys: &KeyMap) -> HashSet<String> {
        self.projects.get(idx).map_or_else(HashSet::new, |p| {
            p.data
                .libraries
                .iter()
                .map(|entry| keys.key(&p.data.resolve_library_path(entry)))
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank_sheet() -> signex_types::schematic::SchematicSheet {
        signex_types::schematic::SchematicSheet {
            uuid: uuid::Uuid::nil(),
            version: 0,
            generator: String::new(),
            generator_version: String::new(),
            paper_size: "A4".to_string(),
            root_sheet_page: "1".to_string(),
            symbols: Vec::new(),
            wires: Vec::new(),
            junctions: Vec::new(),
            labels: Vec::new(),
            child_sheets: Vec::new(),
            no_connects: Vec::new(),
            text_notes: Vec::new(),
            buses: Vec::new(),
            bus_entries: Vec::new(),
            drawings: Vec::new(),
            no_erc_directives: Vec::new(),
            title_block: HashMap::new(),
            lib_symbols: HashMap::new(),
            net_colors: HashMap::new(),
            unknown: Default::default(),
        }
    }

    fn write_sheet(path: &Path, children: &[&str]) {
        let mut sheet = blank_sheet();
        sheet.child_sheets = children
            .iter()
            .map(|f| signex_types::schematic::ChildSheet {
                uuid: uuid::Uuid::nil(),
                name: String::new(),
                filename: (*f).to_string(),
                position: signex_types::schematic::Point::new(0.0, 0.0),
                size: (0.0, 0.0),
                stroke_width: 0.0,
                fill: signex_types::schematic::FillType::None,
                stroke_color: None,
                fill_color: None,
                fields_autoplaced: false,
                pins: Vec::new(),
                instances: Vec::new(),
            })
            .collect();
        let text = signex_types::format::SnxSchematic::new(sheet)
            .write_string()
            .expect("serialise sheet");
        std::fs::write(path, text).expect("write sheet");
    }

    #[test]
    fn name_lists_are_bounded_to_a_count_plus_the_first_few() {
        let names: Vec<String> = (0..200).map(|n| format!("s{n}.snxsch")).collect();
        let line = bounded_names(&names);
        assert!(line.starts_with("s0.snxsch, s1.snxsch"), "{line}");
        assert!(line.ends_with("(+195 more)"), "{line}");
        assert!(line.len() < 100, "{line}");
        assert_eq!(bounded_names(&names[..2]), "s0.snxsch, s1.snxsch");

        let scan = CloseScan {
            mine: Vec::new(),
            kept: names.iter().map(PathBuf::from).collect(),
            ..CloseScan::default()
        };
        let warning = scan.warning("P").expect("kept files warn");
        assert!(warning.contains("kept 200"), "{warning}");
        assert!(warning.len() < 250, "{warning}");

        let refs = SheetRefs {
            unreadable: names
                .iter()
                .map(|n| UnreadableSheet {
                    path: PathBuf::from(n),
                    reason: String::new(),
                    whole_path: false,
                })
                .collect(),
            ..SheetRefs::default()
        };
        let reason = refs.discard_blocked_reason().expect("blocked");
        assert!(reason.len() < 250, "{reason}");
        assert!(SheetRefs::default().discard_blocked_reason().is_none());
    }

    #[test]
    fn a_dotdot_cycle_reads_each_sheet_once_and_reports_nothing() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("d");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let root = dir.join("x.snxsch");
        write_sheet(&root, &["../d/x.snxsch"]);
        let inputs = SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root.clone()],
            projects: Vec::new(),
            extra: Vec::new(),
            trusted: Vec::new(),
            project_dirs: Vec::new(),
        };

        let resolved = inputs.resolve(|_| false);

        assert!(resolved.unreadable.is_empty(), "{:?}", resolved.unreadable);
        assert_eq!(resolved.disk.len(), 1, "{:?}", resolved.disk.keys());
        assert!(resolved.disk.contains_key(&root));
    }

    #[test]
    fn a_case_and_separator_variant_reference_resolves_to_the_loaded_engine() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // Children resolve against the REAL directory, so expect real paths.
        let base = tmp.path().canonicalize().expect("canonical tmp");
        let root = base.join("root.snxsch");
        write_sheet(&root, &["CHILD.snxsch"]);
        // Loaded under the lowercase spelling, with a grandchild the disk
        // never saw (no such file exists).
        let loaded_child = base.join("child.snxsch");
        let inputs = SheetRefInputs {
            loaded: HashMap::from([(loaded_child.clone(), vec!["grand.snxsch".to_string()])]),
            roots: vec![root],
            projects: Vec::new(),
            extra: Vec::new(),
            trusted: Vec::new(),
            project_dirs: Vec::new(),
        };

        let resolved = inputs.resolve(|_| true);

        // The engine's grandchild is followed through the variant spelling.
        assert!(
            resolved.disk.contains_key(&base.join("grand.snxsch")),
            "{:?}",
            resolved.disk.keys(),
        );
        assert!(resolved.unreadable.is_empty());
    }

    #[test]
    fn a_reference_that_escapes_its_parent_directory_is_followed_for_ownership() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // Children resolve against the REAL directory, so expect real paths.
        let base = tmp.path().canonicalize().expect("canonical tmp");
        let dir = base.join("d");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let outside = base.join("outside.snxsch");
        let grandchild = base.join("grand.snxsch");
        write_sheet(&outside, &["grand.snxsch"]);
        write_sheet(&grandchild, &[]);
        let root = dir.join("x.snxsch");
        write_sheet(&root, &["../outside.snxsch"]);
        let inputs = SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root],
            projects: Vec::new(),
            extra: Vec::new(),
            trusted: Vec::new(),
            project_dirs: Vec::new(),
        };

        let resolved = inputs.resolve(|_| false);

        assert!(
            resolved.disk.contains_key(&outside),
            "{:?}",
            resolved.disk.keys()
        );
        assert!(
            resolved.disk.contains_key(&grandchild),
            "{:?}",
            resolved.disk.keys()
        );
    }

    #[test]
    fn an_absolute_reference_is_followed_for_ownership() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("d");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let outside = tmp.path().join("outside.snxsch");
        write_sheet(&outside, &[]);
        let root = dir.join("x.snxsch");
        write_sheet(&root, &[outside.to_str().expect("utf8 path")]);
        let inputs = SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root],
            projects: Vec::new(),
            extra: Vec::new(),
            trusted: Vec::new(),
            project_dirs: Vec::new(),
        };

        let resolved = inputs.resolve(|_| false);

        assert!(
            resolved.disk.contains_key(&outside),
            "{:?}",
            resolved.disk.keys()
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_chain_of_parent_references_past_the_root_terminates() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("d");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let root = dir.join("x.snxsch");
        // `/../../..` clamps to `/`, so this names the sheet itself and the
        // walk folds onto the key it already saw instead of chasing `..`.
        let chained = format!("/../../../..{}", root.display());
        write_sheet(&root, &[chained.as_str(), "../../../../../../../x.snxsch"]);
        let inputs = SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root.clone()],
            projects: Vec::new(),
            extra: Vec::new(),
            trusted: Vec::new(),
            project_dirs: Vec::new(),
        };

        let resolved = inputs.resolve(|_| false);

        assert!(resolved.unreadable.is_empty(), "{:?}", resolved.unreadable);
        assert!(resolved.disk.contains_key(&root));
        assert!(resolved.disk.len() <= 3, "{:?}", resolved.disk.keys());
    }

    fn loaded_project(id: u32, dir: &Path, sheet: &str) -> crate::app::state::LoadedProject {
        crate::app::state::LoadedProject {
            id: ProjectId(id),
            path: dir.join("p.snxprj"),
            data: signex_types::project::ProjectData {
                name: format!("p{id}"),
                dir: dir.to_string_lossy().into_owned(),
                schematic_root: Some(sheet.to_string()),
                pcb_file: None,
                sheets: vec![signex_types::project::SheetEntry {
                    name: sheet.to_string(),
                    filename: sheet.to_string(),
                    symbols_count: 0,
                    wires_count: 0,
                    labels_count: 0,
                    unknown: Default::default(),
                }],
                variant_definitions: Vec::new(),
                active_variant: None,
                libraries: Vec::new(),
                enable_git: false,
                rules: None,
                unknown: Default::default(),
            },
            pending_libraries: HashMap::new(),
            case_fold: Default::default(),
            case_probed: false,
        }
    }

    #[test]
    fn a_case_variant_reference_is_the_same_file_under_the_fold_policy() {
        // Deterministic on every host: the fold policy is explicit, and the
        // variant file never exists on a case-sensitive volume.
        let tmp = tempfile::tempdir().expect("tempdir");
        let (a_dir, b_dir) = (tmp.path().join("A"), tmp.path().join("B"));
        std::fs::create_dir_all(&a_dir).expect("mkdir A");
        std::fs::create_dir_all(&b_dir).expect("mkdir B");
        write_sheet(&b_dir.join("b.snxsch"), &["../A/s.snxsch"]);
        let (mut app, _task) = crate::app::Signex::new();
        let docs = &mut app.document_state;
        docs.projects.push(loaded_project(1, &a_dir, "S.snxsch"));
        docs.projects.push(loaded_project(2, &b_dir, "b.snxsch"));
        let shared = a_dir.join("S.snxsch");
        docs.dirty_paths.insert(shared.clone());

        let refs = docs.sheet_ref_inputs().resolve(|_| true);
        let scan = docs.close_scan(&refs, 0, &[]);

        assert!(scan.mine.is_empty(), "{:?}", scan.mine);
        assert_eq!(scan.kept, vec![shared]);
    }

    /// Nothing on these paths exists, so nothing canonicalises and no volume
    /// can be asked: two spellings differing only by case must still be ONE
    /// identity, or Discard All on A drops the sheet B also lists.
    #[test]
    fn spellings_of_an_unreachable_path_that_differ_by_case_are_one_identity() {
        let (mut app, _task) = crate::app::Signex::new();
        let docs = &mut app.document_state;
        docs.projects.push(loaded_project(
            1,
            Path::new("nonexistent-694/A"),
            "Top.snxsch",
        ));
        docs.projects.push(loaded_project(
            2,
            Path::new("nonexistent-694/a"),
            "top.snxsch",
        ));
        let dirty = PathBuf::from("nonexistent-694/A/Top.snxsch");
        docs.dirty_paths.insert(dirty.clone());

        let refs = docs.sheet_ref_inputs().resolve(|_| false);
        let scan = docs.close_scan(&refs, 0, &[]);

        assert!(
            scan.mine.is_empty(),
            "B lists this sheet too: {:?}",
            scan.mine
        );
        assert_eq!(scan.kept, vec![dirty]);
    }

    /// Project A sits on a case-insensitive stick and references
    /// `../B/x.snxsch`; project B (case-sensitive) holds two REAL files,
    /// `X.snxsch` (dirty, unlisted) and `x.snxsch`. A's rule must not fold B's
    /// files together, or Discard All on A throws away B's unsaved edits.
    #[test]
    fn a_closing_projects_fold_does_not_merge_another_projects_real_files() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (a_dir, b_dir) = (tmp.path().join("A"), tmp.path().join("B"));
        std::fs::create_dir_all(&a_dir).expect("mkdir A");
        std::fs::create_dir_all(&b_dir).expect("mkdir B");
        write_sheet(&b_dir.join("b.snxsch"), &[]);
        write_sheet(&b_dir.join("x.snxsch"), &[]);
        write_sheet(&b_dir.join("X.snxsch"), &[]);
        let distinct = std::fs::canonicalize(b_dir.join("X.snxsch")).ok()
            != std::fs::canonicalize(b_dir.join("x.snxsch")).ok();
        if !distinct || !b_dir.join("x.snxsch").exists() {
            eprintln!("skipped: the temp volume is case-insensitive");
            return;
        }
        write_sheet(&a_dir.join("a.snxsch"), &["../B/x.snxsch"]);
        let (mut app, _task) = crate::app::Signex::new();
        let docs = &mut app.document_state;
        docs.projects.push(loaded_project(1, &a_dir, "a.snxsch"));
        docs.projects[0].case_fold = CaseFold::from_probe(true);
        docs.projects.push(loaded_project(2, &b_dir, "b.snxsch"));
        let unsaved = b_dir.join("X.snxsch");
        docs.dirty_paths.insert(unsaved.clone());

        let refs = docs.sheet_ref_inputs().resolve(|_| false);
        let scan = docs.close_scan(&refs, 0, &[]);

        assert!(
            !scan.mine.contains(&unsaved),
            "B's unsaved X.snxsch must never be A's to discard: {:?}",
            scan.mine
        );
    }

    /// A's never-saved `N.snxsch` and B's saved sheet referencing
    /// `../A/n.snxsch`; neither file exists. Returns A's close scan.
    fn close_scan_of_never_saved_sheet(ignores_case: IgnoresCase) -> CloseScan {
        let tmp = tempfile::tempdir().expect("tempdir");
        let (a_dir, b_dir) = (tmp.path().join("A"), tmp.path().join("B"));
        std::fs::create_dir_all(&a_dir).expect("mkdir A");
        std::fs::create_dir_all(&b_dir).expect("mkdir B");
        write_sheet(&b_dir.join("b.snxsch"), &["../A/n.snxsch"]);
        let (mut app, _task) = crate::app::Signex::new();
        let docs = &mut app.document_state;
        docs.projects.push(loaded_project(1, &a_dir, "N.snxsch"));
        docs.projects.push(loaded_project(2, &b_dir, "b.snxsch"));
        docs.dirty_paths.insert(a_dir.join("N.snxsch"));

        let refs = docs.sheet_ref_inputs().resolve(ignores_case);
        docs.close_scan(&refs, 0, &[])
    }

    #[test]
    fn a_never_saved_case_variant_sheet_is_kept_when_its_volume_ignores_case() {
        let scan = close_scan_of_never_saved_sheet(|_| true);

        assert!(scan.mine.is_empty(), "N is B's sheet too: {:?}", scan.mine);
        assert_eq!(scan.kept.len(), 1, "{:?}", scan.kept);
        assert_eq!(file_label(&scan.kept[0]), "N.snxsch");
    }

    #[test]
    fn a_never_saved_case_variant_sheet_stays_two_files_on_a_case_sensitive_volume() {
        let scan = close_scan_of_never_saved_sheet(|_| false);

        assert_eq!(scan.mine.len(), 1, "N and n are different files");
        assert!(scan.kept.is_empty(), "{:?}", scan.kept);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_never_saved_case_variant_sheet_is_kept_on_the_real_volume() {
        use super::super::volume_case::directory_ignores_case;
        let probe = tempfile::tempdir().expect("tempdir");
        if !directory_ignores_case(probe.path()) {
            eprintln!("skipped: the temp volume is case-sensitive");
            return;
        }

        let scan = close_scan_of_never_saved_sheet(directory_ignores_case);

        assert!(scan.mine.is_empty(), "N is B's sheet too: {:?}", scan.mine);
        assert_eq!(scan.kept.len(), 1, "{:?}", scan.kept);
    }

    #[cfg(unix)]
    #[test]
    fn a_refused_reference_the_resolve_also_keyed_is_current_on_the_next_check() {
        // `//host/share/..` is refused as a network name on every host, but
        // on Unix it is also a real local file whose canonical spelling
        // differs (here through a symlink). The staleness check must find
        // the refused entry under the same key it computes for it.
        let tmp = tempfile::tempdir().expect("tempdir");
        let base = tmp.path().canonicalize().expect("canonical tmp");
        std::fs::create_dir_all(base.join("real")).expect("mkdir real");
        std::os::unix::fs::symlink(base.join("real"), base.join("link")).expect("symlink");
        let a_dir = base.join("A");
        std::fs::create_dir_all(&a_dir).expect("mkdir A");
        let other = base.join("link").join("y.snxsch");
        let reference = format!("/{}", other.display());
        write_sheet(&a_dir.join("a.snxsch"), &[reference.as_str()]);
        let (mut app, _task) = crate::app::Signex::new();
        let docs = &mut app.document_state;
        docs.projects.push(loaded_project(1, &a_dir, "a.snxsch"));
        docs.dirty_paths.insert(other);

        let refs = docs.sheet_ref_inputs().resolve(|_| false);

        assert_eq!(refs.unreadable.len(), 1, "{:?}", refs.unreadable);
        assert_eq!(docs.stale_causes(&refs), Vec::new(), "re-resolve loop");
    }

    /// Resolve a walk whose only root is `root`.
    fn resolve_root(root: &Path, limits: WalkLimits) -> SheetRefs {
        SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root.to_path_buf()],
            projects: Vec::new(),
            extra: Vec::new(),
            trusted: Vec::new(),
            project_dirs: Vec::new(),
        }
        .resolve_within(|_| false, limits)
    }

    fn assert_discard_blocked_by(refs: &SheetRefs, needle: &str) {
        let reason = refs.discard_blocked_reason().expect("Discard All is off");
        assert!(reason.contains("Discard All is disabled"), "{reason}");
        assert!(reason.contains(needle), "{reason}");
    }

    #[test]
    fn a_network_reference_is_refused_unread_and_disables_discard_all() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("x.snxsch");
        write_sheet(
            &root,
            &[r"\\attacker\share\y.snxsch", "//attacker/share/z.snxsch"],
        );

        let resolved = resolve_root(&root, WalkLimits::default());

        assert_eq!(resolved.unreadable.len(), 2, "{:?}", resolved.unreadable);
        assert_discard_blocked_by(&resolved, "attacker");
        // Never canonicalised: no real path was recorded for either.
        assert!(
            resolved.keys.real.keys().all(|k| !k.contains("attacker")),
            "{:?}",
            resolved.keys.real.keys(),
        );
    }

    #[test]
    fn a_refused_reference_is_labelled_with_its_whole_text_on_every_host() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("x.snxsch");
        write_sheet(&root, &[r"\\attacker\share\y.snxsch"]);

        let resolved = resolve_root(&root, WalkLimits::default());

        let label = resolved.unreadable[0].label();
        assert!(label.starts_with(r"\\attacker\share\y.snxsch ("), "{label}");
    }

    #[test]
    fn many_refused_children_are_bounded_and_counted_once_each() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("x.snxsch");
        let children: Vec<String> = (0..5000).map(|n| format!(r"\\h\s\{n}.snxsch")).collect();
        let refs: Vec<&str> = children.iter().map(String::as_str).collect();
        write_sheet(&root, &refs);
        let limits = WalkLimits {
            max_sheets: 50,
            ..WalkLimits::default()
        };
        let started = std::time::Instant::now();

        let resolved = resolve_root(&root, limits);

        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        // 50 listed, plus the single summary entry for the other 4950.
        assert_eq!(
            resolved.unreadable.len(),
            51,
            "{}",
            resolved.unreadable.len()
        );
        assert_eq!(resolved.unreadable[50].label(), "(+4950 more refused)");
        assert_discard_blocked_by(&resolved, "network or device");
    }

    #[test]
    fn a_network_project_sheet_entry_is_refused_unread() {
        let resolved = resolve_root(
            Path::new(r"\\attacker\share\root.snxsch"),
            WalkLimits::default(),
        );

        assert_discard_blocked_by(&resolved, "attacker");
        assert!(resolved.disk.is_empty(), "{:?}", resolved.disk.keys());
    }

    #[cfg(windows)]
    #[test]
    fn a_unc_reference_is_never_opened_on_windows() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("x.snxsch");
        write_sheet(&root, &[r"\\localhost\c$\y.snxsch", r"\\.\NUL"]);

        let resolved = resolve_root(&root, WalkLimits::default());

        assert_eq!(resolved.unreadable.len(), 2, "{:?}", resolved.unreadable);
        assert_discard_blocked_by(&resolved, "network or device");
    }

    #[cfg(windows)]
    #[test]
    fn a_child_that_is_a_symlink_to_a_share_is_unreadable_and_never_opened_on_windows() {
        use std::os::windows::fs::{symlink_dir, symlink_file};
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("x.snxsch");
        write_sheet(&root, &["link.snxsch", r"dirlink\y.snxsch"]);
        // An unresolvable host: nothing can answer for it, whatever the LAN does.
        let file = symlink_file(
            r"\\nonexistent.invalid\s\z.snxsch",
            tmp.path().join("link.snxsch"),
        );
        let dir = symlink_dir(r"\\nonexistent.invalid\s", tmp.path().join("dirlink"));
        if let Some(e) = file.err().or(dir.err()) {
            assert_eq!(e.raw_os_error(), Some(1314), "{e}");
            eprintln!("skipped: cannot create symlinks ({e})");
            return;
        }

        let resolved = resolve_root(&root, WalkLimits::default());

        assert_eq!(resolved.unreadable.len(), 2, "{:?}", resolved.unreadable);
        assert_discard_blocked_by(&resolved, "network or device");
        assert!(resolved.disk.len() == 1, "{:?}", resolved.disk.keys());
    }

    #[cfg(unix)]
    #[test]
    fn a_double_slash_rooted_project_is_walked_not_refused_on_unix() {
        // `//` is UNC to the text classifier on every host, so the project's
        // own share is trusted and its sheets are read; another share is not.
        let tmp = tempfile::tempdir().expect("tempdir");
        let base = tmp.path().canonicalize().expect("canonical tmp");
        write_sheet(
            &base.join("x.snxsch"),
            &["y.snxsch", "//elsewhere/share/z.snxsch"],
        );
        write_sheet(&base.join("y.snxsch"), &[]);
        let root = PathBuf::from(format!("/{}", base.join("x.snxsch").display()));
        let trusted: Vec<Share> = share_of(&root).into_iter().collect();
        assert_eq!(trusted.len(), 1, "{root:?} names a share");

        let resolved = SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root],
            projects: Vec::new(),
            extra: Vec::new(),
            trusted,
            project_dirs: Vec::new(),
        }
        .resolve(|_| false);

        assert_eq!(resolved.unreadable.len(), 1, "{:?}", resolved.unreadable);
        assert_eq!(
            resolved.unreadable[0].path,
            PathBuf::from("//elsewhere/share/z.snxsch")
        );
        assert!(resolved.disk.contains_key(&base.join("y.snxsch")));
    }

    #[cfg(windows)]
    #[test]
    fn a_unc_project_on_its_own_share_is_walked_on_windows() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let local = tmp.path().join("x.snxsch");
        write_sheet(&local, &[]);
        // The administrative share of the drive the temp dir is on.
        let text = local.to_string_lossy().into_owned();
        let (drive, rest) = text.split_at(2);
        let root = PathBuf::from(format!(r"\\localhost\{}${rest}", &drive[..1]));
        if std::fs::metadata(&root).is_err() {
            eprintln!("skipped: no administrative share for {root:?}");
            return;
        }
        let trusted: Vec<Share> = share_of(&root).into_iter().collect();

        let resolved = SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root],
            projects: Vec::new(),
            extra: Vec::new(),
            trusted,
            project_dirs: Vec::new(),
        }
        .resolve(|_| true);

        assert!(resolved.unreadable.is_empty(), "{:?}", resolved.unreadable);
        assert_eq!(resolved.disk.len(), 1, "{:?}", resolved.disk.keys());
    }

    /// A project dir spelled locally (a symlink here, a mapped drive in the
    /// wild) whose real path is on a share: only `project_dirs` can make the
    /// share trusted, so the same walk without it must refuse the root.
    #[cfg(windows)]
    #[test]
    fn project_dirs_make_the_real_share_of_a_local_spelling_trusted_on_windows() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let real = tmp.path().join("real");
        std::fs::create_dir_all(&real).expect("mkdir");
        write_sheet(&real.join("x.snxsch"), &[]);
        let text = real.to_string_lossy().into_owned();
        let (drive, rest) = text.split_at(2);
        let unc_dir = PathBuf::from(format!(r"\\localhost\{}${rest}", &drive[..1]));
        if std::fs::metadata(&unc_dir).is_err() {
            eprintln!("skipped: no administrative share for {unc_dir:?}");
            return;
        }
        let link = tmp.path().join("link");
        if std::os::windows::fs::symlink_dir(&unc_dir, &link).is_err() {
            eprintln!("skipped: cannot create a directory symlink");
            return;
        }
        let root = unc_dir.join("x.snxsch");
        let inputs = |project_dirs: Vec<PathBuf>| SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root.clone()],
            projects: Vec::new(),
            extra: Vec::new(),
            trusted: Vec::new(),
            project_dirs,
        };

        let without = inputs(Vec::new()).resolve(|_| true);
        assert!(
            !without.unreadable.is_empty(),
            "refused without project_dirs"
        );
        assert!(without.disk.is_empty());

        let with = inputs(vec![link]).resolve(|_| true);
        assert!(with.unreadable.is_empty(), "{:?}", with.unreadable);
        assert_eq!(with.disk.len(), 1, "{:?}", with.disk.keys());
    }

    #[cfg(windows)]
    #[test]
    fn a_long_dotdot_chain_from_a_unc_project_stays_on_its_share_on_windows() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let local = tmp.path().join("x.snxsch");
        let deep = format!("{}other\\share\\y.snxsch", r"..\".repeat(30));
        write_sheet(&local, &[&deep, r"\\?\UNC\localhost/c$\z.snxsch"]);
        let text = local.to_string_lossy().into_owned();
        let (drive, rest) = text.split_at(2);
        let root = PathBuf::from(format!(r"\\localhost\{}${rest}", &drive[..1]));
        if std::fs::metadata(&root).is_err() {
            eprintln!("skipped: no administrative share for {root:?}");
            return;
        }
        let trusted: Vec<Share> = share_of(&root).into_iter().collect();

        let resolved = SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root],
            projects: Vec::new(),
            extra: Vec::new(),
            trusted: trusted.clone(),
            project_dirs: Vec::new(),
        }
        .resolve(|_| true);

        // Every sheet the walk touched, read or not, is on the trusted share;
        // only the slash-bearing verbatim spelling is refused.
        let share = trusted.first().cloned();
        for path in resolved.disk.keys() {
            assert_eq!(share_of(path), share, "{path:?}");
        }
        let refused = |s: &UnreadableSheet| s.reason.contains("network or device");
        for sheet in resolved.unreadable.iter().filter(|s| !refused(s)) {
            assert_eq!(share_of(&sheet.path), share, "{sheet:?}");
        }
        assert_eq!(
            resolved.unreadable.iter().filter(|s| refused(s)).count(),
            1,
            "{:?}",
            resolved.unreadable
        );
    }

    #[test]
    fn an_untrusted_network_library_path_is_not_canonicalised() {
        // `extra` carries `.snxprj` library paths; a refused one is keyed
        // lexically, so no real spelling is recorded (the canonicalise that
        // would touch the network never ran).
        let lib = PathBuf::from(r"\\attacker\share\lib.snxlib");
        let resolved = SheetRefInputs {
            loaded: HashMap::new(),
            roots: Vec::new(),
            projects: Vec::new(),
            extra: vec![lib.clone()],
            trusted: Vec::new(),
            project_dirs: Vec::new(),
        }
        .resolve_within(|_| false, WalkLimits::default());

        assert!(!resolved.keys.knows(&lib));
    }

    #[cfg(unix)]
    #[test]
    fn a_device_reference_is_not_read_and_disables_discard_all() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("x.snxsch");
        write_sheet(&root, &["/dev/zero"]);

        let resolved = resolve_root(&root, WalkLimits::default());

        assert_discard_blocked_by(&resolved, "zero (is not a regular file)");
    }

    #[cfg(windows)]
    #[test]
    fn a_windows_device_name_is_refused_unopened_as_root_child_and_extra() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("x.snxsch");
        write_sheet(
            &root,
            &[
                "nul.snxsch",
                "aux .snxsch",
                r"folder\LPT9.snxsch",
                "COM1.txt",
            ],
        );
        let inputs = SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root.clone(), tmp.path().join("NUL")],
            projects: Vec::new(),
            extra: vec![tmp.path().join("PRN")],
            project_dirs: Vec::new(),
            trusted: Vec::new(),
        };

        let resolved = inputs.resolve_within(|_| false, WalkLimits::default());

        // Four children and the NUL root are refused and reported.
        assert_eq!(resolved.unreadable.len(), 5, "{:?}", resolved.unreadable);
        assert_discard_blocked_by(&resolved, "network or device");
        // None was canonicalised: no real path was recorded for a device name.
        for device in ["nul", "aux", "lpt9", "com1", "prn"] {
            assert!(
                resolved
                    .keys
                    .real
                    .keys()
                    .all(|k| !k.to_lowercase().contains(device)),
                "{device}: {:?}",
                resolved.keys.real
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_device_name_used_as_a_folder_is_never_canonicalised() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut volume = VolumeCase::new(|_| true);

        for folder in ["NUL", "com1", "aux .d"] {
            let path = tmp.path().join(folder).join("x.snxsch");
            let real = real_path(&path, &mut volume);
            // Canonicalising any ancestor would yield a verbatim `\?\` path;
            // the device ancestor must stop the walk before that.
            assert!(
                !real.to_string_lossy().starts_with(r"\?\"),
                "{folder}: {real:?}"
            );
            assert_eq!(
                real,
                PathBuf::from(path.to_string_lossy().to_lowercase()),
                "{folder}"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn a_device_folder_in_a_child_or_extra_path_is_keyed_without_canonicalising() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("x.snxsch");
        write_sheet(&root, &[r"NUL\x.snxsch"]);
        let inputs = SheetRefInputs {
            loaded: HashMap::new(),
            roots: vec![root.clone()],
            projects: Vec::new(),
            extra: vec![tmp.path().join("NUL").join("lib.snxlib")],
            project_dirs: Vec::new(),
            trusted: Vec::new(),
        };

        let resolved = inputs.resolve_within(|_| false, WalkLimits::default());

        for real in resolved.keys.real.values() {
            let text = real.to_string_lossy();
            if text.to_lowercase().contains("nul") {
                assert!(!text.starts_with(r"\?\"), "{real:?}");
            }
        }
        for sheet in &resolved.unreadable {
            let label = sheet.label();
            assert!(
                !label.contains("os error 87") && !label.contains("os error 1"),
                "{label}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_fifo_reference_is_not_opened_and_disables_discard_all() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let fifo = tmp.path().join("pipe.snxsch");
        let made = std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .expect("mkfifo runs");
        assert!(made.success());
        let root = tmp.path().join("x.snxsch");
        write_sheet(&root, &["pipe.snxsch"]);

        let resolved = resolve_root(&root, WalkLimits::default());

        assert_discard_blocked_by(&resolved, "pipe.snxsch (is not a regular file)");
    }

    #[test]
    fn an_oversized_sheet_is_unreadable_and_disables_discard_all() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("x.snxsch");
        write_sheet(&root, &["big.snxsch"]);
        std::fs::write(tmp.path().join("big.snxsch"), vec![b'x'; 4096]).expect("write");
        let limits = WalkLimits {
            max_sheet_bytes: 1024,
            ..WalkLimits::default()
        };

        let resolved = resolve_root(&root, limits);

        assert_discard_blocked_by(&resolved, "big.snxsch (is larger than 1024 bytes)");
    }

    #[test]
    fn a_walk_past_the_sheet_cap_stops_and_disables_discard_all() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // root -> s0 -> s1 -> ... a chain longer than the cap.
        let root = tmp.path().join("root.snxsch");
        write_sheet(&root, &["s0.snxsch"]);
        for n in 0..10 {
            let next = format!("s{}.snxsch", n + 1);
            write_sheet(&tmp.path().join(format!("s{n}.snxsch")), &[next.as_str()]);
        }
        let limits = WalkLimits {
            max_sheets: 4,
            ..WalkLimits::default()
        };

        let resolved = resolve_root(&root, limits);

        assert!(resolved.truncated);
        assert!(resolved.disk.len() <= 4, "{:?}", resolved.disk.keys());
        assert_discard_blocked_by(&resolved, "stopped after 4 sheets");
    }

    #[test]
    fn a_walk_within_the_cap_is_not_truncated() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("root.snxsch");
        write_sheet(&root, &[]);

        let resolved = resolve_root(&root, WalkLimits::default());

        assert!(!resolved.truncated);
        assert!(resolved.discard_blocked_reason().is_none());
    }

    #[test]
    fn stale_causes_name_each_thing_that_changed() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("A");
        std::fs::create_dir_all(&dir).expect("mkdir");
        write_sheet(&dir.join("a.snxsch"), &[]);
        let (mut app, _task) = crate::app::Signex::new();
        let docs = &mut app.document_state;
        docs.projects.push(loaded_project(1, &dir, "a.snxsch"));
        let refs = docs.sheet_ref_inputs().resolve(|_| false);
        assert!(docs.stale_causes(&refs).is_empty());

        docs.dirty_paths.insert(dir.join("fresh.snxsch"));
        assert_eq!(docs.stale_causes(&refs), vec![StaleRefs::NewDirtyDocument]);

        docs.dirty_paths.clear();
        docs.projects.push(loaded_project(2, &dir, "a.snxsch"));
        assert_eq!(docs.stale_causes(&refs), vec![StaleRefs::ProjectsChanged]);
        assert_eq!(
            StaleRefs::describe_all(&[StaleRefs::ProjectsChanged, StaleRefs::NewDirtyDocument]),
            "the open projects changed; a document with unsaved changes was added",
        );
    }

    #[test]
    fn an_open_sheets_saved_references_are_followed_as_well_as_its_own() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let base = tmp.path().canonicalize().expect("canonical tmp");
        let root = base.join("root.snxsch");
        write_sheet(&root, &["saved.snxsch"]);
        write_sheet(&base.join("saved.snxsch"), &[]);
        write_sheet(&base.join("edited.snxsch"), &[]);
        // The open copy dropped `saved` and added `edited`.
        let inputs = SheetRefInputs {
            loaded: HashMap::from([(root.clone(), vec!["edited.snxsch".to_string()])]),
            roots: vec![root.clone()],
            projects: Vec::new(),
            extra: Vec::new(),
            trusted: Vec::new(),
            project_dirs: Vec::new(),
        };

        let resolved = inputs.resolve(|_| false);

        assert!(resolved.disk.contains_key(&base.join("saved.snxsch")));
        assert!(resolved.disk.contains_key(&base.join("edited.snxsch")));
        assert_eq!(
            resolved.disk.get(&root),
            Some(&vec!["saved.snxsch".to_string()]),
            "`disk` holds the saved file's references; the engine's merge at use time",
        );
    }
}
