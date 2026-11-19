//! `Command::PlaceDirective` and `Command::UpdateDirective` (#2420): a
//! Blanket, Compile Mask or Differential Pair directive placed or edited as
//! one undo step. `Command::SetNoErcRules` (#1699) edits a No-ERC
//! directive's rule list the same way.

use crate::*;
use signex_types::schematic::SchDirective;

/// The typing-run key of a directive edit. The run is also keyed by the
/// directive's uuid, which no symbol shares, so it never continues a symbol
/// field run.
const DIRECTIVE_RUN_KEY: &str = "directive";

/// Whether `directive` has finite points and values and, for a region, two
/// corners that span an area.
fn is_valid(directive: &SchDirective) -> bool {
    let finite = |p: &signex_types::schematic::Point| p.x.is_finite() && p.y.is_finite();
    match directive {
        SchDirective::Blanket {
            start,
            end,
            track_width,
            ..
        } => {
            finite(start)
                && finite(end)
                && start.x != end.x
                && start.y != end.y
                && track_width.is_none_or(f64::is_finite)
        }
        SchDirective::CompileMask { start, end, .. } => {
            finite(start) && finite(end) && start.x != end.x && start.y != end.y
        }
        SchDirective::DiffPair { position, .. } => finite(position),
        SchDirective::NetParameters {
            position, rules, ..
        } => finite(position) && rules.is_finite(),
    }
}

impl Engine {
    pub(crate) fn exec_directives(
        &mut self,
        before: SchematicSheet,
        cmd: Command,
    ) -> Result<CommandResult, EngineError> {
        if let Command::SetNoErcRules { id, rules } = cmd {
            return self.exec_set_no_erc_rules(before, id, rules);
        }
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

    /// `Command::SetNoErcRules` (#1699): one undo step per change.
    fn exec_set_no_erc_rules(
        &mut self,
        before: SchematicSheet,
        id: uuid::Uuid,
        rules: Vec<String>,
    ) -> Result<CommandResult, EngineError> {
        let rules = signex_types::schematic::canonical_no_erc_rules(rules);
        let Some(slot) = self
            .document
            .no_erc_directives
            .iter_mut()
            .find(|d| d.uuid == id)
        else {
            return Err(EngineError::DirectiveNotFound { kind: "No ERC", id });
        };
        if slot.rules == rules {
            return Ok(CommandResult::unchanged());
        }
        slot.rules = rules;
        let patch_pair = PatchPair {
            semantic: SemanticPatch::StyleUpdated,
            document: DocumentPatch::NO_ERC_DIRECTIVES,
        };
        self.record_history(before, patch_pair);
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
        let mm = |v: &Option<f64>| v.map(|v| format!("{v:.2}")).unwrap_or_default();
        let mut info = Vec::new();
        match directive {
            SchDirective::Blanket {
                start,
                end,
                net_class,
                erc_exclusions,
                track_width,
                pour,
                include_edge_nets,
                ..
            } => {
                info.push(("Type".into(), "Net Blanket".into()));
                info.push(("Start".into(), at(start)));
                info.push(("End".into(), at(end)));
                info.push(("Net Class".into(), net_class.clone().unwrap_or_default()));
                info.push(("ERC Exclusions".into(), erc_exclusions.join(", ")));
                info.push(("Track Width".into(), mm(track_width)));
                let pour = if pour.is_normal() {
                    "Normal"
                } else {
                    "Keep Short"
                };
                info.push(("Pour".into(), pour.into()));
                info.push(("Include Edge Nets".into(), include_edge_nets.to_string()));
            }
            SchDirective::CompileMask { start, end, .. } => {
                info.push(("Type".into(), "Compile Mask".into()));
                info.push(("Start".into(), at(start)));
                info.push(("End".into(), at(end)));
            }
            SchDirective::DiffPair {
                position, class, ..
            } => {
                info.push(("Type".into(), "Differential Pair".into()));
                info.push(("Position".into(), at(position)));
                info.push(("Class".into(), class.clone().unwrap_or_default()));
            }
            SchDirective::NetParameters {
                position,
                net_class,
                diff_pair_class,
                rules,
                show_on_flag,
                ..
            } => {
                info.push(("Type".into(), "Net Parameters".into()));
                info.push(("Position".into(), at(position)));
                let netlist = signex_net::build_netlist(&self.document);
                let net = signex_net::net_parameters(&self.document, &netlist)
                    .into_iter()
                    .find(|on| on.directive == id)
                    .map(|on| on.net_name)
                    .unwrap_or_default();
                info.push(("Net".into(), net));
                info.push(("Net Class".into(), net_class.clone().unwrap_or_default()));
                info.push((
                    "Diff Pair Class".into(),
                    diff_pair_class.clone().unwrap_or_default(),
                ));
                info.push(("Width".into(), mm(&rules.width_mm)));
                info.push(("Clearance".into(), mm(&rules.clearance_mm)));
                info.push(("Via Diameter".into(), mm(&rules.via_diameter_mm)));
                info.push(("Via Hole".into(), mm(&rules.via_hole_mm)));
                info.push(("Current".into(), mm(&rules.current_a)));
                info.push(("Show On Flag".into(), show_on_flag.to_string()));
            }
        }
        Some(info)
    }
}
