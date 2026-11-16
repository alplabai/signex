//! The directive tools (#2420): a Blanket or Compile Mask is two corner
//! clicks, a Differential Pair directive one click on a `_P` or `_N` net.
//! Every placement and edit is one `Command` through the gateway, so one
//! undo step. Design note: `docs/SCHEMATIC_DIRECTIVES.md`.

use signex_types::schematic::{Point, SchDirective};

use super::super::super::*;
use super::shape_tool::ClickKind;
use crate::canvas::ShapePreviewKind;
use crate::ignore::IgnoreResult;

/// A Blanket over the region `start`..`end`.
pub(crate) fn blanket_directive(
    start: Point,
    end: Point,
    net_class: Option<String>,
    erc_exclusions: Vec<String>,
) -> SchDirective {
    SchDirective::Blanket {
        uuid: uuid::Uuid::now_v7(),
        start,
        end,
        net_class: net_class.filter(|c| !c.trim().is_empty()),
        erc_exclusions,
    }
}

/// A Compile Mask over the region `start`..`end`.
pub(crate) fn compile_mask_directive(start: Point, end: Point) -> SchDirective {
    SchDirective::CompileMask {
        uuid: uuid::Uuid::now_v7(),
        start,
        end,
    }
}

/// A Differential Pair directive at `position`.
pub(crate) fn diff_pair_directive(position: Point) -> SchDirective {
    SchDirective::DiffPair {
        uuid: uuid::Uuid::now_v7(),
        position,
    }
}

impl Signex {
    /// One snapped point of the Blanket or Compile Mask tool: the first click
    /// anchors a corner (the rubber band follows the cursor), the second
    /// places the region and leaves the tool armed for the next one. A
    /// double-click stands for the second click and never seeds a corner.
    pub(super) fn directive_region_point(&mut self, tool: Tool, p: Point, kind: ClickKind) {
        match self.interaction_state.shape_anchor.take() {
            None if kind == ClickKind::Click => {
                self.set_shape_anchor(Some((p, ShapePreviewKind::Rect)));
            }
            None => {}
            Some(start) => {
                self.set_shape_anchor(None);
                let directive = if tool == Tool::Blanket {
                    blanket_directive(start, p, None, Vec::new())
                } else {
                    compile_mask_directive(start, p)
                };
                // The gateway logs a refusal itself.
                self.place_directive(directive)
                    .ignore("the gateway logs a refusal itself");
            }
        }
    }

    /// One snapped click of the Differential Pair tool.
    pub(super) fn diff_pair_click(&mut self, at: Point) {
        // The gateway logs a refusal itself.
        self.place_directive(diff_pair_directive(at))
            .ignore("the gateway logs a refusal itself");
    }

    /// Place `directive` on the edit target as one undo step, or say why not.
    pub(crate) fn place_directive(&mut self, directive: SchDirective) -> Result<(), String> {
        self.commit_directive(signex_engine::Command::PlaceDirective { directive })
    }

    /// Replace a placed directive (same uuid and kind). Consecutive edits of
    /// one directive share one undo step, like typing a field; a caller that
    /// is one step on its own ends the run with [`Self::end_directive_edit_run`].
    pub(crate) fn update_directive(&mut self, directive: SchDirective) -> Result<(), String> {
        self.commit_directive(signex_engine::Command::UpdateDirective { directive })
    }

    /// A Properties edit of the Blanket `id`. Keystrokes in the class field
    /// are one undo step (the typing run); each rule checkbox is one step.
    pub(crate) fn edit_blanket_field(&mut self, id: uuid::Uuid, edit: &crate::panels::BlanketEdit) {
        let Some(mut directive) = self.document_state.edit_engine().and_then(|engine| {
            engine
                .document()
                .directives
                .iter()
                .find(|d| d.uuid() == id)
                .cloned()
        }) else {
            return;
        };
        let SchDirective::Blanket {
            net_class,
            erc_exclusions,
            ..
        } = &mut directive
        else {
            return;
        };
        let typing = match edit {
            // Kept as typed, spaces included, so the field does not fight the
            // cursor; readers trim it (`SchDirective::net_class`).
            crate::panels::BlanketEdit::NetClass(value) => {
                *net_class = Some(value.clone()).filter(|c| !c.trim().is_empty());
                true
            }
            crate::panels::BlanketEdit::Rule { rule_id, excluded } => {
                erc_exclusions.retain(|r| r != rule_id);
                if *excluded {
                    erc_exclusions.push(rule_id.clone());
                }
                false
            }
        };
        if !typing {
            self.end_directive_edit_run();
        }
        // The gateway logs a refusal itself; an unchanged value is no edit.
        self.update_directive(directive)
            .ignore("the gateway logs a refusal itself; an unchanged value is no edit");
        if !typing {
            self.end_directive_edit_run();
        }
    }

    /// End the typing run of directive edits on the edit target, so the next
    /// edit is an undo step of its own.
    pub(crate) fn end_directive_edit_run(&mut self) {
        if let Some(engine) = self.document_state.edit_engine_mut() {
            engine.end_field_edit_run();
        }
    }

    fn commit_directive(&mut self, command: signex_engine::Command) -> Result<(), String> {
        let (outcome, refusal) = self.apply_engine_command_reporting(command, true, true);
        match (outcome, refusal) {
            (_, Some(reason)) => Err(reason),
            (crate::app::mutation_gateway::CommandOutcome::Changed, None) => Ok(()),
            _ => Err("the directive is already as given".to_string()),
        }
    }
}
