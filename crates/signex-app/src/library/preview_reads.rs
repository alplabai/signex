//! Disk reads of the Component Preview editors, off the UI thread (#859).
//!
//! Two reads used to run inside `update`: re-reading a row from its
//! library ([`LibraryState::plan_editor_reload`]) and resolving the symbol,
//! footprint or simulation model a preview shows the first time its tab is
//! visited ([`LibraryState::plan_tab_resolve`]). Each is split into a plan
//! made in `update`, a read that runs on the blocking pool behind the
//! library's write order, and an apply that checks the answer against the
//! state it finds when it lands.

use std::sync::Arc;

use signex_library::{
    ComponentRow, Footprint, LibraryAdapter, LibraryError, PrimitiveRef, SimModel, Symbol,
};
use uuid::Uuid;

use super::resolve::{ResolvedKind, report_read_failure};
use super::shared_resolver::{OneShotCell, SharedResolver};
use super::state::{ComponentPreviewState, EditorAddress, LibraryState, PreviewTab};

/// Which primitives a preview tab needs that the preview does not hold yet.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PreviewNeeds {
    symbol: Option<PrimitiveRef>,
    footprint: Option<PrimitiveRef>,
    sim: Option<PrimitiveRef>,
}

impl PreviewNeeds {
    /// What showing `tab` for a preview holding `state`'s resolved copies
    /// has to look up: the Preview tab wants the symbol and footprint, the
    /// Simulation tab the model, the others nothing.
    fn for_tab(row: &ComponentRow, tab: PreviewTab, held: Option<&ComponentPreviewState>) -> Self {
        let has_symbol = held.is_some_and(|s| s.symbol.is_some());
        let has_footprint = held.is_some_and(|s| s.footprint.is_some());
        let has_sim = held.is_some_and(|s| s.sim.is_some());
        match tab {
            PreviewTab::Preview => Self {
                symbol: (!has_symbol).then_some(row.symbol_ref),
                footprint: row.footprint_ref.filter(|_| !has_footprint),
                sim: None,
            },
            PreviewTab::Simulation => Self {
                sim: row.sim_ref.filter(|_| !has_sim),
                ..Self::default()
            },
            _ => Self::default(),
        }
    }

    fn is_empty(&self) -> bool {
        self.symbol.is_none() && self.footprint.is_none() && self.sim.is_none()
    }

    fn read(&self, resolver: &SharedResolver) -> PreviewReads {
        PreviewReads {
            symbol: self.symbol.map(|r| resolver.resolve_symbol(&r)),
            footprint: self.footprint.map(|r| resolver.resolve_footprint(&r)),
            sim: self.sim.map(|r| resolver.resolve_sim(&r)),
        }
    }
}

/// The answers to a [`PreviewNeeds`].
pub struct PreviewReads {
    symbol: Option<Result<Option<Symbol>, LibraryError>>,
    footprint: Option<Result<Option<Footprint>, LibraryError>>,
    sim: Option<Result<Option<SimModel>, LibraryError>>,
}

impl PreviewReads {
    /// Fill the slots of `state` that were looked up, unless they were
    /// filled or the row was re-bound since. A failed lookup is reported to
    /// the Messages panel and leaves the slot empty, as a lazy resolve
    /// always did.
    fn apply_to(self, needs: &PreviewNeeds, state: &mut ComponentPreviewState) {
        if let (Some(r), Some(result)) = (needs.symbol, self.symbol)
            && state.symbol.is_none()
            && state.row.symbol_ref == r
        {
            state.symbol =
                report_read_failure(result, ResolvedKind::Symbol, &r, "component preview tab");
        }
        if let (Some(r), Some(result)) = (needs.footprint, self.footprint)
            && state.footprint.is_none()
            && state.row.footprint_ref == Some(r)
        {
            state.footprint =
                report_read_failure(result, ResolvedKind::Footprint, &r, "component preview tab");
        }
        if let (Some(r), Some(result)) = (needs.sim, self.sim)
            && state.sim.is_none()
            && state.row.sim_ref == Some(r)
        {
            let resolved = report_read_failure(
                result,
                ResolvedKind::Sim,
                &r,
                "component preview simulation tab",
            );
            if let Some(sim) = resolved.as_ref() {
                state.sim_body = Some(iced::widget::text_editor::Content::with_text(
                    sim.body.as_str(),
                ));
            }
            state.sim = resolved;
        }
    }
}

// ── First visit of a tab ────────────────────────────────────────────────

/// What [`LibraryState::plan_tab_resolve`] decided to look up.
pub struct TabResolvePlan {
    address: EditorAddress,
    root: std::path::PathBuf,
    needs: PreviewNeeds,
    resolver: SharedResolver,
}

impl TabResolvePlan {
    /// Run the lookups; safe on any thread.
    pub fn read(self) -> TabResolveOutcome {
        let reads = self.needs.read(&self.resolver);
        TabResolveOutcome {
            address: self.address,
            needs: self.needs,
            reads,
        }
    }
}

/// The answers of one [`TabResolvePlan::read`], not yet applied.
pub struct TabResolveOutcome {
    address: EditorAddress,
    needs: PreviewNeeds,
    reads: PreviewReads,
}

// ── Reloading a row ─────────────────────────────────────────────────────

/// What [`LibraryState::plan_editor_reload`] decided to re-read.
pub struct EditorReloadPlan {
    address: EditorAddress,
    library_id: Uuid,
    root: std::path::PathBuf,
    generation: u64,
    adapter: Arc<dyn LibraryAdapter>,
    resolver: SharedResolver,
    tab: PreviewTab,
    require_clean: bool,
    /// The copy as it was when the reload was planned. A reload the user
    /// asked for replaces the edits that existed then, not newer ones.
    row_at_plan: ComponentRow,
}

impl EditorReloadPlan {
    /// Read the row and what its tab shows; safe on any thread.
    pub fn read(self) -> EditorReloadOutcome {
        let row = self
            .adapter
            .read_row(&self.address.table, self.address.row_id);
        let (needs, reads) = match &row {
            Ok(row) => {
                let needs = PreviewNeeds::for_tab(row, self.tab, None);
                let reads = needs.read(&self.resolver);
                (needs, reads)
            }
            Err(_) => (
                PreviewNeeds::default(),
                PreviewNeeds::default().read(&self.resolver),
            ),
        };
        EditorReloadOutcome {
            address: self.address,
            library_id: self.library_id,
            generation: self.generation,
            tab: self.tab,
            require_clean: self.require_clean,
            row_at_plan: self.row_at_plan,
            row,
            needs,
            reads,
        }
    }
}

/// The answer of one [`EditorReloadPlan::read`], not yet applied.
pub struct EditorReloadOutcome {
    address: EditorAddress,
    library_id: Uuid,
    generation: u64,
    tab: PreviewTab,
    require_clean: bool,
    row_at_plan: ComponentRow,
    row: Result<ComponentRow, LibraryError>,
    needs: PreviewNeeds,
    reads: PreviewReads,
}

impl EditorReloadOutcome {
    /// The preview this reload was made for.
    pub fn address(&self) -> &EditorAddress {
        &self.address
    }
}

/// What [`LibraryState::apply_editor_reload`] did.
#[derive(Debug, PartialEq, Eq)]
pub enum ReloadApplied {
    /// The answer was stale, or the preview is gone or a reload nobody asked
    /// for found edits; nothing changed.
    Dropped,
    /// A reload the user asked for was refused, and said so in the Messages
    /// panel; nothing changed.
    Refused,
    /// The row could not be read; it was reported and the copy stands.
    Failed,
    /// The copy was replaced. `resolve_tab` is set when the user moved to
    /// another tab meanwhile, whose primitives still have to be looked up.
    Replaced { resolve_tab: Option<PreviewTab> },
}

/// A reload that cannot apply: refused out loud when the user asked for it,
/// dropped quietly when nobody did.
fn refused(unrequested: bool, why: &str) -> ReloadApplied {
    if unrequested {
        return ReloadApplied::Dropped;
    }
    crate::diagnostics::log_info(why);
    ReloadApplied::Refused
}

impl LibraryState {
    /// Plan the first-visit lookups of `tab`; `None` when the preview holds
    /// everything the tab shows (or is not open).
    pub fn plan_tab_resolve(
        &self,
        address: &EditorAddress,
        tab: PreviewTab,
    ) -> Option<TabResolvePlan> {
        let state = self.editors.get(address)?;
        let needs = PreviewNeeds::for_tab(&state.row, tab, Some(state));
        if needs.is_empty() {
            return None;
        }
        Some(TabResolvePlan {
            address: address.clone(),
            root: address.library_path.clone(),
            needs,
            resolver: self.shared_resolver(),
        })
    }

    /// Apply a tab lookup to the preview it was made for, if that preview
    /// is still open and still binds the same primitives.
    pub fn apply_tab_resolve(&mut self, outcome: TabResolveOutcome) {
        if let Some(state) = self.editors.get_mut(&outcome.address) {
            outcome.reads.apply_to(&outcome.needs, state);
        }
    }

    /// Plan re-reading the row of `address`. `require_clean` is for
    /// reloads nobody asked for: the copy is then replaced only if the user
    /// has not edited it by the time the read lands.
    pub fn plan_editor_reload(
        &mut self,
        address: &EditorAddress,
        require_clean: bool,
    ) -> Option<EditorReloadPlan> {
        let (library_id, root) = self
            .library_at(&address.library_path)
            .map(|lib| (lib.library_id, lib.root.clone()))?;
        let preview = self.editors.get(address)?;
        let tab = preview.active_tab;
        let row_at_plan = preview.row.clone();
        let Some(adapter) = self.set.get_shared(library_id) else {
            crate::diagnostics::log_warning(format!(
                "Row {} could not be reloaded: {}",
                address.row_id,
                LibraryError::NotFound(address.library_path.display().to_string())
            ));
            return None;
        };
        self.refresh_generation_counter += 1;
        let generation = self.refresh_generation_counter;
        self.editor_reload_generations
            .insert(address.clone(), generation);
        Some(EditorReloadPlan {
            address: address.clone(),
            library_id,
            root,
            generation,
            adapter,
            resolver: self.shared_resolver(),
            tab,
            require_clean,
            row_at_plan,
        })
    }

    /// Apply a reload unless it went stale: its library was closed or
    /// re-opened, a newer reload of the same preview was planned, or the
    /// preview was closed. A reload nobody asked for also yields to edits
    /// typed since it was planned, and no reload replaces a copy that is
    /// being saved.
    pub fn apply_editor_reload(&mut self, outcome: EditorReloadOutcome) -> ReloadApplied {
        let EditorReloadOutcome {
            address,
            library_id,
            generation,
            tab,
            require_clean,
            row_at_plan,
            row,
            needs,
            reads,
        } = outcome;
        let same_mount = self
            .library_at(&address.library_path)
            .is_some_and(|lib| lib.library_id == library_id);
        if !same_mount || self.editor_reload_generations.get(&address) != Some(&generation) {
            return ReloadApplied::Dropped;
        }
        self.editor_reload_generations.remove(&address);
        let row = match row {
            Ok(row) => row,
            Err(e) => {
                crate::diagnostics::log_warning(format!(
                    "Row {} could not be reloaded: {e}",
                    address.row_id
                ));
                return ReloadApplied::Failed;
            }
        };
        let Some(old) = self.editors.get(&address) else {
            return ReloadApplied::Dropped;
        };
        if require_clean && old.has_unsaved_edits() {
            return ReloadApplied::Dropped;
        }
        if old.saving_row.is_some() || old.binding_primitive {
            return refused(
                require_clean,
                "Reload was ignored: this row is being saved. Reload again once the save has finished.",
            );
        }
        if !require_clean && old.row != row_at_plan {
            return refused(
                false,
                "Reload was ignored: this row was edited after the reload was confirmed. \
                 Reload again to discard the new edits.",
            );
        }
        let current_tab = old.active_tab;
        let mut fresh = ComponentPreviewState::from_row(
            address.library_path.clone(),
            address.table.clone(),
            row,
        );
        fresh.active_tab = current_tab;
        let resolve_tab = if current_tab == tab {
            reads.apply_to(&needs, &mut fresh);
            None
        } else {
            Some(current_tab)
        };
        self.editors.insert(address, fresh);
        ReloadApplied::Replaced { resolve_tab }
    }

    /// Plan, read and apply a reload on the calling thread. For the one
    /// save path that is blocking by design (`save_primitive_tab_blocking`),
    /// which has no task to return.
    pub fn reload_editor_blocking(&mut self, address: &EditorAddress, require_clean: bool) {
        if let Some(plan) = self.plan_editor_reload(address, require_clean) {
            // Same thread, same tab: nothing is left to look up.
            self.apply_editor_reload(plan.read());
        }
    }
}

/// Reload the row of `address` off the UI thread.
pub fn reload_task(
    state: &mut LibraryState,
    address: &EditorAddress,
    require_clean: bool,
) -> iced::Task<crate::app::Message> {
    let Some(plan) = state.plan_editor_reload(address, require_clean) else {
        return iced::Task::none();
    };
    let turn = state.writes.read_turn(&plan.root);
    iced::Task::perform(
        async move {
            let outcome = tokio::task::spawn_blocking(move || {
                turn.wait();
                plan.read()
            })
            .await
            .ok();
            OneShotCell::new(outcome)
        },
        |readout| crate::app::Message::Library(super::LibraryMessage::EditorReloaded { readout }),
    )
}

/// Look up what `tab` of the preview at `address` shows, off the UI thread.
pub fn tab_resolve_task(
    state: &mut LibraryState,
    address: &EditorAddress,
    tab: PreviewTab,
) -> iced::Task<crate::app::Message> {
    let Some(plan) = state.plan_tab_resolve(address, tab) else {
        return iced::Task::none();
    };
    let turn = state.writes.read_turn(&plan.root);
    iced::Task::perform(
        async move {
            let outcome = tokio::task::spawn_blocking(move || {
                turn.wait();
                plan.read()
            })
            .await
            .ok();
            OneShotCell::new(outcome)
        },
        |readout| {
            crate::app::Message::Library(super::LibraryMessage::PreviewTabResolved { readout })
        },
    )
}
