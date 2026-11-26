//! The Project tab's Sheets, Variants, Libraries, ERC and annotation and
//! Version control edits (#1689-#1693). Each is one step on the project's
//! General history; the checks that refuse one live in `signex-types`.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use signex_types::pcb::SymbolLink;
use signex_types::project::{
    AnnotateUndo, BomLineAlternates, ChannelNaming, ListMove, PartRef, PlacedPart, ProjectData,
    RequirementEntry, SheetFit, VariantProblem, VariantSettings,
};

/// One edit of a Project tab section. Plain data, so a script can emit it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "edit", rename_all = "snake_case")]
pub enum ProjectSectionCommand {
    /// Move a sheet one page up or down (#1689).
    MoveSheet {
        filename: String,
        step: ListMove,
    },
    /// Drop a sheet at page index `to` (mockup row 31): one step, however
    /// far it moved.
    PlaceSheet {
        filename: String,
        to: usize,
    },
    /// Add a variant after the others (#1690).
    AddVariant {
        name: String,
    },
    RenameVariant {
        from: String,
        to: String,
    },
    RemoveVariant {
        name: String,
    },
    /// `None` makes Base the active variant.
    SetActiveVariant {
        name: Option<String>,
    },
    /// Add `name` as a copy of `from` (mockup row 32). `sheet_fits` is each
    /// part's fit in `from` as its sheets say, where it differs from Base.
    DuplicateVariant {
        from: String,
        name: String,
        #[serde(default)]
        sheet_fits: Vec<SheetFit>,
    },
    /// `None` makes Base the default variant (mockup row 32).
    SetDefaultVariant {
        name: Option<String>,
    },
    /// A part's fit in a variant; `None` follows the sheets (mockup row 32).
    /// `part` is the placed part, which keeps the setting on it through a
    /// re-annotation.
    SetVariantPartFit {
        variant: String,
        designator: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        part: Option<SymbolLink>,
        fitted: Option<bool>,
    },
    /// The fit of several parts in a variant as one step: every channel's
    /// copy of a repeated part, or every part of one channel (#2662).
    SetVariantPartsFit {
        variant: String,
        parts: Vec<PartRef>,
        fitted: Option<bool>,
    },
    /// A part's parameter value in a variant; `None` is its own value again
    /// (mockup row 32).
    SetVariantPartParameter {
        variant: String,
        designator: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        part: Option<SymbolLink>,
        parameter: String,
        value: Option<String>,
    },
    /// Move a library one step up or down the list (#1691).
    MoveLibrary {
        path: PathBuf,
        step: ListMove,
    },
    /// Drop a library at list index `to` (mockup row 33).
    PlaceLibrary {
        path: PathBuf,
        to: usize,
    },
    /// The project's annotate undo choice; `None` follows Preferences
    /// (#1692). Clearing it over an unreadable section changes nothing.
    SetAnnotateUndo {
        choice: Option<AnnotateUndo>,
    },
    /// Commit every save to the project's repository (#1693).
    SetVersionControl {
        on: bool,
    },
    /// How channels of a repeated sheet name designators and local nets
    /// (#1279).
    SetChannelNaming {
        naming: ChannelNaming,
    },
    /// Replace the project's signal harness definitions (#1280), as the
    /// harness definitions view edits them.
    SetHarnessDefinitions {
        definitions: Vec<signex_types::schematic::HarnessDefinition>,
    },
}

impl ProjectSectionCommand {
    /// Apply to `data`. A refused edit changes nothing; a move at the end of
    /// its list is not refused, it just changes nothing.
    pub fn apply(&self, data: &mut ProjectData) -> Result<(), VariantProblem> {
        match self {
            Self::MoveSheet { filename, step } => {
                data.move_sheet(filename, *step);
            }
            Self::PlaceSheet { filename, to } => {
                data.place_sheet(filename, *to);
            }
            Self::AddVariant { name } => data.add_variant(name)?,
            Self::RenameVariant { from, to } => data.rename_variant(from, to)?,
            Self::RemoveVariant { name } => data.remove_variant(name)?,
            Self::SetActiveVariant { name } => data.set_active_variant(name.as_deref())?,
            Self::DuplicateVariant {
                from,
                name,
                sheet_fits,
            } => data.duplicate_variant(from, name, sheet_fits)?,
            Self::SetDefaultVariant { name } => data.set_default_variant(name.as_deref())?,
            Self::SetVariantPartFit {
                variant,
                designator,
                part,
                fitted,
            } => data.set_variant_part_fit(
                variant,
                PlacedPart::new(designator, part.as_ref()),
                *fitted,
            )?,
            Self::SetVariantPartsFit {
                variant,
                parts,
                fitted,
            } => data.set_variant_parts_fit(variant, parts, *fitted)?,
            Self::SetVariantPartParameter {
                variant,
                designator,
                part,
                parameter,
                value,
            } => data.set_variant_part_parameter(
                variant,
                PlacedPart::new(designator, part.as_ref()),
                parameter,
                value.as_deref(),
            )?,
            Self::MoveLibrary { path, step } => {
                data.move_library(path, *step);
            }
            Self::PlaceLibrary { path, to } => {
                data.place_library(path, *to);
            }
            Self::SetAnnotateUndo { choice } => {
                if !(choice.is_none() && data.has_unreadable_annotation()) {
                    data.set_annotate_undo(*choice);
                }
            }
            Self::SetVersionControl { on } => data.enable_git = *on,
            // An unreadable section is never replaced (DECISIONS 2026-11-23).
            Self::SetChannelNaming { naming } => {
                if !data.has_unreadable_schematic_design() {
                    data.schematic_design.channel_naming = naming.clone();
                }
            }
            Self::SetHarnessDefinitions { definitions } => {
                if !data.has_unreadable_schematic_design() {
                    data.schematic_design
                        .harness_definitions
                        .clone_from(definitions);
                }
            }
        }
        Ok(())
    }
}

/// The section fields one undo step holds. Sheets and libraries are held as
/// an order only: their lists have other writers (see
/// `signex_types::project::ListMove`'s module).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SectionsSnapshot {
    sheet_order: Vec<String>,
    library_order: Vec<PathBuf>,
    variants: Vec<String>,
    active_variant: Option<String>,
    /// The default variant and what each variant changes on its parts.
    variant_settings: VariantSettings,
    enable_git: bool,
    /// The Requirements section (#2339): the whole list; only it writes it.
    requirements: Vec<RequirementEntry>,
    channel_naming: ChannelNaming,
    /// The harness definitions (#1280); only the definitions view writes them.
    harness_definitions: Vec<signex_types::schematic::HarnessDefinition>,
    /// The project's rails as stored (#2943); only `project_set_rail`
    /// writes them.
    rails: Option<serde_json::Value>,
    /// The BOM lines' alternates (#2938); only the `bom_*` edits write them.
    bom_alternates: Vec<BomLineAlternates>,
}

impl SectionsSnapshot {
    pub(super) fn of(data: &ProjectData) -> Self {
        Self {
            sheet_order: data.sheet_order(),
            library_order: data.library_order(),
            variants: data.variant_definitions.clone(),
            active_variant: data.active_variant.clone(),
            variant_settings: data.variant_settings.clone(),
            enable_git: data.enable_git,
            requirements: data.requirements.clone(),
            channel_naming: data.schematic_design.channel_naming.clone(),
            harness_definitions: data.schematic_design.harness_definitions.clone(),
            rails: data.rails_value(),
            bom_alternates: data.bom_alternates.clone(),
        }
    }

    pub(super) fn restore(self, data: &mut ProjectData) {
        data.restore_sheet_order(&self.sheet_order);
        data.restore_library_order(&self.library_order);
        data.variant_definitions = self.variants;
        data.active_variant = self.active_variant;
        data.variant_settings = self.variant_settings;
        data.enable_git = self.enable_git;
        data.requirements = self.requirements;
        data.schematic_design.channel_naming = self.channel_naming;
        data.schematic_design.harness_definitions = self.harness_definitions;
        data.restore_rails_value(self.rails);
        data.bom_alternates = self.bom_alternates;
    }
}
