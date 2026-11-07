//! Nothing that worked on trunk is broken (#661): every pick / undo / redo /
//! save sequence below ends in the state trunk ended in. The same table runs
//! unchanged on trunk (it uses no API this PR added).

use super::*;

#[derive(Clone, Copy)]
enum Op {
    Move,
    Pick(&'static str),
    PanelUndo,
    PanelRedo,
    PanelAdd,
    Undo,
    Redo,
    Save,
}
use Op::*;

/// What a user sees after the sequence: the class members, how far the symbol
/// moved, the sheet and project dirty marks, and what Undo / Redo offer.
fn outcome(ops: &[Op]) -> String {
    let (mut app, symbol) = app_with_sheet();
    let start = symbol_x(&app);
    // A sheet just opened from disk: its history is at the saved state.
    save_sheet(&mut app);
    for op in ops {
        match op {
            Move => move_symbol(&mut app, symbol),
            Pick(net) => pick(&mut app, net, Some("Power")),
            PanelUndo => panel(&mut app, NetClassMsg::Undo),
            PanelRedo => panel(&mut app, NetClassMsg::Redo),
            PanelAdd => panel(&mut app, NetClassMsg::Add),
            Undo => app.handle_undo_requested(),
            Redo => app.handle_redo_requested(),
            Save => save_sheet(&mut app),
        }
    }
    let target = app.undo_target(None);
    let (can_undo, can_redo) = app.undo_availability(&target);
    format!(
        "nets={:?} dx={} sheet_dirty={} project_dirty={} undo={can_undo} redo={can_redo} classes={}",
        nets(&app),
        symbol_x(&app) - start,
        sheet_dirty(&app),
        project_dirty(&app),
        class_count(&app),
    )
}

/// `(name, sequence, what trunk (c4ec02dea) shows)`. Every row is equal on this
/// branch: no lost or re-applied edit, no wrong dirty mark, no stuck Undo.
#[rustfmt::skip]
fn table() -> Vec<(&'static str, Vec<Op>, &'static str)> {
    vec![
        ("pick, undo, redo", vec![Pick("GND"), Undo, Redo], r#"nets=["GND"] dx=0 sheet_dirty=false project_dirty=true undo=true redo=false classes=1"#),
        ("move, pick, undo, redo", vec![Move, Pick("GND"), Undo, Redo], r#"nets=["GND"] dx=10 sheet_dirty=true project_dirty=true undo=true redo=false classes=1"#),
        ("move, pick, undo x2, redo x2", vec![Move, Pick("GND"), Undo, Undo, Redo, Redo], r#"nets=["GND"] dx=10 sheet_dirty=true project_dirty=true undo=true redo=false classes=1"#),
        ("pick x2, move, undo x3", vec![Pick("GND"), Pick("VCC"), Move, Undo, Undo, Undo], r#"nets=[] dx=0 sheet_dirty=false project_dirty=false undo=false redo=true classes=1"#),
        ("pick x2, move, undo x3, redo x3", vec![Pick("GND"), Pick("VCC"), Move, Undo, Undo, Undo, Redo, Redo, Redo], r#"nets=["GND", "VCC"] dx=10 sheet_dirty=true project_dirty=true undo=true redo=false classes=1"#),
        ("pick, undo, move, redo", vec![Pick("GND"), Undo, Move, Redo], r#"nets=[] dx=10 sheet_dirty=true project_dirty=false undo=true redo=false classes=1"#),
        ("move, undo, pick, redo", vec![Move, Undo, Pick("GND"), Redo], r#"nets=["GND"] dx=10 sheet_dirty=true project_dirty=true undo=true redo=false classes=1"#),
        ("move, undo, pick, redo, undo x2", vec![Move, Undo, Pick("GND"), Redo, Undo, Undo], r#"nets=[] dx=0 sheet_dirty=false project_dirty=false undo=false redo=true classes=1"#),
        ("pick, panel undo, undo", vec![Pick("GND"), PanelUndo, Undo], r#"nets=[] dx=0 sheet_dirty=false project_dirty=false undo=false redo=false classes=1"#),
        ("pick x2, panel undo, undo x2", vec![Pick("GND"), Pick("VCC"), PanelUndo, Undo, Undo], r#"nets=[] dx=0 sheet_dirty=false project_dirty=false undo=false redo=true classes=1"#),
        ("pick, panel undo, panel add, undo, redo", vec![Pick("GND"), PanelUndo, PanelAdd, Undo, Redo], r#"nets=[] dx=0 sheet_dirty=false project_dirty=true undo=false redo=false classes=2"#),
        ("pick, undo, panel redo, redo, undo", vec![Pick("GND"), Undo, PanelRedo, Redo, Undo], r#"nets=["GND"] dx=0 sheet_dirty=false project_dirty=true undo=false redo=false classes=1"#),
        ("pick, undo, panel add, redo", vec![Pick("GND"), Undo, PanelAdd, Redo], r#"nets=[] dx=0 sheet_dirty=false project_dirty=true undo=false redo=false classes=2"#),
        ("move, pick, panel undo, undo, redo x2", vec![Move, Pick("GND"), PanelUndo, Undo, Redo, Redo], r#"nets=[] dx=10 sheet_dirty=true project_dirty=false undo=true redo=false classes=1"#),
        ("pick, undo, pick, undo, redo", vec![Pick("GND"), Undo, Pick("VCC"), Undo, Redo], r#"nets=["VCC"] dx=0 sheet_dirty=false project_dirty=true undo=true redo=false classes=1"#),
        ("pick, save, undo", vec![Pick("GND"), Save, Undo], r#"nets=[] dx=0 sheet_dirty=false project_dirty=false undo=false redo=true classes=1"#),
        ("pick, save, undo, pick", vec![Pick("GND"), Save, Undo, Pick("VCC")], r#"nets=["VCC"] dx=0 sheet_dirty=false project_dirty=true undo=true redo=false classes=1"#),
        ("pick, save, undo, pick, undo", vec![Pick("GND"), Save, Undo, Pick("VCC"), Undo], r#"nets=[] dx=0 sheet_dirty=false project_dirty=false undo=false redo=true classes=1"#),
        ("pick, save, undo, redo", vec![Pick("GND"), Save, Undo, Redo], r#"nets=["GND"] dx=0 sheet_dirty=false project_dirty=true undo=true redo=false classes=1"#),
        ("move, save, pick, undo x2, redo x2", vec![Move, Save, Pick("GND"), Undo, Undo, Redo, Redo], r#"nets=["GND"] dx=10 sheet_dirty=false project_dirty=true undo=true redo=false classes=1"#),
        ("move, pick, save, move, undo x2", vec![Move, Pick("GND"), Save, Move, Undo, Undo], r#"nets=[] dx=10 sheet_dirty=false project_dirty=false undo=true redo=true classes=1"#),
        ("move, pick, save, undo x2, redo x2", vec![Move, Pick("GND"), Save, Undo, Undo, Redo, Redo], r#"nets=["GND"] dx=10 sheet_dirty=false project_dirty=true undo=true redo=false classes=1"#),
        ("save, pick, move, undo", vec![Save, Pick("GND"), Move, Undo], r#"nets=["GND"] dx=0 sheet_dirty=false project_dirty=true undo=true redo=true classes=1"#),
    ]
}

#[test]
fn no_pick_sequence_regresses_from_trunk() {
    let mut wrong = Vec::new();
    for (name, ops, trunk) in table() {
        let got = outcome(&ops);
        if got != trunk {
            wrong.push(format!("{name}\n  trunk {trunk}\n  here  {got}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "differs from trunk:\n{}",
        wrong.join("\n")
    );
}
