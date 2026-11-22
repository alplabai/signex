//! Picks one object of the active sheet with real canvas clicks, so the
//! selection reaches Properties the way a user's click does.

use super::helpers_f as h;
use signex_app::app::{Message, Signex, Tool};
use signex_app::toolbar::ToolMessage;
use signex_types::schematic::SelectedKind;
use uuid::Uuid;

/// Scan pitch in sheet mm, finer than the click tolerance of a thin stroke.
const STEP_MM: f64 = 0.635;

/// Click across the rectangle `from`..`to` (sheet mm) with the select tool
/// until object `id` of `kind` is picked, and return the point that hit it.
pub fn pick_in(
    app: &mut Signex,
    from: (f64, f64),
    to: (f64, f64),
    id: Uuid,
    kind: SelectedKind,
) -> Result<(f64, f64), String> {
    let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Select)));
    let (x0, x1) = (from.0.min(to.0), from.0.max(to.0));
    let (y0, y1) = (from.1.min(to.1), from.1.max(to.1));
    let columns = ((x1 - x0) / STEP_MM).ceil() as usize;
    let rows = ((y1 - y0) / STEP_MM).ceil() as usize;
    for row in 0..=rows {
        for column in 0..=columns {
            let (x, y) = (x0 + column as f64 * STEP_MM, y0 + row as f64 * STEP_MM);
            h::click(app, x, y);
            let picked = app
                .interaction_state
                .active_canvas()
                .selected
                .iter()
                .any(|item| item.uuid == id && item.kind == kind);
            if picked {
                return Ok((x, y));
            }
        }
    }
    Err(format!(
        "no click in {from:?}..{to:?} picked the {kind:?} {id}"
    ))
}
