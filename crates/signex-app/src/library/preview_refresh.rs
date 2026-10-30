//! Refreshing resolved previews after a primitive save (#968).
//!
//! A Component Preview tab resolves its row's symbol and footprint once
//! and keeps them on [`ComponentPreviewState`]. Saving the primitive in
//! its own editor wrote new bytes, but the tab went on drawing the copy
//! it had resolved before, until it was closed and reopened.
//!
//! The lookups read files, so `update` only plans them
//! ([`LibraryState::plan_preview_refresh`]); [`PreviewRefreshPlan::read`]
//! runs on the blocking pool ([`preview_refresh_task`]) and
//! [`LibraryState::apply_preview_readout`] applies the answer if it is
//! still the newest one for its library (#859).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use signex_library::{
    Footprint, LibraryAdapter, LibraryError, PrimitiveRef, Symbol, absence_is_none,
};
use uuid::Uuid;

use super::resolve::{ResolvedKind, report_read_failure};
use super::state::{EditorAddress, LibraryState};

/// One preview primitive to re-resolve.
struct PreviewItem {
    address: EditorAddress,
    reference: PrimitiveRef,
    kind: ResolvedKind,
}

/// What [`LibraryState::plan_preview_refresh`] decided to re-read.
pub struct PreviewRefreshPlan {
    saved_library: Uuid,
    generation: u64,
    root: PathBuf,
    items: Vec<PreviewItem>,
    adapters: HashMap<Uuid, Arc<dyn LibraryAdapter>>,
}

enum Resolved {
    Symbol(Box<Result<Option<Symbol>, LibraryError>>),
    Footprint(Box<Result<Option<Footprint>, LibraryError>>),
}

/// The answers of one [`PreviewRefreshPlan::read`], not yet applied.
pub struct PreviewReadout {
    saved_library: Uuid,
    generation: u64,
    resolved: Vec<(PreviewItem, Resolved)>,
}

/// One-shot carrier for a readout across the `Task::perform` boundary
/// (`LibraryError` is not `Clone`). `None` inside = the worker panicked.
#[derive(Clone)]
pub struct PreviewReadoutCell(Arc<Mutex<Option<Option<PreviewReadout>>>>);

impl PreviewReadoutCell {
    fn new(readout: Option<PreviewReadout>) -> Self {
        Self(Arc::new(Mutex::new(Some(readout))))
    }

    /// Take the payload; `None` on a second read.
    pub fn take(&self) -> Option<Option<PreviewReadout>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
}

impl std::fmt::Debug for PreviewReadoutCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PreviewReadoutCell(..)")
    }
}

impl PreviewRefreshPlan {
    /// Resolve every planned primitive. Pure with respect to app state;
    /// safe on any thread. A library that is not mounted answers
    /// `Ok(None)`, as `LibrarySet::resolve_*` does.
    pub fn read(self) -> PreviewReadout {
        let Self {
            saved_library,
            generation,
            items,
            adapters,
            ..
        } = self;
        let resolved = items
            .into_iter()
            .map(|item| {
                let adapter = adapters.get(&item.reference.library_id);
                let uuid = item.reference.uuid;
                let answer = match item.kind {
                    ResolvedKind::Footprint => Resolved::Footprint(Box::new(match adapter {
                        Some(a) => absence_is_none(a.get_footprint(uuid)),
                        None => Ok(None),
                    })),
                    _ => Resolved::Symbol(Box::new(match adapter {
                        Some(a) => absence_is_none(a.get_symbol(uuid)),
                        None => Ok(None),
                    })),
                };
                (item, answer)
            })
            .collect();
        PreviewReadout {
            saved_library,
            generation,
            resolved,
        }
    }
}

impl LibraryState {
    /// Plan the re-resolution of every Component Preview whose row binds a
    /// primitive in the library whose directory contains `saved`. The
    /// binding's `library_id` decides, not the library the preview tab
    /// belongs to: a row may bind a primitive from another library.
    ///
    /// Only previews that already resolved a primitive are planned; one
    /// that has not been visited stays lazy. The sim is left alone: its
    /// `sim_body` is a live edit buffer and replacing it would discard
    /// typing. `None` when `saved` is in no open library or nothing needs
    /// re-resolving. Each plan supersedes the earlier one for its library.
    pub fn plan_preview_refresh(&mut self, saved: &Path) -> Option<PreviewRefreshPlan> {
        let (saved_library, root) = self
            .open_libraries
            .iter()
            .find(|lib| {
                lib.root_dir()
                    .map(|d| saved.starts_with(d))
                    .unwrap_or(false)
            })
            .map(|lib| (lib.library_id, lib.root.clone()))?;
        let mut items = Vec::new();
        for (address, state) in &self.editors {
            if state.symbol.is_some() && state.row.symbol_ref.library_id == saved_library {
                items.push(PreviewItem {
                    address: address.clone(),
                    reference: state.row.symbol_ref,
                    kind: ResolvedKind::Symbol,
                });
            }
            if state.footprint.is_some()
                && let Some(reference) = state.row.footprint_ref
                && reference.library_id == saved_library
            {
                items.push(PreviewItem {
                    address: address.clone(),
                    reference,
                    kind: ResolvedKind::Footprint,
                });
            }
        }
        if items.is_empty() {
            return None;
        }
        let adapters = items
            .iter()
            .filter_map(|item| {
                let id = item.reference.library_id;
                self.set.get_shared(id).map(|adapter| (id, adapter))
            })
            .collect();
        self.refresh_generation_counter += 1;
        let generation = self.refresh_generation_counter;
        self.preview_generations.insert(saved_library, generation);
        Some(PreviewRefreshPlan {
            saved_library,
            generation,
            root,
            items,
            adapters,
        })
    }

    /// Apply a readout unless it went stale: its library was closed, or a
    /// newer plan for it was made. A preview that was replaced, closed or
    /// re-bound while the read ran keeps what it has. A lookup that failed
    /// keeps the previous copy and reports the failure, rather than
    /// blanking a preview over a transient read error.
    pub fn apply_preview_readout(&mut self, readout: PreviewReadout) {
        let still_open = self
            .open_libraries
            .iter()
            .any(|lib| lib.library_id == readout.saved_library);
        if !still_open
            || self.preview_generations.get(&readout.saved_library) != Some(&readout.generation)
        {
            return;
        }
        for (item, answer) in readout.resolved {
            let Some(state) = self.editors.get_mut(&item.address) else {
                continue;
            };
            match answer {
                Resolved::Symbol(result)
                    if state.symbol.is_some() && state.row.symbol_ref == item.reference =>
                {
                    if let Some(fresh) = reresolve(*result, item.kind, &item.reference) {
                        state.symbol = fresh;
                    }
                }
                Resolved::Footprint(result)
                    if state.footprint.is_some()
                        && state.row.footprint_ref == Some(item.reference) =>
                {
                    if let Some(fresh) = reresolve(*result, item.kind, &item.reference) {
                        state.footprint = fresh;
                    }
                }
                _ => {}
            }
        }
    }

    /// Plan, read and apply on the calling thread. For the one save path
    /// that is blocking by design (`save_primitive_tab_blocking`); an
    /// `update` handler uses [`preview_refresh_task`].
    pub fn refresh_loaded_previews(&mut self, saved: &Path) {
        if let Some(plan) = self.plan_preview_refresh(saved) {
            self.apply_preview_readout(plan.read());
        }
    }
}

/// Re-resolve the previews of `saved`'s library off the UI thread. Yields
/// no task when there is nothing to re-resolve.
pub fn preview_refresh_task(
    state: &mut LibraryState,
    saved: &Path,
) -> iced::Task<crate::app::Message> {
    let Some(plan) = state.plan_preview_refresh(saved) else {
        return iced::Task::none();
    };
    // Behind every write of this library started so far, so the read never
    // sees a file half-way through a rewrite (#893).
    let turn = state.writes.read_turn(&plan.root);
    iced::Task::perform(
        async move {
            let readout = tokio::task::spawn_blocking(move || {
                turn.wait();
                plan.read()
            })
            .await
            .ok();
            PreviewReadoutCell::new(readout)
        },
        |cell| {
            crate::app::Message::Library(super::LibraryMessage::PreviewsReread { readout: cell })
        },
    )
}

/// `Some(copy)` when the lookup answered (`None` inside = the primitive
/// is gone), `None` when it failed and the old copy must stay.
fn reresolve<T>(
    resolved: Result<Option<T>, LibraryError>,
    kind: ResolvedKind,
    reference: &PrimitiveRef,
) -> Option<Option<T>> {
    if resolved.is_err() {
        report_read_failure(
            resolved,
            kind,
            reference,
            "refreshing a preview after a primitive save",
        );
        return None;
    }
    resolved.ok()
}
