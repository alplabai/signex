//! Graphite Projects panel: a project card, the file filter and the tree
//! grouped by document kind, or, with no project open, the empty card and
//! the recent projects. Legacy themes never reach this module.
//!
//! The grouping, the filter, the group chevrons and the missing / unreadable
//! pills are view-only: the cached `PanelContext::project_tree` keeps its
//! real shape ("Source Documents" ▸ files) and labels, and every message is
//! translated back to a real tree key before it leaves the panel, so the
//! tree handlers see no difference.

use std::collections::BTreeSet;
use std::path::PathBuf;

use iced::widget::column;
use signex_i18n::{I18n, MessageId, ids};
use signex_widgets::tree_view::{BadgeTone, TreeKey};

use super::*;
use crate::styles::{Spacing, TypeScale};

mod card;
mod empty;
mod filter;

pub(crate) use empty::view as empty_view;

const TS: TypeScale = TypeScale::DEFAULT;
const SP: Spacing = Spacing::DEFAULT;

/// Real group of the document rows.
const SOURCE: &str = "Source Documents";
const SCHEMATICS: &str = "Schematics";
const PCB: &str = "PCB";
const BOM: &str = "Bill of Materials";
const OUTPUTS: &str = "Output Jobs";
/// The view-only groups, in display order.
const GROUPS: [&str; 4] = [SCHEMATICS, PCB, BOM, OUTPUTS];

/// View-only state of the Projects panel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectsView {
    /// Text of the Filter files field.
    pub filter: String,
    /// `"<project key>/<group>"` of every collapsed view-only group.
    pub collapsed: BTreeSet<String>,
    /// The Recent projects list of the empty panel is folded.
    pub recent_collapsed: bool,
}

/// Events the Graphite Projects panel emits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectsMsg {
    FilterChanged(String),
    /// The clear (x) button inside the filter field was pressed.
    FilterCleared,
    /// A group header was clicked; carries `"<project key>/<group>"`.
    GroupToggled(String),
    /// The gear on a project card was pressed: that project's options.
    OptionsPressed(crate::app::ProjectId),
    /// The + in the panel's title bar was pressed (mockup row 19).
    AddNewPressed,
    /// New Project… on the empty panel was pressed.
    NewProjectPressed,
    /// Open… on the empty panel was pressed.
    OpenPressed,
    /// The Recent projects header of the empty panel was clicked.
    RecentToggled,
    /// A row of the empty panel's Recent projects was clicked.
    RecentProjectPressed(PathBuf),
}

impl ProjectsView {
    /// Apply a view-only event; the others open something and are routed
    /// by the dock handler, so they change nothing here.
    pub fn apply(&mut self, msg: &ProjectsMsg) {
        match msg {
            ProjectsMsg::FilterChanged(text) => self.filter = text.clone(),
            ProjectsMsg::FilterCleared => self.filter.clear(),
            ProjectsMsg::GroupToggled(path) => {
                if !self.collapsed.remove(path) {
                    self.collapsed.insert(path.clone());
                }
            }
            ProjectsMsg::RecentToggled => self.recent_collapsed = !self.recent_collapsed,
            ProjectsMsg::OptionsPressed(_)
            | ProjectsMsg::AddNewPressed
            | ProjectsMsg::NewProjectPressed
            | ProjectsMsg::OpenPressed
            | ProjectsMsg::RecentProjectPressed(_) => {}
        }
    }

    /// Fold or unfold every view-only group of `roots`: Expand All and
    /// Collapse All reach the groups, which the cached tree does not hold.
    pub fn set_groups_collapsed(&mut self, roots: &[TreeNode], collapsed: bool) {
        if !collapsed {
            self.collapsed.clear();
            return;
        }
        for root in roots {
            for name in GROUPS {
                self.collapsed.insert(group_path(&root.key, name));
            }
        }
    }
}

/// The group a document row belongs to.
fn group_of(node: &TreeNode) -> &'static str {
    // File-extension match, not UI text: ASCII lowercase in every language.
    // ast-grep-ignore: no-case-in-view
    let label = node.label.to_ascii_lowercase();
    if label.contains(".snxbom") {
        BOM
    } else if label.contains(".snxout") {
        OUTPUTS
    } else if matches!(node.icon, TreeIcon::Pcb | TreeIcon::SnxPcb) {
        PCB
    } else {
        SCHEMATICS
    }
}

/// Copy of the tree with each project's "Source Documents" split into the
/// kind groups that have rows. Other children keep their place after them.
fn regroup(roots: &[TreeNode], view: &ProjectsView) -> Vec<TreeNode> {
    roots
        .iter()
        .map(|root| {
            let mut out = root.clone();
            out.children.clear();
            for child in &root.children {
                if child.key != SOURCE {
                    out.children.push(child.clone());
                    continue;
                }
                for name in GROUPS {
                    let mut kids: Vec<TreeNode> = child
                        .children
                        .iter()
                        .filter(|k| group_of(k) == name)
                        .cloned()
                        .collect();
                    if kids.is_empty() {
                        continue;
                    }
                    if name == SCHEMATICS && kids.len() > 1 {
                        let total = kids.len();
                        for (i, kid) in kids.iter_mut().enumerate() {
                            kid.meta = Some(format!("{}/{total}", i + 1));
                        }
                    }
                    let mut group = TreeNode::branch(name, TreeIcon::Folder, kids).with_key(name);
                    group.expanded = !view.collapsed.contains(&group_path(&root.key, name));
                    out.children.push(group);
                }
            }
            out
        })
        .collect()
}

/// The file name and pill text of a row the cached tree marks missing or
/// unreadable.
fn problem_of(label: &str) -> Option<(String, MessageId)> {
    use super::projects::{MISSING_MARK, UNREADABLE_MARK};
    if let Some(name) = label.strip_suffix(MISSING_MARK) {
        return Some((name.to_string(), ids::PANELS_PROJECTS_PILL_MISSING));
    }
    label
        .strip_suffix(UNREADABLE_MARK)
        .map(|name| (name.to_string(), ids::PANELS_PROJECTS_PILL_UNREADABLE))
}

/// The rows of the grouped tree as the view shows them: the group headers
/// named from the catalog in capitals, and a missing / unreadable file as
/// its plain name with a problem pill. Keys are untouched, so routing and
/// navigation still see the cached tree.
fn shown_labels(i18n: &I18n, roots: &mut [TreeNode]) {
    fn walk(i18n: &I18n, node: &mut TreeNode, depth: usize) {
        let branch = match node.key.as_str() {
            SCHEMATICS => Some(ids::PANELS_PROJECTS_GROUP_SCHEMATICS),
            PCB => Some(ids::PANELS_PROJECTS_GROUP_PCB),
            BOM => Some(ids::PANELS_PROJECTS_GROUP_BOM),
            OUTPUTS => Some(ids::PANELS_PROJECTS_GROUP_OUTPUTS),
            super::projects::LIBRARIES_KEY => Some(ids::PANELS_PROJECTS_GROUP_LIBRARIES),
            super::projects::SETTINGS_KEY => Some(ids::PANELS_PROJECTS_GROUP_SETTINGS),
            _ => None,
        };
        if depth == 1
            && let Some(id) = branch
        {
            node.label = i18n.upper_for_display(&i18n.message_plain(id, &[]));
        } else if let Some((name, pill)) = problem_of(&node.label) {
            node.label = name;
            node.badge = Some(i18n.message_plain(pill, &[]));
            node.badge_tone = BadgeTone::Err;
        }
        for child in &mut node.children {
            walk(i18n, child, depth + 1);
        }
    }
    for root in roots {
        walk(i18n, root, 0);
    }
}

fn group_path(project: &str, group: &str) -> String {
    format!("{project}/{group}")
}

fn is_group_key(key: &[String]) -> bool {
    key.len() == 2 && GROUPS.contains(&key[1].as_str())
}

/// The key a row of the grouped tree has in the real tree.
fn to_real_key(key: &[String]) -> TreeKey {
    let mut real = key.to_vec();
    if key.len() >= 2 && GROUPS.contains(&key[1].as_str()) {
        real[1] = SOURCE.to_string();
    }
    real
}

/// The key a row of the real tree has in the grouped tree `grouped`.
fn to_view_key(real: &[String], grouped: &[TreeNode]) -> TreeKey {
    if real.len() >= 3 && real[1] == SOURCE {
        let groups = grouped
            .iter()
            .filter(|root| root.key == real[0])
            .flat_map(|root| root.children.iter())
            .filter(|g| GROUPS.contains(&g.key.as_str()));
        for group in groups {
            if group.children.iter().any(|k| k.key == real[2]) {
                let mut key = real.to_vec();
                key[1] = group.key.clone();
                return key;
            }
        }
    }
    real.to_vec()
}

/// Translate an event of the grouped tree into a panel message.
fn route(msg: TreeMsg) -> PanelMsg {
    let group_toggle = |key: &[String]| {
        PanelMsg::Projects(ProjectsMsg::GroupToggled(group_path(&key[0], &key[1])))
    };
    match msg {
        TreeMsg::Toggle(key) | TreeMsg::Select(key) if is_group_key(&key) => group_toggle(&key),
        TreeMsg::Toggle(key) => PanelMsg::Tree(TreeMsg::Toggle(to_real_key(&key))),
        TreeMsg::Select(key) => PanelMsg::Tree(TreeMsg::Select(to_real_key(&key))),
        TreeMsg::ContextMenu(key) => PanelMsg::Tree(TreeMsg::ContextMenu(to_real_key(&key))),
        TreeMsg::BackgroundContextMenu => PanelMsg::Tree(TreeMsg::BackgroundContextMenu),
    }
}

/// The rows the tree draws, and the project they sit under when its card
/// stands in for its row (mockup row 19). With several projects each keeps
/// its row, which is what says whose groups follow.
fn under_card(decorated: Vec<TreeNode>) -> (Vec<TreeNode>, Option<String>) {
    match <[TreeNode; 1]>::try_from(decorated) {
        Ok([root]) => (root.children, Some(root.key)),
        Err(roots) => (roots, None),
    }
}

/// `key` as the drawn rows know it: without the project when its row is not
/// drawn. The project's own key then names no drawn row.
fn key_under_card<'k>(key: &'k [String], root: Option<&str>) -> Option<&'k [String]> {
    match root {
        None => Some(key),
        Some(_) => key.get(1..).filter(|rest| !rest.is_empty()),
    }
}

/// [`route`] for a tree drawn [`under_card`]: its keys get the project back.
fn route_under(root: Option<&str>, msg: TreeMsg) -> PanelMsg {
    let Some(root) = root else {
        return route(msg);
    };
    let full = |key: TreeKey| -> TreeKey { std::iter::once(root.to_string()).chain(key).collect() };
    route(match msg {
        TreeMsg::Toggle(key) => TreeMsg::Toggle(full(key)),
        TreeMsg::Select(key) => TreeMsg::Select(full(key)),
        TreeMsg::ContextMenu(key) => TreeMsg::ContextMenu(full(key)),
        TreeMsg::BackgroundContextMenu => TreeMsg::BackgroundContextMenu,
    })
}

/// The Graphite Projects panel for a non-empty `ctx.project_tree`.
pub fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let line = crate::styles::tokens::resolve(tokens).line_soft;
    let mut top: Column<Element<'a, PanelMsg>> = Column::new().spacing(SP.l).width(Length::Fill);
    let i18n = &*ctx.i18n;
    for project in &ctx.projects {
        top = top.push(card::project_card(project, ctx));
    }
    top = top.push(filter::filter_field(ctx));
    let edge = iced::border::Side::default().width(1.0);
    let header = container(top)
        .padding(iced::Padding {
            top: 10.0,
            right: 10.0,
            bottom: 8.0,
            left: 10.0,
        })
        .width(Length::Fill)
        .style(move |_: &Theme| {
            let mut border = Border {
                color: line,
                ..Border::default()
            };
            border.bottom = edge;
            container::Style {
                border,
                ..container::Style::default()
            }
        });

    let needle = i18n.fold_for_search(ctx.projects_view.filter.trim());
    let mut roots = regroup(&ctx.project_tree, &ctx.projects_view);
    shown_labels(i18n, &mut roots);
    if !needle.is_empty() {
        roots = filter::filtered(i18n, &roots, &needle);
    }
    let body: Element<'a, PanelMsg> = if roots.is_empty() {
        filter::no_match_hint(ctx)
    } else {
        // Decorated first: the count badges go by depth under the project.
        let (rows, root) = under_card(super::graphite::decorate_tree(&roots));
        let mut tree = TreeView::new(&rows, tokens)
            .text_size(TS.body)
            .style(super::graphite::tree_style(tokens));
        let selected = ctx
            .project_tree_selected
            .as_deref()
            .map(|key| to_view_key(key, &roots));
        let drawn = selected
            .as_deref()
            .and_then(|key| key_under_card(key, root.as_deref()));
        if let Some(sel) = drawn {
            tree = tree.selected(sel);
        }
        tree.view()
            .map(move |msg| route_under(root.as_deref(), msg))
            .boxed()
    };
    let body = container(body).padding(iced::Padding {
        top: 6.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    });
    column![header.boxed(), body.boxed()]
        .width(Length::Fill)
        .boxed()
}

#[cfg(test)]
mod tests;
