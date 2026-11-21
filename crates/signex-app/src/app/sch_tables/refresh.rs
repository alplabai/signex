//! Bound tables follow the design after a compile, never on every edit, so
//! a table does not change under the cursor (#1300). One undo step per
//! sheet that changed; nothing at all when no table changed.

use std::path::PathBuf;

use signex_types::schematic::{SchDrawing, SchTable, SchematicSheet, TableRefresh, TableSource};

use super::rows::{ContentsEntry, contents_table_rows, pin_table_rows};
use crate::app::Signex;
use crate::app::mutation_gateway::MutationOutcome;
use crate::app::state::ProjectId;

/// The pages a contents table lists, from the last compile.
pub(crate) type Contents = Vec<ContentsEntry>;

/// `table` rebuilt from its source on `sheet`; `None` when it is typed,
/// switched off, or its source is gone (the rows stay as they were).
pub(crate) fn rebuilt_table(
    sheet: &SchematicSheet,
    table: &SchTable,
    contents: &dyn Fn(bool) -> Option<Contents>,
) -> Option<SchTable> {
    if table.refresh == TableRefresh::Off {
        return None;
    }
    let mut next = table.clone();
    match &table.source {
        TableSource::Typed => return None,
        TableSource::ComponentPins { designator, sort } => {
            next.rows = pin_table_rows(sheet, designator, *sort, &table.columns)?;
            next.links.clear();
        }
        TableSource::ProjectSheets { per_channel } => {
            let entries = contents(*per_channel)?;
            let (rows, links) = contents_table_rows(&entries, &table.columns);
            next.rows = rows;
            next.links = links;
        }
    }
    (next != *table).then_some(next)
}

/// The `UpdateSchDrawing` commands that bring every bound table of `sheet`
/// up to date.
pub(crate) fn refresh_commands(
    sheet: &SchematicSheet,
    contents: &dyn Fn(bool) -> Option<Contents>,
) -> Vec<signex_engine::Command> {
    sheet
        .drawings
        .iter()
        .filter_map(|d| match d {
            SchDrawing::Table(t) => rebuilt_table(sheet, t, contents),
            _ => None,
        })
        .map(|t| signex_engine::Command::UpdateSchDrawing {
            drawing: SchDrawing::Table(t),
        })
        .collect()
}

impl Signex {
    /// Rebuild the rows and links of every bound table (refresh On compile)
    /// of the open sheets of `project` (`None`: the sheets outside any
    /// project), as one undo step per changed sheet. Contents tables read
    /// that project's own last compile. Called after each successful compile
    /// with the compiled project, so another project's sheets are never
    /// touched.
    ///
    /// Each sheet goes through the normal edit path (the mutation gateway on
    /// that sheet as the edit target), so a read-only sheet and a sheet
    /// showing a channel tab (a compiled view) refuse the refresh and say so
    /// in Messages. See `docs/MULTI_CHANNEL.md`, "Bound tables".
    pub(crate) fn refresh_bound_tables(&mut self, project: Option<ProjectId>) {
        let contents = project.and_then(|id| self.compiled_contents_of(id));
        let pages = |per_channel: bool| contents.as_ref().map(|c| c.pages(per_channel));
        let mut paths: Vec<PathBuf> = self
            .document_state
            .engines
            .keys()
            .filter(|path| self.document_state.document_project(path).map(|p| p.id) == project)
            .cloned()
            .collect();
        paths.sort();
        let edits: Vec<(PathBuf, Vec<signex_engine::Command>)> = paths
            .into_iter()
            .filter_map(|path| {
                let engine = self.document_state.engines.get(&path)?;
                let commands = refresh_commands(engine.document(), &pages);
                (!commands.is_empty()).then_some((path, commands))
            })
            .collect();
        // The tables are the compile's own output: refreshing them does not
        // mark the compile they follow out of date.
        let stale = project.and_then(|id| {
            let result = self.document_state.compiled_design.results.get(&id)?;
            Some((id, result.stale.clone()))
        });
        for (path, commands) in edits {
            let (outcome, reason) = self.on_edit_target(&path, |app| {
                app.apply_engine_commands_reporting(commands, true, false)
            });
            match outcome {
                MutationOutcome::Changed => self.refresh_selection_on(&path),
                MutationOutcome::Unchanged => {}
                MutationOutcome::Failed => crate::diagnostics::log_warning(
                    signex_i18n::t!(
                        self.ui_state.i18n,
                        signex_i18n::ids::TABLE_REFRESH_FAILED,
                        "file" => path.display().to_string(),
                        "reason" => reason.unwrap_or_else(|| signex_i18n::t!(
                            self.ui_state.i18n,
                            signex_i18n::ids::TABLE_REFRESH_NO_REASON
                        )
                        .into_string())
                    )
                    .into_string(),
                ),
            }
        }
        if let Some((id, stale)) = stale
            && let Some(result) = self.document_state.compiled_design.results.get_mut(&id)
        {
            result.stale = stale;
        }
    }

    /// `project`'s pages as its last compile walked them, with the title
    /// block of each sheet that is open (found by the path the compile read
    /// it from, never by bare file name); `None` before a compile.
    pub(super) fn compiled_contents_of(&self, project: ProjectId) -> Option<CompiledPages> {
        let result = self.document_state.compiled_design.results.get(&project)?;
        let design = result.design.as_ref();
        let mut sheets = Vec::with_capacity(design.sheets.len());
        for s in &design.sheets {
            let title_block = result
                .path_of(&s.key)
                .and_then(|path| self.document_state.engines.get(path))
                .map(|engine| engine.document().title_block.clone())
                .unwrap_or_default();
            let field = |key: &str| title_block.get(key).cloned().unwrap_or_default();
            sheets.push(CompiledPage {
                name: s.name.clone(),
                file: s.file.clone(),
                is_channel: s.channel.is_some(),
                is_repeat: s.repeat.is_some(),
                title: field("title"),
                revision: field("revision"),
                date: field("date"),
            });
        }
        Some(CompiledPages { sheets })
    }
}

/// One row of the compiled hierarchy, as a contents table reads it.
#[derive(Debug, Clone)]
pub(crate) struct CompiledPage {
    pub name: String,
    pub file: String,
    pub is_channel: bool,
    pub is_repeat: bool,
    pub title: String,
    pub revision: String,
    pub date: String,
}

/// The compiled hierarchy, depth-first.
#[derive(Debug, Clone, Default)]
pub(crate) struct CompiledPages {
    pub sheets: Vec<CompiledPage>,
}

impl CompiledPages {
    /// The contents entries: one per channel when `per_channel` (a repeated
    /// placement's own row left out), else one per placement (its channels
    /// left out). Pages are numbered in hierarchy order.
    pub(crate) fn pages(&self, per_channel: bool) -> Contents {
        self.sheets
            .iter()
            .filter(|s| {
                if per_channel {
                    !s.is_repeat
                } else {
                    !s.is_channel
                }
            })
            .enumerate()
            .map(|(i, s)| ContentsEntry {
                page: i + 1,
                sheet: if s.is_channel {
                    format!("{} · {}", file_stem(&s.file), s.name)
                } else {
                    s.name.clone()
                },
                title: s.title.clone(),
                revision: s.revision.clone(),
                last_changed: s.date.clone(),
                file: s.file.clone(),
            })
            .collect()
    }
}

fn file_stem(file: &str) -> String {
    std::path::Path::new(file)
        .file_stem()
        .map_or_else(|| file.to_string(), |s| s.to_string_lossy().into_owned())
}
