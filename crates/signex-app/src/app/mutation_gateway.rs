use super::*;

impl Signex {
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

        let updated_sheet = {
            let mut changed_steps = 0usize;

            for command in commands {
                match engine.execute(command) {
                    Ok(result) => {
                        if result.changed {
                            changed_steps += 1;
                        }
                    }
                    Err(error) => {
                        eprintln!("[engine] command execution failed: {error}");
                        return false;
                    }
                }
            }

            if changed_steps > 0 {
                self.undo_stack.record_engine_marker(changed_steps);
                Some(engine.document().clone())
            } else {
                None
            }
        };

        self.finish_schematic_mutation(updated_sheet, clear_overlay_cache, update_selection_info)
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

        let updated_sheet = match engine.execute(command) {
            Ok(result) if result.changed => {
                self.undo_stack.record_engine_marker(1);
                Some(engine.document().clone())
            }
            Ok(_) => None,
            Err(error) => {
                eprintln!("[engine] failed to construct engine: {error}");
                None
            }
        };

        self.finish_schematic_mutation(updated_sheet, clear_overlay_cache, update_selection_info)
    }

    pub(crate) fn place_wire_segment_with_junctions(
        &mut self,
        wire: signex_types::schematic::Wire,
    ) -> bool {
        const TOL: f64 = 0.01;

        let updated_sheet = if let Some(ref mut sheet) = self.schematic {
            let mut commands = vec![crate::undo::EditCommand::AddWire(wire.clone())];

            if let Some(junction) = helpers::needed_junction(wire.start, sheet, TOL) {
                commands.push(crate::undo::EditCommand::AddJunction(junction));
            }
            if let Some(junction) = helpers::needed_junction(wire.end, sheet, TOL) {
                commands.push(crate::undo::EditCommand::AddJunction(junction));
            }

            self.undo_stack.execute(
                sheet,
                crate::undo::EditCommand::Batch(commands),
            );

            for &check_point in &[wire.start, wire.end] {
                if let Some(junction) = helpers::needed_junction(check_point, sheet, TOL) {
                    self.undo_stack
                        .execute(sheet, crate::undo::EditCommand::AddJunction(junction));
                }
            }

            Some(sheet.clone())
        } else {
            None
        };

        self.finish_schematic_mutation(updated_sheet, false, false)
    }

    pub(crate) fn apply_edit_command(
        &mut self,
        command: crate::undo::EditCommand,
        clear_overlay_cache: bool,
        update_selection_info: bool,
    ) -> bool {
        let updated_sheet = if let Some(ref mut sheet) = self.schematic {
            self.undo_stack.execute(sheet, command);
            Some(sheet.clone())
        } else {
            None
        };

        self.finish_schematic_mutation(updated_sheet, clear_overlay_cache, update_selection_info)
    }

    pub(crate) fn apply_edit_batch(
        &mut self,
        commands: Vec<crate::undo::EditCommand>,
        clear_overlay_cache: bool,
        update_selection_info: bool,
    ) -> bool {
        if commands.is_empty() {
            return false;
        }

        self.apply_edit_command(
            crate::undo::EditCommand::Batch(commands),
            clear_overlay_cache,
            update_selection_info,
        )
    }

    pub(crate) fn apply_undo(&mut self, update_selection_info: bool) -> bool {
        let updated_sheet = if let Some(ref mut sheet) = self.schematic {
            if self.undo_stack.undo(sheet) {
                Some(sheet.clone())
            } else {
                None
            }
        } else {
            None
        };

        self.finish_schematic_mutation(updated_sheet, true, update_selection_info)
    }

    pub(crate) fn apply_engine_undo(&mut self, update_selection_info: bool) -> bool {
        let updated_sheet = if let Some(engine) = self.engine.as_mut() {
            let Some(steps) = self.undo_stack.peek_undo_engine_steps() else {
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
                Some(engine.document().clone())
            } else {
                None
            }
        } else {
            None
        };

        self.finish_schematic_mutation(updated_sheet, true, update_selection_info)
    }

    pub(crate) fn apply_redo(&mut self, update_selection_info: bool) -> bool {
        let updated_sheet = if let Some(ref mut sheet) = self.schematic {
            if self.undo_stack.redo(sheet) {
                Some(sheet.clone())
            } else {
                None
            }
        } else {
            None
        };

        self.finish_schematic_mutation(updated_sheet, true, update_selection_info)
    }

    pub(crate) fn apply_engine_redo(&mut self, update_selection_info: bool) -> bool {
        let updated_sheet = if let Some(engine) = self.engine.as_mut() {
            let Some(steps) = self.undo_stack.peek_redo_engine_steps() else {
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
                Some(engine.document().clone())
            } else {
                None
            }
        } else {
            None
        };

        self.finish_schematic_mutation(updated_sheet, true, update_selection_info)
    }

    fn finish_schematic_mutation(
        &mut self,
        updated_sheet: Option<SchematicSheet>,
        clear_overlay_cache: bool,
        update_selection_info: bool,
    ) -> bool {
        let Some(updated_sheet) = updated_sheet else {
            return false;
        };

        let engine_path = self.tabs.get(self.active_tab).map(|tab| tab.path.clone());
        if let Some(engine) = self.engine.as_mut() {
            engine.set_document(updated_sheet.clone());
            engine.set_path(engine_path);
        } else if let Ok(engine) = signex_engine::Engine::new_with_path(updated_sheet.clone(), engine_path) {
            self.engine = Some(engine);
        }
        self.canvas.schematic = Some(updated_sheet.clone());
        self.schematic = Some(updated_sheet);
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
