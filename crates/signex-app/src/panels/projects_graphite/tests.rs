//! #2059: the Projects tree names its branches and file marks from the
//! catalog in the view, while the cached tree, and so navigation, keeps its
//! keys whatever the language. Mockup row 19: missing files read as their plain name
//! with a pill, the filter says when it matches nothing, the card's options button
//! opens Project Options… and the empty panel offers New Project…, Open…
//! and the recent projects.

use super::*;
use crate::layout_probe::{lay_out_texts, messages_on_click, messages_on_click_at};
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

fn texts_of(element: Element<'_, PanelMsg>) -> Vec<String> {
    lay_out_texts(element, WINDOW)
        .into_iter()
        .map(|t| t.text.replace(['\u{2068}', '\u{2069}'], ""))
        .collect()
}

fn texts(ctx: &PanelContext) -> Vec<String> {
    texts_of(view(ctx))
}

/// en-US reads as the cached English labels did, the group headers in
/// capitals and the marked files as their plain names.
#[test]
fn the_tree_reads_as_before_in_en_us() {
    let shown = texts(&context("en-US"));
    let missing: Vec<&str> = [
        "SCHEMATICS",
        "PCB",
        "BILL OF MATERIALS",
        // The `.snxout` group reads by its UI name (DECISIONS 2026-11-17);
        // its grouping key stays "Output Jobs".
        "EXPORT JOBS",
        "LIBRARIES",
        "SETTINGS",
        "a.snxsch",
        "gone.snxsch",
        "broken.snxsch",
        "lib.snxlib",
    ]
    .into_iter()
    .filter(|label| !shown.iter().any(|t| t == label))
    .collect();
    assert!(missing.is_empty(), "{missing:?} not in {shown:#?}");
    // Each group's own row heads its file: name, count, badge, then the file.
    for (group, file) in [
        ("PCB", "board.snxpcb"),
        ("BILL OF MATERIALS", "parts.snxbom"),
        ("EXPORT JOBS", "fab.snxout"),
    ] {
        let at = shown.iter().position(|t| t == file).expect("the file row");
        assert_eq!(shown[at - 3], group, "{shown:#?}");
    }
}

/// Mockup row 19: a missing or unreadable file shows its plain name, no kind tag
/// ahead of it and a pill after it, never the old text suffix.
#[test]
fn a_missing_file_reads_as_its_name_and_a_pill() {
    let shown = texts(&context("en-US"));
    for (file, pill) in [
        ("gone.snxsch", "missing"),
        ("broken.snxsch", "unreadable"),
        ("lib.snxlib", "missing"),
    ] {
        let at = shown.iter().position(|t| t == file).expect("the file row");
        // Behind the name: the page position (schematics only), then the pill.
        let behind = &shown[at + 1..(at + 3).min(shown.len())];
        assert!(
            behind.iter().any(|t| t == pill),
            "no {pill:?} pill after {file:?}: {shown:#?}"
        );
        assert!(
            !["SCH", "LIB"].contains(&shown[at - 1].as_str()),
            "{file:?} still leads with a kind tag: {shown:#?}"
        );
    }
    for old in [
        "gone.snxsch  (missing)",
        "broken.snxsch  (unreadable)",
        "lib.snxlib  (missing)",
    ] {
        assert!(!shown.iter().any(|t| t == old), "{old:?} in {shown:#?}");
    }
}

/// The pill is view-only: the cached tree keeps its marked label, and the
/// shown row carries the problem tone with the plain name under the same key.
#[test]
fn the_pill_leaves_the_cached_tree_alone() {
    let ctx = context("en-US");
    let mut roots = regroup(&ctx.project_tree, &ctx.projects_view);
    shown_labels(&ctx.i18n, &mut roots);
    let decorated = super::super::graphite::decorate_tree(&roots);
    let key = ["proj", SCHEMATICS, "gone.snxsch"].map(String::from);
    let path = resolve_path(&decorated, &key).expect("the shown row");
    let row = get_node(&decorated, &path).expect("the node");
    assert_eq!(row.label, "gone.snxsch");
    assert_eq!(row.badge.as_deref(), Some("missing"));
    assert_eq!(row.badge_tone, BadgeTone::Err);
    let real = ["proj", SOURCE, "gone.snxsch"].map(String::from);
    let path = resolve_path(&ctx.project_tree, &real).expect("the cached row");
    let cached = get_node(&ctx.project_tree, &path).expect("the node");
    assert_eq!(cached.label, "gone.snxsch  (missing)");
}

/// A language switch renames the branches and the pills, never the files.
#[test]
fn the_tree_follows_a_language_switch() {
    let ctx = context("en-XA");
    let shown = texts(&ctx);
    for english in [
        "SCHEMATICS",
        "BILL OF MATERIALS",
        "EXPORT JOBS",
        "LIBRARIES",
        "SETTINGS",
        "missing",
        "unreadable",
    ] {
        assert!(
            !shown.iter().any(|t| t == english),
            "{english:?} in {shown:#?}"
        );
    }
    let xa = &ctx.i18n;
    let upper = |id| xa.upper_for_display(&xa.message_plain(id, &[]));
    for label in [
        upper(ids::PANELS_PROJECTS_GROUP_SCHEMATICS),
        upper(ids::PANELS_PROJECTS_GROUP_LIBRARIES),
        upper(ids::PANELS_PROJECTS_GROUP_PCB),
        upper(ids::PANELS_PROJECTS_GROUP_BOM),
        upper(ids::PANELS_PROJECTS_GROUP_OUTPUTS),
        upper(ids::PANELS_PROJECTS_GROUP_SETTINGS),
        xa.message_plain(ids::PANELS_PROJECTS_PILL_MISSING, &[]),
        xa.message_plain(ids::PANELS_PROJECTS_PILL_UNREADABLE, &[]),
        "lib.snxlib".to_string(),
        "broken.snxsch".to_string(),
    ] {
        assert!(shown.contains(&label), "{label:?} in {shown:#?}");
    }
}

/// Clicking the missing library row in another language still selects the
/// cached row, whose label navigation reads the file name from.
#[test]
fn navigation_works_after_a_language_switch() {
    let ctx = context("en-XA");
    let picked: Vec<TreeKey> = messages_on_click(view(&ctx), WINDOW, "lib.snxlib")
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

/// Every message a left click anywhere on the vertical line `x`, from the
/// top of the panel down to `reach`, publishes.
fn messages_down(ctx: &PanelContext, x: f32, reach: f32) -> Vec<PanelMsg> {
    let mut out = Vec::new();
    let mut y = 1.0;
    while y < reach {
        out.extend(messages_on_click_at(
            view(ctx),
            WINDOW,
            None,
            iced::Point::new(x, y),
        ));
        y += 3.0;
    }
    out
}

/// Mockup row 19: a filter that matches nothing says so, with the text that
/// was typed, instead of a bare caption; the field's clear button empties
/// the filter.
#[test]
fn a_filter_that_matches_nothing_says_so_and_clears() {
    let mut ctx = context("en-US");
    ctx.projects_view.filter = "usbc".to_string();
    let shown = texts(&ctx);
    for line in [
        "No files match \"usbc\"",
        "Clear the filter to see every file.",
    ] {
        assert!(shown.iter().any(|t| t == line), "{line:?} in {shown:#?}");
    }
    assert!(!shown.iter().any(|t| t == "a.snxsch"), "{shown:#?}");
    // The clear button sits at the field's right end, inside its padding.
    let x = WINDOW.width - 10.0 - SP.m - 9.0;
    let clears = |ctx: &PanelContext| {
        messages_down(ctx, x, 200.0)
            .into_iter()
            .any(|m| matches!(m, PanelMsg::Projects(ProjectsMsg::FilterCleared)))
    };
    assert!(clears(&ctx), "no clear button at x = {x}");

    let mut state = ctx.projects_view.clone();
    state.apply(&ProjectsMsg::FilterCleared);
    assert!(state.filter.is_empty());

    // An empty field has no clear button.
    ctx.projects_view.filter.clear();
    assert!(!clears(&ctx), "a clear button on an empty field");
}

/// Push a loaded project `name` with one sheet; returns its id.
fn add_project(app: &mut crate::app::Signex, name: &str) -> crate::app::ProjectId {
    use signex_types::project::{ProjectData, SheetEntry};
    let id = app.document_state.mint_project_id();
    let sheet = format!("{name}.snxsch");
    app.document_state.projects.push(crate::app::LoadedProject {
        id,
        path: std::path::PathBuf::from(format!("/w/{name}/{name}.snxprj")),
        data: ProjectData {
            document_id: None,
            description: None,
            name: name.to_string(),
            dir: format!("/w/{name}"),
            schematic_root: Some(sheet.clone()),
            pcb_file: None,
            sheets: vec![SheetEntry {
                name: name.to_string(),
                filename: sheet,
                symbols_count: 0,
                wires_count: 0,
                labels_count: 0,
                unknown: Default::default(),
            }],
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            parameters: Default::default(),
            schematic_defaults: Default::default(),
            project_defaults: Default::default(),
            agent_journal: Default::default(),
            schematic_design: Default::default(),
            fab_outputs: None,
            router_tool_diameter: None,
            requirements: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: std::collections::HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    id
}

/// Mockup row 19: the options button at the right end of each project card asks for that
/// card's project's options.
#[test]
fn each_card_options_button_asks_for_its_projects_options() {
    let (mut app, _boot) = crate::app::Signex::new();
    let first = add_project(&mut app, "A");
    let second = add_project(&mut app, "B");
    app.refresh_panel_ctx();
    let ctx = app.document_state.panel_ctx.clone();
    // The button's centre at the card's right end, inside the header's 10 px
    // padding.
    let x = WINDOW.width - 10.0 - card::OPTIONS_BUTTON / 2.0;
    let asked: Vec<crate::app::ProjectId> = messages_down(&ctx, x, 200.0)
        .into_iter()
        .filter_map(|m| match m {
            PanelMsg::Projects(ProjectsMsg::OptionsPressed(id)) => Some(id),
            _ => None,
        })
        .collect();
    for id in [first, second] {
        assert!(
            asked.contains(&id),
            "{asked:?} has no options button for {id:?}"
        );
    }
}

/// The Project Options button is as large as the project tile it shares the
/// card row with: a press just inside its left edge reaches it, one just
/// outside does not.
#[test]
fn the_project_options_button_is_as_large_as_the_cards_tile() {
    assert_eq!(card::OPTIONS_BUTTON, card::CARD_ICON_TILE);
    let (mut app, _boot) = crate::app::Signex::new();
    let id = add_project(&mut app, "A");
    app.refresh_panel_ctx();
    let ctx = app.document_state.panel_ctx.clone();
    // The card's right end, inside the header's 10 px padding.
    let right = WINDOW.width - 10.0;
    let asks = |x: f32| {
        messages_down(&ctx, x, 200.0)
            .into_iter()
            .any(|m| matches!(m, PanelMsg::Projects(ProjectsMsg::OptionsPressed(p)) if p == id))
    };
    assert!(asks(right - card::OPTIONS_BUTTON + 1.0), "just inside");
    assert!(!asks(right - card::OPTIONS_BUTTON - 2.0), "just outside");
}

/// The options button runs the tree menu's Project Options for the project whose card
/// it is on, not the active one.
#[test]
fn the_card_options_button_opens_the_clicked_projects_options_tab() {
    use crate::app::Message;
    use crate::dock::DockMessage;
    let (mut app, _boot) = crate::app::Signex::new();
    let active = add_project(&mut app, "A");
    let other = add_project(&mut app, "B");
    app.document_state.active_project = Some(active);
    app.refresh_panel_ctx();
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::Projects(
        ProjectsMsg::OptionsPressed(other),
    ))));
    let opened: Vec<_> = app
        .document_state
        .tabs
        .iter()
        .filter_map(|tab| tab.kind.as_project())
        .collect();
    assert_eq!(opened, [other], "the clicked project's tab");
}

/// Mockup row 19: with one project its groups sit directly under the card,
/// with no row repeating the project's name, and a row still routes by its
/// full key.
#[test]
fn one_projects_groups_sit_directly_under_its_card() {
    let (mut app, _boot) = crate::app::Signex::new();
    add_project(&mut app, "solo");
    app.refresh_panel_ctx();
    let ctx = app.document_state.panel_ctx.clone();
    let shown = texts(&ctx);
    let names = shown.iter().filter(|t| t.as_str() == "solo").count();
    assert_eq!(names, 1, "the card alone names the project: {shown:#?}");
    let (rows, root) = under_card(super::super::graphite::decorate_tree(&regroup(
        &ctx.project_tree,
        &ctx.projects_view,
    )));
    assert_eq!(root.as_deref(), Some(ctx.project_tree[0].key.as_str()));
    assert_eq!(rows[0].key, SCHEMATICS);
    assert!(rows[0].badge.is_some(), "the group keeps its count");
    let key = vec![SCHEMATICS.to_string(), "solo.snxsch".to_string()];
    let sent = route_under(root.as_deref(), TreeMsg::Select(key));
    let want = vec![
        ctx.project_tree[0].key.clone(),
        SOURCE.to_string(),
        "solo.snxsch".to_string(),
    ];
    assert!(matches!(sent, PanelMsg::Tree(TreeMsg::Select(k)) if k == want));
}

/// With several projects each keeps its row: it says whose groups follow.
#[test]
fn several_projects_keep_their_rows() {
    let (mut app, _boot) = crate::app::Signex::new();
    add_project(&mut app, "A");
    add_project(&mut app, "B");
    app.refresh_panel_ctx();
    let ctx = app.document_state.panel_ctx.clone();
    let (rows, root) = under_card(ctx.project_tree.clone());
    assert_eq!(rows.len(), 2);
    assert_eq!(root, None);
    let key = [rows[0].key.clone()];
    assert_eq!(key_under_card(&key, None), Some(&key[..]));
}

/// The project's own key names no drawn row once its card stands in for it.
#[test]
fn a_selected_project_key_highlights_no_row_under_the_card() {
    let root = ["p".to_string()];
    assert_eq!(key_under_card(&root, Some("p")), None);
    let file = ["p".to_string(), SCHEMATICS.to_string(), "a".to_string()];
    assert_eq!(key_under_card(&file, Some("p")), Some(&file[1..]));
}

/// The project menu (mockup row 20) stays reachable with no project row: a
/// right-click on the card opens it for that project.
#[test]
fn a_right_click_on_the_card_opens_the_project_menu() {
    use iced::mouse::{Button, Event};
    let (mut app, _boot) = crate::app::Signex::new();
    let id = add_project(&mut app, "A");
    app.refresh_panel_ctx();
    let ctx = app.document_state.panel_ctx.clone();
    let sent = crate::layout_probe::messages_for_events(
        view(&ctx),
        WINDOW,
        iced::Point::new(24.0, 24.0),
        &[iced::Event::Mouse(Event::ButtonPressed(Button::Right))],
    );
    let want = vec![id.to_string()];
    assert!(
        sent.iter()
            .any(|m| matches!(m, PanelMsg::Tree(TreeMsg::ContextMenu(k)) if *k == want)),
        "{sent:?}"
    );
}

/// Collapse All folds the view-only groups and Expand All opens them.
#[test]
fn expand_and_collapse_all_reach_the_groups() {
    let ctx = context("en-US");
    let mut state = ProjectsView::default();
    state.set_groups_collapsed(&ctx.project_tree, true);
    let group = |roots: Vec<TreeNode>| {
        roots[0]
            .children
            .iter()
            .find(|g| g.key == SCHEMATICS)
            .map(|g| g.expanded)
    };
    assert_eq!(group(regroup(&ctx.project_tree, &state)), Some(false));
    state.set_groups_collapsed(&ctx.project_tree, false);
    assert_eq!(group(regroup(&ctx.project_tree, &state)), Some(true));
}

/// A context with no project open and two recent projects.
fn empty_context() -> PanelContext {
    let mut ctx = context("en-US");
    ctx.project_tree.clear();
    ctx.recent_projects = vec![
        std::path::PathBuf::from("/w/dcdc-buck/dcdc-buck.snxprj"),
        std::path::PathBuf::from("/w/sensor-node/sensor-node.snxprj"),
    ];
    ctx
}

/// Mockup row 19: no project open keeps the two lines and
/// adds New Project…, Open… and the recent projects by name.
#[test]
fn the_empty_panel_offers_new_open_and_the_recent_projects() {
    let ctx = empty_context();
    let shown = texts_of(super::super::view_projects(&ctx));
    for line in [
        "No project open",
        "File > Open to begin",
        "New Project…",
        "Open…",
        "RECENT PROJECTS",
        "dcdc-buck",
        "sensor-node",
    ] {
        assert!(shown.iter().any(|t| t == line), "{line:?} in {shown:#?}");
    }
    let sent = |label: &str, want: ProjectsMsg| {
        let got = messages_on_click(super::super::view_projects(&ctx), WINDOW, label);
        assert!(
            got.iter()
                .any(|m| matches!(m, PanelMsg::Projects(msg) if *msg == want)),
            "{label:?} did not send {want:?}"
        );
    };
    sent("New Project…", ProjectsMsg::NewProjectPressed);
    sent("Open…", ProjectsMsg::OpenPressed);
    sent("RECENT PROJECTS", ProjectsMsg::RecentToggled);
    sent(
        "sensor-node",
        ProjectsMsg::RecentProjectPressed(ctx.recent_projects[1].clone()),
    );
}

/// The Recent projects header folds the list; with no recent project the
/// section is left out.
#[test]
fn the_recent_list_folds_and_hides_when_empty() {
    let mut ctx = empty_context();
    ctx.projects_view.apply(&ProjectsMsg::RecentToggled);
    let folded = texts_of(super::super::view_projects(&ctx));
    assert!(folded.iter().any(|t| t == "RECENT PROJECTS"), "{folded:#?}");
    assert!(!folded.iter().any(|t| t == "dcdc-buck"), "{folded:#?}");
    ctx.recent_projects.clear();
    let none = texts_of(super::super::view_projects(&ctx));
    assert!(!none.iter().any(|t| t == "RECENT PROJECTS"), "{none:#?}");
    assert!(none.iter().any(|t| t == "No project open"), "{none:#?}");
}

/// A recent row opens through File ▸ Open Recent: a project that is gone is
/// dropped from the list with a notice, as on the Start screen, and the
/// panel's copy of the list follows.
#[test]
fn a_recent_row_opens_through_open_recent() {
    use crate::app::Message;
    use crate::dock::DockMessage;
    let (mut app, _boot) = crate::app::Signex::new();
    let gone = std::path::PathBuf::from("/no/such/dir/gone.snxprj");
    app.ui_state.recent_projects.clear();
    app.ui_state.recent_projects.record(&gone);
    let listed = app.ui_state.recent_projects.entries()[0].clone();
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::Projects(
        ProjectsMsg::RecentProjectPressed(listed),
    ))));
    assert!(app.ui_state.recent_projects.entries().is_empty());
    assert!(app.document_state.error_notice.is_some(), "no notice");
    assert!(
        app.document_state.panel_ctx.recent_projects.is_empty(),
        "the panel still lists the dropped project"
    );
}
