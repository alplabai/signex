use std::path::Path;

use signex_library::primitive::footprint::{FpGraphic, FpGraphicKind};
use signex_widgets::active_bar_dropdown::DropdownEntry;

use super::{Footprint, FootprintEditorMsg, indexed_header, menu_entries, silk_delete_msg};
use crate::app::FootprintDocument;
use crate::keymap::{
    AppCommandId, CompiledKeymap, ShortcutBinding, ShortcutBindingAction, ShortcutContext,
    ShortcutProfile, ShortcutProfileKind, ShortcutProfileSet, ShortcutTrigger,
};
use crate::library::editor::footprint::state::context_menu::SelectionAction;
use crate::library::editor::footprint::state::{
    AlignOp, FootprintContextAction, FootprintContextMenuState, FootprintContextSubmenu,
    FootprintContextTarget,
};
use crate::library::messages::{LibraryMessage, PrimitiveEdit};

/// An index shows as the plain digits, not grouped like a count.
#[test]
fn an_index_header_has_no_digit_grouping() {
    let en = signex_i18n::I18n::en_us_ref();
    assert_eq!(
        indexed_header(
            en,
            signex_i18n::menu_ids::CTX_FP_SILK_UNKNOWN,
            "index",
            1234
        ),
        "Silk graphic 1234"
    );
    assert_eq!(
        indexed_header(en, signex_i18n::menu_ids::CTX_FP_PAD_HEADER, "number", 1234),
        "Pad 1234"
    );
}

fn keymap_with(bindings: Vec<(ShortcutBindingAction, &str)>) -> CompiledKeymap {
    CompiledKeymap::compile(&ShortcutProfile {
        id: "t".to_string(),
        name: "T".to_string(),
        kind: ShortcutProfileKind::Custom,
        schema_version: 1,
        description: None,
        base_profile: None,
        bindings: bindings
            .into_iter()
            .map(|(action, trigger)| ShortcutBinding {
                action,
                context: ShortcutContext::Global,
                triggers: vec![ShortcutTrigger::parse(trigger).unwrap()],
                platform: None,
            })
            .collect(),
    })
}

fn command(id: &str) -> ShortcutBindingAction {
    ShortcutBindingAction::Command(AppCommandId::new(id).unwrap())
}

// #1378: an unbound or unbound-by-profile command advertises no key.
#[test]
fn a_context_menu_hint_follows_the_keymap_not_a_fixed_fallback() {
    let copy = AppCommandId::new("copy").unwrap();
    let keymap = keymap_with(vec![
        (ShortcutBindingAction::Command(copy.clone()), "Ctrl+C"),
        (ShortcutBindingAction::Unbind(copy), "Ctrl+C"),
        (command("paste"), "F4"),
    ]);
    assert_eq!(
        keymap.shortcut_label_for_id("copy"),
        "",
        "unbound by the user"
    );
    assert_eq!(keymap.shortcut_label_for_id("cut"), "", "never bound");
    assert_eq!(keymap.shortcut_label_for_id("paste"), "F4");
}

#[test]
fn a_baked_silk_entry_has_no_delete_message() {
    let mut fp = Footprint::empty("t");
    let hand = FpGraphic::drawn(
        FpGraphicKind::Line {
            from: [0.0, 0.0],
            to: [1.0, 0.0],
        },
        0.12,
        false,
    );
    let mut baked = hand.clone();
    baked.sketch_entity = Some(signex_sketch::id::SketchEntityId::new());
    fp.silk_f = vec![baked, hand];

    assert!(silk_delete_msg(&fp, 0).is_none());
    assert!(matches!(
        silk_delete_msg(&fp, 1),
        Some(FootprintEditorMsg::DeleteSilkF)
    ));
}

fn editor_with_menu(
    target: FootprintContextTarget,
    submenu: Option<FootprintContextSubmenu>,
) -> FootprintDocument {
    let file = signex_library::FootprintFile::from_footprint(Footprint::empty("t"));
    let mut editor = FootprintDocument::new(std::path::PathBuf::from("t.snxfpt"), file);
    editor.state.add_pad_at(0.0, 0.0);
    editor.state.add_pad_at(2.0, 0.0);
    editor.state.selected_pad = Some(0);
    editor.state.context_menu = Some(FootprintContextMenuState {
        x: 10.0,
        y: 10.0,
        target,
        submenu,
    });
    editor
}

/// Each row as (label, message), headers and dividers left out.
fn rows(
    editor: &FootprintDocument,
    keymap: &CompiledKeymap,
) -> Vec<(String, Option<FootprintEditorMsg>)> {
    let en = signex_i18n::I18n::en_us_ref();
    menu_entries(editor, Path::new("t.snxfpt"), false, keymap, en)
        .expect("the menu is open")
        .into_iter()
        .filter_map(|entry| match entry {
            DropdownEntry::Item(item) => {
                let msg = item.on_press.map(|m| match m {
                    LibraryMessage::PrimitiveEditorEvent {
                        msg: PrimitiveEdit::Footprint(msg),
                        ..
                    } => msg,
                    other => panic!("not a footprint message: {other:?}"),
                });
                Some((item.label, msg))
            }
            _ => None,
        })
        .collect()
}

fn position(rows: &[(String, Option<FootprintEditorMsg>)], label: &str) -> usize {
    rows.iter()
        .position(|(l, _)| l == label)
        .unwrap_or_else(|| panic!("no row {label:?} in {rows:?}"))
}

fn pick_of(row: &(String, Option<FootprintEditorMsg>)) -> Option<SelectionAction> {
    match row.1 {
        Some(FootprintEditorMsg::ContextMenuAction(FootprintContextAction::OnSelection(pick))) => {
            Some(pick)
        }
        _ => None,
    }
}

/// Mockup row 140: with a pad selected the menu starts with Move Selection By,
/// Align ▸, Rotate Selection and Flip Selection, keeps the shipped rows,
/// and ends with Delete.
#[test]
fn the_selection_menu_lists_the_transforms_and_ends_with_delete() {
    let editor = editor_with_menu(FootprintContextTarget::Pad(0), None);
    let keymap = keymap_with(vec![(command("delete_selection"), "Delete")]);
    let rows = rows(&editor, &keymap);

    let move_by = position(&rows, "Move Selection By…");
    let align = position(&rows, "Align");
    let rotate = position(&rows, "Rotate Selection");
    let flip = position(&rows, "Flip Selection");
    assert!(
        move_by < align && align < rotate && rotate < flip,
        "{rows:?}"
    );
    assert_eq!(pick_of(&rows[move_by]), Some(SelectionAction::MoveBy));
    assert_eq!(pick_of(&rows[rotate]), Some(SelectionAction::Rotate));
    assert_eq!(pick_of(&rows[flip]), Some(SelectionAction::Flip));
    assert_eq!(
        pick_of(&rows[position(&rows, "Rectangular Pattern…")]),
        Some(SelectionAction::RectangularPattern)
    );
    assert_eq!(
        pick_of(&rows[position(&rows, "Edit Sketch")]),
        Some(SelectionAction::EditSketch)
    );
    // The shipped rows stay.
    for kept in [
        "Properties...",
        "Pad Actions",
        "Cut",
        "Copy",
        "Find Similar Objects...",
    ] {
        position(&rows, kept);
    }
    let (last, msg) = rows.last().expect("rows");
    assert_eq!(last, "Delete");
    assert!(matches!(msg, Some(FootprintEditorMsg::DeleteSelected)));
}

/// Align ▸ opens in place with the Align dialog first and the grid snap
/// last; a click on the open launcher closes it.
#[test]
fn the_align_submenu_holds_the_dialog_and_the_grid_snap() {
    let editor = editor_with_menu(
        FootprintContextTarget::Pad(0),
        Some(FootprintContextSubmenu::Align),
    );
    let rows = rows(&editor, &keymap_with(Vec::new()));
    let align = position(&rows, "Align");
    assert!(matches!(
        rows[align].1,
        Some(FootprintEditorMsg::ContextMenuOpenSubmenu(None))
    ));
    assert_eq!(rows[align + 1].0, "  Align…");
    assert_eq!(
        pick_of(&rows[align + 1]),
        Some(SelectionAction::AlignDialog)
    );
    assert_eq!(
        pick_of(&rows[position(&rows, "  Align Left")]),
        Some(SelectionAction::Align(AlignOp::Left))
    );
    assert_eq!(rows[align + 10].0, "  Align To Grid");
    assert_eq!(
        pick_of(&rows[align + 10]),
        Some(SelectionAction::AlignToGrid)
    );
    assert_eq!(rows[align + 11].0, "Rotate Selection");
}

/// The silk menu ends with Delete too.
#[test]
fn the_silk_menu_ends_with_delete() {
    let mut editor = editor_with_menu(FootprintContextTarget::SilkF(0), None);
    editor.primitive_mut().silk_f = vec![FpGraphic::drawn(
        FpGraphicKind::Line {
            from: [0.0, 0.0],
            to: [1.0, 0.0],
        },
        0.12,
        false,
    )];
    let rows = rows(&editor, &keymap_with(Vec::new()));
    let (last, msg) = rows.last().expect("rows");
    assert_eq!(last, "Delete");
    assert!(matches!(msg, Some(FootprintEditorMsg::DeleteSilkF)));
}

/// The empty-canvas menu keeps its rows: no selection rows, no Delete.
#[test]
fn the_empty_canvas_menu_is_unchanged() {
    let editor = editor_with_menu(FootprintContextTarget::Empty, None);
    let labels: Vec<String> = rows(&editor, &keymap_with(Vec::new()))
        .into_iter()
        .map(|(label, _)| label)
        .collect();
    assert_eq!(
        labels,
        [
            "Find Similar Objects...",
            "Place",
            "Selection",
            "View",
            "Properties...",
        ]
    );
}

/// Each row as (label, command it runs, key it shows), headers and
/// dividers left out.
fn keyed_rows(
    editor: &FootprintDocument,
    keymap: &CompiledKeymap,
) -> Vec<(String, Option<&'static str>, String)> {
    let en = signex_i18n::I18n::en_us_ref();
    menu_entries(editor, Path::new("t.snxfpt"), false, keymap, en)
        .expect("the menu is open")
        .into_iter()
        .filter_map(|entry| match entry {
            DropdownEntry::Item(item) => Some((
                item.label,
                item.command_id,
                item.shortcut.unwrap_or_default(),
            )),
            _ => None,
        })
        .collect()
}

fn built_in_keymaps() -> Vec<(String, CompiledKeymap)> {
    let set = ShortcutProfileSet::built_ins().expect("the built-in profiles load");
    set.profiles()
        .map(|profile| (profile.id.clone(), CompiledKeymap::compile(profile)))
        .collect()
}

/// #2603: Move Selection By shows the key the keymap binds to it, and no
/// command opens it in either built-in profile ("M M" is Move Object's),
/// so it shows none.
#[test]
fn move_selection_by_shows_no_key_while_nothing_binds_it() {
    let editor = editor_with_menu(FootprintContextTarget::Pad(0), None);
    for (profile, keymap) in built_in_keymaps() {
        let rows = keyed_rows(&editor, &keymap);
        let (_, command, keys) = rows
            .iter()
            .find(|(label, ..)| label == "Move Selection By…")
            .unwrap_or_else(|| panic!("{profile}: no Move Selection By row in {rows:?}"));
        assert_eq!(*command, None, "{profile}");
        assert_eq!(keys, "", "{profile}: Move Selection By shows {keys:?}");
    }
}

/// #2603: every key a footprint menu shows is the key the active keymap
/// binds to the row's command, or the canvas's own Space and X; a row no
/// key runs shows none.
#[test]
fn every_key_the_menus_show_is_one_the_keymap_or_the_canvas_binds() {
    let en = signex_i18n::I18n::en_us_ref();
    let space = en.message_plain(signex_i18n::key_ids::KEY_SPACE, &[]);
    let canvas_keys = [space.as_str(), "X"];
    let menus = [
        (FootprintContextTarget::Pad(0), None),
        (
            FootprintContextTarget::Pad(0),
            Some(FootprintContextSubmenu::Align),
        ),
        (
            FootprintContextTarget::Pad(0),
            Some(FootprintContextSubmenu::PadActions),
        ),
        (
            FootprintContextTarget::Empty,
            Some(FootprintContextSubmenu::Place),
        ),
        (
            FootprintContextTarget::Empty,
            Some(FootprintContextSubmenu::Selection),
        ),
        (
            FootprintContextTarget::Empty,
            Some(FootprintContextSubmenu::View),
        ),
    ];
    for (profile, keymap) in built_in_keymaps() {
        for (target, submenu) in menus {
            let editor = editor_with_menu(target, submenu);
            for (label, command, keys) in keyed_rows(&editor, &keymap) {
                match command {
                    Some(id) => assert_eq!(
                        keys,
                        keymap.shortcut_label_for_id(id),
                        "{profile}: {label:?} runs {id}"
                    ),
                    None => assert!(
                        keys.is_empty() || canvas_keys.contains(&keys.as_str()),
                        "{profile}: {label:?} shows {keys:?}, which nothing binds"
                    ),
                }
            }
        }
    }
}

/// A binding the user adds shows on the menu row of its command.
#[test]
fn a_selection_row_shows_the_key_the_user_binds() {
    let editor = editor_with_menu(
        FootprintContextTarget::Empty,
        Some(FootprintContextSubmenu::Selection),
    );
    let keymap = keymap_with(vec![(command("unselect_all"), "F7")]);
    let rows = keyed_rows(&editor, &keymap);
    let (_, command, keys) = rows
        .iter()
        .find(|(label, ..)| label == "  Deselect All")
        .unwrap_or_else(|| panic!("no Deselect All row in {rows:?}"));
    assert_eq!(*command, Some("unselect_all"));
    assert_eq!(keys, "F7");
}
