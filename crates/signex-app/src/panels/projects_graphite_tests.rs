//! #2059: the Projects tree names its branches and file marks from the
//! catalog in the view, while the cached tree, and so navigation, keeps its
//! keys whatever the language.

use super::*;
use crate::layout_probe::{lay_out_texts, messages_on_click};
use signex_widgets::tree_view::{get_node, resolve_path};

const WINDOW: iced::Size = iced::Size::new(360.0, 900.0);

/// A context whose cached tree is one project with a sheet, a missing
/// sheet and a missing library, as `build_project_tree` shapes it.
fn context(language: &str) -> PanelContext {
    let (app, _boot) = crate::app::Signex::new();
    let mut ctx = app.document_state.panel_ctx.clone();
    ctx.i18n = std::sync::Arc::new(I18n::new(&signex_i18n::LanguagePref::Locale(
        language.to_owned(),
    )));
    ctx.project_tree = vec![
        TreeNode::branch(
            "proj",
            TreeIcon::Folder,
            vec![
                TreeNode::branch(
                    SOURCE,
                    TreeIcon::Folder,
                    vec![
                        TreeNode::leaf("a.snxsch", TreeIcon::SnxSchematic).with_key("a.snxsch"),
                        TreeNode::leaf("gone.snxsch  (missing)", TreeIcon::SnxSchematic)
                            .with_key("gone.snxsch"),
                        TreeNode::leaf("broken.snxsch  (unreadable)", TreeIcon::SnxSchematic)
                            .with_key("broken.snxsch"),
                        TreeNode::leaf("board.snxpcb", TreeIcon::SnxPcb).with_key("board.snxpcb"),
                        TreeNode::leaf("parts.snxbom", TreeIcon::File).with_key("parts.snxbom"),
                        TreeNode::leaf("fab.snxout", TreeIcon::File).with_key("fab.snxout"),
                    ],
                ),
                TreeNode::branch(
                    super::super::projects::SETTINGS_KEY,
                    TreeIcon::File,
                    vec![TreeNode::leaf("prefs.json", TreeIcon::File).with_key("prefs.json")],
                ),
                TreeNode::branch(
                    super::super::projects::LIBRARIES_KEY,
                    TreeIcon::Library,
                    vec![
                        TreeNode::leaf("lib.snxlib  (missing)", TreeIcon::SnxLibrary)
                            .with_key("/p/lib.snxlib"),
                    ],
                ),
            ],
        )
        .with_key("proj"),
    ];
    ctx
}

fn texts(ctx: &PanelContext) -> Vec<String> {
    lay_out_texts(view(ctx), WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

/// en-US reads as the cached English labels did.
#[test]
fn the_tree_reads_as_before_in_en_us() {
    let shown = texts(&context("en-US"));
    let missing: Vec<&str> = [
        "Schematics",
        "PCB",
        "Bill of Materials",
        "Output Jobs",
        "Libraries",
        "Settings",
        "a.snxsch",
        "gone.snxsch  (missing)",
        "broken.snxsch  (unreadable)",
        "lib.snxlib  (missing)",
    ]
    .into_iter()
    .filter(|label| !shown.iter().any(|t| t == label))
    .collect();
    assert!(missing.is_empty(), "{missing:?} not in {shown:#?}");
}

/// A language switch renames the branches and the marks, never the files.
#[test]
fn the_tree_follows_a_language_switch() {
    let ctx = context("en-XA");
    let shown = texts(&ctx);
    for english in [
        "Schematics",
        "Bill of Materials",
        "Output Jobs",
        "Libraries",
        "Settings",
        "gone.snxsch  (missing)",
        "broken.snxsch  (unreadable)",
    ] {
        assert!(
            !shown.iter().any(|t| t == english),
            "{english:?} in {shown:#?}"
        );
    }
    let xa = &ctx.i18n;
    for label in [
        xa.message_plain(ids::PANELS_PROJECTS_GROUP_SCHEMATICS, &[]),
        xa.message_plain(ids::PANELS_PROJECTS_GROUP_LIBRARIES, &[]),
        xa.message_plain(ids::PANELS_PROJECTS_GROUP_PCB, &[]),
        xa.message_plain(ids::PANELS_PROJECTS_GROUP_BOM, &[]),
        xa.message_plain(ids::PANELS_PROJECTS_GROUP_OUTPUTS, &[]),
        xa.message_plain(ids::PANELS_PROJECTS_GROUP_SETTINGS, &[]),
        xa.message_plain(
            ids::PANELS_PROJECTS_FILE_MISSING,
            &[("name", "lib.snxlib".into())],
        ),
        xa.message_plain(
            ids::PANELS_PROJECTS_FILE_UNREADABLE,
            &[("name", "broken.snxsch".into())],
        ),
    ] {
        assert!(shown.contains(&label), "{label:?} in {shown:#?}");
    }
}

/// Clicking the renamed library row still selects the cached row, whose
/// label navigation reads the file name from.
#[test]
fn navigation_works_after_a_language_switch() {
    let ctx = context("en-XA");
    let row = ctx.i18n.message_plain(
        ids::PANELS_PROJECTS_FILE_MISSING,
        &[("name", "lib.snxlib".into())],
    );
    let picked: Vec<TreeKey> = messages_on_click(view(&ctx), WINDOW, &row)
        .into_iter()
        .filter_map(|m| match m {
            PanelMsg::Tree(TreeMsg::Select(key)) => Some(key),
            _ => None,
        })
        .collect();
    let key = picked.first().expect("the row was selected");
    assert_eq!(
        key,
        &["proj", "Libraries", "/p/lib.snxlib"].map(String::from)
    );
    let path = resolve_path(&ctx.project_tree, key).expect("a cached row");
    let node = get_node(&ctx.project_tree, &path).expect("the node");
    assert_eq!(
        node.label
            .strip_suffix(super::super::projects::MISSING_MARK),
        Some("lib.snxlib")
    );
}
