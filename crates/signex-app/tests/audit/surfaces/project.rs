//! Mockup group `project` (rows m19-m39).

use crate::fixture::{self, Fixture};
use crate::render::{self, Crop};
use crate::settle_support;
use crate::surface::{Check, Surface, WindowSel};
use signex_app::app::{
    ContextMenuMsg, ContextSubmenu, DocumentSetupMsg, EditMsg, EnableVersionControlMsg, ExportMsg,
    Message, NewProjectMsg, OverlayMsg, ProjectCloseChoice, ProjectId, ProjectMsg, ProjectSection,
    ProjectTabMsg, ProjectTreeAction, RemoveMsg, RenameMsg, SetupKind, Signex, Tool,
};
use signex_app::canvas::CanvasEvent;
use signex_app::dock::PanelPosition;
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::PanelKind;
use signex_app::toolbar::ToolMessage;
use signex_widgets::tree_view::{self, TreeNode};
use std::path::{Path, PathBuf};

/// The left dock, where the Projects panel sits.
const LEFT_DOCK: Crop = Crop {
    x: 0,
    y: 0,
    w: 560,
    h: 900,
};
/// The left dock and the menus that open beside it.
const TREE_MENU: Crop = Crop {
    x: 0,
    y: 0,
    w: 760,
    h: 760,
};
/// The right dock, where the History panel sits.
const RIGHT_DOCK: Crop = Crop {
    x: 880,
    y: 0,
    w: 560,
    h: 900,
};

pub fn rows() -> Vec<Surface> {
    let mut rows = dialogs();
    rows.extend(project_tab());
    rows
}

/// m19-m28 and m39: the Projects panel, its menu, the dialogs and History.
fn dialogs() -> Vec<Surface> {
    vec![
        Surface {
            id: "m19",
            title: "Projects panel",
            // The mock shows the active sheet edited (its dirty mark).
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                edit_sample(app, r);
                fixture::select_panel(app, PanelKind::Projects)
            },
            window: WindowSel::MainCrop(LEFT_DOCK),
            checks: PROJECTS_PANEL,
        },
        Surface {
            id: "m20",
            title: "Project tree context menu",
            // Main state: the project root menu with Add New open. The
            // document-leaf and library menus are the mock's other states.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                fixture::select_panel(app, PanelKind::Projects)?;
                app.interaction_state.last_mouse_pos = (160.0, 140.0);
                let root = root_key(app)?;
                let _ = app.update(Message::ContextMenu(ContextMenuMsg::ShowProjectTree(Some(
                    root,
                ))));
                let _ = app.update(Message::ContextMenu(ContextMenuMsg::SubmenuOpen(
                    ContextSubmenu::AddNewToProject,
                )));
                Ok(())
            },
            window: WindowSel::MainCrop(TREE_MENU),
            checks: TREE_MENU_CHECKS,
        },
        Surface {
            id: "m21",
            title: "New Project setup",
            open: |app, _, fx| {
                settle_support::settle(app, Message::Menu(MenuMessage::NewProject));
                // A location inside the fixture copy, so the target check
                // never reads the user's own folders.
                let location = workspace_dir(fx)?;
                for msg in [
                    NewProjectMsg::NameEdited("dcdc-buck".into()),
                    NewProjectMsg::LocationEdited(location),
                ] {
                    settle_support::settle(app, Message::from(msg));
                }
                Ok(())
            },
            window: WindowSel::Auto,
            checks: NEW_PROJECT,
        },
        Surface {
            id: "m22",
            title: "New PCB / New Schematic setup",
            // Main state: New PCB. New Schematic is the same dialog with
            // `SetupKind::Sheet`.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                settle_support::settle(
                    app,
                    Message::from(DocumentSetupMsg::OpenRequested {
                        kind: SetupKind::Pcb,
                        project: None,
                    }),
                );
                Ok(())
            },
            window: WindowSel::Auto,
            checks: DOCUMENT_SETUP,
        },
        Surface {
            id: "m23",
            title: "Rename sheet",
            // The project root's "Rename Project" is the other state.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                let leaf = sheet_key(app)?;
                tree_action(app, ProjectTreeAction::OpenRenameDialog(leaf));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: RENAME,
        },
        Surface {
            id: "m24",
            title: "Remove from project",
            // The mock's sample is a child sheet placed by one sheet symbol;
            // the sample project has only its root sheet, so that one is used.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                let leaf = sheet_key(app)?;
                tree_action(app, ProjectTreeAction::OpenRemoveDialog(leaf));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: REMOVE,
        },
        Surface {
            id: "m25",
            title: "Close Project - Unsaved Edits",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                edit_sample(app, r);
                let root = root_key(app)?;
                // Close resolves the child sheets on a task; settle runs it
                // through to the prompt.
                tree_action(app, ProjectTreeAction::CloseProject(root));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: CLOSE_CONFIRM,
        },
        Surface {
            id: "m26",
            title: "Enable Version Control",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                open_enable_version_control(app)
            },
            window: WindowSel::Auto,
            checks: ENABLE_VC,
        },
        Surface {
            id: "m27",
            title: "Create Sheet (child sheet file)",
            // Main state: a new file. The "name already exists" state needs a
            // second sheet file beside the parent.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                place_sheet_symbol(app);
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: CREATE_SHEET,
        },
        Surface {
            id: "m28",
            title: "Netlist incomplete - export anyway?",
            // A placed sheet symbol whose file was never created leaves a
            // page out; the netlist export then asks. The save path stands in
            // for the native file dialog's answer.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                place_sheet_symbol(app);
                if app.ui_state.create_child_sheet_confirm.is_none() {
                    return Err("placing a sheet symbol opened no Create Sheet dialog".into());
                }
                settle_support::settle(
                    app,
                    Message::Project(ProjectMsg::CreateChildSheetAnswered { create: false }),
                );
                settle_support::settle(
                    app,
                    Message::Export(ExportMsg::NetlistFinished {
                        source: fx.sample.with_extension("snxsch"),
                        result: Ok(fx.sample.with_extension("net")),
                    }),
                );
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: NETLIST_PROMPT,
        },
        Surface {
            id: "m39",
            title: "History panel",
            // A repository is started first so the panel lists the first
            // commit; the branch switcher and the merge dialog are other
            // states. The commit needs a git identity where the audit runs.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                open_enable_version_control(app)?;
                settle_support::settle(
                    app,
                    Message::EnableVersionControl(EnableVersionControlMsg::Confirm),
                );
                settle_support::settle(
                    app,
                    Message::Overlay(OverlayMsg::OpenPanel(PanelKind::History)),
                );
                fixture::select_panel(app, PanelKind::History)?;
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::MainCrop(RIGHT_DOCK),
            checks: HISTORY,
        },
    ]
}

/// m29-m38: the Project tab, one row per section.
fn project_tab() -> Vec<Surface> {
    vec![
        Surface {
            id: "m29",
            title: "Project tab: General",
            open: |app, r, fx| open_section(app, r, fx, ProjectSection::General),
            window: WindowSel::Main,
            checks: GENERAL,
        },
        Surface {
            id: "m30",
            title: "Project tab: Parameters",
            open: |app, r, fx| {
                open_section(app, r, fx, ProjectSection::Parameters)?;
                // One custom variable, so the Variables group has a row.
                let project_id = first_project(app)?;
                for msg in [
                    ProjectTabMsg::NewParameterNameEdited {
                        project_id,
                        text: "REVISION".into(),
                    },
                    ProjectTabMsg::ParameterAdded { project_id },
                    ProjectTabMsg::ParameterEdited {
                        project_id,
                        name: "REVISION".into(),
                        value: "B".into(),
                    },
                ] {
                    tab_msg(app, msg);
                }
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::Main,
            checks: PARAMETERS,
        },
        Surface {
            id: "m31",
            title: "Project tab: Sheets",
            open: |app, r, fx| open_section(app, r, fx, ProjectSection::Sheets),
            window: WindowSel::Main,
            checks: SHEETS,
        },
        Surface {
            id: "m32",
            title: "Variant Manager",
            // The Variants section is the Variant Manager. One variant is
            // added so the list and the matrix have a column past Base.
            open: |app, r, fx| {
                open_section(app, r, fx, ProjectSection::Variants)?;
                let project_id = first_project(app)?;
                tab_msg(app, ProjectTabMsg::NewVariantRequested { project_id });
                tab_msg(app, ProjectTabMsg::VariantRenameSubmitted { project_id });
                settle_support::settle(
                    app,
                    Message::Project(ProjectMsg::Tab(ProjectTabMsg::VariantPartsReadRequested {
                        project_id,
                    })),
                );
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::Main,
            checks: VARIANTS,
        },
        Surface {
            id: "m33",
            title: "Project tab: Libraries",
            // The sample project mounts no library, so the table is empty.
            open: |app, r, fx| open_section(app, r, fx, ProjectSection::Libraries),
            window: WindowSel::Main,
            checks: LIBRARIES,
        },
        Surface {
            id: "m34",
            title: "Project tab: ERC & Annotation",
            // The sample stores no ERC rules: it uses this machine's.
            open: |app, r, fx| open_section(app, r, fx, ProjectSection::ErcAnnotation),
            window: WindowSel::Main,
            checks: ERC,
        },
        Surface {
            id: "m35",
            title: "Project tab: Version Control",
            // Main state: a folder with no repository. The other state is
            // the folder after Enable Version Control.
            open: |app, r, fx| open_section(app, r, fx, ProjectSection::VersionControl),
            window: WindowSel::Main,
            checks: VERSION_CONTROL,
        },
        Surface {
            id: "m36",
            title: "Project tab: Schematic Defaults",
            open: |app, r, fx| open_section(app, r, fx, ProjectSection::SchematicDefaults),
            window: WindowSel::Main,
            checks: SCHEMATIC_DEFAULTS,
        },
        Surface {
            id: "m37",
            title: "Project tab: Setup",
            open: |app, r, fx| open_section(app, r, fx, ProjectSection::Setup),
            window: WindowSel::Main,
            checks: SETUP,
        },
        Surface {
            id: "m38",
            title: "Project tab: Requirements",
            // One requirement per kind. The mock shows them after a run (4
            // passed, 2 failed); Run is a `RequirementsMsg`, which the crate
            // does not export, so the cards are captured before a run.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                let project = app
                    .document_state
                    .projects
                    .first_mut()
                    .ok_or("the sample project is not loaded")?;
                for kind in signex_types::project::REQUIREMENT_KINDS {
                    let entry = serde_json::from_value(serde_json::json!({
                        "id": kind,
                        "kind": kind,
                    }))
                    .map_err(|e| format!("requirement {kind}: {e}"))?;
                    project.data.requirements.push(entry);
                }
                show_section(app, r, ProjectSection::Requirements)
            },
            window: WindowSel::Main,
            checks: REQUIREMENTS,
        },
    ]
}

// ---- open helpers ---------------------------------------------------------

fn tree_action(app: &mut Signex, action: ProjectTreeAction) {
    settle_support::settle(
        app,
        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(action)),
    );
}

fn tab_msg(app: &mut Signex, msg: ProjectTabMsg) {
    let _ = app.update(Message::Project(ProjectMsg::Tab(msg)));
}

fn first_project(app: &Signex) -> Result<ProjectId, String> {
    app.document_state
        .projects
        .first()
        .map(|p| p.id)
        .ok_or_else(|| "the sample project is not loaded".to_string())
}

/// The tree key of the first project's root row.
fn root_key(app: &Signex) -> Result<Vec<String>, String> {
    Ok(vec![first_project(app)?.to_string()])
}

/// The tree key of the sample's root sheet row.
fn sheet_key(app: &Signex) -> Result<Vec<String>, String> {
    let tree = &app.document_state.panel_ctx.project_tree;
    let path = find_row(tree, "linear-regulator", 0, &mut Vec::new())
        .ok_or("no sheet row for linear-regulator in the Projects tree")?;
    tree_view::key_of(tree, &path).ok_or_else(|| "the sheet row has no key".to_string())
}

/// Index path of the first leaf below the roots whose label names `stem`.
fn find_row(
    nodes: &[TreeNode],
    stem: &str,
    depth: usize,
    prefix: &mut Vec<usize>,
) -> Option<Vec<usize>> {
    for (i, n) in nodes.iter().enumerate() {
        prefix.push(i);
        if depth > 0 && n.children.is_empty() && n.label.contains(stem) {
            return Some(prefix.clone());
        }
        if let Some(found) = find_row(&n.children, stem, depth + 1, prefix) {
            return Some(found);
        }
        prefix.pop();
    }
    None
}

/// The folder holding the sample copy, inside the fixture's temp dir.
fn workspace_dir(fx: &Fixture) -> Result<String, String> {
    fx.sample
        .parent()
        .and_then(Path::parent)
        .map(|dir| dir.display().to_string())
        .ok_or_else(|| "the sample copy has no parent folder".to_string())
}

/// Select U1 and rotate it: a real edit, so the sheet is dirty.
fn edit_sample(app: &mut Signex, r: &mut iced::Renderer) {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: 88.9,
        world_y: 101.6,
    }));
    let _ = app.update(Message::Edit(EditMsg::RotateSelectedClockwise));
    let _ = render::draw_main(r, app);
}

fn open_enable_version_control(app: &mut Signex) -> Result<(), String> {
    let root = root_key(app)?;
    tree_action(app, ProjectTreeAction::OpenEnableVersionControl(root));
    Ok(())
}

/// Place a sheet symbol in an empty corner of the sample with the Place
/// Sheet tool; its file does not exist, so Create Sheet opens.
fn place_sheet_symbol(app: &mut Signex) {
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Sheet)));
    for (x, y) in [(180.0, 40.0), (220.0, 70.0)] {
        settle_support::settle(
            app,
            Message::CanvasEvent(CanvasEvent::Clicked {
                world_x: x,
                world_y: y,
            }),
        );
    }
}

/// Open the sample project's Project tab at `section`.
fn open_section(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
    section: ProjectSection,
) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    show_section(app, r, section)
}

/// Project Options... on the loaded project, then `section` picked.
fn show_section(
    app: &mut Signex,
    r: &mut iced::Renderer,
    section: ProjectSection,
) -> Result<(), String> {
    let project_id = first_project(app)?;
    tree_action(
        app,
        ProjectTreeAction::OpenProjectOptions(vec![project_id.to_string()]),
    );
    settle_support::settle(
        app,
        Message::Project(ProjectMsg::Tab(ProjectTabMsg::SectionSelected {
            project_id,
            section,
        })),
    );
    let _ = render::draw_main(r, app);
    Ok(())
}

// ---- checks ---------------------------------------------------------------

const PROJECTS_PANEL: &[Check] = &[
    ("the tree lists the project", tree_has_rows),
    ("the edited sheet is dirty", sample_dirty),
];

const TREE_MENU_CHECKS: &[Check] = &[("the tree menu is open", tree_menu_open)];

const NEW_PROJECT: &[Check] = &[
    ("the New Project dialog is open", new_project_open),
    ("Cancel closes it and makes nothing", new_project_cancel),
];

const DOCUMENT_SETUP: &[Check] = &[
    ("the New PCB dialog is open", document_setup_open),
    ("Cancel closes it", document_setup_cancel),
];

const RENAME: &[Check] = &[
    ("the Rename dialog targets the sheet", rename_targets_sheet),
    ("typing changes the name", rename_buffer_follows),
];

const REMOVE: &[Check] = &[
    ("the Remove dialog targets the sheet", remove_targets_sheet),
    ("Close keeps the sheet on disk", remove_close),
];

const CLOSE_CONFIRM: &[Check] = &[
    ("the unsaved-edits prompt is up", close_confirm_open),
    ("Cancel keeps the project open", close_confirm_cancel),
];

const ENABLE_VC: &[Check] = &[
    ("the Enable Version Control dialog is open", enable_vc_open),
    ("Close dismisses it", enable_vc_close),
];

const CREATE_SHEET: &[Check] = &[
    ("the Create Sheet dialog is open", create_sheet_open),
    ("Later closes it and writes nothing", create_sheet_later),
];

const NETLIST_PROMPT: &[Check] = &[
    ("the incomplete-netlist prompt is up", netlist_prompt_open),
    ("nothing is written before a choice", netlist_not_written),
    ("Cancel clears the prompt", netlist_cancel),
];

const HISTORY: &[Check] = &[("the History panel is docked", history_docked)];

const GENERAL: &[Check] = &[
    ("the General section is shown", general_shown),
    (
        "a description edit dirties the project",
        description_dirties,
    ),
];

const PARAMETERS: &[Check] = &[
    ("the Parameters section is shown", parameters_shown),
    ("the filter keeps what was typed", parameter_filter_kept),
];

const SHEETS: &[Check] = &[("the Sheets section is shown", sheets_shown)];

const VARIANTS: &[Check] = &[
    ("the Variants section is shown", variants_shown),
    ("New variant adds a variant", new_variant_adds),
];

const LIBRARIES: &[Check] = &[("the Libraries section is shown", libraries_shown)];

const ERC: &[Check] = &[("the ERC section is shown", erc_shown)];

const VERSION_CONTROL: &[Check] = &[
    (
        "the Version control section is shown",
        version_control_shown,
    ),
    (
        "Enable Version Control... opens the dialog",
        vc_section_opens_dialog,
    ),
];

const SCHEMATIC_DEFAULTS: &[Check] = &[
    ("the Schematic defaults section is shown", defaults_shown),
    ("a typed wire width is kept", wire_width_kept),
];

const SETUP: &[Check] = &[
    ("the Project setup section is shown", setup_shown),
    ("a card pick drafts the setup", setup_card_drafts),
];

const REQUIREMENTS: &[Check] = &[("the Requirements section is shown", requirements_shown)];

fn tree_has_rows(app: &mut Signex) -> Result<(), String> {
    if app.document_state.panel_ctx.project_tree.is_empty() {
        Err("the Projects tree is empty".into())
    } else {
        Ok(())
    }
}

fn sample_dirty(app: &mut Signex) -> Result<(), String> {
    let path = app
        .document_state
        .tabs
        .get(app.document_state.active_tab)
        .map(|t| t.path.clone())
        .ok_or("no active tab")?;
    if app.document_state.is_document_dirty(&path) {
        Ok(())
    } else {
        Err(format!("{} is not dirty after the rotate", path.display()))
    }
}

fn tree_menu_open(app: &mut Signex) -> Result<(), String> {
    if app.interaction_state.project_tree_context_menu.is_some() {
        Ok(())
    } else {
        Err("project_tree_context_menu is None".into())
    }
}

fn new_project_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.new_project.is_some() {
        Ok(())
    } else {
        Err("ui_state.new_project is None".into())
    }
}

fn new_project_cancel(app: &mut Signex) -> Result<(), String> {
    settle_support::settle(app, Message::from(NewProjectMsg::CancelRequested));
    if app.ui_state.new_project.is_some() {
        return Err("the dialog is still open after Cancel".into());
    }
    match app.document_state.projects.len() {
        0 => Ok(()),
        n => Err(format!("{n} projects are open after Cancel")),
    }
}

fn document_setup_open(app: &mut Signex) -> Result<(), String> {
    match app.ui_state.document_setup.as_ref() {
        Some(state) if state.kind == SetupKind::Pcb => Ok(()),
        Some(state) => Err(format!("the setup dialog is for {:?}", state.kind)),
        None => Err("ui_state.document_setup is None".into()),
    }
}

fn document_setup_cancel(app: &mut Signex) -> Result<(), String> {
    settle_support::settle(app, Message::from(DocumentSetupMsg::CancelRequested));
    if app.ui_state.document_setup.is_some() {
        Err("the dialog is still open after Cancel".into())
    } else {
        Ok(())
    }
}

/// The sample sheet's path, beside the loaded project file.
fn sample_sheet(app: &Signex) -> Result<PathBuf, String> {
    let project = app
        .document_state
        .projects
        .first()
        .ok_or("the sample project is not loaded")?;
    Ok(project.path.with_extension("snxsch"))
}

/// `got` names the sample sheet; compared by file name, so a canonical
/// spelling of the same folder still matches.
fn is_sample_sheet(app: &Signex, got: &Path) -> Result<(), String> {
    let want = sample_sheet(app)?;
    if got.file_name() == want.file_name() {
        Ok(())
    } else {
        Err(format!("the dialog targets {}", got.display()))
    }
}

fn rename_targets_sheet(app: &mut Signex) -> Result<(), String> {
    let target = app
        .ui_state
        .rename_dialog
        .as_ref()
        .map(|d| d.target_path.clone())
        .ok_or("ui_state.rename_dialog is None")?;
    is_sample_sheet(app, &target)
}

fn rename_buffer_follows(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Rename(RenameMsg::BufferChanged("power".into())));
    match app.ui_state.rename_dialog.as_ref() {
        Some(d) if d.buffer == "power" => Ok(()),
        Some(d) => Err(format!("the name box reads {:?}", d.buffer)),
        None => Err("the dialog closed on typing".into()),
    }
}

fn remove_targets_sheet(app: &mut Signex) -> Result<(), String> {
    let target = app
        .ui_state
        .remove_dialog
        .as_ref()
        .map(|d| d.target_path.clone())
        .ok_or("ui_state.remove_dialog is None")?;
    is_sample_sheet(app, &target)
}

fn remove_close(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Remove(RemoveMsg::Close));
    if app.ui_state.remove_dialog.is_some() {
        return Err("the dialog is still open after Close".into());
    }
    let sheet = sample_sheet(app)?;
    if sheet.is_file() {
        Ok(())
    } else {
        Err(format!("{} is gone after Close", sheet.display()))
    }
}

fn close_confirm_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.project_close_confirm.is_some() {
        Ok(())
    } else {
        Err("ui_state.project_close_confirm is None".into())
    }
}

fn close_confirm_cancel(app: &mut Signex) -> Result<(), String> {
    settle_support::settle(
        app,
        Message::Project(ProjectMsg::CloseConfirm(ProjectCloseChoice::Cancel)),
    );
    if app.ui_state.project_close_confirm.is_some() {
        return Err("the prompt is still up after Cancel".into());
    }
    match app.document_state.projects.len() {
        1 => Ok(()),
        n => Err(format!("{n} projects are open after Cancel")),
    }
}

fn enable_vc_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.enable_version_control.is_some() {
        Ok(())
    } else {
        Err("ui_state.enable_version_control is None".into())
    }
}

fn enable_vc_close(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::EnableVersionControl(
        EnableVersionControlMsg::Close,
    ));
    if app.ui_state.enable_version_control.is_some() {
        Err("the dialog is still open after Close".into())
    } else {
        Ok(())
    }
}

fn create_sheet_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.create_child_sheet_confirm.is_some() {
        Ok(())
    } else {
        Err("ui_state.create_child_sheet_confirm is None".into())
    }
}

fn create_sheet_later(app: &mut Signex) -> Result<(), String> {
    let planned = {
        let c = app
            .ui_state
            .create_child_sheet_confirm
            .as_ref()
            .ok_or("no Create Sheet dialog")?;
        c.parent.parent().map(|dir| dir.join(&c.path_text))
    };
    settle_support::settle(
        app,
        Message::Project(ProjectMsg::CreateChildSheetAnswered { create: false }),
    );
    if app.ui_state.create_child_sheet_confirm.is_some() {
        return Err("the dialog is still open after Later".into());
    }
    match planned {
        Some(path) if path.exists() => Err(format!("Later wrote {}", path.display())),
        _ => Ok(()),
    }
}

fn netlist_prompt_open(app: &mut Signex) -> Result<(), String> {
    match app.document_state.netlist_incomplete_prompt.as_ref() {
        Some(p) if !p.messages.is_empty() => Ok(()),
        Some(_) => Err("the prompt names no missing page".into()),
        None => Err("document_state.netlist_incomplete_prompt is None".into()),
    }
}

fn netlist_not_written(app: &mut Signex) -> Result<(), String> {
    let prompt = app
        .document_state
        .netlist_incomplete_prompt
        .as_ref()
        .ok_or("no prompt")?;
    if prompt.save_path.exists() {
        Err(format!("{} was written", prompt.save_path.display()))
    } else {
        Ok(())
    }
}

fn netlist_cancel(app: &mut Signex) -> Result<(), String> {
    settle_support::settle(app, Message::Export(ExportMsg::NetlistCancelIncomplete));
    if app.document_state.netlist_incomplete_prompt.is_some() {
        Err("the prompt is still up after Cancel".into())
    } else {
        Ok(())
    }
}

fn history_docked(app: &mut Signex) -> Result<(), String> {
    let docked = [
        PanelPosition::Left,
        PanelPosition::Right,
        PanelPosition::Bottom,
    ]
    .into_iter()
    .any(|pos| {
        app.document_state
            .dock
            .panel_kinds(pos)
            .contains(&PanelKind::History)
    });
    if docked {
        Ok(())
    } else {
        Err("no History tab in any dock".into())
    }
}

/// The section the first project's tab shows is `want`.
fn section_is(app: &Signex, want: ProjectSection) -> Result<(), String> {
    let id = first_project(app)?;
    match app.document_state.project_tabs.get(&id) {
        Some(tab) if tab.section == want => Ok(()),
        Some(tab) => Err(format!("the tab shows {:?}, not {want:?}", tab.section)),
        None => Err("the project has no Project tab state".into()),
    }
}

fn general_shown(app: &mut Signex) -> Result<(), String> {
    section_is(app, ProjectSection::General)
}

fn parameters_shown(app: &mut Signex) -> Result<(), String> {
    section_is(app, ProjectSection::Parameters)
}

fn sheets_shown(app: &mut Signex) -> Result<(), String> {
    section_is(app, ProjectSection::Sheets)
}

fn variants_shown(app: &mut Signex) -> Result<(), String> {
    section_is(app, ProjectSection::Variants)
}

fn libraries_shown(app: &mut Signex) -> Result<(), String> {
    section_is(app, ProjectSection::Libraries)
}

fn erc_shown(app: &mut Signex) -> Result<(), String> {
    section_is(app, ProjectSection::ErcAnnotation)
}

fn version_control_shown(app: &mut Signex) -> Result<(), String> {
    section_is(app, ProjectSection::VersionControl)
}

fn defaults_shown(app: &mut Signex) -> Result<(), String> {
    section_is(app, ProjectSection::SchematicDefaults)
}

fn setup_shown(app: &mut Signex) -> Result<(), String> {
    section_is(app, ProjectSection::Setup)
}

fn requirements_shown(app: &mut Signex) -> Result<(), String> {
    section_is(app, ProjectSection::Requirements)
}

fn project_dirty(app: &Signex) -> bool {
    app.document_state
        .projects
        .first()
        .is_some_and(|p| app.document_state.is_document_dirty(&p.path))
}

fn description_dirties(app: &mut Signex) -> Result<(), String> {
    let project_id = first_project(app)?;
    tab_msg(
        app,
        ProjectTabMsg::DescriptionEdited {
            project_id,
            text: "Buck converter, 12 V to 3V3".into(),
        },
    );
    if project_dirty(app) {
        Ok(())
    } else {
        Err("the project is not dirty after a description edit".into())
    }
}

fn parameter_filter_kept(app: &mut Signex) -> Result<(), String> {
    let project_id = first_project(app)?;
    tab_msg(
        app,
        ProjectTabMsg::ParameterFilterEdited {
            project_id,
            text: "REV".into(),
        },
    );
    match app.document_state.project_tabs.get(&project_id) {
        Some(tab) if tab.parameter_filter == "REV" => Ok(()),
        Some(tab) => Err(format!("the filter reads {:?}", tab.parameter_filter)),
        None => Err("the project has no Project tab state".into()),
    }
}

fn new_variant_adds(app: &mut Signex) -> Result<(), String> {
    let project_id = first_project(app)?;
    let before = app.document_state.projects[0]
        .data
        .variant_definitions
        .len();
    tab_msg(app, ProjectTabMsg::NewVariantRequested { project_id });
    let after = app.document_state.projects[0]
        .data
        .variant_definitions
        .len();
    if after == before + 1 {
        Ok(())
    } else {
        Err(format!(
            "{before} variants before New variant, {after} after"
        ))
    }
}

fn vc_section_opens_dialog(app: &mut Signex) -> Result<(), String> {
    let project_id = first_project(app)?;
    settle_support::settle(
        app,
        Message::Project(ProjectMsg::Tab(
            ProjectTabMsg::EnableVersionControlRequested { project_id },
        )),
    );
    enable_vc_open(app)
}

fn wire_width_kept(app: &mut Signex) -> Result<(), String> {
    use signex_types::project::SchematicDefaultField;
    let project_id = first_project(app)?;
    tab_msg(
        app,
        ProjectTabMsg::SchematicDefaultEdited {
            project_id,
            field: SchematicDefaultField::WireWidth,
            text: "0.25".into(),
        },
    );
    let drafted = app
        .document_state
        .project_tabs
        .get(&project_id)
        .and_then(|tab| tab.default_drafts.get(&SchematicDefaultField::WireWidth))
        .is_some_and(|text| text == "0.25");
    if drafted || project_dirty(app) {
        Ok(())
    } else {
        Err("the typed wire width was neither drafted nor stored".into())
    }
}

fn setup_card_drafts(app: &mut Signex) -> Result<(), String> {
    let project_id = first_project(app)?;
    tab_msg(
        app,
        ProjectTabMsg::SetupEdited {
            project_id,
            msg: NewProjectMsg::ProfilePicked(signex_types::project::SetupProfile::Hdi),
        },
    );
    match app.document_state.project_tabs.get(&project_id) {
        Some(tab) if tab.setup_draft.is_some() => Ok(()),
        Some(_) => Err("no setup draft after a card pick".into()),
        None => Err("the project has no Project tab state".into()),
    }
}
