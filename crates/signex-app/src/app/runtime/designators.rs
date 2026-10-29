//! The designators a canvas draws for the placement its tab was opened
//! through (#1151).
//!
//! A child sheet placed more than once carries a designator per placement
//! (#829). Descending through a sheet symbol records that placement for the
//! tab (#1133), and the export prints its designators; the canvas draws the
//! same ones, so the screen matches the page.

use std::collections::HashMap;

use signex_types::schematic::InstancePath;

use super::super::*;

impl Signex {
    /// Re-resolve the active canvas's designators, clearing its content cache
    /// only when they changed.
    pub(crate) fn refresh_canvas_designators(&mut self) {
        let key = self.designators_key();
        if key.is_some() && self.interaction_state.active_canvas().designators_key == key {
            return;
        }
        let designators = match self.document_state.active_path.clone() {
            Some(path) => self.designators_for_path(&path),
            None => HashMap::new(),
        };
        let canvas = self.interaction_state.active_canvas_mut();
        canvas.designators_key = key;
        if canvas.designators != designators {
            canvas.designators = designators;
            canvas.clear_content_cache();
        }
    }

    /// What the active canvas's designators are a function of; `None` with no
    /// active sheet. The engine revision moves on every document change.
    fn designators_key(&self) -> Option<crate::canvas::DesignatorsKey> {
        let path = self.document_state.active_path.as_deref()?;
        let engine = self
            .document_state
            .engines
            .get(path)
            .map(|engine| (engine.instance_id(), engine.revision()));
        Some((path.to_path_buf(), engine, self.shown_placement(path)))
    }

    /// The placement whose designators the schematic at `path` shows and
    /// edits (#1200):
    /// - the tab's recorded placement while the walk still reaches the sheet
    ///   through it;
    /// - else the default placement, as the "Current page" export picks it,
    ///   so a child opened from the Projects panel shows what it prints;
    /// - `None` when the walk does not reach the sheet (or no project
    ///   hierarchy is known for it), and then `reference` is shown.
    ///
    /// The Navigator's hierarchy describes the focused document's project, so
    /// it is consulted only for the focused document; another path (an
    /// undocked tab) keeps its recorded placement.
    pub(crate) fn shown_placement(&self, path: &std::path::Path) -> Option<InstancePath> {
        let document_state = &self.document_state;
        let recorded = document_state.tab_placements.get(path);
        let hierarchy = document_state
            .panel_ctx
            .navigator
            .hierarchy
            .as_ref()
            .filter(|_| document_state.active_path.as_deref() == Some(path));
        let Some(hierarchy) = hierarchy else {
            return recorded.cloned();
        };
        let live = hierarchy.placements_of(path);
        match recorded {
            Some(recorded) if live.contains(recorded) => Some(recorded.clone()),
            _ => crate::app::state::default_placement(live),
        }
    }

    /// The designators the schematic at `path` shows at its
    /// [shown placement](Self::shown_placement), for the symbols where they
    /// differ from `reference`. Empty when there is no such placement or
    /// `path` has no engine.
    pub(crate) fn designators_for_path(
        &self,
        path: &std::path::Path,
    ) -> HashMap<uuid::Uuid, String> {
        let (Some(placement), Some(engine)) = (
            self.shown_placement(path),
            self.document_state.engines.get(path),
        ) else {
            return HashMap::new();
        };
        engine
            .document()
            .symbols
            .iter()
            .filter(|symbol| symbol.retargets_designator_in(&placement))
            .map(|symbol| (symbol.uuid, symbol.designator_in(&placement).to_string()))
            .collect()
    }

    /// Whether the Navigator's walk saw the whole project and found the sheet
    /// at `path` placed at most once (#1200), and no other open project reaches
    /// it (#1224).
    pub(crate) fn known_single_placement(&self, path: &std::path::Path) -> bool {
        !self.shared_with_another_open_project(path)
            // A pending hierarchy may predate an edit that added a placement.
            && self.navigator_is_settled()
            && self
                .document_state
                .panel_ctx
                .navigator
                .hierarchy
                .as_ref()
                .is_some_and(|h| h.is_complete() && h.placements_of(path).len() <= 1)
    }

    /// The commands that set `symbol`'s designator to `value` on the active
    /// tab, applied as one undo step:
    /// - by default, the instance entry of the shown placement, so the edit
    ///   changes what this tab shows and exports and every other placement
    ///   keeps its designator;
    /// - on a sheet the walk saw *completely* and found placed once (or not
    ///   at all), the symbol's own `reference`, dropping that placement's
    ///   entry if it has one, so a simple design keeps a simple file (#1200).
    ///   An incomplete walk (an unreadable or missing sheet on the way, a
    ///   cut-off) may hide other placements that read `reference`, so it
    ///   never takes this branch. Nor does a sheet another open project
    ///   reaches (or might reach, through a sheet it has not read), since
    ///   that project reads `reference` for its own placement (#1224). The
    ///   check only sees open projects, so a second open project whose walk
    ///   is incomplete forces the entry branch for every sheet, shared or
    ///   not: safe, at the cost of a less simple file;
    /// - with no placement shown (or the root chain), `reference`.
    pub(crate) fn designator_edit_commands(
        &self,
        symbol: uuid::Uuid,
        value: String,
    ) -> Vec<signex_engine::Command> {
        let active = self.document_state.active_path.as_deref();
        let placement = active.and_then(|path| self.shown_placement(path));
        let known_single = active.is_some_and(|path| self.known_single_placement(path));
        let Some(engine) = self.document_state.active_engine() else {
            return Vec::new();
        };
        signex_engine::designator_commands(
            engine.document(),
            symbol,
            placement.as_ref(),
            known_single,
            value,
        )
    }

    /// Does another open project reach the sheet at `path`, or has some other
    /// project's walk not read every sheet (so it may)? Either way the file's
    /// `reference` is read by placements this project cannot see, so an edit
    /// must not write it (#1224). Only open projects can be consulted; the
    /// fold is the widest any open project uses, since folding only reports
    /// more sharing.
    fn shared_with_another_open_project(&self, path: &std::path::Path) -> bool {
        let doc = &self.document_state;
        let Some(owner) = doc.document_project(path).map(|p| p.id) else {
            return false;
        };
        let Some(index) = doc.projects.iter().position(|p| p.id == owner) else {
            return false;
        };
        let mut fold = doc.fold_for_project(owner);
        fold.fold_case |= doc.projects.iter().any(|p| p.case_fold.fold_case);
        let reach = doc.lexical_reach_sets(fold);
        reach.reached_by_other(index, path) || reach.other_walk_incomplete(index)
    }

    /// `symbol`'s designator as the active tab shows it.
    pub(crate) fn shown_designator<'a>(
        &'a self,
        symbol: &'a signex_types::schematic::Symbol,
    ) -> &'a str {
        self.interaction_state
            .active_canvas()
            .designators
            .get(&symbol.uuid)
            .map_or(symbol.reference.as_str(), String::as_str)
    }
}
