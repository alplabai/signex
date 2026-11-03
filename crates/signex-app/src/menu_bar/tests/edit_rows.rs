//! What the Edit menu's rows do.

use super::*;
use signex_types::schematic::{SelectedItem, SelectedKind};

#[test]
fn delete_removes_the_selection_as_one_undo_step() {
    let mut app = one_symbol_selected();
    let before = probe(&app);
    let _ = click(&mut app, "Edit/Delete");
    assert_eq!(
        probe(&app),
        Probe {
            symbols: 0,
            selected: 0,
            refs: Vec::new(),
            can_undo: true,
            dirty: true,
            ..before
        }
    );
}

#[test]
fn undo_and_redo_step_the_sheet_history() {
    let mut app = one_symbol_selected();
    let _ = click(&mut app, "Edit/Delete");
    let deleted = probe(&app);

    let _ = click(&mut app, "Edit/Undo");
    let undone = probe(&app);
    assert_eq!(undone.symbols, 1, "the symbol is back");
    assert!(undone.can_redo && !undone.can_undo);
    assert_eq!(
        Probe {
            symbols: 0,
            selected: deleted.selected,
            refs: Vec::new(),
            can_undo: true,
            can_redo: false,
            ..undone.clone()
        },
        deleted
    );

    let _ = click(&mut app, "Edit/Redo");
    let redone = probe(&app);
    assert_eq!(redone.symbols, 0, "the delete is redone");
    assert!(redone.can_undo && !redone.can_redo);
}

#[test]
fn copy_fills_the_clipboard_and_leaves_the_sheet_alone() {
    let mut app = one_symbol_selected();
    let before = probe(&app);
    let _ = click(&mut app, "Edit/Copy");
    assert_eq!(
        probe(&app),
        Probe {
            clipboard: true,
            ..before
        }
    );
    assert!(app.interaction_state.clipboard_text.is_some());
}

#[test]
fn cut_copies_then_removes_the_selection() {
    let mut app = one_symbol_selected();
    let before = probe(&app);
    let _ = click(&mut app, "Edit/Cut");
    assert_eq!(
        probe(&app),
        Probe {
            clipboard: true,
            symbols: 0,
            selected: 0,
            refs: Vec::new(),
            can_undo: true,
            dirty: true,
            ..before
        }
    );
}

#[test]
fn paste_puts_the_copy_on_the_cursor() {
    let (mut app, text) = app_that_copied();
    let before = probe(&app);
    let task = click(&mut app, "Edit/Paste");
    settle_answering_clipboard(&mut app, task, &text);
    let session = app.interaction_state.placement.as_ref();
    assert!(
        session.is_some_and(|s| s.kind.is_paste()),
        "no paste on the cursor"
    );
    assert_eq!(
        probe(&app),
        Probe {
            placement: true,
            tool: "Component".to_string(),
            ..before
        },
        "a paste is placed by a click, nothing else moves"
    );
}

#[test]
fn paste_special_places_the_copy_at_once() {
    let (mut app, text) = app_that_copied();
    let before = probe(&app);
    let task = click(&mut app, "Edit/Paste Special");
    settle_answering_clipboard(&mut app, task, &text);
    let after = probe(&app);
    assert_eq!(after.symbols, 2, "the copy was not placed");
    assert!(!after.placement);
    assert_eq!(
        after,
        Probe {
            symbols: 2,
            selected: after.selected,
            refs: vec!["R1".into(), "R2".into()],
            can_undo: true,
            dirty: true,
            ..before
        }
    );
}

#[test]
fn duplicate_adds_a_copy_of_the_selection() {
    let mut app = one_symbol_selected();
    let before = probe(&app);
    let task = click(&mut app, "Edit/Duplicate");
    settle_answering_clipboard(&mut app, task, "");
    let after = probe(&app);
    assert_eq!(after.symbols, 2, "the symbol was not duplicated");
    assert_eq!(
        after,
        Probe {
            symbols: 2,
            selected: after.selected,
            refs: vec!["R1".into(), "R2".into()],
            clipboard: true,
            can_undo: true,
            dirty: true,
            ..before
        }
    );
}

#[test]
fn select_all_selects_every_object() {
    let sheet = sheet_of(vec![
        symbol(1, "R1", 10.0, 10.0),
        symbol(2, "R2", 30.0, 10.0),
    ]);
    let mut app = app_with(sheet.clone());
    app.interaction_state
        .active_canvas_mut()
        .set_render_cache(Some(
            crate::schematic_runtime::SchematicRenderCache::from_sheet(&sheet),
        ));
    let before = probe(&app);
    let _ = click(&mut app, "Edit/Select All");
    assert_eq!(
        probe(&app),
        Probe {
            selected: 2,
            ..before
        }
    );
}

#[test]
fn find_and_replace_open_the_dialog_in_their_mode() {
    let mut app = one_symbol_selected();
    let before = probe(&app);
    let _ = click(&mut app, "Edit/Find");
    assert_eq!(
        probe(&app),
        Probe {
            find_open: true,
            ..before.clone()
        }
    );

    let mut app = one_symbol_selected();
    let _ = click(&mut app, "Edit/Find and Replace");
    assert_eq!(
        probe(&app),
        Probe {
            find_open: true,
            replace_mode: true,
            ..before
        }
    );
}

#[test]
fn arrange_rows_reorder_the_selected_drawing() {
    let cases: [(&str, u128, [u128; 4]); 4] = [
        ("Edit/Arrange/Bring to Front", 2, [1, 3, 4, 2]),
        ("Edit/Arrange/Send to Back", 3, [3, 1, 2, 4]),
        ("Edit/Arrange/Bring Forward", 3, [1, 2, 4, 3]),
        ("Edit/Arrange/Send Backward", 3, [1, 3, 2, 4]),
    ];
    for (path, picked, expected) in cases {
        let mut sheet = sheet_of(Vec::new());
        sheet.drawings = (1..=4).map(line).collect();
        let mut app = app_with(sheet);
        app.interaction_state.active_canvas_mut().selected = vec![SelectedItem::new(
            Uuid::from_u128(picked),
            SelectedKind::Drawing,
        )];
        let _ = click(&mut app, path);
        let order: Vec<u128> = doc(&app)
            .drawings
            .iter()
            .map(|d| d.uuid().as_u128())
            .collect();
        assert_eq!(order, expected, "{path}");
    }
}

#[test]
fn rows_with_nothing_to_act_on_are_greyed() {
    let app = app_with(sheet_of(vec![symbol(1, "R1", 10.0, 10.0)]));
    for path in [
        "Edit/Undo",
        "Edit/Redo",
        "Edit/Cut",
        "Edit/Copy",
        "Edit/Duplicate",
        "Edit/Arrange/Bring to Front",
    ] {
        assert!(
            !row_is_live(&app, path),
            "{path} is live with nothing to act on"
        );
    }
    for path in [
        "Edit/Paste",
        "Edit/Paste Special",
        "Edit/Select All",
        "Edit/Find",
    ] {
        assert!(row_is_live(&app, path), "{path}");
    }
}
