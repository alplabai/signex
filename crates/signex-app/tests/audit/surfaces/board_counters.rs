//! The board counter popovers of mockup row 164 with something to list: a
//! board with open connections, a DRC run with edits after it, and two
//! selected parts. Every state comes from the fixture board and real events.

use super::helpers_d::reached;
use super::helpers_f as h;
use super::states_drawn::{counter_popover, show_counter};
use crate::fixture::{self, Fixture};
use crate::render;
use pcb_geom::{ClickMode, ItemRef};
use signex_app::app::{Message, PcbMsg, Signex};
use signex_app::pcb_canvas::PcbCanvasMsg;
use signex_app::status_bar::BoardCounter;
use signex_types::format::SnxPcb;
use signex_types::pcb::{PcbBoard, Point};
use std::time::Duration;
use uuid::Uuid;

/// R1 and D2 of the fixture board: their origins lie between their pads.
const R1: (f64, f64) = (41.0, 15.0);
const D2: (f64, f64) = (36.0, 15.0);
/// Nets +12V, +5V and LED_A, whose tracks the Unrouted state takes up.
const OPENED_NETS: [u32; 3] = [3, 4, 5];
/// How long ago the DRC counter says the check ran.
const DRC_AGE: Duration = Duration::from_secs(120);
/// Written rules for the width and silkscreen faults; a loose board reads
/// them from this machine's rule layer, held in memory for this row.
const DRC_RULES: &str = "\
rule widths on track require width >= 0.2mm then error
rule silk on silk require pad_clearance >= 0.01mm then warning
";
/// Net LED_A, which carries the DRC state's narrow track and stub.
const LED_NET: u32 = 5;
/// Pad 1 of J2, under the silkscreen the DRC state moves.
const J2_PAD: Point = Point { x: 44.0, y: 25.0 };

fn edit_board(fx: &Fixture, edit: impl FnOnce(&mut PcbBoard)) -> Result<(), String> {
    let text = std::fs::read_to_string(&fx.pcb).map_err(|e| format!("read board: {e}"))?;
    let mut board = SnxPcb::parse(&text)
        .map_err(|e| format!("parse board: {e:?}"))?
        .board;
    edit(&mut board);
    let text = SnxPcb::new(board)
        .write_string()
        .map_err(|e| format!("encode board: {e:?}"))?;
    std::fs::write(&fx.pcb, text).map_err(|e| format!("write board: {e}"))
}

/// The Unrouted popover on the board with three nets' tracks taken up.
pub(super) fn open_unrouted(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    edit_board(fx, |board| {
        board.segments.retain(|s| !OPENED_NETS.contains(&s.net));
    })?;
    counter_popover(app, r, fx, BoardCounter::Unrouted)
}

/// Drag the part at `from` to `to` and drop it: one board edit.
fn move_part(app: &mut Signex, from: (f64, f64), to: (f64, f64)) -> Result<(), String> {
    let (from, to) = (Point::new(from.0, from.1), Point::new(to.0, to.1));
    h::settle(
        app,
        Message::PcbCanvas(PcbCanvasMsg::FootprintDragged {
            from,
            to,
            tolerance_mm: 0.05,
            leaves_tracks: false,
        }),
    )?;
    h::settle(
        app,
        Message::PcbCanvas(PcbCanvasMsg::FootprintDropped { from, to }),
    )
}

/// Real faults for the check to find: a track under the board's minimum
/// width, a stub too close to the +5V track and silkscreen over a pad.
fn add_drc_faults(board: &mut PcbBoard) -> Result<(), String> {
    let led = board
        .segments
        .iter_mut()
        .find(|s| s.net == LED_NET)
        .ok_or("the board has no LED_A track")?;
    led.width = 0.1;
    let mut stub = led.clone();
    stub.uuid = Uuid::now_v7();
    stub.end = Point::new(40.6, 18.0);
    stub.start = led.start;
    stub.width = 0.2;
    board.segments.push(stub);
    let silk = board
        .texts
        .iter_mut()
        .find(|t| t.text.contains("7805"))
        .ok_or("the board has no silkscreen title")?;
    silk.position = J2_PAD;
    Ok(())
}

/// The DRC popover after a real run, two edits and two minutes.
pub(super) fn open_drc(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    let mut faults = Ok(());
    edit_board(fx, |board| faults = add_drc_faults(board))?;
    faults?;
    app.ui_state.machine_drc_rules.dsl = Some(DRC_RULES.to_string());
    fixture::open_board(app, r, fx);
    h::unlogged(|| -> Result<(), String> {
        h::settle(app, Message::Pcb(PcbMsg::DrcRequested))?;
        let moved = (R1.0 + 1.0, R1.1);
        move_part(app, R1, moved)?;
        move_part(app, moved, R1)
    })?;
    let revision = app
        .document_state
        .pcb_revisions
        .get(&fx.pcb)
        .copied()
        .unwrap_or(0);
    let done = app
        .interaction_state
        .pcb_drc
        .done_for(&fx.pcb)
        .ok_or("the DRC run did not finish")?;
    let (found, edits, finished) = (
        done.report.violations.len(),
        done.edits_since(revision),
        done.finished_at,
    );
    reached(found > 0, "the DRC run found no violation")?;
    reached(edits == 2, &format!("{edits} edits since the run, not 2"))?;
    app.ui_state.board_status.now = Some(finished + DRC_AGE);
    show_counter(app, BoardCounter::Drc)
}

/// The Selection popover over two parts picked with real clicks.
pub(super) fn open_selection(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    fixture::open_board(app, r, fx);
    for (at, mode) in [(R1, ClickMode::Replace), (D2, ClickMode::Add)] {
        h::settle(
            app,
            Message::PcbCanvas(PcbCanvasMsg::Clicked {
                at: Point::new(at.0, at.1),
                tolerance_mm: 0.05,
                mode,
            }),
        )?;
    }
    let _ = render::draw_main(r, app);
    let selection = app.interaction_state.pcb_selection.selection();
    let parts = selection
        .items()
        .filter(|item| matches!(item, ItemRef::Footprint(_)))
        .count();
    reached(
        parts == 2 && selection.len() == 2,
        &format!("{} selected, {parts} of them parts", selection.len()),
    )?;
    show_counter(app, BoardCounter::Selected)
}
