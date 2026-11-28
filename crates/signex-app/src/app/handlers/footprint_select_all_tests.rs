//! #3233: on a footprint editor tab, the key the footprint canvas menu
//! shows on Select All and Deselect All runs that row, and the row follows
//! the key when the user rebinds it. Driven through `Signex::update` as the
//! keyboard drives it.

use std::path::PathBuf;

use crate::app::{FootprintDocument, Message, Signex, TabInfo, TabKind, UiMsg};
use crate::keymap::{
    AppCommandId, CompiledKeymap, Platform, ShortcutBinding, ShortcutBindingAction,
    ShortcutContext, ShortcutProfile, ShortcutProfileKind, ShortcutTrigger,
};
use crate::library::editor::footprint::apply_footprint_primitive_edit;
use crate::library::messages::FootprintEditorMsg;

const PATH: &str = "/w/part.snxfpt";

/// A footprint editor tab with three pads and nothing selected, active in
/// the main window, under a keymap of `bindings` (trigger, command id).
fn footprint_tab_with(bindings: &[(&str, &str)]) -> Signex {
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    let path = PathBuf::from(PATH);
    let file =
        signex_library::FootprintFile::from_footprint(signex_library::Footprint::empty("fp"));
    let mut editor = FootprintDocument::new(path.clone(), file);
    for x_mm in [0.0, 3.0, 6.0] {
        apply_footprint_primitive_edit(&mut editor, FootprintEditorMsg::AddPad { x_mm, y_mm: 0.0 });
    }
    apply_footprint_primitive_edit(&mut editor, FootprintEditorMsg::SelectPad(None));
    app.document_state
        .footprint_editors
        .insert(path.clone(), editor);
    app.document_state.tabs.push(TabInfo {
        title: "part".to_string(),
        path: path.clone(),
        project_id: None,
        kind: TabKind::FootprintEditor(path),
    });
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    let profile = ShortcutProfile {
        id: "t".to_string(),
        name: "T".to_string(),
        kind: ShortcutProfileKind::Custom,
        schema_version: 1,
        description: None,
        base_profile: None,
        bindings: bindings
            .iter()
            .map(|(trigger, id)| ShortcutBinding {
                action: ShortcutBindingAction::Command(AppCommandId::new(*id).unwrap()),
                context: ShortcutContext::Global,
                triggers: vec![ShortcutTrigger::parse(trigger).unwrap()],
                platform: None,
            })
            .collect(),
    };
    app.ui_state.active_keymap =
        std::sync::Arc::new(CompiledKeymap::compile_for(&profile, Platform::Windows));
    app
}

/// The key the footprint canvas menu shows on the row of `command`.
fn shown_key(app: &Signex, command: &str) -> String {
    app.ui_state.active_keymap.shortcut_label_for_id(command)
}

fn press(app: &mut Signex, keys: &str) {
    let ShortcutTrigger::KeySequence(mut strokes) = ShortcutTrigger::parse(keys).unwrap() else {
        panic!("{keys} is a key");
    };
    let window = app.ui_state.main_window_id;
    let _ = app.update(Message::Ui(UiMsg::KeymapStroke {
        window,
        stroke: strokes.remove(0),
    }));
}

fn selected_pads(app: &Signex) -> Vec<usize> {
    app.document_state.footprint_editors[&PathBuf::from(PATH)]
        .state
        .selected_pad_indices()
}

#[test]
fn the_keys_the_selection_rows_show_select_and_deselect_every_pad() {
    let mut app = footprint_tab_with(&[("Ctrl+A", "select_all"), ("F8", "unselect_all")]);
    assert_eq!(shown_key(&app, "select_all"), "Ctrl+A");
    assert_eq!(shown_key(&app, "unselect_all"), "F8");
    press(&mut app, "Ctrl+A");
    assert_eq!(selected_pads(&app), vec![0, 1, 2], "Select All runs");
    press(&mut app, "F8");
    assert!(selected_pads(&app).is_empty(), "Deselect All runs");
}

#[test]
fn a_rebound_select_all_runs_from_its_new_key_only() {
    let mut app = footprint_tab_with(&[("F7", "select_all"), ("F8", "unselect_all")]);
    assert_eq!(shown_key(&app, "select_all"), "F7");
    press(&mut app, "Ctrl+A");
    assert!(
        selected_pads(&app).is_empty(),
        "the old key no longer selects"
    );
    press(&mut app, "F7");
    assert_eq!(selected_pads(&app), vec![0, 1, 2]);
    press(&mut app, "F8");
    assert!(selected_pads(&app).is_empty());
}
