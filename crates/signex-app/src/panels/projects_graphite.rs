//! Graphite Projects panel: a project card, the file filter and the tree
//! grouped by document kind. Legacy themes never reach this file.
//!
//! The grouping, the filter and the group chevrons are view-only: the cached
//! `PanelContext::project_tree` keeps its real shape ("Source Documents" ▸
//! files) and every message is translated back to a real tree key before it
//! leaves the panel, so the tree handlers see no difference.

use std::collections::BTreeSet;

use iced::widget::{column, text_input};
use signex_widgets::tree_view::TreeKey;

use super::*;
use crate::styles::tokens::resolve;
use crate::styles::{self, PillTone, Spacing, TypeScale, ti};

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

/// Height of the Filter files field.
const FILTER_HEIGHT: f32 = 28.0;
/// Side of the project icon tile.
const CARD_ICON_TILE: f32 = 26.0;

/// View-only state of the Projects panel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectsView {
    /// Text of the Filter files field.
    pub filter: String,
    /// `"<project key>/<group>"` of every collapsed view-only group.
    pub collapsed: BTreeSet<String>,
}

/// Events the Graphite Projects panel emits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectsMsg {
    FilterChanged(String),
    /// A group header was clicked; carries `"<project key>/<group>"`.
    GroupToggled(String),
}

impl ProjectsView {
    pub fn apply(&mut self, msg: &ProjectsMsg) {
        match msg {
            ProjectsMsg::FilterChanged(text) => self.filter = text.clone(),
            ProjectsMsg::GroupToggled(path) => {
                if !self.collapsed.remove(path) {
                    self.collapsed.insert(path.clone());
                }
            }
        }
    }
}

/// The group a document row belongs to.
fn group_of(node: &TreeNode) -> &'static str {
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

fn group_path(project: &str, group: &str) -> String {
    format!("{project}/{group}")
}

/// The rows whose label contains `needle` (lower case), each with the
/// branches above it; a matching branch keeps all of its rows.
fn filtered(nodes: &[TreeNode], needle: &str) -> Vec<TreeNode> {
    nodes
        .iter()
        .filter_map(|node| {
            if node.label.to_lowercase().contains(needle) && !node.children.is_empty() {
                return Some(node.clone());
            }
            if node.children.is_empty() {
                return (!node.is_folder && node.label.to_lowercase().contains(needle))
                    .then(|| node.clone());
            }
            let kids = filtered(&node.children, needle);
            (!kids.is_empty()).then(|| {
                let mut out = node.clone();
                out.children = kids;
                out.expanded = true;
                out
            })
        })
        .collect()
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

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

fn count_sheets(sheets: &[SheetInfo]) -> usize {
    sheets.iter().map(|s| 1 + count_sheets(&s.children)).sum()
}

/// "3 sheets · 1 board · 2 libraries": the counts the project really has.
fn card_summary(project: &ProjectPanelInfo) -> String {
    let sheets = if project.sheets.is_empty() {
        usize::from(project.project_file.is_some())
    } else {
        count_sheets(&project.sheets)
    };
    let mut parts = Vec::new();
    if sheets > 0 {
        parts.push(plural(sheets, "sheet", "sheets"));
    }
    if project.pcb_file.is_some() {
        parts.push(plural(1, "board", "boards"));
    }
    if !project.libraries.is_empty() {
        parts.push(plural(project.libraries.len(), "library", "libraries"));
    }
    parts.join(" · ")
}

/// Project card: icon tile, name, real counts, and a "git" pill when the
/// project has version control on.
fn project_card<'a>(project: &'a ProjectPanelInfo, tokens: &ThemeTokens) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let (tile_fill, ink, ink4) = (r.accent_soft, ti(tokens.text), r.ink4);
    let tint = r.accent_text;
    let icon = svg(TreeIcon::SnxProject.svg())
        .width(14)
        .height(14)
        .style(move |_: &Theme, _| svg::Style { color: Some(tint) });
    let tile = container(icon)
        .width(CARD_ICON_TILE)
        .height(CARD_ICON_TILE)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(tile_fill)),
            border: Border {
                radius: 7.0.into(),
                ..Border::default()
            },
            ..container::Style::default()
        });
    let name = text(raw(project.name.clone()))
        .size(TS.title)
        .color(ink)
        .font(iced::Font {
            weight: iced::font::Weight::Semibold,
            ..iced::Font::DEFAULT
        })
        .wrapping(iced::widget::text::Wrapping::None);
    let summary = text(raw(card_summary(project)))
        .size(TS.caption)
        .color(ink4)
        .wrapping(iced::widget::text::Wrapping::None);
    let titles: Column<Element<'a, PanelMsg>> =
        Column::new().push(name.boxed()).push(summary.boxed());
    let mut head: iced::widget::Row<Element<'a, PanelMsg>> = iced::widget::Row::new()
        .spacing(SP.l)
        .align_y(iced::Alignment::Center)
        .push(tile.boxed())
        .push(titles.boxed());
    if project.version_control {
        let pill = container(
            text(raw("git"))
                .size(TS.caption)
                .font(iced::Font::MONOSPACE)
                .wrapping(iced::widget::text::Wrapping::None),
        )
        .padding([0.0, 7.0])
        .height(20.0)
        .align_y(iced::alignment::Vertical::Center)
        .style(styles::pill(tokens, PillTone::Neutral));
        head = head
            .push(iced::widget::space::horizontal().boxed())
            .push(pill.boxed());
    }
    head.boxed()
}

/// The Filter files field.
fn filter_field<'a>(view: &'a ProjectsView, tokens: &ThemeTokens) -> Element<'a, PanelMsg> {
    let r = resolve(tokens);
    let (ink, ink4) = (ti(tokens.text), r.ink4);
    let input = text_input("Filter files", &view.filter)
        .on_input(|s| PanelMsg::Projects(ProjectsMsg::FilterChanged(s)))
        .padding(iced::Padding::ZERO)
        .size(TS.body)
        .width(Length::Fill)
        .style(move |_: &Theme, _| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            placeholder: ink4,
            value: ink,
            selection: Color { a: 0.4, ..ink },
        });
    container(input)
        .padding([0.0, SP.l])
        .height(FILTER_HEIGHT)
        .align_y(iced::alignment::Vertical::Center)
        .width(Length::Fill)
        .style(styles::field(tokens, false))
        .boxed()
}

/// The Graphite Projects panel for a non-empty `ctx.project_tree`.
pub fn view<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    let tokens = &ctx.tokens;
    let r = resolve(tokens);
    let line = r.line_soft;
    let mut top: Column<Element<'a, PanelMsg>> = Column::new().spacing(SP.l).width(Length::Fill);
    for project in &ctx.projects {
        top = top.push(project_card(project, tokens));
    }
    top = top.push(filter_field(&ctx.projects_view, tokens));
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

    let needle = ctx.projects_view.filter.trim().to_lowercase();
    let mut roots = regroup(&ctx.project_tree, &ctx.projects_view);
    if !needle.is_empty() {
        roots = filtered(&roots, &needle);
    }
    let body: Element<'a, PanelMsg> = if roots.is_empty() {
        text(raw("No files match"))
            .size(TS.caption)
            .color(r.ink4)
            .boxed()
    } else {
        let decorated = super::graphite::decorate_tree(&roots);
        let mut tree = TreeView::new(&decorated, tokens)
            .text_size(TS.body)
            .style(super::graphite::tree_style(tokens));
        let selected = ctx
            .project_tree_selected
            .as_deref()
            .map(|key| to_view_key(key, &roots));
        if let Some(sel) = selected.as_deref() {
            tree = tree.selected(sel);
        }
        tree.view().map(route).boxed()
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
