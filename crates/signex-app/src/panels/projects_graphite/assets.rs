//! The `assets/` group of the Projects tree (#1116, mockup row 204):
//! `images/` lists the project's image files with how many images use each,
//! a missing one with the missing pill and a pick waiting for its save with
//! "new". Picking a row puts the image on the cursor over the sheet in view;
//! a click places it. The rows come from the last asset scan
//! (`app/images/assets.rs`), kept on [`super::ProjectsView`].

use signex_i18n::{I18n, ids};
use signex_widgets::tree_view::{BadgeTone, TreeIcon, TreeNode};

use super::{PanelMsg, ProjectsMsg, ProjectsView, group_path};

/// Key and label of the group, a folder of the project.
pub(super) const ASSETS: &str = "assets/";
/// Key and label of its images folder.
pub(super) const IMAGES: &str = "images/";

/// One image file of a project's `assets/images/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetRow {
    /// The file name, as the row shows it.
    pub name: String,
    /// The project-relative path a placement names.
    pub file: String,
    pub uses: usize,
    pub missing: bool,
    /// Picked but not saved yet: the save copies it in.
    pub pending: bool,
}

fn row_node(i18n: &I18n, row: &AssetRow) -> TreeNode {
    let mut node = TreeNode::leaf(row.name.clone(), TreeIcon::File).with_key(row.name.clone());
    if row.missing {
        node.badge = Some(i18n.message_plain(ids::PANELS_PROJECTS_PILL_MISSING, &[]));
        node.badge_tone = BadgeTone::Err;
    } else if row.pending {
        node.badge = Some(i18n.message_plain(ids::IMAGES_TREE_NEW, &[]));
        node.badge_tone = BadgeTone::Info;
    }
    if row.uses > 0 {
        node.meta =
            Some(signex_i18n::t!(i18n, ids::IMAGES_TREE_USES, "count" => row.uses).into_string());
    }
    node
}

/// `roots` with an `assets/` group under each project the view has image
/// rows for, after its other groups.
pub(super) fn with_assets(i18n: &I18n, roots: &mut [TreeNode], view: &ProjectsView) {
    for root in roots {
        let Some(rows) = view.assets.get(&root.key).filter(|rows| !rows.is_empty()) else {
            continue;
        };
        let files: Vec<TreeNode> = rows.iter().map(|row| row_node(i18n, row)).collect();
        let count = files.len();
        let mut images = TreeNode::branch(IMAGES, TreeIcon::Folder, files).with_key(IMAGES);
        images.badge = Some(count.to_string());
        images.expanded = !view
            .collapsed
            .contains(&group_path(&root.key, &format!("{ASSETS}{IMAGES}")));
        let mut assets = TreeNode::branch(ASSETS, TreeIcon::Folder, vec![images]).with_key(ASSETS);
        // No document count on the group: its folder carries the file count.
        assets.is_folder = false;
        assets.expanded = !view.collapsed.contains(&group_path(&root.key, ASSETS));
        root.children.push(assets);
    }
}

/// The message a press on an `assets/` row of the tree sends, for a key
/// relative to the project's row: `None` for a row outside the group.
pub(super) fn route(project: &str, key: &[String]) -> Option<PanelMsg> {
    if key.first().map(String::as_str) != Some(ASSETS) {
        return None;
    }
    let msg = match key {
        [_] => ProjectsMsg::GroupToggled(group_path(project, ASSETS)),
        [_, images] if images == IMAGES => {
            ProjectsMsg::GroupToggled(group_path(project, &format!("{ASSETS}{IMAGES}")))
        }
        [_, images, name] if images == IMAGES => ProjectsMsg::AssetPressed(format!(
            "{}/{name}",
            signex_types::image_asset::IMAGE_ASSET_DIR
        )),
        _ => return Some(PanelMsg::Noop),
    };
    Some(PanelMsg::Projects(msg))
}
