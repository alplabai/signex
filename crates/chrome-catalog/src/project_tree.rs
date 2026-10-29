use iced::widget::container;
use iced::{Element, Length};
use signex_types::theme::ThemeTokens;
use signex_widgets::tree_view::{TreeIcon, TreeNode, TreeView};

use crate::catalog::Message;

/// One leaf per open / dirty / active combination, as the project tree
/// draws them.
fn sample_nodes() -> Vec<TreeNode> {
    [
        ("clean.standard_sch", false, false, false),
        ("open.standard_sch", true, false, false),
        ("dirty.standard_sch", true, true, false),
        ("active.standard_sch", true, false, true),
        ("active+dirty.standard_sch", true, true, true),
    ]
    .into_iter()
    .map(|(label, open, dirty, active)| {
        TreeNode::leaf(label, TreeIcon::for_path(label))
            .with_open(open)
            .with_dirty(dirty)
            .with_active(active)
    })
    .collect()
}

/// The real project-tree widget, so this section can never drift from it.
/// The rows are static samples: tree events are ignored.
pub(crate) fn view<'a>(tokens: &ThemeTokens) -> Element<'a, Message> {
    let nodes = sample_nodes();
    let tree = TreeView::new(&nodes, tokens)
        .view()
        .map(|_| Message::TreeInteracted);
    container(tree)
        .width(Length::Fixed(360.0))
        .height(Length::Fixed(120.0))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_cover_every_indicator_combination() {
        let flags: Vec<_> = sample_nodes()
            .iter()
            .map(|n| (n.is_open, n.is_dirty, n.is_active))
            .collect();
        assert_eq!(
            flags,
            [
                (false, false, false),
                (true, false, false),
                (true, true, false),
                (true, false, true),
                (true, true, true),
            ]
        );
        assert!(sample_nodes().iter().all(|n| n.children.is_empty()));
    }
}
