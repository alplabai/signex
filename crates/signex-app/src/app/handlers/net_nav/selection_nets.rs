//! The nets a schematic selection sits on, and the items of named nets on a
//! sheet, for Cross Probe and Cross Select Mode (#1312). Read from the
//! project index already held; neither builds one, so a passive cross select
//! never starts work the user did not ask for.

use std::path::{Path, PathBuf};

use signex_types::schematic::{InstancePath, SelectedItem, SelectedKind};

use super::super::super::*;
use super::navigate::selected_kind_of;
use super::target::{is_net_kind, net_of_target};
use crate::app::state::ProjectId;

impl Signex {
    /// The names of the nets the wires, labels, junctions and sheet entries
    /// of `items` sit on, on the sheet at `path`. A part is matched by its
    /// link instead, so a symbol adds no net here.
    pub(crate) fn net_names_of_selection(
        &self,
        path: &Path,
        items: &[SelectedItem],
    ) -> Vec<String> {
        let Some(data) = self
            .project_of_sheet(path)
            .and_then(|project| self.document_state.net_nav.index.latest(project))
        else {
            return Vec::new();
        };
        let Some(key) = data.key_of(path) else {
            return Vec::new();
        };
        let placement = self.net_occurrence(&data, path);
        let mut names: Vec<String> = items
            .iter()
            .filter(|item| item.kind != SelectedKind::Symbol && is_net_kind(item.kind))
            .filter_map(|item| net_of_target(&data, &key, item, None, placement.as_ref()))
            .filter_map(|net| data.index.net(net).map(|n| n.name.clone()))
            .filter(|name| !name.trim().is_empty())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// The members of the nets named `names` on the sheet at `path`, in the
    /// placement it is shown through.
    pub(crate) fn net_items_named(&self, path: &Path, names: &[String]) -> Vec<SelectedItem> {
        if names.is_empty() {
            return Vec::new();
        }
        let Some(data) = self
            .project_of_sheet(path)
            .and_then(|project| self.document_state.net_nav.index.latest(project))
        else {
            return Vec::new();
        };
        let Some(key) = data.key_of(path) else {
            return Vec::new();
        };
        let placement = self.net_occurrence(&data, path);
        let mut items: Vec<SelectedItem> = Vec::new();
        for net in (0..data.index.net_count()).filter_map(|i| data.index.net(i)) {
            if !names.iter().any(|name| name.trim() == net.name.trim()) {
                continue;
            }
            let here = net.members.iter().filter(|m| {
                m.sheet == key && placement.as_ref().is_none_or(|p| &m.occurrence == p)
            });
            for member in here {
                // A pin stands for its symbol, which the part match selects.
                if selected_kind_of(member) == SelectedKind::Symbol {
                    continue;
                }
                let item = SelectedItem::new(member.id, selected_kind_of(member));
                if !items.contains(&item) {
                    items.push(item);
                }
            }
        }
        items
    }

    /// The sheet and placement of the first member of the nets named `names`
    /// in `project`, in walk order.
    pub(crate) fn sheet_of_nets(
        &self,
        project: ProjectId,
        names: &[String],
    ) -> Option<(PathBuf, InstancePath)> {
        let data = self.document_state.net_nav.index.latest(project)?;
        let member = (0..data.index.net_count())
            .filter_map(|i| data.index.net(i))
            .filter(|net| names.iter().any(|name| name.trim() == net.name.trim()))
            .find_map(|net| net.members.first())?;
        let path = data.path_of(&member.sheet)?.to_path_buf();
        Some((path, member.occurrence.clone()))
    }
}
