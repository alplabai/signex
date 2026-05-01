use super::*;

/// Build the project tree from panel context data. Produces one root
/// per loaded project so multi-project workspaces show all their
/// projects side by side. Single-project users see the same shape as
/// before (one root). `PanelContext::projects` is the source of truth;
/// the legacy `project_name` / `sheets` singletons are ignored here so
/// we never emit a duplicate root for the active project.
pub fn build_project_tree(ctx: &PanelContext) -> Vec<TreeNode> {
    if ctx.projects.is_empty() {
        return vec![];
    }

    ctx.projects
        .iter()
        .map(|project| project_root_node(project, ctx.lib_symbol_count))
        .collect()
}

/// One project root - "Source Documents" / "Libraries" / "Settings".
/// `fallback_lib_count` is the workspace-wide library count that we
/// fall through to when the project's own sheet sym totals don't give
/// a useful number; accurate per-project lib counting is a follow-up
/// once the library system is project-scoped (v0.9+).
fn project_root_node(project: &ProjectPanelInfo, fallback_lib_count: usize) -> TreeNode {
    let mut source_docs: Vec<TreeNode> = Vec::new();

    if !project.sheets.is_empty() {
        for sheet in &project.sheets {
            let icon = TreeIcon::for_path(&sheet.filename);
            source_docs.push(
                TreeNode::leaf(sheet.filename.clone(), icon)
                    .with_open(sheet.is_open)
                    .with_dirty(sheet.is_dirty)
                    .with_active(sheet.is_active),
            );
        }
    } else if let Some(file) = &project.project_file {
        let icon = TreeIcon::for_path(file);
        source_docs.push(
            TreeNode::leaf(file.clone(), icon)
                .with_open(project.project_file_open)
                .with_dirty(project.project_file_dirty)
                .with_active(project.project_file_active),
        );
    }

    if let Some(pcb) = &project.pcb_file {
        let icon = TreeIcon::for_path(pcb);
        source_docs.push(
            TreeNode::leaf(pcb.clone(), icon)
                .with_open(project.pcb_file_open)
                .with_dirty(project.pcb_file_dirty)
                .with_active(project.pcb_file_active),
        );
    }

    let lib_count = if fallback_lib_count > 0 {
        fallback_lib_count
    } else {
        project.sheets.iter().map(|s| s.sym_count).sum::<usize>()
    };
    let lib_children = vec![TreeNode::leaf(
        format!("{} symbols loaded", lib_count),
        TreeIcon::Component,
    )];

    let mut settings = TreeNode::branch("Settings".to_string(), TreeIcon::File, vec![]);
    settings.expanded = false;

    TreeNode::branch(
        project.name.clone(),
        TreeIcon::Folder,
        vec![
            TreeNode::branch(
                "Source Documents".to_string(),
                TreeIcon::Folder,
                source_docs,
            ),
            TreeNode::branch("Libraries".to_string(), TreeIcon::Library, lib_children),
            settings,
        ],
    )
    .with_accent(project.is_active)
}

pub(super) fn view_projects<'a>(ctx: &'a PanelContext) -> Element<'a, PanelMsg> {
    if ctx.project_tree.is_empty() {
        let muted = theme_ext::text_secondary(&ctx.tokens);
        iced::widget::column![
            text("No project open").size(11).color(muted),
            text("").size(4),
            text("File > Open to begin").size(10).color(muted),
        ]
        .spacing(2)
        .padding(6)
        .width(Length::Fill)
        .into()
    } else {
        // Render the persistent tree - toggle state is preserved.
        // Wrap in a container with a small top inset so the tree's
        // first row doesn't sit flush against the panel's tab-strip
        // border (matches the breathing room Altium leaves below its
        // panel tabs).
        container({
            let mut tv = TreeView::new(&ctx.project_tree, &ctx.tokens);
            if let Some(sel) = ctx.selected_tree_path.as_deref() {
                tv = tv.selected(sel);
            }
            tv.view().map(PanelMsg::Tree)
        })
        .padding(iced::Padding {
            top: 6.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        })
        .width(Length::Fill)
        .into()
    }
}
