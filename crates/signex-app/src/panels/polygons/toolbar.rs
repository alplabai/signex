//! The tab's toolbar (mockup row 178): Repour all and Repour selected, then
//! Shelve all, Unshelve all and Shelve selected, and the live status at the
//! right edge. Repour modified only and the Auto-repour switch are not
//! drawn: a pour's copper always follows the board, so nothing is out of
//! date.

use iced::{Element, Widget as _};
use signex_i18n::{MessageId, ids, t};
use signex_widgets::doc_tab_shell::Toolbar;
use signex_widgets::text;

use super::widgets::action;
use super::{PolygonFacts, PolygonMsg, PourState, TS, msg};
use crate::panels::{PanelContext, PanelMsg};
use crate::styles::ti;

pub(super) fn view<'a>(ctx: &'a PanelContext, facts: &'a PolygonFacts) -> Toolbar<'a, PanelMsg> {
    let board = || facts.board.clone();
    let has_pours = !facts.rows.is_empty();
    let editable = has_pours && !facts.read_only;
    let ticked = !ctx.pcb_panel.polygons.selected_in(facts).is_empty();
    let shelved = facts.count(PourState::Shelved);
    let live = |on: bool, event: PolygonMsg| on.then(|| msg(event));
    Toolbar::new()
        .push(action(
            ctx,
            ids::PCB_PRIM_POUR_REPOUR_ALL,
            live(has_pours, PolygonMsg::RepourAllPressed { board: board() }),
            ids::POLYGON_MANAGER_REPOUR_ALL_TIP,
        ))
        .push(on_ticked(
            ctx,
            ticked,
            ids::POLYGON_MANAGER_REPOUR_SELECTED,
            PolygonMsg::RepourSelectedPressed { board: board() },
            ids::POLYGON_MANAGER_REPOUR_SELECTED_TIP,
        ))
        .separator()
        .push(action(
            ctx,
            ids::PCB_PRIM_POUR_SHELVE_ALL,
            live(
                editable && shelved < facts.rows.len(),
                PolygonMsg::ShelveAllPressed { board: board() },
            ),
            ids::PCB_PRIM_POUR_SHELVE_TIP,
        ))
        .push(action(
            ctx,
            ids::PCB_PRIM_POUR_UNSHELVE_ALL,
            live(
                editable && shelved > 0,
                PolygonMsg::UnshelveAllPressed { board: board() },
            ),
            ids::PCB_PRIM_POUR_SHELVE_NOTE,
        ))
        .push(on_ticked(
            ctx,
            ticked && !facts.read_only,
            ids::POLYGON_MANAGER_SHELVE_SELECTED,
            PolygonMsg::ShelveSelectedPressed { board: board() },
            ids::PCB_PRIM_POUR_SHELVE_TIP,
        ))
        .push_end(status(ctx, facts))
}

/// A button that acts on the ticked rows; inert, with a hint that says so,
/// while none is ticked.
fn on_ticked<'a>(
    ctx: &'a PanelContext,
    ticked: bool,
    label: MessageId,
    event: PolygonMsg,
    tip: MessageId,
) -> Element<'a, PanelMsg> {
    let tip = if ticked {
        tip
    } else {
        ids::POLYGON_MANAGER_SELECT_TIP
    };
    action(ctx, label, ticked.then(|| msg(event)), tip)
}

/// "11 pours · 1 shelved", and how many have no fill.
fn status<'a>(ctx: &'a PanelContext, facts: &PolygonFacts) -> Element<'a, PanelMsg> {
    let unfilled = facts.count(PourState::NotFilled);
    let id = if unfilled > 0 {
        ids::POLYGON_MANAGER_STATUS_NOT_FILLED
    } else {
        ids::POLYGON_MANAGER_STATUS
    };
    let line = t!(
        ctx.i18n,
        id,
        "pours" => facts.rows.len(),
        "shelved" => facts.count(PourState::Shelved),
        "unfilled" => unfilled
    );
    text(line).size(TS.dense).color(ti(ctx.tokens.text)).boxed()
}
