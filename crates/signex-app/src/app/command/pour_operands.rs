//! A polygon pour's corners (#2578) and fill (#2579) for an agent or the
//! CLI: `set_pour_corners` and `set_pour_fill` act on the board `document`
//! names, never on the active tab, run the same engine step as the pour
//! panel's Corners and Fill Mode rows (`zone_edits_command`), one undo step
//! each, and answer with what the pours then hold and what their fill came
//! to.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Value, json};
use signex_engine::pcb::primitive_edit::{ZoneEdit, zone_edits_command};
use signex_i18n::{MessageId, ids, t};
use signex_types::zone_pour_shape::{PourCorners, PourFillMode};
use uuid::Uuid;

use super::CommandArgs;
use super::board_edit_operands::{Ran, answered, board, named_board, refused, text, uuids};
use super::operand_mm::operand_mm_or_zero;
use crate::app::Signex;
use crate::app::handlers::pcb_pour::pour_report;

/// `set_pour_corners document=.. ids=.. corners=sharp|rounded|chamfer
/// [corner_size=<mm>]`: the pours' Corners, and their Corner Size when
/// given (0 or more).
pub(super) fn set_pour_corners(app: &mut Signex, args: &CommandArgs) -> Ran {
    let corners = match text(args, "corners").map(str::trim) {
        Some(token) => match PourCorners::from_token(token) {
            Some(corners) => corners,
            None => {
                return refused(format!(
                    "`{token}` is not a corner style (sharp, rounded, chamfer)"
                ));
            }
        },
        None => return refused("`set_pour_corners` needs the operand `corners`"),
    };
    let mut edits = vec![ZoneEdit::Corners(corners)];
    match length(args, "corner_size") {
        Ok(Some(size)) => edits.push(ZoneEdit::CornerSize(size)),
        Ok(None) => {}
        Err(why) => return refused(why),
    }
    run(
        app,
        args,
        "`set_pour_corners`",
        &edits,
        ids::PCB_PRIM_POUR_CORNERS,
    )
}

/// `set_pour_fill document=.. ids=.. fill=solid|hatched [width=<mm>]
/// [gap=<mm>]`: the pours' Fill Mode, and their hatch width and gap when
/// given (0 or more; 0 uses the default). There is no `none`: an outline
/// that holds no copper is a shelved pour.
pub(super) fn set_pour_fill(app: &mut Signex, args: &CommandArgs) -> Ran {
    let mode = match text(args, "fill").map(str::trim) {
        Some(token) => match PourFillMode::from_token(token) {
            Some(mode) => mode,
            None => {
                return refused(format!(
                    "`{token}` is not a pour fill (solid, hatched); a pour that keeps \
                     only its outline is shelved"
                ));
            }
        },
        None => return refused("`set_pour_fill` needs the operand `fill`"),
    };
    let mut edits = vec![ZoneEdit::FillMode(mode)];
    match (length(args, "width"), length(args, "gap")) {
        (Err(why), _) | (_, Err(why)) => return refused(why),
        (Ok(width), Ok(gap)) => {
            edits.extend(width.map(ZoneEdit::HatchWidth));
            edits.extend(gap.map(ZoneEdit::HatchGap));
        }
    }
    run(
        app,
        args,
        "`set_pour_fill`",
        &edits,
        ids::PCB_PRIM_POUR_FILL_MODE,
    )
}

/// The optional length operand `name`, in mm, 0 or more (the engine refuses
/// a negative one too), read by the one `operand_mm`.
fn length(args: &CommandArgs, name: &str) -> Result<Option<f64>, String> {
    match text(args, name) {
        None => Ok(None),
        Some(raw) => operand_mm_or_zero(raw).map(Some).ok_or_else(|| {
            format!("`{name}` must be a length of 0 mm or more the board can store, not `{raw}`")
        }),
    }
}

/// Set `edits` on the pours `ids` names, on the board `document` names, as
/// one undo step labelled `what`, and answer with the pours.
fn run(
    app: &mut Signex,
    args: &CommandArgs,
    name: &str,
    edits: &[ZoneEdit],
    what: MessageId,
) -> Ran {
    let path = match named_board(app, args, name) {
        Ok(path) => path,
        Err(reason) => return refused(reason),
    };
    let targets = match uuids(args) {
        Ok(targets) => targets,
        Err(reason) => return refused(reason),
    };
    let Some(b) = board(app, &path) else {
        return refused("the board is no longer open");
    };
    let all_pours = targets
        .iter()
        .all(|id| b.zones.iter().any(|z| z.uuid == *id && z.is_pour()));
    if !all_pours {
        return refused("`ids` names polygon pours of the board and nothing else");
    }
    let command = match zone_edits_command(b, &targets, edits) {
        Ok(command) => command,
        Err(refusal) => return refused(refusal.to_string()),
    };
    if let Some(command) = command {
        let label = t!(app.ui_state.i18n, what).into_string();
        if let Err(why) = app.run_board_step(&path, vec![command], &label) {
            return refused(why);
        }
    }
    answered(json!({ "document": path, "pours": held(app, &path, &targets) }))
}

/// What each pour in `targets` holds after the edit: its corners, fill and
/// hatch, and what its fill came to (`pour_report`).
fn held(app: &Signex, path: &Path, targets: &[Uuid]) -> Vec<Value> {
    let Some(b) = board(app, path) else {
        return Vec::new();
    };
    let only: BTreeSet<Uuid> = targets.iter().copied().collect();
    let fills = pcb_geom::zone_fills(b);
    let mut report = pour_report(b, &fills, Some(&only));
    for entry in &mut report {
        let zone = b
            .zones
            .iter()
            .find(|z| entry["zone"].as_str() == Some(z.uuid.to_string().as_str()));
        if let (Some(zone), Some(fields)) = (zone, entry.as_object_mut()) {
            fields.insert("corners".into(), json!(zone.corners.token()));
            fields.insert("corner_size".into(), json!(zone.corner_size));
            fields.insert("fill".into(), json!(zone.fill_mode.token()));
            fields.insert("hatch_width".into(), json!(zone.hatch_width));
            fields.insert("hatch_gap".into(), json!(zone.hatch_gap));
        }
    }
    report
}
