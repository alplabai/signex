//! States that need an open menu or pick list (`WindowSel::MainPointer`),
//! real document changes, or seeded preference data. `open` fails when the
//! state was not reached.

use super::helpers_d::{self, reached};
use super::helpers_f as h;
use super::pointer::PointerStep;
use super::{annot_valid, helpers_c, lib, states};
use crate::fixture::{self, Fixture};
use crate::render::{self, Crop};
use crate::settle_support;
use crate::surface::{OpenFn, Surface, WindowSel};
use signex_app::app::{
    ContextMenuMsg, ContextSubmenu, EditMsg, FileMsg, Message, NetNavMsg, PreferencesMsg, Signex,
    Tool, UiMsg,
};
use signex_app::dock::DockMessage;
use signex_app::keymap::ShortcutTrigger;
use signex_app::library::LibraryMessage;
use signex_app::library::messages::BrowserEditMsg;
use signex_app::panels::{PanelKind, PanelMsg, ProjectsMsg};
use signex_app::preferences::{PrefMsg, PrefNav, ProfilesMsg};
use signex_app::rule_profiles::{ListedProfile, ProfileKind, ProfileListing};
use signex_app::toolbar::ToolMessage;
use signex_app::workspace_layouts::{
    BuiltinLayout, LayoutId, ManageLayoutsMsg, SaveLayoutMsg, WorkspaceLayoutMsg,
    WorkspaceLayoutsStore,
};
use signex_erc::project_rules::profile::builtin_profile;
use signex_types::format::SnxPcb;
use signex_types::pcb::SymbolLink;
use signex_types::quantity::SiPrefix;
use signex_types::schematic::{InstancePath, SelectedItem, SelectedKind};
use signex_widgets::tree_view::{self, TreeNode};
use std::path::Path;
use std::sync::Arc;

/// The status bar along the bottom of the window.
const STATUS_BAR: Crop = Crop {
    x: 0,
    y: render::HEIGHT - 60,
    w: render::WIDTH,
    h: 60,
};

fn row(id: &'static str, title: &'static str, open: OpenFn) -> Surface {
    surface(id, title, open, WindowSel::Main)
}

fn surface(id: &'static str, title: &'static str, open: OpenFn, window: WindowSel) -> Surface {
    Surface {
        id,
        title,
        open,
        window,
        checks: &[],
    }
}

pub fn rows() -> Vec<Surface> {
    let mut rows = pointer_rows();
    rows.extend(chrome_rows());
    rows.extend(preference_rows());
    rows
}

fn pointer_rows() -> Vec<Surface> {
    vec![
        surface(
            "m4-file",
            "File menu open",
            open_sheet,
            WindowSel::MainPointer(&[PointerStep::Click("File")]),
        ),
        surface(
            "m4-reports",
            "Tools > Reports open",
            open_sheet,
            // Reports is the second row of the Tools menu.
            WindowSel::MainPointer(&[PointerStep::Click("Tools"), PointerStep::HoverAt(400, 77)]),
        ),
        surface(
            "m166-menu",
            "View > Window > Layouts open",
            open_layouts_menu,
            // The View menu is taller than 900 px. Window follows 34 rows
            // and 4 separators. Its submenu opens upward from that row, so
            // Layouts is at y 1009. A point outside every open menu closes
            // them all.
            WindowSel::MainPointerTall(
                &[
                    PointerStep::Click("View"),
                    PointerStep::HoverAt(260, 1037),
                    PointerStep::HoverAt(560, 1009),
                ],
                1400,
            ),
        ),
        surface(
            "m139-prefix",
            "Measurement menu open on a prefix-taking unit",
            |app, _, fx| open_measurement(app, fx, "4.7", "ohm", SiPrefix::Kilo),
            WindowSel::Auto,
        ),
        surface(
            "m139-unit",
            "Measurement menu open on a unit without prefixes",
            |app, _, fx| open_measurement(app, fx, "100", "ppm/C", SiPrefix::One),
            WindowSel::Auto,
        ),
    ]
}

fn chrome_rows() -> Vec<Surface> {
    vec![
        row("m5-tools", "Tool document tab", |app, r, fx| {
            annot_valid::open_rules(app, r, fx).map(|_| ())
        }),
        surface(
            "m6-tool",
            "Status bar: tool and live hint",
            |app, r, fx| {
                fixture::open_sample(app, r, fx);
                let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Wire)));
                reached(
                    app.interaction_state.current_tool == Tool::Wire,
                    "the Wire tool did not arm",
                )
            },
            WindowSel::MainCrop(STATUS_BAR),
        ),
        surface(
            "m6-selection",
            "Status bar: selection pill",
            |app, r, fx| {
                helpers_d::open_primitives(app, r, fx)?;
                helpers_d::board_click(app, r, 5.0, 0.0);
                Ok(())
            },
            WindowSel::MainCrop(STATUS_BAR),
        ),
        row("m10-net", "Net hover card", open_net_hover),
        row("m11-disk", "File changed on disk", open_disk_changed),
        row("m19-filter", "Projects filtered", |app, r, fx| {
            open_projects_filter(app, r, fx, "linear")
        }),
        row("m19-no-match", "Projects filter: no match", |app, r, fx| {
            open_projects_filter(app, r, fx, "zzz")
        }),
        // The dock shows only with a document open: a library tab has no
        // project.
        row("m19-no-project", "Projects: none open", |app, _, fx| {
            fixture::mount_stock(app, fx)?;
            settle_support::settle(
                app,
                Message::Library(LibraryMessage::OpenLibraryBrowser(fx.stock.clone())),
            );
            helpers_c::show_panel(app, PanelKind::Projects)
        }),
        row("m20-planned", "Project menu: Planned", |app, r, fx| {
            fixture::open_sample(app, r, fx);
            fixture::select_panel(app, PanelKind::Projects)?;
            app.interaction_state.last_mouse_pos = (160.0, 140.0);
            let root = app
                .document_state
                .projects
                .first()
                .map(|p| p.id.to_string())
                .ok_or("the sample project is not loaded")?;
            tree_menu(app, vec![root], ContextSubmenu::ProjectPlanned)
        }),
        row("m20-library", "Library node menu", |app, r, fx| {
            states::open_libraries(app, r, fx)?;
            fixture::select_panel(app, PanelKind::Projects)?;
            app.interaction_state.last_mouse_pos = (160.0, 260.0);
            let tree = &app.document_state.panel_ctx.project_tree;
            let path = find_label(tree, "Power", &mut Vec::new())
                .ok_or("the Projects tree lists no library row")?;
            let key = tree_view::key_of(tree, &path).ok_or("the library row has no key")?;
            tree_menu(app, key, ContextSubmenu::AddNewToLibrary)
        }),
        row("m134-libraries", "Copy stock: libraries", open_stock_copy),
    ]
}

fn preference_rows() -> Vec<Surface> {
    vec![
        // Preferences is its own window, so these capture with `Auto`.
        surface(
            "m145-recorder",
            "Shortcut recorder",
            open_recorder,
            WindowSel::Auto,
        ),
        surface(
            "m148-profiles",
            "Rule profiles",
            open_profiles,
            WindowSel::Auto,
        ),
        surface(
            "m149-connected",
            "DigiKey connected",
            |app, _, _| open_distributors(app, Some("j.doe@example.com")),
            WindowSel::Auto,
        ),
        surface(
            "m149-not-connected",
            "DigiKey not connected",
            |app, _, _| open_distributors(app, None),
            WindowSel::Auto,
        ),
    ]
}

fn open_sheet(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    Ok(())
}

/// Keeps the layouts this row saves out of `prefs.json`.
struct NoLayoutWrites;

impl WorkspaceLayoutsStore for NoLayoutWrites {
    fn write(&self, _: &serde_json::Value) -> Result<(), String> {
        Ok(())
    }
}

/// The sample with two saved layouts and an edited Schematic built-in on
/// screen, so the submenu has its check mark, Your Layouts and Reset rows.
fn open_layouts_menu(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    let layouts = &mut app.ui_state.workspace_layouts;
    layouts.layouts = Default::default();
    layouts.store = Arc::new(NoLayoutWrites);
    h::unlogged(|| save_layouts(app))?;
    let layouts = &app.ui_state.workspace_layouts.layouts;
    reached(
        layouts.user().len() == 2 && layouts.resettable() == Some(BuiltinLayout::Schematic),
        "the layouts were not saved and the built-in not edited",
    )
}

/// Save two layouts, then store the arrangement as the Schematic built-in's
/// edit and apply it, through the Save and Manage dialogs.
fn save_layouts(app: &mut Signex) -> Result<(), String> {
    for name in ["Bring-up bench", "Dual monitor review"] {
        for msg in [
            SaveLayoutMsg::Requested,
            SaveLayoutMsg::NameEdited(name.into()),
            SaveLayoutMsg::Confirmed,
        ] {
            h::settle(app, msg.into())?;
        }
    }
    let schematic = LayoutId::Builtin(BuiltinLayout::Schematic);
    for msg in [
        ManageLayoutsMsg::Requested,
        ManageLayoutsMsg::RowSelected(schematic),
        ManageLayoutsMsg::UpdateFromCurrentRequested,
        ManageLayoutsMsg::ApplyRequested,
        ManageLayoutsMsg::Closed,
    ] {
        h::settle(app, WorkspaceLayoutMsg::Manage(msg).into())?;
    }
    Ok(())
}

fn inner(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

fn prefs_page(app: &mut Signex, nav: PrefNav) {
    settle_support::settle(app, Message::Preferences(PreferencesMsg::Open));
    settle_support::settle(app, Message::Preferences(PreferencesMsg::Nav(nav)));
}

fn tree_menu(app: &mut Signex, key: Vec<String>, submenu: ContextSubmenu) -> Result<(), String> {
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ShowProjectTree(Some(
        key,
    ))));
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::SubmenuOpen(submenu)));
    reached(
        app.interaction_state.project_tree_context_menu.is_some(),
        "the tree row opened no menu",
    )
}

/// Index path of the first row whose label names `needle`.
fn find_label(nodes: &[TreeNode], needle: &str, prefix: &mut Vec<usize>) -> Option<Vec<usize>> {
    for (i, node) in nodes.iter().enumerate() {
        prefix.push(i);
        if node.label.contains(needle) {
            return Some(prefix.clone());
        }
        if let Some(found) = find_label(&node.children, needle, prefix) {
            return Some(found);
        }
        prefix.pop();
    }
    None
}

/// The Edit row modal with one measurement parameter.
fn open_measurement(
    app: &mut Signex,
    fx: &Fixture,
    value: &str,
    unit: &str,
    prefix: SiPrefix,
) -> Result<(), String> {
    lib::open_edit_row(app, fx)?;
    let before = param_keys(app, fx);
    add_measurement(app, fx, value, unit, prefix)?;
    let key = param_keys(app, fx)
        .into_iter()
        .find(|k| !before.contains(k))
        .ok_or("the measurement row is missing")?;
    settle_support::settle(
        app,
        Message::Library(LibraryMessage::BrowserEdit {
            library_path: fx.stock.clone(),
            msg: BrowserEditMsg::UnitMenuOpened { key },
        }),
    );
    Ok(())
}

/// Mockup row 139's two slots, with the first slot's menu open.
pub(super) fn open_measurement_pair_menu(app: &mut Signex, fx: &Fixture) -> Result<(), String> {
    open_measurement_pair(app, fx)?;
    settle_support::settle(
        app,
        Message::Library(LibraryMessage::BrowserEdit {
            library_path: fx.stock.clone(),
            msg: BrowserEditMsg::UnitMenuOpened {
                key: "value".into(),
            },
        }),
    );
    Ok(())
}

/// The Edit row modal with the two measurement slots mockup row 139 draws.
pub(super) fn open_measurement_pair(app: &mut Signex, fx: &Fixture) -> Result<(), String> {
    lib::open_edit_row(app, fx)?;
    let edit = |msg| {
        Message::Library(LibraryMessage::BrowserEdit {
            library_path: fx.stock.clone(),
            msg,
        })
    };
    for (name, value) in [("value", "4.7"), ("tcr", "100")] {
        for msg in [
            BrowserEditMsg::AddSlotParam { name: name.into() },
            BrowserEditMsg::SetParamValue {
                key: name.into(),
                value: value.into(),
            },
        ] {
            settle_support::settle(app, edit(msg));
        }
    }
    settle_support::settle(
        app,
        edit(BrowserEditMsg::ParamPrefixPicked {
            key: "value".into(),
            prefix: SiPrefix::Kilo,
        }),
    );
    let keys = param_keys(app, fx);
    reached(
        ["value", "tcr"].iter().all(|k| keys.iter().any(|h| h == k)),
        "the Resistor template did not add its value and tcr slots",
    )
}

/// Add a parameter row holding this measurement to the open modal.
fn add_measurement(
    app: &mut Signex,
    fx: &Fixture,
    value: &str,
    unit: &str,
    prefix: SiPrefix,
) -> Result<(), String> {
    let edit = |msg| {
        Message::Library(LibraryMessage::BrowserEdit {
            library_path: fx.stock.clone(),
            msg,
        })
    };
    let before: Vec<String> = param_keys(app, fx);
    settle_support::settle(app, edit(BrowserEditMsg::AddParam));
    let key = param_keys(app, fx)
        .into_iter()
        .find(|k| !before.contains(k))
        .ok_or("Add parameter added no row")?;
    for msg in [
        BrowserEditMsg::SetParamValue {
            key: key.clone(),
            value: value.to_string(),
        },
        BrowserEditMsg::SetParamUnit {
            key: key.clone(),
            unit: unit.to_string(),
        },
        BrowserEditMsg::ParamPrefixPicked { key, prefix },
    ] {
        settle_support::settle(app, edit(msg));
    }
    Ok(())
}

fn param_keys(app: &Signex, fx: &Fixture) -> Vec<String> {
    app.library
        .library_browsers
        .get(&fx.stock)
        .and_then(|browser| browser.edit_modal.as_ref())
        .map(|modal| modal.param_buf.keys().cloned().collect())
        .unwrap_or_default()
}

/// The pointer rested on a wire past the dwell, so the net card answers.
fn open_net_hover(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    helpers_d::sample_project(app, fx);
    let (wire, world) = {
        let wire = app
            .document_state
            .active_engine()
            .ok_or("no active sheet")?
            .document()
            .wires
            .first()
            .ok_or("the sample has no wire")?;
        (
            wire.uuid,
            (
                (wire.start.x + wire.end.x) / 2.0,
                (wire.start.y + wire.end.y) / 2.0,
            ),
        )
    };
    app.interaction_state.last_mouse_pos = (720.0, 420.0);
    app.observe_net_hover(Some(SelectedItem::new(wire, SelectedKind::Wire)), world);
    std::thread::sleep(std::time::Duration::from_millis(500));
    h::settle(app, Message::Ui(UiMsg::NetNav(NetNavMsg::HoverDwellTicked)))?;
    reached(
        app.document_state.net_nav.hover.card.is_some(),
        "the dwell produced no net card",
    )
}

/// An edited sheet whose file changed underneath it, then Save.
fn open_disk_changed(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    h::click(app, 88.9, 101.6);
    let _ = app.update(Message::Edit(EditMsg::RotateSelectedClockwise));
    let sheet = fx.sample.with_extension("snxsch");
    let mut text = std::fs::read_to_string(&sheet).map_err(|e| format!("read sheet: {e}"))?;
    text.push('\n');
    std::fs::write(&sheet, text).map_err(|e| format!("change sheet: {e}"))?;
    h::settle(app, Message::File(FileMsg::Save))?;
    reached(
        app.document_state.error_notice.is_some(),
        "Save did not report the changed file",
    )
}

fn open_projects_filter(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
    filter: &str,
) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    fixture::select_panel(app, PanelKind::Projects)?;
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::Projects(
        ProjectsMsg::FilterChanged(filter.to_string()),
    ))));
    Ok(())
}

/// A second, writable library beside the read-only stock one, so the copy
/// dialog lists where a stock symbol can go.
fn open_stock_copy(app: &mut Signex, _: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let stock_dir = fx.stock.parent().ok_or("stock has no folder")?;
    let copy_dir = stock_dir.with_file_name("passives");
    copy_tree(stock_dir, &copy_dir).map_err(|e| format!("copy library: {e}"))?;
    // A file carrying the stock id is refused as a copy of stock: this one
    // is the user's own library, with its own id and name.
    let library = copy_dir.join("Stock.snxlib");
    let text = std::fs::read_to_string(&library).map_err(|e| format!("read library: {e}"))?;
    let text = text
        .replace(
            &signex_app::library::state::STOCK_LIBRARY_ID.to_string(),
            &uuid::Uuid::now_v7().to_string(),
        )
        .replace("name = \"Stock\"", "name = \"passives\"");
    std::fs::write(&library, text).map_err(|e| format!("write library: {e}"))?;
    app.library.pending_mounts.clear();
    app.library.stock_copy_target = None;
    app.library.stock_library_root = Some(fx.stock.clone());
    app.library
        .open_library_read_only(fx.stock.clone())
        .map_err(|e| format!("mount stock read-only: {e:?}"))?;
    app.library
        .open_library(library)
        .map_err(|e| format!("mount the writable library: {e:?}"))?;
    settle_support::settle(app, Message::File(FileMsg::Opened(Some(fx.symbol.clone()))));
    reached(
        app.library.stock_copy.is_some() && !app.library.writable_libraries().is_empty(),
        "the copy dialog lists no writable library",
    )
}

fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let dest = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &dest)?;
        } else {
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    Ok(())
}

/// The sample with its board's footprints linked to its parts, then a
/// renamed part, a changed value, a changed footprint and a shared
/// designator on the board, as Back Annotate lists them.
pub(super) fn open_back_annotate_changes(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    helpers_d::sample_with_board(app, r, fx)?;
    let parts: Vec<_> = app
        .document_state
        .active_engine()
        .ok_or("no active sheet")?
        .document()
        .symbols
        .iter()
        .filter(|s| !s.reference.starts_with('#'))
        .map(|s| {
            (
                s.uuid,
                s.reference.clone(),
                s.value.clone(),
                s.footprint.clone(),
            )
        })
        .collect();
    let path = fx.sample.with_extension("snxpcb");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read board: {e}"))?;
    let mut board = SnxPcb::parse(&text)
        .map_err(|e| format!("parse board: {e:?}"))?
        .board;
    for (footprint, (uuid, reference, value, id)) in board.footprints.iter_mut().zip(&parts) {
        footprint.schematic_link = Some(SymbolLink::new(InstancePath::root(), [*uuid]));
        footprint.reference.clone_from(reference);
        footprint.value.clone_from(value);
        footprint.footprint_id.clone_from(id);
    }
    let renamed = format!("{}9", parts.first().ok_or("no parts")?.1);
    if let [first, second, third, fourth, ..] = board.footprints.as_mut_slice() {
        first.reference.clone_from(&renamed);
        second.value = "22k".to_string();
        third.footprint_id = "R_0805_2012Metric".to_string();
        fourth.reference = renamed;
    }
    let text = SnxPcb::new(board)
        .write_string()
        .map_err(|e| format!("encode board: {e:?}"))?;
    std::fs::write(&path, text).map_err(|e| format!("write board: {e}"))?;
    helpers_d::command(app, "back_annotate_schematics")?;
    reached(
        app.ui_state.back_annotate.is_some(),
        "back_annotate_schematics opened no dialog",
    )
}

/// Open the recorder on the row bound to catalog command `id`.
fn record_on(app: &mut Signex, id: &str) -> Result<(), String> {
    let row = app
        .ui_state
        .preferences_keymap_editor
        .rows_in(signex_i18n::I18n::en_us_ref())
        .into_iter()
        .find(|row| row.command.as_ref().is_some_and(|c| c.as_str() == id))
        .ok_or_else(|| format!("the Keyboard page lists no {id} row"))?;
    let command = row.command.ok_or("the row has no command")?;
    let _ = app.update(inner(PrefMsg::KeymapRecorderOpen {
        slot: row.slot,
        command,
        label: row.label,
        context: row.context,
        trigger: row.trigger,
    }));
    Ok(())
}

/// Press `keys` into the open recorder, as the keyboard subscription does.
fn press(app: &mut Signex, keys: &str) -> Result<(), String> {
    let trigger = ShortcutTrigger::parse(keys).map_err(|e| format!("{keys}: {e:?}"))?;
    let ShortcutTrigger::KeySequence(strokes) = trigger else {
        return Err(format!("{keys} is not a key sequence"));
    };
    let _ = app.update(inner(PrefMsg::KeymapRecorderClear));
    for stroke in strokes {
        let _ = app.update(inner(PrefMsg::KeymapRecorderKeyPressed(stroke)));
    }
    Ok(())
}

/// A custom profile where Rubber Stamp takes Smart Paste's stroke, then the
/// recorder on Smart Paste holding a new stroke. A conflict needs one shared
/// context, and Place Via is PCB-only while Smart Paste is global. The search
/// brings the row, and the popover anchored to it, onto the first screen.
fn open_recorder(app: &mut Signex, _: &mut iced::Renderer, _: &Fixture) -> Result<(), String> {
    prefs_page(app, PrefNav::Keyboard);
    let _ = app.update(inner(PrefMsg::KeymapCreateCustomProfile));
    record_on(app, "rubber_stamp_copy")?;
    press(app, "Mod+Shift+V")?;
    let _ = app.update(inner(PrefMsg::KeymapRecorderApply));
    reached(
        !app.ui_state
            .preferences_keymap_editor
            .active_conflicts()
            .is_empty(),
        "the shared stroke raised no conflict",
    )?;
    let _ = app.update(inner(PrefMsg::KeymapSearchChanged("Smart Paste".into())));
    record_on(app, "smart_paste")?;
    press(app, "Ctrl+Alt+V")?;
    reached(
        app.ui_state
            .preferences_keymap_recorder
            .as_ref()
            .is_some_and(|recorder| recorder.recording && !recorder.strokes.is_empty()),
        "the recorder is not recording a stroke",
    )
}

/// Five profiles handed to the page as a library read would, Automotive
/// selected. Nothing is written.
fn open_profiles(app: &mut Signex, _: &mut iced::Renderer, _: &Fixture) -> Result<(), String> {
    prefs_page(app, PrefNav::RuleProfiles);
    let mut profiles = vec![ListedProfile {
        profile: builtin_profile(),
        kind: ProfileKind::Builtin,
    }];
    for name in ["Automotive", "Consumer", "Industrial", "Medical"] {
        let mut profile = builtin_profile();
        profile.name = name.to_string();
        profiles.push(ListedProfile {
            profile,
            kind: ProfileKind::User,
        });
    }
    let listing = ProfileListing {
        profiles,
        problems: Vec::new(),
    };
    let _ = app.update(inner(PrefMsg::Profiles(ProfilesMsg::Listed(Arc::new(
        listing,
    )))));
    let _ = app.update(inner(PrefMsg::Profiles(ProfilesMsg::Selected(
        "Automotive".into(),
    ))));
    reached(
        app.ui_state.profile_manager.selected_profile().is_some(),
        "no profile is selected",
    )
}

/// The Distributor APIs page with the DigiKey client configured, seeded as
/// a finished sign-in leaves it: the OAuth flow needs the network.
fn open_distributors(app: &mut Signex, account: Option<&str>) -> Result<(), String> {
    prefs_page(app, PrefNav::LibraryDistributors);
    app.library.settings.digikey_client_configured = true;
    app.library.settings.digikey_account_email = account.map(str::to_string);
    let _ = app.update(inner(PrefMsg::Noop));
    reached(app.ui_state.preferences_open, "Preferences closed")
}
