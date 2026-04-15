use super::*;

impl Signex {
    fn render_invalidation_for_selected_items(
        items: &[signex_types::schematic::SelectedItem],
    ) -> signex_render::schematic::RenderInvalidation {
        use signex_render::schematic::RenderInvalidation;
        use signex_types::schematic::SelectedKind;

        let mut invalidation = RenderInvalidation::NONE;
        for item in items {
            invalidation |= match item.kind {
                SelectedKind::Symbol
                | SelectedKind::SymbolRefField
                | SelectedKind::SymbolValField => {
                    RenderInvalidation::SYMBOLS | RenderInvalidation::LIB_SYMBOLS
                }
                SelectedKind::Wire => RenderInvalidation::WIRES,
                SelectedKind::Bus => RenderInvalidation::BUSES,
                SelectedKind::BusEntry => RenderInvalidation::BUS_ENTRIES,
                SelectedKind::Junction => RenderInvalidation::JUNCTIONS,
                SelectedKind::NoConnect => RenderInvalidation::NO_CONNECTS,
                SelectedKind::Label => RenderInvalidation::LABELS,
                SelectedKind::TextNote => RenderInvalidation::TEXT_NOTES,
                SelectedKind::ChildSheet => RenderInvalidation::CHILD_SHEETS,
                SelectedKind::Drawing => RenderInvalidation::DRAWINGS,
            };
        }

        if invalidation == RenderInvalidation::NONE {
            RenderInvalidation::FULL
        } else {
            invalidation
        }
    }

    fn render_invalidation_for_command(
        command: &signex_engine::Command,
    ) -> signex_render::schematic::RenderInvalidation {
        use signex_render::schematic::RenderInvalidation;

        match command {
            signex_engine::Command::ReplaceDocument { .. } => RenderInvalidation::FULL,
            signex_engine::Command::MoveSelection { items, .. }
            | signex_engine::Command::RotateSelection { items, .. }
            | signex_engine::Command::MirrorSelection { items, .. }
            | signex_engine::Command::DeleteSelection { items } => {
                Self::render_invalidation_for_selected_items(items)
            }
            signex_engine::Command::UpdateText { target, .. } => match target {
                signex_engine::TextTarget::Label(_) => RenderInvalidation::LABELS,
                signex_engine::TextTarget::TextNote(_) => RenderInvalidation::TEXT_NOTES,
                signex_engine::TextTarget::SymbolReference(_)
                | signex_engine::TextTarget::SymbolValue(_) => RenderInvalidation::SYMBOLS,
            },
            signex_engine::Command::UpdateSymbolFields { .. } => RenderInvalidation::SYMBOLS,
            signex_engine::Command::PlaceWireSegment { .. } => {
                RenderInvalidation::WIRES | RenderInvalidation::JUNCTIONS
            }
            signex_engine::Command::PlaceBus { .. } => RenderInvalidation::BUSES,
            signex_engine::Command::PlaceLabel { .. } => RenderInvalidation::LABELS,
            signex_engine::Command::PlaceSymbol { .. } => {
                RenderInvalidation::SYMBOLS | RenderInvalidation::LIB_SYMBOLS
            }
            signex_engine::Command::PlaceJunction { .. } => RenderInvalidation::JUNCTIONS,
            signex_engine::Command::PlaceNoConnect { .. } => RenderInvalidation::NO_CONNECTS,
            signex_engine::Command::PlaceBusEntry { .. } => RenderInvalidation::BUS_ENTRIES,
            signex_engine::Command::PlaceTextNote { .. } => RenderInvalidation::TEXT_NOTES,
        }
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

        let Some(engine) = self.engine.as_mut() else {
            return false;
        };

        let invalidation = {
            let mut changed_steps = 0usize;
            let mut invalidation = signex_render::schematic::RenderInvalidation::NONE;

            for command in commands {
                let command_invalidation = Self::render_invalidation_for_command(&command);
                match engine.execute(command) {
                    Ok(result) => {
                        if result.changed {
                            changed_steps += 1;
                            invalidation |= command_invalidation;
                        }
                    }
                    Err(error) => {
                        eprintln!("[engine] command execution failed: {error}");
                        return false;
                    }
                }
            }

            if changed_steps > 0 {
                self.undo_stack
                    .record_engine_marker(changed_steps, invalidation);
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
        let Some(engine) = self.engine.as_mut() else {
            return false;
        };

        let invalidation = match engine.execute(command.clone()) {
            Ok(result) if result.changed => {
                let invalidation = Self::render_invalidation_for_command(&command);
                self.undo_stack.record_engine_marker(1, invalidation);
                invalidation
            }
            Ok(_) => signex_render::schematic::RenderInvalidation::NONE,
            Err(error) => {
                eprintln!("[engine] failed to construct engine: {error}");
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
        let invalidation = if let Some(engine) = self.engine.as_mut() {
            let Some((steps, invalidation)) = self.undo_stack.peek_undo_engine_marker() else {
                return false;
            };

            let mut undone_steps = 0usize;
            for _ in 0..steps {
                match engine.undo() {
                    Ok(Some(_)) => undone_steps += 1,
                    Ok(None) => break,
                    Err(error) => {
                        eprintln!("[engine] undo failed: {error}");
                        return false;
                    }
                }
            }

            if undone_steps == steps && self.undo_stack.step_back() {
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
        let invalidation = if let Some(engine) = self.engine.as_mut() {
            let Some((steps, invalidation)) = self.undo_stack.peek_redo_engine_marker() else {
                return false;
            };

            let mut redone_steps = 0usize;
            for _ in 0..steps {
                match engine.redo() {
                    Ok(Some(_)) => redone_steps += 1,
                    Ok(None) => break,
                    Err(error) => {
                        eprintln!("[engine] redo failed: {error}");
                        return false;
                    }
                }
            }

            if redone_steps == steps && self.undo_stack.step_forward() {
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

        let engine_path = self.tabs.get(self.active_tab).map(|tab| tab.path.clone());
        if let Some(engine) = self.engine.as_mut() {
            engine.set_path(engine_path);
        } else {
            return false;
        }
        self.sync_canvas_from_visible_schematic(invalidation);
        self.canvas.clear_content_cache();
        if clear_overlay_cache {
            self.canvas.clear_overlay_cache();
        }
        self.mark_dirty();
        self.commit_schematic();
        if update_selection_info {
            self.update_selection_info();
        }
        true
    }
}
