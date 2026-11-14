//! Characterization of the main menu bar's content.
//!
//! `testdata/menu_bar.golden.tsv` pins every menu, submenu, row, separator,
//! label, shortcut, message and enablement of the bar, per context variant.
//! It is the contract of the menu-model refactor: it must not change.
//!
//! The fixture's labels and shortcuts are checked against the live helpers,
//! every enablement bit against the live predicates (the `gate` column names
//! the rule the row was built with), and the model must reproduce it all.

use super::{MenuContext, cmd_label, enabled, shortcut_for};
use crate::app::command::CommandContext;
use crate::app::command::enablement::{new_part_menu_enabled, remove_part_menu_enabled};

const GOLDEN: &str = include_str!("testdata/menu_bar.golden.tsv");
const RECENT_GOLDEN: &str = include_str!("testdata/menu_bar_recent.golden.tsv");

/// Context variants, in the order of the `enabled[...]` column.
pub(super) fn variants() -> Vec<MenuContext<'static>> {
    let with = |commands: CommandContext| MenuContext {
        commands,
        ..MenuContext::default()
    };
    let base = CommandContext::default();
    vec![
        with(base),
        with(CommandContext {
            has_schematic: true,
            document_is_schematic: true,
            has_pcb: true,
            has_sheets: true,
            has_selection: true,
            can_undo: true,
            can_redo: true,
            project_dirty: true,
            document_has_file: true,
            has_dirty_documents: true,
            has_document: true,
            has_open_project: true,
            has_parent_sheet: true,
            has_active_path: true,
            has_open_library: true,
            has_project_sheet: true,
            ..base
        }),
        with(CommandContext {
            has_pcb: true,
            // Turned, so Reset View Rotation is live (#2316).
            pcb_view_turned: true,
            has_document: true,
            // An unlocked footprint selected, so the board's rows are live.
            pcb: crate::app::command::enablement::PcbTargets {
                shown: true,
                any: true,
                placeable: true,
                deletable: true,
            },
            ..base
        }),
        with(CommandContext {
            has_symbol_editor: true,
            document_is_editor: true,
            can_remove_part: true,
            has_document: true,
            ..base
        }),
        with(CommandContext {
            has_sheets: true,
            has_open_project: true,
            has_project_sheet: true,
            ..base
        }),
        with(CommandContext {
            undocked_has_canvas: true,
            can_tile_windows: true,
            ..base
        }),
        with(CommandContext {
            has_footprint_editor: true,
            document_is_editor: true,
            has_document: true,
            ..base
        }),
        with(CommandContext {
            project_dirty: true,
            document_has_file: true,
            has_dirty_documents: true,
            has_open_project: true,
            ..base
        }),
    ]
}

fn eval_gate(gate: &str, ctx: &MenuContext) -> bool {
    match gate {
        "always" => true,
        "never" => false,
        "sch" => ctx.commands.has_schematic,
        "dirty" => ctx.commands.has_dirty_documents,
        "project" => ctx.commands.has_open_project,
        "newpart" => new_part_menu_enabled(&ctx.commands),
        "removepart" => remove_part_menu_enabled(&ctx.commands),
        "libeditor" => ctx.commands.has_library_editor,
        other => {
            let id = other.strip_prefix("cmd:").expect("known gate");
            enabled(ctx, id)
        }
    }
}

fn rows() -> Vec<Vec<&'static str>> {
    GOLDEN
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| l.split('\t').collect())
        .collect()
}

#[test]
fn golden_enablement_matches_live_predicates() {
    let ctxs = variants();
    for row in rows() {
        let (path, bits, gate) = (row[0], row[6], row[7]);
        if matches!(row[1], "menu" | "submenu" | "separator") {
            continue;
        }
        let got: String = ctxs
            .iter()
            .map(|c| if eval_gate(gate, c) { '1' } else { '0' })
            .collect();
        assert_eq!(got, bits, "enablement of {path} (gate {gate})");
    }
}

#[test]
fn golden_labels_and_shortcuts_match_helpers() {
    let ctx = MenuContext::default();
    for row in rows() {
        let (label, id, shortcut) = (row[2], row[3], row[4]);
        if id == "-" {
            continue;
        }
        assert_eq!(cmd_label(&ctx.i18n, id, label), label, "label of {id}");
        let expected = (shortcut != "-").then(|| shown_on_host(shortcut));
        assert_eq!(
            shortcut_for(&ctx, id, shortcut),
            expected,
            "shortcut of {id}"
        );
    }
}

/// A fixture fallback as this OS shows it. Only a `Mod` spelling is a
/// platform chord; every other spelling must come out exactly as written, so
/// a literal that drifts from the fixture fails on every OS.
fn shown_on_host(shortcut: &str) -> String {
    if is_mod_spelled(shortcut) {
        crate::keymap::shown(shortcut)
    } else {
        shortcut.to_string()
    }
}

fn is_mod_spelled(shortcut: &str) -> bool {
    shortcut.split('+').any(|part| part == "Mod")
}

/// The fixture spells exactly the standard-set rows with `Mod` (so a revert
/// to `Ctrl+N` fails here on Linux too), and on macOS each shows as Apple
/// symbols.
#[test]
fn golden_mod_fallbacks_are_the_standard_set_and_show_apple_symbols_on_macos() {
    use crate::keymap::{Platform, ShortcutTrigger};
    let mod_rows: Vec<(&str, &str)> = rows()
        .into_iter()
        .filter(|row| is_mod_spelled(row[4]))
        .map(|row| (row[3], row[4]))
        .collect();
    let ids: Vec<&str> = mod_rows.iter().map(|(id, _)| *id).collect();
    assert_eq!(
        ids,
        [
            "new_document",
            "open_document",
            "save_document",
            "save_document_as",
            "print",
            "undo",
            "cut",
            "copy",
            "paste",
            "duplicate",
            "select_all",
            "find",
            "toggle_cross_select_mode",
            "open_preferences",
        ]
    );
    for (id, spelling) in mod_rows {
        let shown = ShortcutTrigger::parse(spelling)
            .unwrap()
            .display_text_for(Platform::MacOs);
        assert!(
            shown.contains('\u{2318}') && !shown.contains('+'),
            "{id}: {shown}"
        );
    }
}

/// Whitespace and trailing commas removed, so a call rustfmt wrapped over
/// several lines still matches.
fn squash(text: &str) -> String {
    text.split_whitespace()
        .collect::<String>()
        .replace(",)", ")")
}

/// The model resolves labels through the command catalog (the label lives in
/// `commands.ftl`, no longer as a literal in the model), so the fixture
/// label alone would not notice a changed catalog text: pin each fixture row
/// twice. The catalog must render the fixture label for the id (no fallback
/// applies), and the `cmd(id, shortcut)` call in the model source must carry
/// the fixture shortcut.
#[test]
fn golden_fallback_literals_are_pinned_in_the_model_source() {
    let source = squash(include_str!("model/menus.rs"));
    let mut pinned = 0;
    for row in rows() {
        let (label, id, shortcut) = (row[2], row[3], row[4]);
        if id == "-" {
            continue;
        }
        assert_eq!(
            cmd_label(signex_i18n::I18n::en_us_ref(), id, "<missing>"),
            label,
            "the catalog text of {id} differs from the fixture"
        );
        let call = squash(&format!("cmd(\"{id}\", \"{shortcut}\")"));
        assert!(source.contains(&call), "fallbacks of {id} differ: {call}");
        pinned += 1;
    }
    assert!(pinned >= 30, "only {pinned} command rows pinned");
}

// ─── The declarative model reproduces the golden fixture ────────────────

use super::MenuMessage;
use super::model::{ItemState, Menu, MenuEntry, menu_bar_model};

/// Every row that carries a command id publishes `Command(id)` (#1922), and
/// the fixture names each row by a stable message-like name: the message it
/// sent before, or a new name for a row added since (`OpenErcDialog`, #1991).
/// This maps the id back to that name, so the fixture stays stable and still
/// pins which row runs which command.
const COMMAND_ROW_NAMES: &[(&str, &str)] = &[
    ("new_document", "NewProject"),
    // #1312: fixture names only, like #2247's.
    ("new_pcb_document", "NewPcb"),
    ("new_schematic_sheet", "NewSchematic"),
    ("update_pcb_from_schematic", "UpdatePcb"),
    // #2234: Design ▸ Board Sketch.
    ("board_sketch", "BoardSketch"),
    ("open_document", "OpenProject"),
    ("close_active_document", "CloseDocument"),
    ("save_document", "Save"),
    ("save_document_as", "SaveAs"),
    ("print", "Print"),
    ("undo", "Undo"),
    ("redo", "Redo"),
    ("cut", "Cut"),
    ("copy", "Copy"),
    ("paste", "Paste"),
    ("smart_paste", "SmartPaste"),
    ("duplicate", "Duplicate"),
    ("delete_selection", "Delete"),
    ("move_object", "MoveObject"),
    ("move_selection", "MoveSelection"),
    ("rotate_counterclockwise", "Rotate"),
    ("rotate_clockwise", "RotateClockwise"),
    ("mirror_x", "MirrorX"),
    ("mirror_y", "MirrorY"),
    ("flip_selection", "FlipSelection"),
    ("select_all", "SelectAll"),
    ("find", "Find"),
    ("find_and_replace", "Replace"),
    ("zoom_in_at_cursor", "ZoomIn"),
    ("zoom_out_at_cursor", "ZoomOut"),
    ("zoom_to_fit", "ZoomFit"),
    ("toggle_visible_grid", "ToggleGrid"),
    ("toggle_auto_focus", "ToggleAutoFocus"),
    ("toggle_ratsnest", "ToggleRatsnest"),
    ("next_layer", "NextLayer"),
    ("prev_layer", "PreviousLayer"),
    ("toggle_layer_visibility", "ToggleLayerVisibility"),
    ("show_all_layers", "ShowAllLayers"),
    ("view_rotate", "RotateView"),
    ("view_rotate_ccw", "RotateViewCounterClockwise"),
    ("view_rotate_180", "RotateView180"),
    ("view_rotate_reset", "ResetViewRotation"),
    ("place_wire", "PlaceWire"),
    ("place_bus", "PlaceBus"),
    ("place_net_label", "PlaceLabel"),
    ("open_components_panel", "PlaceComponent"),
    ("place_power_symbol", "PlacePowerPort"),
    ("place_junction", "PlaceJunction"),
    // #2247: fixture names only; no `MenuMessage` variant carries these.
    ("place_sheet_entry", "PlaceSheetEntry"),
    ("open_library_browser", "OpenLibraryEditor"),
    ("close_all_documents", "CloseAllDocuments"),
    ("number_schematic_sheets", "NumberSchematicSheets"),
    ("tile_windows_horizontally", "TileHorizontally"),
    ("tile_windows_vertically", "TileVertically"),
    ("next_grid", "CycleGrid"),
    ("open_erc_dialog", "OpenErcDialog"),
    ("run_drc", "RunDrc"),
    ("reannotate_board", "ReannotateBoard"),
    ("back_annotate_schematics", "BackAnnotate"),
    ("cross_probe", "CrossProbe"),
    ("toggle_cross_select_mode", "ToggleCrossSelectMode"),
    ("open_parameter_manager", "OpenParameterManager"),
    // #2248 / #1269: fixture names only.
    ("assign_footprints", "AssignFootprints"),
    ("open_net_inspector", "OpenNetInspector"),
    ("annotate_schematic_quietly", "AnnotateQuietly"),
    ("force_annotate_all_schematics", "AnnotateForceAll"),
    ("open_preferences", "OpenPreferences"),
    ("show_current_command_hotkeys", "OpenKeyboardShortcuts"),
];

fn action_name(action: &MenuMessage) -> String {
    match action {
        MenuMessage::Command(id) => COMMAND_ROW_NAMES
            .iter()
            .find(|(command, _)| *command == id.as_str())
            .unwrap_or_else(|| panic!("command row {id:?} is not in COMMAND_ROW_NAMES"))
            .1
            .to_string(),
        other => format!("{other:?}"),
    }
}

/// One fixture row per entry, columns `path kind label command_id shortcut
/// action` plus the entry's enablement as `1` / `0` (`-` for containers).
fn flatten(menu: &Menu, parent: Option<&str>, out: &mut Vec<(String, char)>) {
    let path = parent.map_or(menu.title.clone(), |p| format!("{p}/{}", menu.title));
    let kind = if parent.is_some() { "submenu" } else { "menu" };
    out.push((format!("{path}\t{kind}\t{}\t-\t-\t-", menu.title), '-'));
    for entry in &menu.entries {
        match entry {
            MenuEntry::Separator => out.push((format!("{path}/---\tseparator\t-\t-\t-\t-"), '-')),
            MenuEntry::Submenu(sub) => flatten(sub, Some(&path), out),
            MenuEntry::Item(item) => {
                let (kind, action) = match &item.state {
                    ItemState::Placeholder | ItemState::Unavailable => ("stub", "-".to_string()),
                    ItemState::Live { action, .. } => ("item", action_name(action)),
                };
                let (id, text) = item
                    .shortcut
                    .as_ref()
                    .map_or(("-", "-"), |s| (s.command_id.as_str(), s.text.as_str()));
                let cols = format!(
                    "{path}/{}\t{kind}\t{}\t{id}\t{text}\t{action}",
                    item.label, item.label
                );
                out.push((cols, if item.is_enabled() { '1' } else { '0' }));
            }
        }
    }
}

fn flat_rows(ctx: &MenuContext) -> Vec<(String, char)> {
    let mut out = Vec::new();
    for menu in &menu_bar_model(ctx).menus {
        flatten(menu, None, &mut out);
    }
    out
}

const RECENT_PREFIX: &str = "File/Open Recent/";

#[test]
fn model_reproduces_golden_menu_bar() {
    let per_variant: Vec<Vec<(String, char)>> = variants().iter().map(flat_rows).collect();
    let keep = |(cols, _): &(String, char)| !cols.starts_with(RECENT_PREFIX);
    let first: Vec<&(String, char)> = per_variant[0].iter().filter(|r| keep(r)).collect();
    let mut got = String::new();
    for (i, (cols, _)) in first.iter().enumerate() {
        let bits: String = per_variant
            .iter()
            .map(|rows| rows.iter().filter(|r| keep(r)).nth(i).expect("same shape"))
            .map(|(other, bit)| {
                assert_eq!(other, cols, "row shape must not depend on context");
                *bit
            })
            .collect();
        let bits = if bits.chars().all(|c| c == '-') {
            "-"
        } else {
            &bits
        };
        got.push_str(&format!("{cols}\t{bits}\n"));
    }
    // The fixture spells a fallback hint (`Mod+N`); the model shows it as
    // this OS writes it.
    let want: String = rows()
        .iter()
        .map(|r| {
            let mut cols: Vec<String> = r[..7].iter().map(|c| c.to_string()).collect();
            if cols[4] != "-" {
                cols[4] = shown_on_host(&cols[4]);
            }
            format!("{}\n", cols.join("\t"))
        })
        .collect();
    assert_eq!(got, want);
}

#[test]
fn model_reproduces_golden_recent_projects() {
    let mut want: Vec<(&str, Vec<String>)> = Vec::new();
    for line in RECENT_GOLDEN
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        if let Some(name) = line.strip_prefix("@variant ") {
            want.push((name, Vec::new()));
        } else {
            want.last_mut()
                .expect("variant header")
                .1
                .push(line.to_string());
        }
    }
    for (name, expected) in want {
        let recent: Vec<std::path::PathBuf> = name
            .strip_prefix("paths=")
            .map(|p| p.split(',').map(Into::into).collect())
            .unwrap_or_default();
        let ctx = MenuContext {
            recent_projects: &recent,
            ..MenuContext::default()
        };
        let got: Vec<String> = flat_rows(&ctx)
            .into_iter()
            .filter(|(cols, _)| cols.starts_with(RECENT_PREFIX))
            .map(|(cols, bit)| format!("{cols}\t{bit}"))
            .collect();
        assert_eq!(got, expected, "Open Recent for {name}");
    }
}

#[test]
fn model_shortcuts_follow_the_active_keymap() {
    let keymap = crate::keymap::ShortcutProfileSet::built_ins()
        .expect("built-in profiles")
        .compile_active();
    let ctx = MenuContext {
        active_keymap: Some(std::sync::Arc::new(keymap)),
        ..MenuContext::default()
    };
    let mut checked = 0;
    for menu in &menu_bar_model(&ctx).menus {
        check_shortcuts(menu, &ctx, &mut checked);
    }
    // Edit ▸ Move adds Move, Rotate, Rotate Clockwise, Mirror X and Mirror Y (#2240).
    // View ▸ Next Layer and Previous Layer add two more (#2223).
    // Tools ▸ Cross Select Mode adds Ctrl+Shift+X (#1312).
    assert_eq!(checked, 38, "the Altium profile labels 38 menu rows");
}

fn check_shortcuts(menu: &Menu, ctx: &MenuContext, checked: &mut usize) {
    for entry in &menu.entries {
        match entry {
            MenuEntry::Submenu(sub) => check_shortcuts(sub, ctx, checked),
            MenuEntry::Item(item) => {
                let Some(sc) = &item.shortcut else { continue };
                let keymap = ctx.active_keymap.as_ref().expect("keymap");
                let command = sc.command_id.clone();
                // With an active keymap a row shows the keymap's label or
                // no shortcut at all, never the catalog fallback (#1378).
                assert_eq!(
                    keymap.shortcut_label(&command).as_deref(),
                    Some(sc.text.as_str()),
                    "shortcut of {}",
                    sc.command_id
                );
                *checked += 1;
            }
            MenuEntry::Separator => {}
        }
    }
}

/// The chrome clamps its search bar with this width; it is derived from the
/// model's root titles and must follow the root titles (52 label chars, 10 roots).
#[test]
fn approx_menu_bar_width_is_unchanged() {
    let titles: Vec<String> = menu_bar_model(&MenuContext::default())
        .menus
        .into_iter()
        .map(|menu| menu.title)
        .collect();
    assert_eq!(titles, super::model::ROOT_TITLES);
    assert_eq!(
        super::model::ROOT_TITLES,
        [
            "File", "Edit", "View", "Project", "Place", "Design", "Tools", "Reports", "Window",
            "Help"
        ]
    );
    assert_eq!(
        super::approx_menu_bar_width(&signex_i18n::I18n::en_us()),
        622.0
    );
}

/// The estimate counts the titles the bar draws: under the pseudolocale the
/// titles are longer, and the width grows by exactly that many glyphs.
#[test]
fn approx_menu_bar_width_follows_the_catalog_titles() {
    let xa = signex_i18n::I18n::new(&signex_i18n::LanguagePref::Locale("en-XA".into()));
    let en = signex_i18n::I18n::en_us();
    let drawn = |i18n: &signex_i18n::I18n| {
        super::model::root_titles(i18n)
            .iter()
            .map(|title| title.chars().count())
            .sum::<usize>()
    };
    assert!(drawn(&xa) > drawn(&en), "en-XA titles are longer");
    let grown = super::approx_menu_bar_width(&xa) - super::approx_menu_bar_width(&en);
    assert_eq!(grown, (drawn(&xa) - drawn(&en)) as f32 * 7.5);
}
