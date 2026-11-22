//! m142, Grid Properties: Step, Multiplier, Fine and Coarse display; OK
//! writes them, Cancel discards them, a bad step is refused.

use signex_app::app::{GridPropertiesMsg, Message};
use signex_app::library::editor::footprint::state::GridDisplay;
use signex_library::Footprint;

use super::support::{FpRig, near, open_footprint};
use crate::settle_support::settle;

fn rig() -> FpRig {
    let mut rig = open_footprint(Footprint::empty("grid"));
    grid(&mut rig, GridPropertiesMsg::Open);
    rig
}

fn grid(rig: &mut FpRig, msg: GridPropertiesMsg) {
    settle(&mut rig.app, Message::GridProperties(msg));
}

fn is_open(rig: &FpRig) -> bool {
    rig.app.ui_state.grid_properties.is_some()
}

#[test]
fn m142_ok_writes_step_multiplier_and_both_displays() {
    let mut rig = rig();

    grid(&mut rig, GridPropertiesMsg::SetStep("0.25".into()));
    grid(&mut rig, GridPropertiesMsg::SetMultiplier(2));
    grid(
        &mut rig,
        GridPropertiesMsg::SetFineDisplay(GridDisplay::Dots),
    );
    grid(
        &mut rig,
        GridPropertiesMsg::SetCoarseDisplay(GridDisplay::Hidden),
    );
    grid(&mut rig, GridPropertiesMsg::Apply);

    assert!(!is_open(&rig));
    let snap = &rig.editor().state.snap_options;
    assert!(near(snap.grid_step_mm, 0.25));
    assert_eq!(snap.coarse_multiplier, 2);
    assert_eq!(snap.fine_grid_display, GridDisplay::Dots);
    assert_eq!(snap.coarse_grid_display, GridDisplay::Hidden);
}

#[test]
fn m142_cancel_discards_every_field() {
    let mut rig = rig();
    let before = rig.editor().state.snap_options;

    grid(&mut rig, GridPropertiesMsg::SetStep("0.25".into()));
    grid(&mut rig, GridPropertiesMsg::SetMultiplier(10));
    grid(
        &mut rig,
        GridPropertiesMsg::SetFineDisplay(GridDisplay::Hidden),
    );
    grid(&mut rig, GridPropertiesMsg::CancelPressed);

    assert!(!is_open(&rig));
    let snap = &rig.editor().state.snap_options;
    assert_eq!(snap.grid_step_mm, before.grid_step_mm);
    assert_eq!(snap.coarse_multiplier, before.coarse_multiplier);
    assert_eq!(snap.fine_grid_display, before.fine_grid_display);
}

#[test]
fn m142_a_bad_step_is_refused_and_the_dialog_stays_open() {
    for bad in ["0", "-0.5", "nan", "abc"] {
        let mut rig = rig();
        let before = rig.editor().state.snap_options.grid_step_mm;

        grid(&mut rig, GridPropertiesMsg::SetStep(bad.into()));
        grid(&mut rig, GridPropertiesMsg::Apply);

        assert_eq!(
            rig.editor().state.snap_options.grid_step_mm,
            before,
            "typed {bad:?}"
        );
        assert!(is_open(&rig), "typed {bad:?}");
    }
}

#[test]
fn m142_the_preview_expand_button_toggles_and_writes_nothing() {
    let mut rig = rig();
    let before = rig.editor().state.snap_options.grid_step_mm;
    let expanded = |rig: &FpRig| {
        rig.app
            .ui_state
            .grid_properties
            .as_ref()
            .is_some_and(|state| state.preview_expanded)
    };

    grid(&mut rig, GridPropertiesMsg::PreviewExpandToggled);
    assert!(expanded(&rig));
    grid(&mut rig, GridPropertiesMsg::PreviewExpandToggled);
    assert!(!expanded(&rig));

    assert_eq!(rig.editor().state.snap_options.grid_step_mm, before);
    assert!(!rig.dirty());
}
