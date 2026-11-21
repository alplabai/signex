//! The schematic edit gateway. Each edit tail here is a wrapper (#2704,
//! until slice 6): its core half in `state::sheet_change`, then the
//! projection in `app::projector`, inside the same call.

use super::core::event::ChangeCause;
use super::projector::RefreshScope;
use super::state::sheet_change;
use super::undo_groups::StepDirection;
use super::*;

pub(crate) use super::core::mutation_outcome::MutationOutcome;

impl Signex {
    /// Surface the notices a command raised (a rubber-band pin that could
    /// not be kept connected) in the Messages panel: the engine's older
    /// English lines as they are, its typed notices worded through the
    /// catalog (`app::engine_notices`). A GUI log helper for engine calls
    /// outside the edit tails, not a tail wrapper.
    pub(crate) fn report_engine_warnings(engine: &mut signex_engine::Engine) {
        for notice in core::event::engine_notices(engine) {
            super::projector::log_notice(&notice);
        }
    }

    pub(crate) fn apply_engine_commands(
        &mut self,
        commands: Vec<signex_engine::Command>,
        clear_overlay_cache: bool,
        update_selection_info: bool,
    ) -> bool {
        self.apply_engine_commands_checked(commands, clear_overlay_cache, update_selection_info)
            == MutationOutcome::Changed
    }

    /// [`Self::apply_engine_commands`] that says whether the batch changed
    /// the document, left it as it was, or was rejected (and rolled back),
    /// for callers that report on the outcome.
    pub(crate) fn apply_engine_commands_checked(
        &mut self,
        commands: Vec<signex_engine::Command>,
        clear_overlay_cache: bool,
        update_selection_info: bool,
    ) -> MutationOutcome {
        self.apply_engine_commands_reporting(commands, clear_overlay_cache, update_selection_info)
            .0
    }

    /// [`Self::apply_engine_commands_checked`] that also hands back the
    /// refusal text of a failed batch, so a caller can print the reason even
    /// when the gateway's own line was already shown (or throttled).
    pub(crate) fn apply_engine_commands_reporting(
        &mut self,
        commands: Vec<signex_engine::Command>,
        clear_overlay_cache: bool,
        update_selection_info: bool,
    ) -> (MutationOutcome, Option<String>) {
        // A channel tab of a repeated sheet is a compiled view (#1279).
        if !commands.is_empty()
            && let Some(reason) = self.refuse_if_channel_view()
        {
            return (MutationOutcome::Failed, Some(reason));
        }
        let edit = self.document_state.execute_batch_on_edit_path(commands);
        self.project_sheet_edit(edit, clear_overlay_cache, update_selection_info)
    }

    /// Project an edit's events on the active canvas; its outcome and refusal.
    fn project_sheet_edit(
        &mut self,
        edit: sheet_change::SheetEdit,
        clear_overlay: bool,
        update_selection_info: bool,
    ) -> (MutationOutcome, Option<String>) {
        let scope = RefreshScope::ActiveCanvas {
            clear_overlay,
            update_selection_info,
        };
        self.project_events(edit.events, scope);
        (edit.outcome, edit.refusal)
    }

    pub(crate) fn apply_engine_command(
        &mut self,
        command: signex_engine::Command,
        clear_overlay_cache: bool,
        update_selection_info: bool,
    ) -> bool {
        self.apply_engine_command_checked(command, clear_overlay_cache, update_selection_info)
            == MutationOutcome::Changed
    }

    /// [`Self::apply_engine_command`] that says whether the command changed
    /// the document, left it as it was, or was rejected, for callers that
    /// report on the outcome.
    pub(crate) fn apply_engine_command_checked(
        &mut self,
        command: signex_engine::Command,
        clear_overlay_cache: bool,
        update_selection_info: bool,
    ) -> MutationOutcome {
        self.apply_engine_command_reporting(command, clear_overlay_cache, update_selection_info)
            .0
    }

    /// [`Self::apply_engine_command_checked`] that also hands back the
    /// engine's refusal text, for callers that must show the user why an
    /// edit did not happen. `None` means the engine accepted the command.
    pub(crate) fn apply_engine_command_reporting(
        &mut self,
        command: signex_engine::Command,
        clear_overlay_cache: bool,
        update_selection_info: bool,
    ) -> (MutationOutcome, Option<String>) {
        self.apply_engine_command_recording(
            command,
            true,
            clear_overlay_cache,
            update_selection_info,
        )
    }

    /// [`Self::apply_engine_command_reporting`] for load-time maintenance:
    /// the edit lands without an undo step, so the first Ctrl+Z after
    /// opening a sheet does not revert something the user never did.
    pub(crate) fn apply_engine_command_unrecorded(
        &mut self,
        command: signex_engine::Command,
    ) -> (MutationOutcome, Option<String>) {
        self.apply_engine_command_recording(command, false, true, true)
    }

    fn apply_engine_command_recording(
        &mut self,
        command: signex_engine::Command,
        record_history: bool,
        clear_overlay_cache: bool,
        update_selection_info: bool,
    ) -> (MutationOutcome, Option<String>) {
        if let Some(reason) = self.refuse_if_channel_view() {
            return (MutationOutcome::Failed, Some(reason));
        }
        let edit = self
            .document_state
            .execute_on_edit_path(command, record_history);
        self.project_sheet_edit(edit, clear_overlay_cache, update_selection_info)
    }

    /// Mark the active document as having unsaved edits, without running a
    /// render invalidation.
    ///
    /// For the one edit path that mutates the engine through a method that
    /// is not a `Command` — `Engine::annotate_with_seed_and_locks`, which
    /// records its own history entry — so it cannot go through
    /// [`Signex::apply_engine_command`] but still has to be
    /// marked dirty (#585). Everything else should use the gateway, which
    /// does the same as part of `finish_schematic_mutation`.
    ///
    /// Returns `false` when there is no active schematic session to mark.
    pub(crate) fn mark_active_document_dirty(&mut self) -> bool {
        self.document_state.note_active_edit()
    }

    /// Undo one step of the **active** engine's own history.
    ///
    /// One `Engine::undo` per invocation: a batch recorded through
    /// `apply_engine_commands` is already a single entry, so grouping needs
    /// no second stack to count it. This used to be driven by an app-side
    /// marker stack, which was one global stack shared by every open
    /// document's engine and was never cleared on tab switch — so its
    /// counts drifted out of step with the history they were counting and
    /// an edit could be reverted with no invalidation and no dirty flag
    /// (#533). Reading the history straight off the engine that owns it
    /// also makes the `Engine::can_undo` the Edit menu reads
    /// (`app/view/mod.rs`) agree with what Undo will actually do.
    pub(crate) fn apply_engine_undo(&mut self, update_selection_info: bool) -> bool {
        self.apply_engine_step(StepDirection::Undo, update_selection_info)
    }

    /// Redo one step of the **active** engine's own history. Mirror of
    /// [`Signex::apply_engine_undo`].
    pub(crate) fn apply_engine_redo(&mut self, update_selection_info: bool) -> bool {
        self.apply_engine_step(StepDirection::Redo, update_selection_info)
    }

    fn apply_engine_step(&mut self, direction: StepDirection, update_selection_info: bool) -> bool {
        // Undo and redo do not reach past a channel tab either (#1279).
        if self.refuse_if_channel_view().is_some() {
            return false;
        }
        if let Some(group) = self
            .document_state
            .active_engine()
            .and_then(|engine| direction.top(engine))
        {
            let undo = direction == StepDirection::Undo;
            let stepped = if self.undo_group_has_boards(group) {
                self.step_board_undo_group(group, undo)
            } else {
                let undocked = self.undocked_sheet_paths();
                let step = self.document_state.step_undo_group(group, undo, &undocked);
                self.project_group_step(step, update_selection_info)
            };
            // An agent's step says what it was (docs/AGENT_EDITS.md, "Undo").
            if stepped && let Some(label) = self.document_state.agent_edits.step_text(group) {
                let verb = if direction == StepDirection::Undo {
                    "Undone"
                } else {
                    "Redone"
                };
                crate::diagnostics::log_info(format!("{verb}: {label}"));
            }
            return stepped;
        }
        let Some(step) = self
            .document_state
            .step_active_history(direction == StepDirection::Undo)
        else {
            // No engine behind the active document: nothing moved, but the
            // drift check is asked for as it always was (pinned by
            // `state/sheet_change/tests.rs`).
            self.library.drift.check_pending = true;
            return false;
        };
        let scope = RefreshScope::ActiveCanvas {
            clear_overlay: true,
            update_selection_info,
        };
        self.project_events(step.events, scope);
        step.stepped
    }

    /// Drop selected wires the edit removed (a merge folds one wire into its
    /// neighbour), so the selection never names a wire that is gone.
    pub(crate) fn prune_selected_wires(&mut self) {
        let Some(engine) = self.document_state.active_engine() else {
            return;
        };
        let wires: std::collections::HashSet<uuid::Uuid> =
            engine.document().wires.iter().map(|w| w.uuid).collect();
        self.interaction_state
            .active_canvas_mut()
            .selected
            .retain(|item| {
                item.kind != signex_types::schematic::SelectedKind::Wire
                    || wires.contains(&item.uuid)
            });
    }

    /// The tail of an edit made outside the engine's commands: a multi-sheet
    /// `run`'s other members and its group first, then the active sheet's
    /// `patch`. `false` when the active sheet took no change.
    pub(super) fn finish_schematic_mutation(
        &mut self,
        patch: Option<signex_engine::DocumentPatch>,
        run: Option<sheet_change::SheetRun>,
        clear_overlay_cache: bool,
        update_selection_info: bool,
    ) -> bool {
        if let Some(run) = run {
            let members = self.document_state.settle_run(run, patch.is_some());
            self.project_events(members, RefreshScope::Member);
        }
        let Some(patch) = patch else {
            return false;
        };
        let edit = self
            .document_state
            .settle_active_sheet(patch, ChangeCause::Edit);
        let (outcome, _) =
            self.project_sheet_edit(edit, clear_overlay_cache, update_selection_info);
        outcome == MutationOutcome::Changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reconcile(filename: &str, port: &str) -> signex_engine::Command {
        signex_engine::Command::ReconcileChildSheetPins {
            child_filename: filename.to_string(),
            ports: vec![signex_engine::SheetPort {
                name: port.to_string(),
                direction: "input".to_string(),
            }],
            renames: None,
        }
    }

    #[test]
    fn checked_command_tells_changed_from_unchanged_from_failed() {
        use crate::app::handlers::menu::export::tests as fx;
        // No active schematic: the command cannot run at all.
        let (mut bare, _task) = Signex::new();
        assert_eq!(
            bare.apply_engine_command_checked(reconcile("child.snxsch", "A"), true, true),
            MutationOutcome::Failed
        );

        let top = std::path::PathBuf::from("/w/gw/top.snxsch");
        let mut app = fx::app_workspace("/w/gw", &["top.snxsch"]);
        let mut sheet = fx::sheet_with_net("R_TOP", "TOP_NET", &["child.snxsch"]);
        // A zero-sized symbol has no edge slot for the new pin, and the
        // fixture's wire sits at the origin; give it room away from it.
        sheet.child_sheets[0].position = signex_types::schematic::Point::new(100.0, 100.0);
        sheet.child_sheets[0].size = (30.0, 30.0);
        fx::open_with(&mut app.document_state, &top, sheet);
        // A mutation only counts as changed once it reaches a tab's session.
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "top".to_string(),
            path: top.clone(),
            cached_document: None,
            project_id: None,
            kind: crate::app::TabKind::Schematic,
        });
        app.document_state.active_tab = 0;
        app.document_state.active_path = Some(top);

        // A file no sheet symbol references: nothing to reconcile.
        assert_eq!(
            app.apply_engine_command_checked(reconcile("other.snxsch", "A"), true, true),
            MutationOutcome::Unchanged
        );
        assert_eq!(
            app.apply_engine_command_checked(reconcile("child.snxsch", "A"), true, true),
            MutationOutcome::Changed
        );
        // The same port again is already in place.
        assert_eq!(
            app.apply_engine_command_checked(reconcile("child.snxsch", "A"), true, true),
            MutationOutcome::Unchanged
        );
    }

    /// Main window + one undocked window, both showing `top.snxsch`, each
    /// with a render cache of the unedited sheet and a selected wire.
    fn main_and_undocked_on_one_sheet() -> (Signex, iced::window::Id, iced::window::Id) {
        use crate::app::handlers::menu::export::tests as fx;
        use signex_types::schematic::{SelectedItem, SelectedKind};
        let top = std::path::PathBuf::from("/w/gw/top.snxsch");
        let mut app = fx::app_workspace("/w/gw", &["top.snxsch"]);
        let sheet = fx::sheet_with_net("R_TOP", "TOP_NET", &[]);
        let wire = SelectedItem::new(sheet.wires[0].uuid, SelectedKind::Wire);
        fx::open_with(&mut app.document_state, &top, sheet.clone());
        app.document_state.tabs.push(crate::app::TabInfo {
            title: "top".to_string(),
            path: top.clone(),
            cached_document: None,
            project_id: None,
            kind: crate::app::TabKind::Schematic,
        });
        app.document_state.active_tab = 0;
        app.document_state.active_path = Some(top.clone());
        let main = iced::window::Id::unique();
        app.ui_state.main_window_id = Some(main);
        let window = iced::window::Id::unique();
        app.ui_state.windows.insert(
            window,
            crate::app::state::WindowKind::UndockedTab {
                path: top,
                title: "top".to_string(),
            },
        );
        let mut slot = crate::canvas::CanvasSlot::default();
        slot.set_render_cache(Some(
            crate::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
        slot.selected = vec![wire];
        app.interaction_state.canvases.insert(window, slot);
        app.interaction_state.canvas.set_render_cache(Some(
            crate::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
        app.interaction_state.canvas.selected = vec![wire];
        (app, main, window)
    }

    fn drag_selection(app: &mut Signex, window_id: iced::window::Id) {
        let event = crate::canvas::CanvasEvent::MoveSelected {
            dx: 10.0,
            dy: 0.0,
            connections: Default::default(),
        };
        let _task = app.dispatch_update(Message::CanvasEventInWindow { window_id, event });
    }

    fn engine_wire_end(app: &Signex) -> f64 {
        app.document_state.active_engine().unwrap().document().wires[0]
            .end
            .x
    }

    fn cached_wire_end(slot: &crate::canvas::CanvasSlot) -> f64 {
        slot.active_snapshot().unwrap().wires[0].end.x
    }

    /// An edit made from the undocked window must reach the main window's
    /// cache and clear its drawn content cache (the swap-back in `ui.rs`).
    #[test]
    fn edit_from_undocked_window_refreshes_the_main_slot() {
        let (mut app, _main, window) = main_and_undocked_on_one_sheet();
        let before = engine_wire_end(&app);
        let clears = app.interaction_state.canvas.content_cache_clears;
        drag_selection(&mut app, window);
        assert_ne!(engine_wire_end(&app), before, "the edit must apply");
        assert_eq!(
            cached_wire_end(&app.interaction_state.canvas),
            engine_wire_end(&app)
        );
        assert_eq!(
            cached_wire_end(&app.interaction_state.canvases[&window]),
            engine_wire_end(&app)
        );
        assert!(app.interaction_state.canvas.content_cache_clears > clears);
    }

    /// An edit made from the main window must reach the undocked window's
    /// cache and clear its drawn content cache.
    #[test]
    fn edit_from_main_window_refreshes_the_undocked_slot() {
        let (mut app, main, window) = main_and_undocked_on_one_sheet();
        let before = engine_wire_end(&app);
        let clears = app.interaction_state.canvases[&window].content_cache_clears;
        drag_selection(&mut app, main);
        assert_ne!(engine_wire_end(&app), before, "the edit must apply");
        let other = &app.interaction_state.canvases[&window];
        assert_eq!(cached_wire_end(other), engine_wire_end(&app));
        assert!(other.content_cache_clears > clears);
    }

    /// A hover over the undocked window changes no document, so the main
    /// slot is not refreshed: its deliberately stale cache stays stale and no
    /// content cache is cleared.
    #[test]
    fn hover_over_undocked_window_does_not_resync() {
        let (mut app, _main, window) = main_and_undocked_on_one_sheet();
        let mut stale_sheet = app
            .document_state
            .active_engine()
            .unwrap()
            .document()
            .clone();
        stale_sheet.wires[0].end.x = 12345.0;
        app.interaction_state.canvas.set_render_cache(Some(
            crate::schematic_runtime::SchematicRenderCache::from_sheet(&stale_sheet),
        ));
        let main_clears = app.interaction_state.canvas.content_cache_clears;
        let window_clears = app.interaction_state.canvases[&window].content_cache_clears;
        for event in [
            crate::canvas::CanvasEvent::CursorMoved,
            crate::canvas::CanvasEvent::CursorAt {
                x: 1.0,
                y: 1.0,
                zoom_pct: 100.0,
            },
        ] {
            let _task = app.dispatch_update(Message::CanvasEventInWindow {
                window_id: window,
                event,
            });
        }
        assert_eq!(cached_wire_end(&app.interaction_state.canvas), 12345.0);
        assert_eq!(
            app.interaction_state.canvas.content_cache_clears,
            main_clears
        );
        assert_eq!(
            app.interaction_state.canvases[&window].content_cache_clears,
            window_clears
        );
    }

    /// The export fixture, on purpose: the whole point is that both paths
    /// answer "what sheets does this project consist of" the same way.
    fn app_with_a_child_only_on_disk() -> (Signex, std::path::PathBuf) {
        use crate::app::handlers::menu::export::tests as fx;
        let dir =
            std::env::temp_dir().join(format!("signex-gateway-disk-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&dir).expect("tempdir");
        let child = signex_types::format::SnxSchematic::new(fx::sheet_with_net(
            "R_DISK",
            "ON_DISK_NET",
            &[],
        ))
        .write_string()
        .expect("serialize child");
        std::fs::write(dir.join("child.snxsch"), child).expect("write child");

        let mut app = fx::app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
        let top = dir.join("top.snxsch");
        fx::open_with(
            &mut app.document_state,
            &top,
            fx::sheet_with_net("R_TOP", "TOP_NET", &["child.snxsch"]),
        );
        app.document_state.active_path = Some(top);
        (app, dir)
    }

    #[test]
    fn the_stitched_netlist_stitches_a_child_that_is_only_on_disk() {
        // #406 verbatim, on the path the issue is actually about: this netlist
        // is the same stitch ERC and export derive. It used to be built from
        // the open engines plus the declared pages and no child-sheet walk at
        // all, so an unopened, unlisted child sitting next to its parent came
        // back as `MissingChild` and shipped that phantom "could not be found"
        // to the Messages panel — while the export, on the same state at the
        // same instant, stitched the very same child in without complaint.
        let (app, dir) = app_with_a_child_only_on_disk();

        let result = crate::app::stitched_netlist::stitched_for_active_project(&app.document_state);

        std::fs::remove_dir_all(&dir).ok();
        assert!(
            result.warnings.is_empty(),
            "the child is on disk beside its parent; nothing is missing: {:?}",
            result.warnings
        );
        let names = net_names(&result);
        assert!(
            names.contains(&"ON_DISK_NET".to_string()),
            "the child's nets must be in the stitched netlist: {names:?}"
        );
    }

    #[test]
    fn erc_checks_a_child_that_is_only_on_disk() {
        // Same assembler, third consumer: ERC used to see the open tabs plus
        // the declared pages, so an unopened hierarchical child was neither
        // checked nor available to resolve `BadHierSheetPin`.
        let (mut app, dir) = app_with_a_child_only_on_disk();
        let child = dir.join("child.snxsch");

        app.run_erc_blocking();

        let checked: Vec<std::path::PathBuf> = app
            .ui_state
            .erc_violations_by_path
            .keys()
            .cloned()
            .collect();
        std::fs::remove_dir_all(&dir).ok();
        assert!(
            checked.contains(&child),
            "the project's on-disk child must be part of the ERC run: {checked:?}"
        );
    }

    #[test]
    fn resetting_duplicate_designators_sees_a_child_that_is_only_on_disk() {
        // Fourth consumer of the same question, and the one that writes to
        // disk. `top.snxsch` and its unlisted, unopened child both carry R1.
        // Counting only the open engines plus the declared pages made the
        // child invisible, so R1 looked unique, nothing was reset, and the
        // duplicate went out in the netlist and onto the board.
        use crate::app::handlers::menu::export::tests as fx;
        let dir = std::env::temp_dir().join(format!("signex-dupe-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&dir).expect("tempdir");
        let child_path = dir.join("child.snxsch");
        let child =
            signex_types::format::SnxSchematic::new(fx::sheet_with_net("R1", "ON_DISK_NET", &[]))
                .write_string()
                .expect("serialize child");
        std::fs::write(&child_path, child).expect("write child");

        let mut app = fx::app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
        let top = dir.join("top.snxsch");
        fx::open_with(
            &mut app.document_state,
            &top,
            fx::sheet_with_net("R1", "TOP_NET", &["child.snxsch"]),
        );
        app.document_state.active_path = Some(top);

        let task = app.handle_reset_duplicate_designators();
        crate::app::path_ops::settle_task(&mut app, task);

        let written = std::fs::read_to_string(&child_path).expect("read child back");
        std::fs::remove_dir_all(&dir).ok();
        assert!(
            written.contains("R?"),
            "the on-disk child's duplicate R1 must have been reset: {written}"
        );
    }

    /// Project `top` -> `child`, both open, `child` focused.
    fn app_with_the_child_tab_focused() -> Signex {
        use crate::app::handlers::menu::export::tests as fx;
        let dir = std::path::PathBuf::from("/w/rooted");
        let mut app = fx::app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
        let top = dir.join("top.snxsch");
        let child = dir.join("child.snxsch");
        fx::open_with(
            &mut app.document_state,
            &top,
            fx::sheet_with_net("R_TOP", "TOP_NET", &["child.snxsch"]),
        );
        fx::open_with(
            &mut app.document_state,
            &child,
            fx::sheet_with_net("R_CHILD", "CHILD_NET", &[]),
        );
        app.document_state.active_path = Some(child);
        app
    }

    fn net_names(stitched: &crate::app::stitched_netlist::Stitched) -> Vec<String> {
        stitched
            .netlist
            .as_ref()
            .expect("the root sheet must assemble")
            .nets
            .iter()
            .map(|n| n.name.clone())
            .collect()
    }

    fn stitched_net_names(app: &Signex) -> Vec<String> {
        net_names(&crate::app::stitched_netlist::stitched_for_active_project(
            &app.document_state,
        ))
    }

    #[test]
    fn the_stitched_netlist_is_rooted_at_the_project_not_the_focused_child() {
        // #338: with the child tab focused, the netlist used to be rooted at
        // the child and silently lost every net that only the parent carries.
        let app = app_with_the_child_tab_focused();

        let names = stitched_net_names(&app);
        assert!(names.contains(&"TOP_NET".to_string()), "{names:?}");
        assert!(names.contains(&"CHILD_NET".to_string()), "{names:?}");
    }

    #[test]
    fn a_loose_document_roots_the_netlist_at_itself() {
        // The explicit fallback: no owning project -> the active sheet.
        use crate::app::handlers::menu::export::tests as fx;
        let (mut app, _task) = Signex::new();
        let loose = std::path::PathBuf::from("/w/loose/scratch.snxsch");
        fx::open_with(
            &mut app.document_state,
            &loose,
            fx::sheet_with_net("R_L", "LOOSE_NET", &[]),
        );
        app.document_state.active_path = Some(loose.clone());

        assert_eq!(
            crate::app::project_sheets::resolve_netlist_root(&app.document_state),
            Some(loose)
        );
        assert!(stitched_net_names(&app).contains(&"LOOSE_NET".to_string()));
    }

    #[test]
    fn a_project_without_a_declared_root_falls_back_to_the_active_sheet() {
        let mut app = app_with_the_child_tab_focused();
        app.document_state.projects[0].data.schematic_root = None;
        let child = std::path::PathBuf::from("/w/rooted/child.snxsch");

        assert_eq!(
            crate::app::project_sheets::resolve_netlist_root(&app.document_state),
            Some(child.clone())
        );
        let warning = crate::app::project_sheets::netlist_root_fallback_warning_for(
            &app.document_state,
            &child,
        )
        .expect("a rootless project warns");
        assert!(warning.contains("declares no root sheet"), "{warning}");
        assert!(warning.contains("child.snxsch"), "{warning}");
    }

    #[test]
    fn a_declared_root_or_a_loose_document_raises_no_fallback_warning() {
        let app = app_with_the_child_tab_focused();
        assert_eq!(
            crate::app::project_sheets::netlist_root_fallback_warning_for(
                &app.document_state,
                std::path::Path::new("/w/rooted/child.snxsch"),
            ),
            None
        );
    }
}
