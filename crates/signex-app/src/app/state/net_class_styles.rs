//! The resolved net-class style of every drawn wire and junction.
//!
//! The class table lives in the `.snxprj`; a net's members come from the
//! project's stitched netlist, so a net that reaches `VCC` only through a
//! sheet entry is styled on the child sheet exactly as on the parent. The
//! canvas needs only the join of the two, per element, and reads it from here
//! rather than from the project, so `view` never touches project data.
//!
//! Resolution is per project and keyed by a [`ProjectKey`]: everything the
//! answer depends on. A project is re-resolved only when its key changed, and
//! an answer is applied only if its key is still the one wanted for that
//! project, or differs from it only in connectivity and is newer than what is
//! stored (see [`NetClassStyleCache::apply`]). A late answer for a project that
//! has not changed since is kept; one for a changed class table, page list or
//! root is dropped. Work for one project never invalidates another's.

use std::collections::HashMap;
use std::path::PathBuf;

use signex_types::net_class::{NetClass, NetClassStyle};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use super::scope::CaseFold;
use super::{LoadedProject, ProjectId};

/// Everything one project's resolution depends on. Two equal keys resolve to
/// the same styles.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectKey {
    /// Stamp over the project's open sheets (which sheets, which engine, which
    /// connectivity revision). Moves on an edit that can change a net, not on
    /// one that cannot (moving a text note).
    pub connectivity: u64,
    /// The project's declared pages: the stitcher reads them from disk.
    pub pages: Vec<PathBuf>,
    /// The sheet the netlist is rooted at.
    pub root: PathBuf,
    /// The project's key rules: they decide whether two spellings of a child
    /// sheet are one file, so a probe that moves them changes the stitch.
    pub fold: CaseFold,
    pub classes: Vec<NetClass>,
}

impl ProjectKey {
    /// `true` when `other` differs from `self` at most in the connectivity
    /// stamp: the same class table, pages, root and key rules. An answer for
    /// such a key is the right join for older connectivity.
    #[must_use]
    pub fn same_inputs_as(&self, other: &Self) -> bool {
        self.pages == other.pages
            && self.root == other.root
            && self.fold == other.fold
            && self.classes == other.classes
    }
}

/// The request a project's answer is awaited for.
struct Wanted {
    key: ProjectKey,
    /// Orders requests of one project: a larger number was asked later.
    seq: u64,
}

struct ProjectEntry {
    key: ProjectKey,
    /// The request this answer came from.
    seq: u64,
    styles: HashMap<Uuid, NetClassStyle>,
    nets: HashMap<Uuid, String>,
    placed_styles: PlacedStyles,
    placed_nets: PlacedNets,
    warnings: Vec<String>,
}

/// Class styles per sheet placement, then per element.
pub type PlacedStyles = HashMap<InstancePath, HashMap<Uuid, NetClassStyle>>;
/// Net names per sheet placement, then per element.
pub type PlacedNets = HashMap<InstancePath, HashMap<Uuid, String>>;

/// What [`NetClassStyleCache::needs_refresh`] saw last time.
#[derive(Default)]
struct Gate {
    sheets: u64,
    projects: Vec<GateProject>,
}

struct GateProject {
    id: ProjectId,
    classes: Vec<NetClass>,
    root: Option<String>,
    sheet_files: Vec<String>,
    case_fold: CaseFold,
}

impl GateProject {
    fn of(project: &LoadedProject) -> Self {
        Self {
            id: project.id,
            classes: project.data.net_classes.clone(),
            root: project.data.schematic_root.clone(),
            sheet_files: project
                .data
                .sheets
                .iter()
                .map(|s| s.filename.clone())
                .collect(),
            case_fold: project.case_fold,
        }
    }

    fn matches(&self, project: &LoadedProject) -> bool {
        self.id == project.id
            && self.classes == project.data.net_classes
            && self.root == project.data.schematic_root
            && self.case_fold == project.case_fold
            && self
                .sheet_files
                .iter()
                .eq(project.data.sheets.iter().map(|s| &s.filename))
    }
}

/// One project's resolution, computed off the UI thread.
#[derive(Debug)]
pub struct ResolvedProject {
    pub project: ProjectId,
    pub key: ProjectKey,
    /// The request number the cache gave this resolution
    /// ([`NetClassStyleCache::request_seq`]); orders answers of one project.
    pub seq: u64,
    pub styles: HashMap<Uuid, NetClassStyle>,
    /// Net name of every wire and junction, for the label picker. Empty for a
    /// project with no classes.
    pub nets: HashMap<Uuid, String>,
    /// [`Self::styles`] per placement: a sheet placed twice draws each
    /// placement's own nets.
    pub placed_styles: PlacedStyles,
    /// [`Self::nets`] per placement.
    pub placed_nets: PlacedNets,
    /// Sheets that would not parse and stitch issues, for the Messages panel.
    pub warnings: Vec<String>,
}

/// What a background resolution hands back: the projects it recomputed.
#[derive(Debug)]
pub struct ResolvedStyles {
    pub projects: Vec<ResolvedProject>,
}

/// A background resolution that never produced an answer (its task panicked),
/// and each project with the key it was asked to resolve, so only a request
/// for that very key is forgotten.
#[derive(Debug)]
pub struct ResolveFailed {
    pub projects: Vec<(ProjectId, ProjectKey)>,
    pub reason: String,
}

/// What applying a resolution changed, so the caller knows what to redraw.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Applied {
    /// The merged style map changed: the schematic canvases must repaint.
    pub styles_changed: bool,
    /// A project's element-to-net names changed: the picker must be rebuilt.
    pub nets_changed: bool,
    /// Warnings that are new since the project's previous resolution.
    pub warnings: Vec<String>,
}

#[derive(Default)]
pub struct NetClassStyleCache {
    gate: Gate,
    /// The key each project was last asked to resolve for: the answer in
    /// flight, or the one already stored.
    wanted: HashMap<ProjectId, Wanted>,
    /// Last request number handed out.
    last_seq: u64,
    /// The key whose resolution failed for each project. It is not asked for
    /// again until the project's key moves, so a task that fails every time
    /// reports once instead of on every click.
    failed: HashMap<ProjectId, ProjectKey>,
    projects: HashMap<ProjectId, ProjectEntry>,
    merged: HashMap<Uuid, NetClassStyle>,
    /// `merged`, per placement.
    merged_placed: PlacedStyles,
    /// What a placement with no styled element reads.
    no_styles: HashMap<Uuid, NetClassStyle>,
    /// Counts rebuilds of `merged`, so a consumer that caches something drawn
    /// from the map can tell it changed without comparing it entry by entry.
    revision: u64,
}

impl NetClassStyleCache {
    /// Element uuid to class style, over every cached project. Element uuids
    /// are unique across sheets, so one flat map serves every window.
    #[must_use]
    pub fn styles(&self) -> &HashMap<Uuid, NetClassStyle> {
        &self.merged
    }

    /// The styles drawn on a sheet shown through `placement`: its own
    /// placement's nets, so a child placed twice draws each placement's class.
    /// With no known placement, the flat map ([`Self::styles`]).
    #[must_use]
    pub fn styles_in(&self, placement: Option<&InstancePath>) -> &HashMap<Uuid, NetClassStyle> {
        match placement {
            None => &self.merged,
            Some(p) => self.merged_placed.get(p).unwrap_or(&self.no_styles),
        }
    }

    /// [`Self::revision`] for what [`Self::styles_in`] returns for
    /// `placement`: moves when the styles change or the placement does.
    #[must_use]
    pub fn revision_in(&self, placement: Option<&InstancePath>) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (self.revision, placement).hash(&mut hasher);
        hasher.finish()
    }

    /// Moves at least whenever [`Self::styles`] changes (it may also move on a
    /// rebuild that left the map as it was), so the scene cache compares one
    /// number per frame instead of the whole map.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The key `project` was last asked to resolve for, if any.
    #[must_use]
    pub fn wanted_key(&self, project: ProjectId) -> Option<&ProjectKey> {
        self.wanted.get(&project).map(|w| &w.key)
    }

    /// The number of the request `project` is awaiting, for the worker to
    /// stamp its answer with; `0` when none.
    #[must_use]
    pub fn request_seq(&self, project: ProjectId) -> u64 {
        self.wanted.get(&project).map_or(0, |w| w.seq)
    }

    /// `true` when no element carries a class style, so a caller with no
    /// class table anywhere can skip gathering inputs.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.merged.is_empty()
    }

    /// The name the project netlist gives the net of `element` (a wire or
    /// junction) in `project`, or `None` while that project has not been
    /// resolved or has no classes.
    #[must_use]
    pub fn net_of(&self, project: ProjectId, element: Uuid) -> Option<&str> {
        self.projects
            .get(&project)?
            .nets
            .get(&element)
            .map(String::as_str)
    }

    /// [`Self::net_of`] in the sheet placement the canvas shows: the net of
    /// that placement, so a picker names the net net navigation names. With no
    /// known placement, the flat answer.
    #[must_use]
    pub fn net_in(
        &self,
        project: ProjectId,
        element: Uuid,
        placement: Option<&InstancePath>,
    ) -> Option<&str> {
        let Some(placement) = placement else {
            return self.net_of(project, element);
        };
        self.projects
            .get(&project)?
            .placed_nets
            .get(placement)?
            .get(&element)
            .map(String::as_str)
    }

    /// `true` when the open sheets or the loaded projects differ from the
    /// last call: `sheets` is a stamp over every open sheet's connectivity
    /// revision, and the projects are compared by class table, root, sheet
    /// list and case fold. An update that touched neither (a pointer move, a text-note edit)
    /// returns `false` without any per-project work. Records what it saw, so
    /// call it once per update and plan when it says so.
    pub fn needs_refresh(&mut self, sheets: u64, projects: &[LoadedProject]) -> bool {
        let unchanged = self.gate.sheets == sheets
            && self.gate.projects.len() == projects.len()
            && self
                .gate
                .projects
                .iter()
                .zip(projects)
                .all(|(seen, project)| seen.matches(project));
        if unchanged {
            return false;
        }
        self.gate = Gate {
            sheets,
            projects: projects.iter().map(GateProject::of).collect(),
        };
        true
    }

    /// Forget what the gate last saw, so the next [`Self::needs_refresh`]
    /// opens it.
    pub fn reopen_gate(&mut self) {
        self.gate = Gate::default();
    }

    /// Record that `project` should hold the resolution for `key`. Returns
    /// `true` when a worker has to compute it: the cache neither holds it nor
    /// has it in flight. A project moving back to a key the cache already
    /// holds cancels the answer in flight for the key it left.
    pub fn want(&mut self, project: ProjectId, key: &ProjectKey) -> bool {
        if self.failed.get(&project) == Some(key) {
            return false;
        }
        self.failed.remove(&project);
        let held = self
            .projects
            .get(&project)
            .filter(|entry| entry.key == *key);
        let in_flight = self.wanted.get(&project).is_some_and(|w| w.key == *key);
        if !in_flight {
            let seq = match held {
                Some(entry) => entry.seq,
                None => {
                    self.last_seq += 1;
                    self.last_seq
                }
            };
            self.wanted.insert(
                project,
                Wanted {
                    key: key.clone(),
                    seq,
                },
            );
        }
        held.is_none() && !in_flight
    }

    /// Drop every project not in `keep`. Returns `true` when that changed the
    /// merged style map.
    pub fn retain_projects(&mut self, keep: &[ProjectId]) -> bool {
        self.wanted.retain(|id, _| keep.contains(id));
        let before = self.projects.len();
        self.projects.retain(|id, _| keep.contains(id));
        let dropped = self.projects.len() != before;
        if dropped {
            self.rebuild_merged();
        }
        dropped
    }

    /// A background resolution failed, so no answer is coming for the keys it
    /// ran: forget a request still in flight for such a key and remember it as
    /// failed, so it is not retried (and not reported again) until the
    /// project's key moves. A project whose wanted key has moved on since, and
    /// other projects, are left alone.
    pub fn give_up(&mut self, failed: &[(ProjectId, ProjectKey)]) {
        for (id, key) in failed {
            if self.wanted.get(id).is_none_or(|w| w.key != *key) {
                continue;
            }
            if self.projects.get(id).is_none_or(|entry| entry.key != *key) {
                self.failed.insert(*id, key.clone());
                self.wanted.remove(id);
            }
        }
    }

    /// Forget every request in flight and every recorded failure, and reopen
    /// the gate, for a caller that lost its tasks.
    pub fn give_up_pending(&mut self) {
        self.forget_pending(|_| true);
        self.failed.clear();
        self.reopen_gate();
    }

    fn forget_pending(&mut self, forget: impl Fn(&ProjectId) -> bool) {
        let projects = &self.projects;
        self.wanted.retain(|id, wanted| {
            !forget(id)
                || projects
                    .get(id)
                    .is_some_and(|entry| entry.key == wanted.key)
        });
    }

    /// Apply a background resolution. An answer for the key now wanted lands.
    /// So does one for older connectivity of the same inputs (same class
    /// table, pages, root and rules) that is newer than what is stored and
    /// older than the request now awaited, so styles keep following an edit
    /// that outruns its resolve; the newer answer replaces it when it comes.
    /// Any other answer is skipped; the styles already drawn stay.
    pub fn apply(&mut self, resolved: &ResolvedStyles) -> Applied {
        let mut applied = Applied::default();
        for entry in &resolved.projects {
            let previous = self.projects.get(&entry.project);
            let Some(wanted) = self.wanted.get(&entry.project) else {
                continue;
            };
            let current = wanted.key == entry.key;
            let older_connectivity = wanted.key.same_inputs_as(&entry.key)
                && entry.seq < wanted.seq
                && previous.is_none_or(|old| entry.seq > old.seq);
            if !current && !older_connectivity {
                continue;
            }
            applied.styles_changed |= previous.map_or(
                !entry.styles.is_empty() || !entry.placed_styles.is_empty(),
                |old| old.styles != entry.styles || old.placed_styles != entry.placed_styles,
            );
            applied.nets_changed |= previous.map_or(
                !entry.nets.is_empty() || !entry.placed_nets.is_empty(),
                |old| old.nets != entry.nets || old.placed_nets != entry.placed_nets,
            );
            let old_warnings = previous.map(|old| old.warnings.as_slice()).unwrap_or(&[]);
            applied.warnings.extend(
                entry
                    .warnings
                    .iter()
                    .filter(|w| !old_warnings.contains(w))
                    .cloned(),
            );
            self.projects.insert(
                entry.project,
                ProjectEntry {
                    key: entry.key.clone(),
                    seq: entry.seq,
                    styles: entry.styles.clone(),
                    nets: entry.nets.clone(),
                    placed_styles: entry.placed_styles.clone(),
                    placed_nets: entry.placed_nets.clone(),
                    warnings: entry.warnings.clone(),
                },
            );
        }
        if applied.styles_changed {
            self.rebuild_merged();
        }
        applied
    }

    fn rebuild_merged(&mut self) {
        self.revision += 1;
        self.merged = self
            .projects
            .values()
            .flat_map(|entry| entry.styles.iter().map(|(id, style)| (*id, *style)))
            .collect();
        // Element uuids are unique across sheets, so two projects' maps for
        // the same placement path (the root's, say) never collide.
        self.merged_placed.clear();
        for entry in self.projects.values() {
            for (placement, styles) in &entry.placed_styles {
                self.merged_placed
                    .entry(placement.clone())
                    .or_default()
                    .extend(styles.iter().map(|(id, style)| (*id, *style)));
            }
        }
    }
}

#[cfg(test)]
mod tests;
