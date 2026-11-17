//! `Command::PlaceDirective` and `Command::UpdateDirective` (#2420): a
//! Blanket, Compile Mask or Differential Pair directive placed or edited as
//! one undo step.

use crate::*;
use signex_types::schematic::SchDirective;

/// The typing-run key of a directive edit. The run is also keyed by the
/// directive's uuid, which no symbol shares, so it never continues a symbol
/// field run.
const DIRECTIVE_RUN_KEY: &str = "directive";

/// Whether `directive` has finite points and, for a region, two corners
/// that span an area.
fn is_valid(directive: &SchDirective) -> bool {
    let finite = |p: &signex_types::schematic::Point| p.x.is_finite() && p.y.is_finite();
    match directive {
        SchDirective::Blanket { start, end, .. } | SchDirective::CompileMask { start, end, .. } => {
            finite(start) && finite(end) && start.x != end.x && start.y != end.y
        }
        SchDirective::DiffPair { position, .. } => finite(position),
    }
}

impl Engine {
    pub(crate) fn exec_directives(
        &mut self,
        before: SchematicSheet,
        cmd: Command,
    ) -> Result<CommandResult, EngineError> {
        // An update is recorded as a typing run: consecutive edits of one
        // directive share one undo step (`Engine::end_field_edit_run`).
        let (semantic, run) = match cmd {
            Command::PlaceDirective { directive } => {
                if !is_valid(&directive) {
                    return Err(EngineError::InvalidDirectiveRegion);
                }
                self.document.directives.push(directive);
                (SemanticPatch::ObjectPlaced, None)
            }
            Command::UpdateDirective { directive } => {
                if !is_valid(&directive) {
                    return Err(EngineError::InvalidDirectiveRegion);
                }
                let id = directive.uuid();
                let Some(slot) = self
                    .document
                    .directives
                    .iter_mut()
                    .find(|d| d.uuid() == id && d.kind() == directive.kind())
                else {
                    return Err(EngineError::DirectiveNotFound {
                        kind: directive.kind().as_str(),
                        id,
                    });
                };
                if *slot == directive {
                    return Ok(CommandResult::unchanged());
                }
                *slot = directive;
                (SemanticPatch::StyleUpdated, Some(id))
            }
            _ => unreachable!("exec_directives routes only the directive commands"),
        };
        let patch_pair = PatchPair {
            semantic,
            document: DocumentPatch::DIRECTIVES,
        };
        match run {
            Some(id) => self.record_field_edit(before, patch_pair, id, DIRECTIVE_RUN_KEY),
            None => self.record_history(before, patch_pair),
        }
        Ok(CommandResult::changed(patch_pair))
    }
}

impl Engine {
    /// The Properties rows of the directive `id`; `None` when the sheet has
    /// no such directive.
    pub(crate) fn directive_info(&self, id: uuid::Uuid) -> Option<Vec<(String, String)>> {
        let directive = self.document.directives.iter().find(|d| d.uuid() == id)?;
        let at = |p: &signex_types::schematic::Point| {
            format!("{:.2}, {:.2}", p.x, self.document.page_y(p.y))
        };
        let mut info = Vec::new();
        match directive {
            SchDirective::Blanket {
                start,
                end,
                net_class,
                erc_exclusions,
                ..
            } => {
                info.push(("Type".into(), "Blanket".into()));
                info.push(("Start".into(), at(start)));
                info.push(("End".into(), at(end)));
                info.push(("Net Class".into(), net_class.clone().unwrap_or_default()));
                info.push(("ERC Exclusions".into(), erc_exclusions.join(", ")));
            }
            SchDirective::CompileMask { start, end, .. } => {
                info.push(("Type".into(), "Compile Mask".into()));
                info.push(("Start".into(), at(start)));
                info.push(("End".into(), at(end)));
            }
            SchDirective::DiffPair { position, .. } => {
                info.push(("Type".into(), "Differential Pair".into()));
                info.push(("Position".into(), at(position)));
            }
        }
        Some(info)
    }
}
