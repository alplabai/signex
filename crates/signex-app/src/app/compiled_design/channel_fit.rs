//! The channel cards' variant state (#2662): each part of a channel as the
//! last compile lists it, with its fit in the active variant, and a card's
//! click that leaves the whole channel out of that variant (or lets the
//! sheets decide again). The fit is the BOM's (`placed_part_fitted`); the
//! edit is one undo step of the project's General history. See
//! `docs/MULTI_CHANNEL.md`, "Per-channel variant state".

use std::path::Path;

use signex_engine::ProjectSectionCommand;
use signex_types::pcb::SymbolLink;
use signex_types::project::{PartRef, is_base_variant};
use signex_types::schematic::Symbol;

use super::state::CompiledResult;
use crate::app::Signex;
use crate::panels::compiled_design::ChannelPartFit;

impl Signex {
    /// The active variant of the compile target's project, as defined;
    /// `None` is Base.
    pub(super) fn channel_view_variant(&self) -> Option<String> {
        let project = self.document_state.project_by_id(self.compile_target()?)?;
        project
            .data
            .active_variant
            .clone()
            .filter(|name| !is_base_variant(name))
    }

    /// Whether the compile target's variant settings could not be read:
    /// nothing in them can change.
    pub(super) fn channel_fit_is_locked(&self) -> bool {
        self.compile_target()
            .and_then(|id| self.document_state.project_by_id(id))
            .is_none_or(|project| project.data.has_unreadable_variant_settings())
    }

    /// Every part of channel `index` of the sheet at `path` in the last
    /// compile, with its fit in `variant` (`None` is Base). Empty before a
    /// compile.
    pub(super) fn channel_part_fits(
        &self,
        result: &CompiledResult,
        path: &Path,
        index: u32,
        variant: Option<&str>,
    ) -> Vec<ChannelPartFit> {
        let Some(sheet) = result
            .design
            .sheets
            .iter()
            .filter(|s| s.channel.as_ref().is_some_and(|c| c.index == index))
            .find(|s| result.path_of(&s.key).is_some_and(|p| p == path))
        else {
            return Vec::new();
        };
        let settings = self
            .compile_target()
            .and_then(|id| self.document_state.project_by_id(id))
            .map(|project| &project.data.variant_settings);
        let symbols = self
            .document_state
            .engines
            .get(path)
            .map(|engine| engine.document().symbols.as_slice())
            .unwrap_or_default();
        sheet
            .components
            .iter()
            .map(|component| {
                let target = &component.target;
                let link = SymbolLink::new(target.occurrence.clone(), [target.id]);
                let part = PartRef::new(component.designator.clone(), Some(link));
                let units = units_of(symbols, target.id);
                let is_fitted = match settings {
                    Some(settings) => {
                        signex_output::placed_part_fitted(settings, variant, part.placed(), &units)
                    }
                    None => signex_output::part_is_fitted(&units, variant),
                };
                ChannelPartFit { part, is_fitted }
            })
            .collect()
    }

    /// A channel card's fit was clicked: leave every part of the channel
    /// out of the active variant, or let the sheets decide for each again
    /// (`fitted` `None`). One undo step; Base is not set here.
    pub(super) fn pick_channel_fit(&mut self, parts: Vec<PartRef>, fitted: Option<bool>) {
        let (Some(project_id), Some(variant)) =
            (self.compile_target(), self.channel_view_variant())
        else {
            return;
        };
        if parts.is_empty() {
            return;
        }
        let command = ProjectSectionCommand::SetVariantPartsFit {
            variant,
            parts,
            fitted,
        };
        if let Err(refused) = self.apply_project_section(project_id, &command) {
            crate::diagnostics::log_warning(format!(
                "Channel fit: the edit was refused ({refused:?}); nothing changed"
            ));
        }
    }
}

/// The units of the part one of whose units is `unit` on its sheet: the
/// symbols sharing its designator there. Empty when the sheet is not open.
fn units_of(symbols: &[Symbol], unit: uuid::Uuid) -> Vec<&Symbol> {
    let Some(own) = symbols.iter().find(|s| s.uuid == unit) else {
        return Vec::new();
    };
    let reference = own.reference.trim();
    if reference.is_empty() {
        return vec![own];
    }
    symbols
        .iter()
        .filter(|s| s.reference.trim() == reference)
        .collect()
}
