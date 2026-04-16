use super::*;

impl Signex {
    fn render_invalidation_for_patch(
        patch: signex_engine::DocumentPatch,
    ) -> signex_render::schematic::RenderInvalidation {
        use signex_render::schematic::RenderInvalidation;

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

        let Some(engine) = self.document_state.engine.as_mut() else {
            return false;
        };

        let invalidation = {
            let mut changed_steps = 0usize;
            let mut invalidation = signex_render::schematic::RenderInvalidation::NONE;

            for command in commands {
                match engine.execute(command) {
                    Ok(result) => {
                        if let Some(patch_pair) = result.patch_pair {
                            changed_steps += 1;
                            invalidation |= Self::render_invalidation_for_patch(patch_pair.document);
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
                self.interaction_state.undo_stack.record_engine_marker(changed_steps);
                invalidation
            } else {
                signex_render::schematic::RenderInvalidation::NONE
            }
        };

        self.finish_schematic_mutation(
            invalidation,
            clear_overlay_cache,
            update_selection_info,
        )
    }

    pub(crate) fn apply_engine_command(
        &mut self,
        command: signex_engine::Command,
        clear_overlay_cache: bool,
        update_selection_info: bool,
    ) -> bool {
        let Some(engine) = self.document_state.engine.as_mut() else {
            return false;
        };

        let invalidation = match engine.execute(command) {
            Ok(result) if result.changed => {
                let invalidation = result
                    .patch_pair
                    .map(|patch_pair| Self::render_invalidation_for_patch(patch_pair.document))
                    .unwrap_or(signex_render::schematic::RenderInvalidation::NONE);
                self.interaction_state.undo_stack.record_engine_marker(1);
                invalidation
            }
            Ok(_) => signex_render::schematic::RenderInvalidation::NONE,
            Err(error) => {
                let error = anyhow::Error::new(error);
                crate::diagnostics::log_error("Engine command failed", &error);
                signex_render::schematic::RenderInvalidation::NONE
            }
        };

        self.finish_schematic_mutation(
            invalidation,
            clear_overlay_cache,
            update_selection_info,
        )
    }

    pub(crate) fn apply_engine_undo(&mut self, update_selection_info: bool) -> bool {
        let invalidation = if let Some(engine) = self.document_state.engine.as_mut() {
            let Some(steps) = self.interaction_state.undo_stack.peek_undo_engine_steps() else {
                return false;
            };

            let mut undone_steps = 0usize;
            let mut invalidation = signex_render::schematic::RenderInvalidation::NONE;
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
                signex_render::schematic::RenderInvalidation::NONE
            }
        } else {
            signex_render::schematic::RenderInvalidation::NONE
        };

        self.finish_schematic_mutation(invalidation, true, update_selection_info)
    }

    pub(crate) fn apply_engine_redo(&mut self, update_selection_info: bool) -> bool {
        let invalidation = if let Some(engine) = self.document_state.engine.as_mut() {
            let Some(steps) = self.interaction_state.undo_stack.peek_redo_engine_steps() else {
                return false;
            };

            let mut redone_steps = 0usize;
            let mut invalidation = signex_render::schematic::RenderInvalidation::NONE;
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
                signex_render::schematic::RenderInvalidation::NONE
            }
        } else {
            signex_render::schematic::RenderInvalidation::NONE
        };

        self.finish_schematic_mutation(invalidation, true, update_selection_info)
    }

    fn finish_schematic_mutation(
        &mut self,
        invalidation: signex_render::schematic::RenderInvalidation,
        clear_overlay_cache: bool,
        update_selection_info: bool,
    ) -> bool {
        if invalidation == signex_render::schematic::RenderInvalidation::NONE {
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
        self.interaction_state.canvas.clear_content_cache();
        if clear_overlay_cache {
            self.interaction_state.canvas.clear_overlay_cache();
        }
        if update_selection_info {
            self.update_selection_info();
        }
        true
    }
}
