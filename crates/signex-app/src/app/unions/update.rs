//! The Unions slice's update (mockup row 158): the popover, the panel actions, the
//! selection that grows to whole unions, and the panel view rebuilt from the
//! open sheets. Every edit is one engine command; hiding is not one.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_engine::Command;
use signex_i18n::{ids, t};
use signex_types::schematic::{SchUnion, SelectedItem};
use uuid::Uuid;

use super::state::{
    CreateUnionPopover, ScopeSheet, UnionPopoverMsg, UnionScope, UnionsMsg, panel_view,
};
use crate::app::{Message, Signex};

#[cfg(test)]
thread_local! {
    /// Test probe: how many times this thread built the Unions panel's view.
    pub(crate) static UNIONS_VIEW_BUILDS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}

/// The hash of one item, for an order-independent sum over a map.
fn stamp(item: impl std::hash::Hash) -> u64 {
    use std::hash::Hasher;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    item.hash(&mut hasher);
    hasher.finish()
}

impl Signex {
    pub(crate) fn handle_unions_message(&mut self, msg: UnionsMsg) -> Task<Message> {
        match msg {
            UnionsMsg::CreateRequested => {
                self.open_create_union();
                if self.ui_state.unions.popover.is_some() {
                    return iced::widget::operation::focus(super::UNION_NAME_INPUT_ID.clone());
                }
            }
            UnionsMsg::AddSelectionRequested => self.add_selection_to_union(),
            UnionsMsg::RemoveSelectionRequested => self.remove_selection_from_union(),
            UnionsMsg::BreakRequested => self.break_current_union(),
            UnionsMsg::Popover(msg) => self.handle_union_popover(msg),
            UnionsMsg::FilterEdited(text) => self.ui_state.unions.filter = text,
            UnionsMsg::ScopePicked(scope) => self.ui_state.unions.scope = scope,
            UnionsMsg::TagChipToggled(tag) => {
                let on = &mut self.ui_state.unions.tags_on;
                if !on.remove(&tag) {
                    on.insert(tag);
                }
            }
            UnionsMsg::RowPicked { sheet, union } => {
                self.ui_state.unions.picked = Some((sheet, union));
            }
            UnionsMsg::MembersSelected { sheet, union } => self.select_union_members(&sheet, union),
            UnionsMsg::LockToggled { sheet, union } => self.toggle_union_lock(&sheet, union),
            UnionsMsg::HideToggled { sheet, union } => {
                // A view setting: no engine command, no undo step, nothing saved.
                self.ui_state.unions.toggle_hidden(&sheet, union);
                self.interaction_state
                    .active_canvas_mut()
                    .clear_overlay_cache();
            }
            UnionsMsg::RenameEdited(name) => self.rename_picked_union(name),
        }
        self.sync_unions_panel();
        Task::none()
    }

    /// What the canvas of the sheet at `path` needs to draw its unions.
    pub(crate) fn union_view(
        &self,
        path: Option<&Path>,
    ) -> crate::schematic_runtime::unions::UnionView<'_> {
        let hidden: &[Uuid] = match path {
            Some(path) => self.ui_state.unions.hidden_in(path),
            None => &[],
        };
        crate::schematic_runtime::unions::UnionView {
            hidden,
            ink: crate::schematic_runtime::unions::UnionInk::from_tokens(
                &self.tokens_for(self.ui_state.preferences_draft_theme),
            ),
        }
    }

    /// The selection of the sheet in view, and that sheet.
    fn union_selection(&self) -> Option<(PathBuf, Vec<SelectedItem>)> {
        let path = self.document_state.active_path.clone()?;
        self.document_state.engines.get(&path)?;
        Some((
            path,
            self.interaction_state.active_canvas().selected.clone(),
        ))
    }

    fn open_create_union(&mut self) {
        let Some((sheet, selected)) = self.union_selection() else {
            crate::diagnostics::log_info(
                t!(self.ui_state.i18n, ids::UNIONS_MSG_NOT_A_SHEET).into_string(),
            );
            return;
        };
        let mut members: Vec<Uuid> = Vec::new();
        for item in &selected {
            if !members.contains(&item.uuid) {
                members.push(item.uuid);
            }
        }
        if members.is_empty() {
            crate::diagnostics::log_info(
                t!(self.ui_state.i18n, ids::UNIONS_MSG_NO_SELECTION).into_string(),
            );
            return;
        }
        let anchor = self.interaction_state.last_mouse_pos;
        self.ui_state.unions.popover = Some(CreateUnionPopover {
            sheet,
            members,
            name: String::new(),
            tags: Vec::new(),
            tag_input: String::new(),
            lock: false,
            anchor,
        });
    }

    fn handle_union_popover(&mut self, msg: UnionPopoverMsg) {
        let Some(popover) = self.ui_state.unions.popover.as_mut() else {
            return;
        };
        match msg {
            UnionPopoverMsg::NameEdited(name) => popover.name = name,
            UnionPopoverMsg::TagInputEdited(text) => popover.tag_input = text,
            UnionPopoverMsg::TagSubmitted => {
                popover.tags = popover.tags_with_input();
                popover.tag_input.clear();
            }
            UnionPopoverMsg::TagRemoved(at) => {
                if at < popover.tags.len() {
                    popover.tags.remove(at);
                }
            }
            UnionPopoverMsg::LockToggled(lock) => popover.lock = lock,
            UnionPopoverMsg::Cancelled => self.ui_state.unions.popover = None,
            UnionPopoverMsg::Confirmed => self.create_union_from_popover(),
        }
    }

    /// Create: one engine command, so one undo step. The popover stays
    /// open on a refusal, which the Messages panel explains.
    fn create_union_from_popover(&mut self) {
        let Some(popover) = self.ui_state.unions.popover.clone() else {
            return;
        };
        if !popover.can_create() {
            return;
        }
        if self.document_state.active_path.as_deref() != Some(popover.sheet.as_path()) {
            self.ui_state.unions.popover = None;
            crate::diagnostics::log_info(
                t!(self.ui_state.i18n, ids::UNIONS_MSG_SHEET_CHANGED).into_string(),
            );
            return;
        }
        let uuid = Uuid::now_v7();
        let command = Command::CreateUnion {
            union: SchUnion {
                uuid,
                name: popover.name.trim().to_string(),
                tags: popover.tags_with_input(),
                members: popover.members.clone(),
                locked: popover.lock,
            },
        };
        if self.apply_engine_command(command, true, true) {
            self.ui_state.unions.popover = None;
            self.ui_state.unions.picked = Some((popover.sheet, uuid));
        }
    }

    /// Run a union command on the sheet at `sheet`: the gateway for the
    /// sheet in view, the sheet's own engine for another open sheet.
    fn run_union_command(&mut self, sheet: &Path, command: Command) -> bool {
        if self.document_state.active_path.as_deref() == Some(sheet) {
            return self.apply_engine_command(command, true, true);
        }
        match self.execute_on_sheet(sheet, command) {
            Ok(changed) => changed,
            Err(reason) => {
                crate::diagnostics::log_warning(
                    t!(self.ui_state.i18n, ids::UNIONS_MSG_REFUSED, "reason" => reason.to_string())
                        .into_string(),
                );
                false
            }
        }
    }

    /// The union an action without a row acts on: the picked one on the
    /// sheet in view, else the union the selection touches.
    fn current_union(&self) -> Option<(PathBuf, SchUnion)> {
        let (path, selected) = self.union_selection()?;
        let sheet = self.document_state.engines.get(&path)?.document();
        let picked = self
            .ui_state
            .unions
            .picked
            .as_ref()
            .filter(|(p, _)| *p == path)
            .and_then(|(_, id)| signex_engine::unions::union_by_id(sheet, *id));
        let union = picked.or_else(|| {
            let touched = signex_engine::unions::unions_of_items(sheet, &selected);
            touched
                .first()
                .and_then(|id| signex_engine::unions::union_by_id(sheet, *id))
        })?;
        Some((path, union.clone()))
    }

    fn add_selection_to_union(&mut self) {
        let Some((path, union)) = self.current_union() else {
            crate::diagnostics::log_info(
                t!(self.ui_state.i18n, ids::UNIONS_MSG_ADD_NO_UNION).into_string(),
            );
            return;
        };
        let selected = &self.interaction_state.active_canvas().selected;
        let mut members = union.members.clone();
        for item in selected {
            if !members.contains(&item.uuid) {
                members.push(item.uuid);
            }
        }
        if members.len() == union.members.len() {
            return;
        }
        let command = Command::SetUnionMembers {
            uuid: union.uuid,
            members,
        };
        self.run_union_command(&path, command);
    }

    fn remove_selection_from_union(&mut self) {
        let Some((path, union)) = self.current_union() else {
            crate::diagnostics::log_info(
                t!(self.ui_state.i18n, ids::UNIONS_MSG_REMOVE_NO_MEMBERS).into_string(),
            );
            return;
        };
        let selected = &self.interaction_state.active_canvas().selected;
        let members: Vec<Uuid> = union
            .members
            .iter()
            .copied()
            .filter(|m| !selected.iter().any(|i| i.uuid == *m))
            .collect();
        if members.len() == union.members.len() {
            return;
        }
        let command = Command::SetUnionMembers {
            uuid: union.uuid,
            members,
        };
        self.run_union_command(&path, command);
    }

    fn break_current_union(&mut self) {
        // The panel's picked union first, on whichever open sheet it is.
        let picked = self
            .ui_state
            .unions
            .picked
            .clone()
            .and_then(|(path, id)| self.union_on(&path, id).map(|u| (path, u)));
        let Some((path, union)) = picked.or_else(|| self.current_union()) else {
            crate::diagnostics::log_info(
                t!(self.ui_state.i18n, ids::UNIONS_MSG_BREAK_NO_UNION).into_string(),
            );
            return;
        };
        if self.run_union_command(&path, Command::BreakUnion { uuid: union.uuid }) {
            self.ui_state.unions.picked = None;
        }
    }

    fn union_on(&self, sheet: &Path, union: Uuid) -> Option<SchUnion> {
        let doc = self.document_state.engines.get(sheet)?.document();
        signex_engine::unions::union_by_id(doc, union).cloned()
    }

    fn toggle_union_lock(&mut self, sheet: &Path, union: Uuid) {
        let Some(found) = self.union_on(sheet, union) else {
            return;
        };
        let command = Command::UpdateUnion {
            uuid: found.uuid,
            name: found.name,
            tags: found.tags,
            locked: !found.locked,
        };
        self.run_union_command(sheet, command);
    }

    /// Rename the picked union as the name is typed; the engine keeps the
    /// keystrokes of one union as one undo step and refuses a locked union.
    /// A blank name is not sent.
    fn rename_picked_union(&mut self, name: String) {
        let Some((sheet, id)) = self.ui_state.unions.picked.clone() else {
            return;
        };
        let Some(found) = self.union_on(&sheet, id) else {
            return;
        };
        if name.trim().is_empty() {
            return;
        }
        let command = Command::UpdateUnion {
            uuid: id,
            name,
            tags: found.tags,
            locked: found.locked,
        };
        self.run_union_command(&sheet, command);
    }

    /// Select every member of `union`; a union on another sheet is picked
    /// in the panel only.
    fn select_union_members(&mut self, sheet: &Path, union: Uuid) {
        self.ui_state.unions.picked = Some((sheet.to_path_buf(), union));
        if self.document_state.active_path.as_deref() != Some(sheet) {
            return;
        }
        let Some(doc) = self.document_state.engines.get(sheet).map(|e| e.document()) else {
            return;
        };
        let Some(found) = signex_engine::unions::union_by_id(doc, union) else {
            return;
        };
        let items = signex_engine::unions::member_items(doc, found);
        self.replace_selection(items);
    }

    /// A click selected `hit`: grow it to the whole union it belongs to.
    pub(crate) fn expand_selection_to_unions(&mut self) {
        let Some(path) = self.document_state.active_path.clone() else {
            return;
        };
        let Some(doc) = self.document_state.engines.get(&path).map(|e| e.document()) else {
            return;
        };
        if doc.unions.is_empty() {
            return;
        }
        let selected = &self.interaction_state.active_canvas().selected;
        let grown = signex_engine::unions::expand_to_unions(doc, selected);
        // The union clicked becomes the panel's picked one, so the panel
        // foot and Break Union act on what was clicked.
        if let [one] = signex_engine::unions::unions_of_items(doc, &grown).as_slice() {
            self.ui_state.unions.picked = Some((path, *one));
        }
        if grown.len() != selected.len() {
            let canvas = self.interaction_state.active_canvas_mut();
            canvas.selected = grown;
            canvas.clear_overlay_cache();
        }
    }

    /// A click on a union's name tab at world `(x, y)` selects the union.
    /// `true` when it landed on one.
    pub(crate) fn select_union_at_tab(&mut self, x: f64, y: f64) -> bool {
        let Some(snapshot) = self.active_render_snapshot() else {
            return false;
        };
        let canvas = self.interaction_state.active_canvas();
        let scale = canvas.live_camera().2;
        let hit = crate::schematic_runtime::unions::union_tab_at(
            snapshot,
            &canvas.designators,
            self.active_wire_widths(),
            scale,
            x,
            y,
        );
        let (Some(union), Some(path)) = (hit, self.document_state.active_path.clone()) else {
            return false;
        };
        self.interaction_state.clicked_pin = None;
        self.select_union_members(&path, union);
        true
    }

    /// A stamp of everything the panel's view is built from: the slice's
    /// state, the sheet in view and its project, the revision of every sheet
    /// in scope and the selection.
    fn unions_panel_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let state = &self.ui_state.unions;
        let docs = &self.document_state;
        let active = docs.active_path.as_deref();
        let owner = active
            .and_then(|p| docs.document_project(p))
            .map(|p| p.path.as_path());
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (active, owner, state.scope, &state.filter).hash(&mut hasher);
        (&state.tags_on, &state.picked).hash(&mut hasher);
        // Order-independent: the hidden unions, and the sheets in scope.
        let hidden = state.hidden.iter().map(stamp).fold(0u64, u64::wrapping_add);
        let project_wide = state.scope == UnionScope::Project && owner.is_some();
        let sheets = docs
            .engines
            .iter()
            .filter(|(path, _)| {
                Some(path.as_path()) == active
                    || (project_wide
                        && docs.document_project(path).map(|p| p.path.as_path()) == owner)
            })
            .map(|(path, engine)| stamp((path, engine.revision())))
            .fold(0u64, u64::wrapping_add);
        (hidden, state.hidden.len(), sheets).hash(&mut hasher);
        self.interaction_state
            .active_canvas()
            .selected
            .hash(&mut hasher);
        hasher.finish()
    }

    /// Rebuild the panel's view from the open sheets, when what it is built
    /// from changed. Cheap when the panel is closed: nothing is read.
    pub(crate) fn sync_unions_panel(&mut self) {
        if !self.is_panel_open(crate::panels::PanelKind::Unions) {
            self.document_state.panel_ctx.unions.built_from = None;
            return;
        }
        let key = self.unions_panel_key();
        if self.document_state.panel_ctx.unions.built_from == Some(key) {
            return;
        }
        #[cfg(test)]
        UNIONS_VIEW_BUILDS.with(|n| n.set(n.get() + 1));
        let state = &self.ui_state.unions;
        let docs = &self.document_state;
        let active = docs.active_path.clone();
        let owner = active
            .as_deref()
            .and_then(|p| docs.document_project(p))
            .map(|p| p.path.clone());
        let file = |path: &Path| {
            path.file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
        };
        let mut sheets: Vec<ScopeSheet<'_>> = Vec::new();
        if let Some(path) = active.as_deref()
            && let Some(engine) = docs.engines.get(path)
        {
            sheets.push(ScopeSheet {
                path,
                name: file(path),
                sheet: engine.document(),
            });
        }
        if state.scope == UnionScope::Project && owner.is_some() {
            let mut others: Vec<ScopeSheet<'_>> = docs
                .engines
                .iter()
                .filter(|(path, _)| Some(path.as_path()) != active.as_deref())
                .filter(|(path, _)| docs.document_project(path).map(|p| p.path.clone()) == owner)
                .map(|(path, engine)| ScopeSheet {
                    path: path.as_path(),
                    name: file(path),
                    sheet: engine.document(),
                })
                .collect();
            others.sort_by(|a, b| a.name.cmp(&b.name));
            sheets.extend(others);
        }
        let selected = &self.interaction_state.active_canvas().selected;
        let mut view = panel_view(state, &sheets, active.as_deref(), selected);
        view.built_from = Some(key);
        *self.document_state.panel_ctx.unions = view;
    }
}
