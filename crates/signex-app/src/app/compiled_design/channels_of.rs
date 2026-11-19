//! Which channels a sheet has, the channel tabs, opening a compiled row and
//! the read-only gate of a channel tab. A sheet's channels come from the
//! repeated sheet symbols that place it: no compile is needed for them.

use std::path::{Path, PathBuf};

use iced::Task;
use signex_types::schematic::{ChannelOccurrence, ChannelRepeat};

use crate::app::handlers::canvas::SheetOpen;
use crate::app::{Message, Signex};

impl Signex {
    /// The last compile of the active project, if it was compiled.
    pub(crate) fn compiled_design(&self) -> Option<&signex_net::CompiledDesign> {
        let project = self.compile_target()?;
        let result = self.document_state.compiled_design.results.get(&project)?;
        Some(result.design.as_ref())
    }

    /// The channels of every repeated placement of the sheet file `sheet`
    /// found on the project's open sheets (and in the last compile of the
    /// active project), in placement then channel order; empty when the
    /// sheet is not repeated.
    pub(crate) fn sheet_channels(&self, sheet: &Path) -> Vec<ChannelOccurrence> {
        let mut channels: Vec<ChannelOccurrence> = Vec::new();
        for (parent, engine) in &self.document_state.engines {
            let dir = parent.parent().unwrap_or_else(|| Path::new(""));
            for cs in &engine.document().child_sheets {
                if !self.same_sheet(&dir.join(&cs.filename), sheet) {
                    continue;
                }
                if let Some(repeat) = ChannelRepeat::parse(&cs.name) {
                    push_new(&mut channels, repeat.occurrences());
                }
            }
        }
        if channels.is_empty()
            && let Some(project) = self.compile_target()
            && let Some(result) = self.document_state.compiled_design.results.get(&project)
        {
            for row in &result.design.sheets {
                let is_this = result.path_of(&row.key).is_some_and(|p| p == sheet);
                if let (true, Some(repeat)) = (is_this, &row.repeat) {
                    push_new(&mut channels, repeat.occurrences());
                }
            }
        }
        channels
    }

    /// The designator `base` takes in `channel`, through the active
    /// project's channel naming (the default naming outside a project).
    pub(crate) fn channel_designator(&self, base: &str, channel: &ChannelOccurrence) -> String {
        self.channel_naming().designator(base, channel)
    }

    /// Two spellings of one sheet file, under the project's key rules.
    fn same_sheet(&self, a: &Path, b: &Path) -> bool {
        let fold =
            crate::app::state::project_case::fold_of(self.document_state.active_document_project());
        crate::app::project_sheets::sheet_key_with(a, None, fold)
            == crate::app::project_sheets::sheet_key_with(b, None, fold)
            && a.parent().map(normal) == b.parent().map(normal)
    }

    /// The channel (1-based index) the sheet at `path` shows, `None` for
    /// Editor.
    pub(crate) fn shown_channel(&self, path: &Path) -> Option<u32> {
        self.ui_state
            .compiled_design
            .channel_tabs
            .get(path)
            .copied()
    }

    /// A tab of the channel strip was picked.
    pub(super) fn pick_channel(&mut self, sheet: PathBuf, channel: Option<u32>) {
        let store = &mut self.ui_state.compiled_design;
        store.refusal_noted = None;
        match channel {
            Some(index) => {
                store.channel_tabs.insert(sheet, index);
            }
            None => {
                store.channel_tabs.remove(&sheet);
            }
        }
    }

    /// A tree row was clicked: select it and open its sheet; a channel row
    /// opens the sheet on that channel's tab.
    pub(super) fn open_compiled_row(&mut self, row: usize) -> Task<Message> {
        let Some(project) = self.compile_target() else {
            return Task::none();
        };
        let Some(result) = self.document_state.compiled_design.results.get(&project) else {
            return Task::none();
        };
        let Some(sheet) = result.design.sheets.get(row) else {
            return Task::none();
        };
        let Some(path) = result.path_of(&sheet.key).cloned() else {
            return Task::none();
        };
        let channel = sheet.channel.as_ref().map(|c| c.index);
        let placement = (sheet.channel.is_none()).then(|| sheet.path.clone());
        let store = &mut self.ui_state.compiled_design;
        store.selected = Some(row);
        match channel {
            Some(index) => store.channel_tabs.insert(path.clone(), index),
            None => store.channel_tabs.remove(&path),
        };
        let open = match placement {
            Some(placement) => SheetOpen::at(Some(placement)),
            None => SheetOpen::plain(),
        };
        self.open_sheet(path, open)
    }

    /// `Some(reason)` when the sheet an edit acts on shows a channel tab:
    /// a compiled view is read-only. Said in Messages once per channel.
    pub(crate) fn refuse_if_channel_view(&mut self) -> Option<String> {
        let path = self.document_state.edit_path()?.to_path_buf();
        let index = self.shown_channel(&path)?;
        let name = self
            .sheet_channels(&path)
            .into_iter()
            .find(|c| c.index == index)
            .map_or_else(|| index.to_string(), |c| c.name);
        let i18n = signex_i18n::I18n::en_us_ref();
        let line = signex_i18n::t!(
            i18n,
            signex_i18n::ids::PANELS_CHANNEL_READ_ONLY,
            "channel" => name.as_str()
        )
        .into_string();
        let noted = &mut self.ui_state.compiled_design.refusal_noted;
        if noted.as_ref() != Some(&(path.clone(), index)) {
            *noted = Some((path, index));
            crate::diagnostics::log_warning(&line);
        }
        Some(line)
    }
}

/// Add the channels not listed yet, keeping the order.
fn push_new(into: &mut Vec<ChannelOccurrence>, channels: Vec<ChannelOccurrence>) {
    for channel in channels {
        if !into.contains(&channel) {
            into.push(channel);
        }
    }
}

/// A folder path without `.` steps, for comparing two spellings.
fn normal(dir: &Path) -> PathBuf {
    dir.components()
        .filter(|c| !matches!(c, std::path::Component::CurDir))
        .collect()
}
