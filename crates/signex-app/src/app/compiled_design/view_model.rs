//! Build what the multi-channel surfaces show from the store, the last
//! compile and the sheet being edited. Runs in update (`refresh_panel_ctx`
//! and the dispatcher); the views only read the result.

use std::path::Path;

use signex_types::schematic::{ChannelRepeat, SelectedKind, check_format, repeated_entry_name};

use super::repeat_edit::default_repeat;
use super::state::{CompiledResult, RepeatDraft};
use crate::app::Signex;
use crate::panels::compiled_design::{
    ChannelStripView, ChannelTabView, CompiledChildRow, CompiledDesignView, CompiledNetRow,
    CompiledPanelView, CompiledRow, CompiledRowKind, CompiledStatus, CompiledTotals,
    NamingMenuView, RepeatField, SheetSymbolRepeatView,
};

impl Signex {
    /// Put a fresh view of the multi-channel surfaces in the panel context.
    pub(crate) fn sync_compiled_design_view(&mut self) {
        *self.document_state.panel_ctx.compiled_design = self.compiled_design_view();
    }

    /// Point the sheet symbol's Repeat rows at the current selection. A
    /// selection change refreshes only this part of the view.
    pub(crate) fn sync_sheet_symbol_repeat_view(&mut self) {
        self.document_state.panel_ctx.compiled_design.sheet_symbol =
            self.sheet_symbol_repeat_view();
    }

    /// The view of the multi-channel surfaces for the active document.
    pub(crate) fn compiled_design_view(&self) -> CompiledDesignView {
        CompiledDesignView {
            panel: self.compiled_panel_view(),
            sheet_symbol: self.sheet_symbol_repeat_view(),
            strip: self.channel_strip_view(),
        }
    }

    fn compiled_panel_view(&self) -> CompiledPanelView {
        let store = &self.document_state.compiled_design;
        let ui = &self.ui_state.compiled_design;
        let mut view = CompiledPanelView {
            mode: ui.mode,
            find: ui.find.clone(),
            ..CompiledPanelView::default()
        };
        let Some(project_id) = self.compile_target() else {
            return view;
        };
        view.sheet_files = self
            .document_state
            .project_by_id(project_id)
            .map_or(0, |p| p.data.sheets.len());
        let Some(result) = store.results.get(&project_id) else {
            view.status = if store.pending.is_some_and(|(p, _)| p == project_id) {
                CompiledStatus::Compiling
            } else {
                CompiledStatus::NotCompiled
            };
            return view;
        };
        let design = self.compiled_design().unwrap_or(result.design.as_ref());
        view.status = CompiledStatus::Ready;
        view.compiled_at = self.ui_state.i18n.time(result.compiled_at);
        view.stale_count = result.stale.len();
        view.totals = CompiledTotals {
            parts: design.part_count,
            nets: design.net_count,
            sheets: design.occurrence_count(),
            channels: design.channel_count,
        };
        let needle = ui.find.trim().to_lowercase();
        view.rows = tree_rows(result, &needle, &ui.folded, &ui.expanded, ui.selected);
        view.nets = design
            .nets
            .iter()
            .filter(|n| needle.is_empty() || n.name.to_lowercase().contains(&needle))
            .map(|n| CompiledNetRow {
                name: n.name.clone(),
                sheets: n.sheets,
                is_power: n.is_power,
                target: n.target.clone(),
            })
            .collect();
        view
    }

    /// The Repeat, Channel naming and Entries rows of the one selected
    /// sheet symbol.
    fn sheet_symbol_repeat_view(&self) -> Option<SheetSymbolRepeatView> {
        let [item] = self.interaction_state.active_canvas().selected.as_slice() else {
            return None;
        };
        if item.kind != SelectedKind::ChildSheet {
            return None;
        }
        let cs = self.edited_sheet_symbol(item.uuid)?;
        let store = &self.ui_state.compiled_design;
        let repeat = ChannelRepeat::parse(&cs.name);
        let shown = repeat.clone().unwrap_or_else(default_repeat);
        let draft = store
            .repeat_draft
            .as_ref()
            .filter(|d| d.sheet_symbol == cs.uuid);
        let field = |f: RepeatField, now: String| {
            draft.and_then(|d| d.fields.get(&f).cloned()).unwrap_or(now)
        };
        let menu = store
            .naming_menu
            .as_ref()
            .map(|m| NamingMenuView {
                target: Some(m.target),
                custom: m.custom.clone(),
                error: (!m.custom.is_empty())
                    .then(|| check_format(&m.custom).err())
                    .flatten(),
            })
            .unwrap_or_default();
        Some(SheetSymbolRepeatView {
            sheet_symbol: cs.uuid,
            file: cs.filename.clone(),
            designator: cs.name.clone(),
            repeat,
            channel_text: field(RepeatField::Channel, shown.channel.clone()),
            from_text: field(RepeatField::From, shown.from.to_string()),
            to_text: field(RepeatField::To, shown.to.to_string()),
            is_from_refused: is_range_field_refused(draft, RepeatField::From),
            is_to_refused: is_range_field_refused(draft, RepeatField::To),
            naming: self.channel_naming(),
            is_naming_locked: self.channel_naming_is_locked(),
            menu,
            entries: cs
                .pins
                .iter()
                .map(|p| match repeated_entry_name(&p.name) {
                    Some(net) => (net.to_string(), true),
                    None => (p.name.trim().to_string(), false),
                })
                .collect(),
            size_mm: cs.size,
            hover: store.illustration_hover.clone(),
        })
    }

    /// The channel strip of the active sheet, when a repeated sheet symbol
    /// places it.
    fn channel_strip_view(&self) -> Option<ChannelStripView> {
        let path = self.document_state.active_path.clone()?;
        let channels = self.sheet_channels(&path);
        if channels.is_empty() {
            return None;
        }
        let result = self
            .compile_target()
            .and_then(|id| self.document_state.compiled_design.results.get(&id));
        let active = self.shown_channel(&path);
        let variant = self.channel_view_variant();
        let tabs = channels
            .iter()
            .map(|channel| {
                let row = result.and_then(|r| {
                    let index = channel_row(r, &path, channel.index)?;
                    Some((&r.design.sheets[index], r.miniatures.get(&index)))
                });
                ChannelTabView {
                    channel: channel.clone(),
                    parts: row.map(|(sheet, _)| sheet.parts),
                    nets: row.map(|(sheet, _)| sheet.nets),
                    miniature: row.and_then(|(_, miniature)| miniature.cloned()),
                    designators: self
                        .designator_map(&path, channel)
                        .into_iter()
                        .map(|(_, _, named)| named)
                        .collect(),
                    fits: result
                        .map(|r| {
                            self.channel_part_fits(r, &path, channel.index, variant.as_deref())
                        })
                        .unwrap_or_default(),
                }
            })
            .collect();
        let designator_map = active
            .and_then(|index| channels.iter().find(|c| c.index == index))
            .map(|channel| self.designator_map(&path, channel))
            .unwrap_or_default();
        Some(ChannelStripView {
            file: file_name(&path),
            sheet: path,
            tabs,
            active,
            designator_map,
            variant,
            is_fit_locked: self.channel_fit_is_locked(),
        })
    }

    /// Every part of the sheet at `path` with its designator in `channel`.
    fn designator_map(
        &self,
        path: &Path,
        channel: &signex_types::schematic::ChannelOccurrence,
    ) -> Vec<(String, String, String)> {
        let Some(engine) = self.document_state.engines.get(path) else {
            return Vec::new();
        };
        let mut rows: Vec<(String, String, String)> = engine
            .document()
            .symbols
            .iter()
            .filter(|s| !s.is_power && !s.reference.trim().is_empty())
            .map(|s| {
                let base = s.reference.trim();
                (
                    base.to_string(),
                    s.value.clone(),
                    self.channel_designator(base, channel),
                )
            })
            .collect();
        rows.sort_by(|a, b| signex_types::designator::compare_references(&a.0, &b.0));
        rows.dedup_by(|a, b| a.0 == b.0);
        rows
    }
}

/// Did Enter refuse the From or To text in `field`? A bad channel name is
/// not the range's fault, so it marks neither.
fn is_range_field_refused(draft: Option<&RepeatDraft>, field: RepeatField) -> bool {
    draft.is_some_and(|d| {
        let channel_reads = d
            .fields
            .get(&RepeatField::Channel)
            .is_none_or(|name| ChannelRepeat::parse(&format!("Repeat({name}, 1, 1)")).is_some());
        d.refused && channel_reads && d.fields.contains_key(&field)
    })
}

/// The parts and nets of `sheet` a search keeps, as rows.
fn child_rows(
    sheet: &signex_net::CompiledSheet,
    needle: &str,
) -> (Vec<CompiledChildRow>, Vec<CompiledChildRow>) {
    let shown = |label: &str| needle.is_empty() || label.to_lowercase().contains(needle);
    let parts = sheet
        .components
        .iter()
        .filter(|p| shown(&p.designator))
        .map(|p| CompiledChildRow {
            label: p.designator.clone(),
            is_power: false,
            target: p.target.clone(),
        })
        .collect();
    let nets = sheet
        .net_names
        .iter()
        .filter(|n| shown(&n.name))
        .map(|n| CompiledChildRow {
            label: n.name.clone(),
            is_power: n.is_power,
            target: n.target.clone(),
        })
        .collect();
    (parts, nets)
}

/// The tree rows the search keeps; channels of a folded repeated row are
/// left out unless a search is on. A sheet instance shows its parts and nets
/// while expanded, and the ones a search finds in it.
fn tree_rows(
    result: &CompiledResult,
    needle: &str,
    folded: &std::collections::HashSet<usize>,
    expanded: &std::collections::HashSet<usize>,
    selected: Option<usize>,
) -> Vec<CompiledRow> {
    let mut rows = Vec::new();
    let mut hidden_below: Option<usize> = None;
    for (index, sheet) in result.design.sheets.iter().enumerate() {
        if let Some(depth) = hidden_below {
            if sheet.depth > depth {
                continue;
            }
            hidden_below = None;
        }
        let caption = match &sheet.repeat {
            Some(repeat) => Some(repeat.to_designator()),
            None if sheet.channel.is_some() => None,
            None => Some(sheet.file.clone()),
        };
        let kind = match (&sheet.repeat, &sheet.channel) {
            (Some(_), _) => CompiledRowKind::Repeated {
                open: !folded.contains(&index),
            },
            (None, Some(_)) => CompiledRowKind::Channel,
            (None, None) => CompiledRowKind::Sheet,
        };
        if needle.is_empty() && matches!(kind, CompiledRowKind::Repeated { open: false }) {
            hidden_below = Some(sheet.depth);
        }
        let finds = |label: &str| label.to_lowercase().contains(needle);
        let child_hit = !needle.is_empty()
            && (sheet.components.iter().any(|p| finds(&p.designator))
                || sheet.net_names.iter().any(|n| finds(&n.name)));
        let matches = needle.is_empty()
            || child_hit
            || sheet.name.to_lowercase().contains(needle)
            || caption
                .as_deref()
                .is_some_and(|c| c.to_lowercase().contains(needle));
        if !matches {
            continue;
        }
        let has_children = !sheet.components.is_empty() || !sheet.net_names.is_empty();
        let is_expanded = child_hit || (needle.is_empty() && expanded.contains(&index));
        let is_expanded = has_children && is_expanded;
        let (part_rows, net_rows) = if is_expanded {
            child_rows(sheet, needle)
        } else {
            (Vec::new(), Vec::new())
        };
        let is_stale = result
            .path_of(&sheet.key)
            .is_some_and(|p| result.stale.contains(p));
        rows.push(CompiledRow {
            index,
            depth: sheet.depth,
            name: sheet.name.clone(),
            caption,
            kind,
            parts: sheet.parts,
            nets: sheet.nets,
            is_stale,
            is_selected: selected == Some(index),
            has_children,
            is_expanded,
            part_rows,
            net_rows,
        });
    }
    rows
}

/// The row index of the first channel row of the sheet at `path` with
/// channel index `index`.
fn channel_row(result: &CompiledResult, path: &Path, index: u32) -> Option<usize> {
    result.design.sheets.iter().position(|s| {
        s.channel.as_ref().is_some_and(|c| c.index == index)
            && result.path_of(&s.key).is_some_and(|p| p == path)
    })
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}
