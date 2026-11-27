//! #3059 (mockup row 162): a Missing on board row for a footprint that is on
//! the board but not linked to its part shows the missing link in the Board
//! column, not the designator again, and says what Update PCB does.

use std::path::PathBuf;

use iced::Size;
use signex_engine::pcb::eco::{
    BackChangeList, CompareKind, EcoChange, EcoChangeList, EcoGroup, EcoKey, EcoRow, compare,
};
use signex_types::pcb::SymbolLink;
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use crate::app::compare_board::{CompareBoardMsg, CompareBoardState};
use crate::app::update_pcb::Phase;
use crate::app::{Message, Signex};
use crate::layout_probe::{LaidOutText, find, lay_out_texts};

/// J1 is on the board, unlinked; Update PCB links it to its part.
fn link_j1() -> EcoRow {
    let footprint = Uuid::from_u128(0xa6);
    EcoRow {
        key: EcoKey::Link(footprint),
        group: EcoGroup::Links,
        change: EcoChange::Link {
            footprint,
            reference: "J1".into(),
            link: SymbolLink::new(InstancePath::root(), [Uuid::from_u128(0x103)]),
        },
        default_on: true,
        on: true,
        blocked: None,
        needs: Vec::new(),
    }
}

/// The dialog open on a report of the one link row, ready.
fn app_with_link_row() -> Signex {
    let (mut app, _t) = Signex::new();
    app.ui_state.window_size = (1440.0, 900.0);
    let project = app.document_state.mint_project_id();
    let mut state = CompareBoardState::new(
        project,
        "p.snxsch".to_string(),
        PathBuf::from("/w/p.snxpcb"),
    );
    let eco = EcoChangeList {
        revision: 1,
        rows: vec![link_j1()],
        notes: Vec::new(),
        pad_locks: Vec::new(),
    };
    state.report = Some(compare(&eco, &BackChangeList::default()));
    state.phase = Phase::Ready;
    app.ui_state.compare_board = Some(Box::new(state));
    app
}

fn laid_out(app: &Signex) -> Vec<LaidOutText> {
    lay_out_texts(app.view_compare_board_card(), Size::new(1440.0, 900.0))
        .into_iter()
        .map(|mut t| {
            t.text = t.text.replace(['\u{2068}', '\u{2069}'], "");
            t
        })
        .collect()
}

#[test]
fn a_missing_link_row_shows_the_link_missing_and_what_update_pcb_does() {
    let mut app = app_with_link_row();
    let _ = app.update(Message::from(CompareBoardMsg::KindFilterPicked(Some(
        CompareKind::MissingOnBoard,
    ))));
    let laid = laid_out(&app);
    let texts: Vec<&str> = laid.iter().map(|t| t.text.as_str()).collect();
    // Object and Schematic name J1; the Board column does not repeat it.
    assert_eq!(texts.iter().filter(|t| **t == "J1").count(), 2, "{texts:?}");
    // The first "Board" is the column heading, before the jump link.
    let board_column = find(&laid, "Board").bounds;
    let schematic_column = find(&laid, "Schematic").bounds;
    let missing = find(&laid, "not linked").bounds;
    assert!(
        missing.x >= board_column.x - 1.0 && missing.x > schematic_column.x,
        "in the Board column: {missing:?} under {board_column:?}"
    );
    let j1_cells: Vec<f32> = laid
        .iter()
        .filter(|t| t.text == "J1")
        .map(|t| t.bounds.x)
        .collect();
    assert!(
        j1_cells.iter().all(|x| *x < board_column.x - 1.0),
        "J1 left of the Board column: {j1_cells:?}"
    );
    assert!(texts.contains(&"Link to its part"), "{texts:?}");
    assert!(!texts.contains(&"Link footprint"), "{texts:?}");
}
