//! The channel cards' variant state (#2662): each part of a channel as the
//! last compile lists it, with its fit in the active variant, and a card's
//! click that leaves the whole channel out of that variant (or lets the
//! sheets decide again). The fit is the BOM's (`signex_output::
//! not_fitted_symbols` over the channel's copy of the sheet) and so is the
//! part link (every unit at the channel's placement); the edit is one undo
//! step of the project's General history. See `docs/MULTI_CHANNEL.md`,
//! "Per-channel variant state".

use std::collections::{HashMap, HashSet};
use std::path::Path;

use signex_engine::ProjectSectionCommand;
use signex_net::CompiledPart;
use signex_types::pcb::SymbolLink;
use signex_types::project::{PartRef, VariantSettings, is_base_variant};
use signex_types::schematic::{InstancePath, SchematicSheet};

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
    /// compile. A sheet that is not open lists its parts by the settings
    /// alone.
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
        let sheets_only = VariantSettings::default();
        let settings = self
            .compile_target()
            .and_then(|id| self.document_state.project_by_id(id))
            .map_or(&sheets_only, |project| &project.data.variant_settings);
        let Some(engine) = self.document_state.engines.get(path) else {
            return sheet
                .components
                .iter()
                .map(|component| {
                    let target = &component.target;
                    let link = SymbolLink::new(target.occurrence.clone(), [target.id]);
                    let part = PartRef::new(component.designator.clone(), Some(link));
                    let is_fitted =
                        signex_output::placed_part_fitted(settings, variant, part.placed(), &[]);
                    ChannelPartFit { part, is_fitted }
                })
                .collect();
        };
        let mut copies: HashMap<&InstancePath, ChannelCopy> = HashMap::new();
        let mut out = Vec::new();
        for component in &sheet.components {
            let occurrence = &component.target.occurrence;
            let copy = copies.entry(occurrence).or_insert_with(|| {
                let parts = &sheet.components;
                ChannelCopy::new(engine.document(), occurrence, parts, variant, settings)
            });
            out.extend(copy.fit_of(component));
        }
        out
    }

    /// Whether `variant` leaves out every part of the channel at row `index`
    /// of the last compile, as its card shows it greyed: the Compiled
    /// Design's "Not fitted (<variant>)" tag (#2662). A sheet row, or a
    /// channel with no parts, is never left out.
    pub(super) fn channel_row_left_out(
        &self,
        result: &CompiledResult,
        index: usize,
        variant: &str,
    ) -> bool {
        let Some(sheet) = result.design.sheets.get(index) else {
            return false;
        };
        let (Some(channel), Some(path)) = (&sheet.channel, result.path_of(&sheet.key)) else {
            return false;
        };
        let fits = self.channel_part_fits(result, path, channel.index, Some(variant));
        !fits.is_empty() && fits.iter().all(|fit| !fit.is_fitted)
    }

    /// A channel card's fit was clicked: leave every part of the channel
    /// out of the active variant, or let the sheets decide for each again
    /// (`fitted` `None`). One undo step; Base is not set here. The canvas
    /// menu's In variant rows set a part's copies through it too.
    pub(crate) fn pick_channel_fit(&mut self, parts: Vec<PartRef>, fitted: Option<bool>) {
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
        if let Err(problem) = self.apply_project_section(project_id, &command) {
            crate::diagnostics::log_warning(
                signex_i18n::t!(
                    self.ui_state.i18n,
                    signex_i18n::ids::PANELS_COMPILED_DESIGN_FIT_REFUSED,
                    "why" => problem.to_string()
                )
                .into_string(),
            );
        }
    }
}

/// One channel's copy of a sheet as the BOM reads it: every unit of a part
/// under the channel designator the compile gives it at `occurrence`, and
/// which symbols are not fitted there.
struct ChannelCopy {
    occurrence: InstancePath,
    sheet: SchematicSheet,
    not_fitted: HashSet<uuid::Uuid>,
}

impl ChannelCopy {
    /// `document`'s symbols at `occurrence`, named by `parts` (one unit of
    /// each part, as the compile lists them), decided in `variant`. A
    /// throwaway copy: nothing of it reaches a file.
    fn new(
        document: &SchematicSheet,
        occurrence: &InstancePath,
        parts: &[CompiledPart],
        variant: Option<&str>,
        settings: &VariantSettings,
    ) -> Self {
        let named: HashMap<String, &str> = parts
            .iter()
            .filter(|part| part.target.occurrence == *occurrence)
            .filter_map(|part| {
                let unit = document.symbols.iter().find(|s| s.uuid == part.target.id)?;
                let base = unit.designator_in(occurrence).trim().to_string();
                Some((base, part.designator.as_str()))
            })
            .collect();
        // Only the symbols decide a fit; the rest of the sheet is not copied.
        let mut sheet = SchematicSheet {
            symbols: document.symbols.clone(),
            ..SchematicSheet::default()
        };
        for symbol in &mut sheet.symbols {
            let base = symbol.designator_in(occurrence).trim().to_string();
            if let Some(name) = named.get(&base) {
                symbol.set_instance_designator(occurrence, (*name).to_string());
            }
        }
        let not_fitted = signex_output::not_fitted_symbols(&sheet, occurrence, variant, settings);
        Self {
            occurrence: occurrence.clone(),
            sheet,
            not_fitted,
        }
    }

    /// The card row of `component`, linked by every unit of its part at the
    /// channel as the BOM links it; `None` for a power port, not a part.
    fn fit_of(&self, component: &CompiledPart) -> Option<ChannelPartFit> {
        let id = component.target.id;
        let own = self.sheet.symbols.iter().find(|s| s.uuid == id);
        if own.is_some_and(|s| !signex_output::bom::is_part_symbol(s)) {
            return None;
        }
        let designator = component.designator.trim();
        let mut units: Vec<uuid::Uuid> = self
            .sheet
            .symbols
            .iter()
            .filter(|s| signex_output::bom::is_part_symbol(s))
            .filter(|s| s.designator_in(&self.occurrence).trim() == designator)
            .map(|s| s.uuid)
            .collect();
        if !units.contains(&id) {
            units.push(id);
        }
        let link = SymbolLink::new(self.occurrence.clone(), units);
        Some(ChannelPartFit {
            part: PartRef::new(component.designator.clone(), Some(link)),
            is_fitted: !self.not_fitted.contains(&id),
        })
    }
}
