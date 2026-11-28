//! Opening the Alternate part picker and the Edit value popover from the
//! canvas's In variant flyout (mockup rows 208 and 209), and the library
//! read behind the picker's candidates, which runs off the update thread.

use std::sync::LazyLock;

use iced::Task;
use signex_types::pcb::SymbolLink;
use signex_types::project::{PartRef, is_base_variant};
use signex_types::schematic::{InstancePath, SchematicSheet, Symbol, normalized_lib_id};

use super::VariantPartEditMsg;
use super::candidates::{CandidateQuery, read_candidates};
use super::state::{
    AlternatePicker, CandidateRead, EditedPart, PartFacts, ValuePopover, VariantEditKind,
    VariantPartEdit,
};
use crate::app::state::ProjectId;
use crate::app::{Message, Signex};

/// The popover's value field, focused when it opens.
pub static VARIANT_VALUE_INPUT_ID: LazyLock<iced::widget::Id> =
    LazyLock::new(|| iced::widget::Id::new("signex.variant_value"));

/// A part the Variant Manager names, for the picker: its project, the
/// cell's variant, the part (a channel's copy on a repeated sheet), and for a
/// copy its channel, the first and last channel and every channel's copy.
#[derive(Debug, Clone)]
pub(crate) struct ManagedPart {
    pub project: ProjectId,
    pub variant: String,
    pub this_part: PartRef,
    pub channel: Option<String>,
    pub channels: Option<(String, String)>,
    pub every_channel: Vec<PartRef>,
}

impl Signex {
    /// In variant ▸ Alternate… or Edit value for this variant…, picked in
    /// the menu opened at `anchor` in `window` (`None`: the main window).
    /// Nothing opens without one placed part selected in a variant.
    pub(crate) fn open_variant_part_edit(
        &mut self,
        kind: VariantEditKind,
        anchor: (f32, f32),
        window: Option<iced::window::Id>,
    ) -> Task<Message> {
        let Some(part) = self.edited_part(anchor, window) else {
            return Task::none();
        };
        match kind {
            VariantEditKind::Alternate => {
                let picker = AlternatePicker::new(part);
                self.ui_state.variant_part_edit =
                    Some(Box::new(VariantPartEdit::Alternate(Box::new(picker))));
                self.read_variant_candidates()
            }
            VariantEditKind::Value => {
                let Some(project) = self.document_state.project_by_id(part.project) else {
                    return Task::none();
                };
                let popover = ValuePopover::new(part, &project.data.variant_settings);
                self.ui_state.variant_part_edit =
                    Some(Box::new(VariantPartEdit::Value(Box::new(popover))));
                iced::widget::operation::focus(VARIANT_VALUE_INPUT_ID.clone())
            }
        }
    }

    /// Alternate… of a Variant Manager cell (mockup rows 32, 208): the
    /// picker for `part`, read from the open sheet that places its first
    /// unit. Nothing opens in Base or when no open sheet places it.
    pub(crate) fn open_alternate_for(&mut self, part: ManagedPart) -> Task<Message> {
        let Some(part) = self.managed_edited_part(part) else {
            return Task::none();
        };
        let picker = AlternatePicker::new(part);
        self.ui_state.variant_part_edit =
            Some(Box::new(VariantPartEdit::Alternate(Box::new(picker))));
        self.read_variant_candidates()
    }

    /// `part` with the sheet that places it: its first placed unit on the
    /// project's open sheets (a channel's copy names its own placement).
    fn managed_edited_part(&self, part: ManagedPart) -> Option<EditedPart> {
        if is_base_variant(&part.variant) {
            return None;
        }
        let link = part.this_part.part.as_ref();
        let designator = part.this_part.designator.as_str();
        let mut paths: Vec<&std::path::PathBuf> = self
            .document_state
            .engines
            .keys()
            .filter(|path| {
                self.document_state
                    .document_project(path)
                    .is_some_and(|project| project.id == part.project)
            })
            .collect();
        paths.sort();
        let (path, sheet, symbol) = paths.into_iter().find_map(|path| {
            let sheet = self.document_state.engines.get(path)?.document();
            let symbol = sheet.symbols.iter().find(|symbol| match link {
                Some(link) => link.units.contains(&symbol.uuid),
                None => !symbol.is_power && symbol.reference == designator,
            })?;
            Some((path, sheet, symbol))
        })?;
        let placement = link.map_or_else(InstancePath::root, |link| link.sheet.clone());
        Some(EditedPart {
            project: part.project,
            sheet: file_name(path),
            path: path.clone(),
            sheet_fitted: sheets_fit(sheet, symbol, &placement, &part.variant),
            variant: part.variant,
            this_part: part.this_part,
            channel: part.channel,
            channels: part.channels,
            every_channel: part.every_channel,
            base: PartFacts::of(sheet, symbol),
            row: symbol.library_id.zip(symbol.row_id),
            anchor: (0.0, 0.0),
            window: None,
        })
    }

    /// The part selected in `window` and where an edit of it applies; `None`
    /// in Base, off a project, before a compile on a repeated sheet, or
    /// without one placed part selected.
    fn edited_part(
        &self,
        anchor: (f32, f32),
        window: Option<iced::window::Id>,
    ) -> Option<EditedPart> {
        let window_id = window.or(self.ui_state.main_window_id)?;
        let path = self
            .document_state
            .sheet_path_for_window(window_id, &self.ui_state)?;
        let project = self.document_state.document_project(path)?;
        let variant = project.data.active_variant_name();
        if is_base_variant(variant) {
            return None;
        }
        let engine = self
            .document_state
            .engine_for_window(window_id, &self.ui_state)?;
        let canvas = self.interaction_state.canvas_for_window(window_id);
        let symbol = crate::app::handlers::placed_part::selected_placed_symbol(engine, canvas)?;
        let sheet = engine.document();
        let placement = self
            .shown_placement(path)
            .unwrap_or_else(InstancePath::root);
        let (this_part, channel, channels, every_channel) =
            match self.canvas_menu_channel_copies(window_id) {
                Some(copies) => {
                    let first = copies.all.first()?.clone();
                    let strip = self.document_view.panel_ctx.compiled_design.strip.as_ref();
                    let channels = strip.and_then(|strip| {
                        let first = strip.tabs.first()?.channel.name.clone();
                        let last = strip.tabs.last()?.channel.name.clone();
                        Some((first, last))
                    });
                    match copies.shown {
                        Some((channel, shown)) => (shown, Some(channel), channels, copies.all),
                        None => (first, None, channels, copies.all),
                    }
                }
                None => {
                    let designator = symbol.designator_in(&placement).trim().to_string();
                    let lib_id = normalized_lib_id(&symbol.lib_id);
                    let units = sheet
                        .symbols
                        .iter()
                        .filter(|unit| {
                            unit.designator_in(&placement).trim() == designator
                                && normalized_lib_id(&unit.lib_id) == lib_id
                        })
                        .map(|unit| unit.uuid);
                    let link = SymbolLink::new(placement.clone(), units);
                    (PartRef::new(designator, Some(link)), None, None, Vec::new())
                }
            };
        Some(EditedPart {
            project: project.id,
            variant: variant.to_string(),
            sheet: file_name(path),
            path: path.clone(),
            sheet_fitted: sheets_fit(sheet, symbol, &placement, variant),
            this_part,
            channel,
            channels,
            every_channel,
            base: PartFacts::of(sheet, symbol),
            row: symbol.library_id.zip(symbol.row_id),
            anchor,
            window,
        })
    }

    /// Read the picker's candidates for its query off the update thread; a
    /// read started before is dropped when it lands.
    pub(super) fn read_variant_candidates(&mut self) -> Task<Message> {
        let Some(VariantPartEdit::Alternate(picker)) = self.ui_state.variant_part_edit.as_deref()
        else {
            return Task::none();
        };
        let query = self.candidate_query(picker);
        let Some(VariantPartEdit::Alternate(picker)) =
            self.ui_state.variant_part_edit.as_deref_mut()
        else {
            return Task::none();
        };
        picker.generation += 1;
        picker.read = CandidateRead::Reading;
        let generation = picker.generation;
        let part = picker.part.this_part.designator.clone();
        let asked = query.query.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || read_candidates(&query))
                    .await
                    .map_err(|e| e.to_string())
            },
            move |read| {
                Message::from(VariantPartEditMsg::CandidatesRead {
                    generation,
                    part: part.clone(),
                    query: asked.clone(),
                    read: read.map(Box::new),
                })
            },
        )
    }

    /// What `picker`'s read looks for: the part's row, its own part number,
    /// the query and the libraries, as paths.
    fn candidate_query(&self, picker: &AlternatePicker) -> CandidateQuery {
        let project_libraries = self
            .document_state
            .project_by_id(picker.part.project)
            .map(|project| {
                project
                    .data
                    .libraries
                    .iter()
                    .map(|entry| {
                        let resolved = project.data.resolve_library_path(entry);
                        self.library
                            .library_at(&resolved)
                            .map_or(resolved, |lib| lib.root.clone())
                    })
                    .collect()
            })
            .unwrap_or_default();
        // The part's wiring is read once, with the picker's first read.
        let sheet = picker
            .wired
            .is_none()
            .then(|| self.document_state.engines.get(&picker.part.path))
            .flatten()
            .map(|engine| std::sync::Arc::new(engine.document().clone()));
        let units = picker
            .part
            .this_part
            .part
            .as_ref()
            .map(|link| link.units.clone())
            .unwrap_or_default();
        CandidateQuery {
            row: picker.part.row,
            base_mpn: picker.part.base.mpn.clone(),
            query: picker.query.trim().to_string(),
            project_libraries,
            open_libraries: self
                .library
                .open_libraries
                .iter()
                .map(|lib| (lib.library_id, lib.root.clone()))
                .collect(),
            sheet,
            units,
        }
    }
}

/// A sheet's file name, for the picker's heading.
pub(super) fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Whether the sheets fit the part `symbol` belongs to in `variant`: its
/// units on `sheet` at `placement`, as the BOM decides before the variant's
/// own fit.
pub(super) fn sheets_fit(
    sheet: &SchematicSheet,
    symbol: &Symbol,
    placement: &InstancePath,
    variant: &str,
) -> bool {
    let designator = symbol.designator_in(placement).trim();
    let lib_id = normalized_lib_id(&symbol.lib_id);
    let units: Vec<&Symbol> = sheet
        .symbols
        .iter()
        .filter(|unit| {
            unit.designator_in(placement).trim() == designator
                && normalized_lib_id(&unit.lib_id) == lib_id
        })
        .collect();
    signex_output::part_is_fitted(&units, Some(variant))
}
