//! The project root's Planned › flyout (#20): the project features that are
//! not built yet, each a greyed row with the "planned" pill, so every row at
//! the top level of the menu does something.

use super::*;

use super::super::items::dd_planned_pill;
use crate::icons as ic;
use signex_types::theme::ThemeId;

/// The Planned flyout rows. Variants... stays here until the Variant Manager
/// tab exists.
pub(in crate::app::view::context_menu) fn planned_entries(
    i18n: &I18n,
    tid: ThemeId,
) -> Vec<DropdownEntry<Message>> {
    [
        (
            ic::icon_menu_cloud(tid),
            menu_ids::CTX_TREE_MAKE_AVAILABLE_ONLINE,
        ),
        (ic::icon_menu_layers(tid), menu_ids::CTX_TREE_VARIANTS),
        (ic::icon_menu_package(tid), menu_ids::CTX_TREE_PACKAGER),
        (ic::icon_menu_export(tid), menu_ids::CTX_TREE_RELEASER),
        (ic::icon_menu_share(tid), menu_ids::CTX_TREE_SHARE),
    ]
    .into_iter()
    .map(|(icon, label)| dd_planned_pill(i18n, Some(icon), &text(i18n, label)))
    .collect()
}
