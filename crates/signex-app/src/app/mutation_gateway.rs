use super::*;

impl Signex {
    fn render_invalidation_for_patch(
        patch: signex_engine::DocumentPatch,
    ) -> crate::schematic_runtime::RenderInvalidation {
        use crate::schematic_runtime::RenderInvalidation;

        if patch.contains(signex_engine::DocumentPatch::FULL) {
            return RenderInvalidation::FULL;
        }

        let mut invalidation = RenderInvalidation::NONE;
        if patch.contains(signex_engine::DocumentPatch::SYMBOLS) {
            invalidation |= RenderInvalidation::SYMBOLS;
        }
        if patch.contains(signex_engine::DocumentPatch::WIRES) {
            invalidation |= RenderInvalidation::WIRES;
        }
        if patch.contains(signex_engine::DocumentPatch::LABELS) {
            invalidation |= RenderInvalidation::LABELS;
        }
        if patch.contains(signex_engine::DocumentPatch::TEXT_NOTES) {
            invalidation |= RenderInvalidation::TEXT_NOTES;
        }
        if patch.contains(signex_engine::DocumentPatch::BUSES) {
            invalidation |= RenderInvalidation::BUSES;
        }
        if patch.contains(signex_engine::DocumentPatch::BUS_ENTRIES) {
            invalidation |= RenderInvalidation::BUS_ENTRIES;
        }
        if patch.contains(signex_engine::DocumentPatch::JUNCTIONS) {
            invalidation |= RenderInvalidation::JUNCTIONS;
        }
        if patch.contains(signex_engine::DocumentPatch::NO_CONNECTS) {
            invalidation |= RenderInvalidation::NO_CONNECTS;
        }
        if patch.contains(signex_engine::DocumentPatch::CHILD_SHEETS) {
            invalidation |= RenderInvalidation::CHILD_SHEETS;
        }
        if patch.contains(signex_engine::DocumentPatch::DRAWINGS) {
            invalidation |= RenderInvalidation::DRAWINGS;
        }
        if patch.contains(signex_engine::DocumentPatch::LIB_SYMBOLS) {
            invalidation |= RenderInvalidation::LIB_SYMBOLS;
        }
        if patch.contains(signex_engine::DocumentPatch::PAPER) {
            invalidation |= RenderInvalidation::PAPER;
        }

        invalidation
    }

    pub(crate) fn apply_engine_commands(
        &mut self,
        commands: Vec<signex_engine::Command>,
        clear_overlay_cache: bool,
        update_selection_info: bool,
    ) -> bool {
        if commands.is_empty() {
            return false;
        }

        let Some(engine) = self.document_state.active_engine_mut() else {
            return false;
        };

        let invalidation = {
            let mut changed_steps = 0usize;
            let mut invalidation = crate::schematic_runtime::RenderInvalidation::NONE;

            for command in commands {
                match engine.execute(command) {
                    Ok(result) => {
                        if let Some(patch_pair) = result.patch_pair {
                            changed_steps += 1;
                            invalidation |=
                                Self::render_invalidation_for_patch(patch_pair.document);
                        }
                    }
                    Err(error) => {
                        let error = anyhow::Error::new(error);
                        crate::diagnostics::log_error("Engine command execution failed", &error);
                        return false;
                    }
                }
            }

            if changed_steps > 0 {
                self.interaction_state
                    .undo_stack
                    .record_engine_marker(changed_steps);
                invalidation
            } else {
                crate::schematic_runtime::RenderInvalidation::NONE
            }
        };

        self.finish_schematic_mutation(invalidation, clear_overlay_cache, update_selection_info)
    }

    pub(crate) fn apply_engine_command(
        &mut self,
        command: signex_engine::Command,
        clear_overlay_cache: bool,
        update_selection_info: bool,
    ) -> bool {
        let Some(engine) = self.document_state.active_engine_mut() else {
            return false;
        };

        let invalidation = match engine.execute(command) {
            Ok(result) if result.changed => {
                let invalidation = result
                    .patch_pair
                    .map(|patch_pair| Self::render_invalidation_for_patch(patch_pair.document))
                    .unwrap_or(crate::schematic_runtime::RenderInvalidation::NONE);
                self.interaction_state.undo_stack.record_engine_marker(1);
                invalidation
            }
            Ok(_) => crate::schematic_runtime::RenderInvalidation::NONE,
            Err(error) => {
                let error = anyhow::Error::new(error);
                crate::diagnostics::log_error("Engine command failed", &error);
                crate::schematic_runtime::RenderInvalidation::NONE
            }
        };

        self.finish_schematic_mutation(invalidation, clear_overlay_cache, update_selection_info)
    }

    pub(crate) fn apply_engine_undo(&mut self, update_selection_info: bool) -> bool {
        let invalidation = if let Some(engine) = self.document_state.active_engine_mut() {
            let Some(steps) = self.interaction_state.undo_stack.peek_undo_engine_steps() else {
                return false;
            };

            let mut undone_steps = 0usize;
            let mut invalidation = crate::schematic_runtime::RenderInvalidation::NONE;
            for _ in 0..steps {
                match engine.undo() {
                    Ok(Some(patch_pair)) => {
                        undone_steps += 1;
                        invalidation |= Self::render_invalidation_for_patch(patch_pair.document);
                    }
                    Ok(None) => break,
                    Err(error) => {
                        let error = anyhow::Error::new(error);
                        crate::diagnostics::log_error("Engine undo failed", &error);
                        return false;
                    }
                }
            }

            if undone_steps == steps && self.interaction_state.undo_stack.step_back() {
                invalidation
            } else {
                crate::schematic_runtime::RenderInvalidation::NONE
            }
        } else {
            crate::schematic_runtime::RenderInvalidation::NONE
        };

        self.finish_schematic_mutation(invalidation, true, update_selection_info)
    }

    pub(crate) fn apply_engine_redo(&mut self, update_selection_info: bool) -> bool {
        let invalidation = if let Some(engine) = self.document_state.active_engine_mut() {
            let Some(steps) = self.interaction_state.undo_stack.peek_redo_engine_steps() else {
                return false;
            };

            let mut redone_steps = 0usize;
            let mut invalidation = crate::schematic_runtime::RenderInvalidation::NONE;
            for _ in 0..steps {
                match engine.redo() {
                    Ok(Some(patch_pair)) => {
                        redone_steps += 1;
                        invalidation |= Self::render_invalidation_for_patch(patch_pair.document);
                    }
                    Ok(None) => break,
                    Err(error) => {
                        let error = anyhow::Error::new(error);
                        crate::diagnostics::log_error("Engine redo failed", &error);
                        return false;
                    }
                }
            }

            if redone_steps == steps && self.interaction_state.undo_stack.step_forward() {
                invalidation
            } else {
                crate::schematic_runtime::RenderInvalidation::NONE
            }
        } else {
            crate::schematic_runtime::RenderInvalidation::NONE
        };

        self.finish_schematic_mutation(invalidation, true, update_selection_info)
    }

    fn finish_schematic_mutation(
        &mut self,
        invalidation: crate::schematic_runtime::RenderInvalidation,
        clear_overlay_cache: bool,
        update_selection_info: bool,
    ) -> bool {
        if invalidation == crate::schematic_runtime::RenderInvalidation::NONE {
            return false;
        }

        if self
            .with_active_schematic_session_mut(|session| {
                session.set_dirty(true);
            })
            .is_none()
        {
            return false;
        }
        self.sync_canvas_from_visible_schematic(invalidation);
        self.interaction_state
            .active_canvas_mut()
            .clear_content_cache();
        if clear_overlay_cache {
            self.interaction_state
                .active_canvas_mut()
                .clear_overlay_cache();
        }
        if update_selection_info {
            self.update_selection_info();
        }
        // Invalidate + re-derive the cached project netlist when this edit
        // touched connectivity (ADR-0002 D7). Same electrical bits that drive
        // net membership — deliberately including no-connects and buses, which
        // `point_is_connected` reads.
        if invalidation.intersects(Self::netlist_render_mask()) {
            self.ui_state.project_netlist = None;
            self.refresh_project_netlist();
        }
        // Re-derive panel context — `tab.dirty` has just transitioned
        // false→true (or stayed true), and `panel_ctx.projects[*].sheets[*]
        // .is_dirty` is what drives the red dot on the project-tree row.
        // Without this refresh, the dot only appears after the next
        // unrelated event that happens to call `refresh_panel_ctx`.
        self.refresh_panel_ctx();
        true
    }

    /// The `RenderInvalidation` bits that mean project connectivity changed, so
    /// the cached netlist must be re-derived. Mirrors the electrical
    /// `DocumentPatch` bits: symbols, wires, labels, junctions, child sheets,
    /// and — easy to miss — no-connects and buses, which `point_is_connected`
    /// reads when deciding whether a pin lands on a net.
    fn netlist_render_mask() -> crate::schematic_runtime::RenderInvalidation {
        use crate::schematic_runtime::RenderInvalidation as R;
        R::SYMBOLS
            | R::WIRES
            | R::LABELS
            | R::JUNCTIONS
            | R::CHILD_SHEETS
            | R::NO_CONNECTS
            | R::BUSES
            | R::BUS_ENTRIES
    }

    /// Re-derive the cached project netlist off the shared sheet view (rooted at
    /// the active sheet) and surface any stitch issues in the Messages panel.
    /// A cheap no-op while the cache is still valid.
    pub(crate) fn refresh_project_netlist(&mut self) {
        if self.ui_state.project_netlist.is_some() {
            return;
        }
        let Some(active_path) = self.document_state.active_path.clone() else {
            return;
        };
        // The same assembler the export uses, so the canvas/ERC netlist and
        // the exported one cannot disagree about what the project consists of.
        // This path used to take "every open engine + the declared pages",
        // which walked no child-sheet references at all: a child sitting on
        // disk next to its parent, unopened and unlisted, produced a phantom
        // "could not be found" warning here while the export stitched it in
        // without complaint (#406).
        //
        // Rooted at the active sheet, not the project root — this netlist
        // backs NET_NAME() on the canvas you are looking at. The declared page
        // set is deliberately not seeded: pages the active sheet does not
        // reach are not part of *its* netlist.
        let set = crate::app::project_sheets::assemble_project_sheets(
            &self.document_state,
            &[],
            &active_path,
        );
        let Some(root) = set.sheets.get(&active_path).cloned() else {
            return;
        };
        let children = crate::app::project_sheets::project_children_map(&set.sheets);
        let project_dir = self
            .document_state
            .active_document_project()
            .map(|p| p.dir().to_path_buf());
        let root_filename =
            crate::app::project_sheets::root_reference_name(&active_path, project_dir.as_deref());
        let result = signex_net::build_project_netlist(&root, &children, root_filename.as_deref());
        // A child that exists but will not parse reaches the stitcher as a
        // plain MissingChild; say which it was, or the user hunts for a file
        // that is sitting right there.
        for (path, why) in &set.unreadable {
            crate::diagnostics::log_warning(format!("Netlist: sheet '{}' {why}", path.display()));
        }
        for issue in &result.issues {
            crate::diagnostics::log_warning(crate::app::project_sheets::stitch_issue_message(
                issue,
            ));
        }
        self.ui_state.project_netlist = Some(result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_engine::DocumentPatch;

    fn touches_netlist(patch: DocumentPatch) -> bool {
        Signex::render_invalidation_for_patch(patch).intersects(Signex::netlist_render_mask())
    }

    #[test]
    fn connectivity_edits_invalidate_the_netlist_cache() {
        // Every bit that changes net membership must invalidate the cache —
        // including the two the review flagged as easy to miss: no-connects and
        // buses, which `point_is_connected` reads.
        for bit in [
            DocumentPatch::SYMBOLS,
            DocumentPatch::WIRES,
            DocumentPatch::LABELS,
            DocumentPatch::JUNCTIONS,
            DocumentPatch::CHILD_SHEETS,
            DocumentPatch::NO_CONNECTS,
            DocumentPatch::BUSES,
            DocumentPatch::BUS_ENTRIES,
        ] {
            assert!(touches_netlist(bit), "{bit:?} must invalidate the netlist");
        }
    }

    #[test]
    fn non_connectivity_edits_leave_the_netlist_cache() {
        for bit in [
            DocumentPatch::TEXT_NOTES,
            DocumentPatch::DRAWINGS,
            DocumentPatch::LIB_SYMBOLS,
            DocumentPatch::PAPER,
        ] {
            assert!(
                !touches_netlist(bit),
                "{bit:?} must not invalidate the netlist"
            );
        }
    }

    /// The export fixture, on purpose: the whole point is that both paths
    /// answer "what sheets does this project consist of" the same way.
    fn app_with_a_child_only_on_disk() -> (Signex, std::path::PathBuf) {
        use crate::app::handlers::menu::export::tests as fx;
        let dir =
            std::env::temp_dir().join(format!("signex-gateway-disk-{}", uuid::Uuid::new_v4()));
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
    fn the_cached_netlist_stitches_a_child_that_is_only_on_disk() {
        // #406 verbatim, on the path the issue is actually about: this netlist
        // backs ERC and the canvas NET_NAME() display. It used to be built from
        // the open engines plus the declared pages and no child-sheet walk at
        // all, so an unopened, unlisted child sitting next to its parent came
        // back as `MissingChild` and shipped that phantom "could not be found"
        // to the Messages panel — while the export, on the same state at the
        // same instant, stitched the very same child in without complaint.
        let (mut app, dir) = app_with_a_child_only_on_disk();

        app.refresh_project_netlist();

        let result = app
            .ui_state
            .project_netlist
            .clone()
            .expect("the netlist must be cached");
        std::fs::remove_dir_all(&dir).ok();
        assert!(
            result.issues.is_empty(),
            "the child is on disk beside its parent; nothing is missing: {:?}",
            result.issues
        );
        assert!(
            result.netlist.nets.iter().any(|n| n.name == "ON_DISK_NET"),
            "the child's nets must be in the cached netlist: {:?}",
            result
                .netlist
                .nets
                .iter()
                .map(|n| &n.name)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn erc_checks_a_child_that_is_only_on_disk() {
        // Same assembler, third consumer: ERC used to see the open tabs plus
        // the declared pages, so an unopened hierarchical child was neither
        // checked nor available to resolve `BadHierSheetPin`.
        let (mut app, dir) = app_with_a_child_only_on_disk();
        let child = dir.join("child.snxsch");

        let _ = app.handle_run_erc();

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
        let dir = std::env::temp_dir().join(format!("signex-dupe-{}", uuid::Uuid::new_v4()));
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

        let _ = app.handle_reset_duplicate_designators();

        let written = std::fs::read_to_string(&child_path).expect("read child back");
        std::fs::remove_dir_all(&dir).ok();
        assert!(
            written.contains("R?"),
            "the on-disk child's duplicate R1 must have been reset: {written}"
        );
    }
}
