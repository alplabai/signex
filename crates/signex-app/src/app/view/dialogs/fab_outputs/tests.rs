//! Mockup row 115 on screen: the group counts and the footer count follow the ticks,
//! the ODB++ folder form is deaf until ODB++ is ticked, a folder glyph leads
//! the output folder, and after a run the footer offers Generate again and
//! Close over the notes.

use std::path::PathBuf;

use iced::Size;
use signex_i18n::{ids, t};
use signex_output::fab::FabNote;

use super::kit;
use crate::app::fab_outputs::{
    FabOutputKinds, FabOutputsMsg, FabOutputsState, FabWritten, NoteSeverity,
};
use crate::app::{Message, PcbMsg, Signex};
use crate::layout_probe::{
    find, lay_out_inputs, lay_out_texts, messages_for_events, messages_on_click_at,
};

const WINDOW: Size = Size::new(1400.0, 900.0);

/// The kinds of mockup row 115: five ticked.
fn mockup_kinds() -> FabOutputKinds {
    FabOutputKinds {
        gerber: true,
        drill: true,
        centroid: true,
        test_netlist: true,
        ipc2581: false,
        odb: false,
        odb_folder: false,
        assembly_pdf: true,
        assembly_svg: false,
        step: false,
    }
}

fn app_with(kinds: FabOutputKinds) -> Signex {
    let (mut app, _boot) = Signex::new();
    let mut state = FabOutputsState::new(PathBuf::from("dcdc.snxpcb"), "dcdc.snxpcb".into());
    state.kinds = kinds;
    app.ui_state.fab_outputs = Some(Box::new(state));
    app
}

fn texts(app: &Signex) -> Vec<String> {
    lay_out_texts(app.view_fab_outputs_dialog(), WINDOW)
        .into_iter()
        .map(|t| t.text)
        .collect()
}

fn has(texts: &[String], want: &str) -> bool {
    texts.iter().any(|t| t == want)
}

#[test]
fn the_group_counts_and_the_footer_count_follow_the_ticks() {
    let app = app_with(mockup_kinds());
    let i18n = app.ui_state.i18n.clone();
    let shown = texts(&app);
    let count = |on: usize, total: usize| {
        t!(i18n, ids::FAB_OUTPUTS_GROUP_COUNT, "ticked" => on, "total" => total).into_string()
    };
    for (on, total) in [(3, 3), (0, 2), (2, 3), (0, 1)] {
        assert!(has(&shown, &count(on, total)), "{on} of {total}: {shown:?}");
    }
    let five = t!(i18n, ids::FAB_OUTPUTS_SELECTED, "count" => 5usize).into_string();
    assert!(has(&shown, &five), "{shown:?}");
    assert!(has(
        &shown,
        &t!(i18n, ids::FAB_OUTPUTS_CANCEL).into_string()
    ));
    assert!(has(
        &shown,
        &t!(i18n, ids::FAB_OUTPUTS_GENERATE).into_string()
    ));
    let again = t!(i18n, ids::FAB_OUTPUTS_GENERATE_AGAIN).into_string();
    assert!(!has(&shown, &again), "no Generate again before a run");

    let mut kinds = mockup_kinds();
    kinds.step = true;
    kinds.odb = true;
    let shown = texts(&app_with(kinds));
    assert!(has(&shown, &count(1, 2)), "{shown:?}");
    assert!(has(&shown, &count(1, 1)), "{shown:?}");
    let seven = t!(i18n, ids::FAB_OUTPUTS_SELECTED, "count" => 7usize).into_string();
    assert!(has(&shown, &seven), "{shown:?}");
}

#[test]
fn the_odb_folder_form_takes_clicks_only_once_odb_is_ticked() {
    let mut kinds = mockup_kinds();
    kinds.odb = true;
    let live = app_with(kinds);
    let i18n = live.ui_state.i18n.clone();
    let label = t!(i18n, ids::FAB_OUTPUTS_ODB_FOLDER).into_string();
    let laid = lay_out_texts(live.view_fab_outputs_dialog(), WINDOW);
    let at = find(&laid, &label).bounds.center();
    let sent = messages_on_click_at(live.view_fab_outputs_dialog(), WINDOW, None, at);
    assert!(
        sent.iter().any(|m| matches!(
            m,
            Message::Pcb(PcbMsg::FabOutputs(FabOutputsMsg::OdbFolderToggled))
        )),
        "a live form toggles"
    );

    // Unticked ODB++ leaves the row in place, greyed: the same spot is deaf.
    let deaf = app_with(mockup_kinds());
    let sent = messages_on_click_at(deaf.view_fab_outputs_dialog(), WINDOW, None, at);
    assert!(
        !sent.iter().any(|m| matches!(
            m,
            Message::Pcb(PcbMsg::FabOutputs(FabOutputsMsg::OdbFolderToggled))
        )),
        "greyed until ODB++ is ticked"
    );
}

/// The app with a file name and an output folder typed in.
fn app_with_destination() -> Signex {
    let mut app = app_with(mockup_kinds());
    if let Some(state) = app.ui_state.fab_outputs.as_mut() {
        state.name = "dcdc".into();
        state.folder = "fab-out".into();
    }
    app
}

/// Mockup row 115: a folder glyph leads the Output folder path inside its
/// field. The input's own box is the field outline, lined up with the file
/// name above it, so it lights on hover and focus as the file name does,
/// and the path starts after the glyph.
#[test]
fn the_output_folder_path_is_led_by_a_folder_glyph() {
    let app = app_with_destination();
    let inputs = lay_out_inputs(app.view_fab_outputs_dialog(), WINDOW);
    let name = find(&inputs, "dcdc").bounds;
    let folder = find(&inputs, "fab-out").bounds;
    assert_eq!(
        folder.x, name.x,
        "the folder input is the field's outline: name {name:?}, folder {folder:?}"
    );
    let glyph = kit::field_glyph_bounds(folder);
    assert!(
        folder.x + kit::GLYPH_TEXT_INSET >= glyph.x + glyph.width,
        "the path starts after the glyph: {glyph:?}"
    );
}

/// Mockup row 115: the folder glyph takes no click, so a click on it
/// focuses the Output folder and the next key edits it.
#[test]
fn a_click_on_the_folder_glyph_focuses_the_output_folder() {
    let app = app_with_destination();
    let inputs = lay_out_inputs(app.view_fab_outputs_dialog(), WINDOW);
    let at = kit::field_glyph_bounds(find(&inputs, "fab-out").bounds).center();
    let sent = messages_for_events(
        app.view_fab_outputs_dialog(),
        WINDOW,
        at,
        &kit::click_then_type("x"),
    );
    assert!(
        sent.iter().any(|m| matches!(
            m,
            Message::Pcb(PcbMsg::FabOutputs(FabOutputsMsg::FolderEdited(v))) if v.contains('x')
        )),
        "{sent:?}"
    );
}

#[test]
fn after_a_run_the_footer_offers_generate_again_and_close_over_the_notes() {
    let mut app = app_with(mockup_kinds());
    let i18n = app.ui_state.i18n.clone();
    let written = FabWritten {
        files: vec!["dcdc-job.gbrjob".into(); 14],
        folder: PathBuf::from("fab"),
        notes: vec![FabNote::ZonesNotFilled { count: 1 }],
        board_notes: Vec::new(),
        stale: vec!["dcdc-old.drl".into()],
    };
    let notes = crate::app::fab_outputs::outcome_notes(&written, &i18n);
    assert_eq!(notes.last().map(|n| n.0), Some(NoteSeverity::Error));
    assert_eq!(
        super::outcome::note_tone(NoteSeverity::Error),
        crate::styles::PillTone::Err,
        "a stale file is drawn as an error"
    );
    if let Some(state) = app.ui_state.fab_outputs.as_mut() {
        state.outcome = Some(Ok(written.clone()));
        state.generated_at = chrono::NaiveTime::from_hms_opt(14, 36, 0);
    }
    let shown = texts(&app);
    for id in [
        ids::FAB_OUTPUTS_GENERATE_AGAIN,
        ids::FAB_OUTPUTS_CLOSE,
        ids::FAB_OUTPUTS_OPEN_FOLDER,
    ] {
        let words = t!(i18n, id).into_string();
        assert!(has(&shown, &words), "{words}: {shown:?}");
    }
    let generated = t!(i18n, ids::FAB_OUTPUTS_GENERATED, "time" => "14:36").into_string();
    assert!(has(&shown, &generated), "{shown:?}");
    for (_, words) in notes {
        let words = words.into_string();
        assert!(has(&shown, &words), "the note {words}: {shown:?}");
    }
    let cancel = t!(i18n, ids::FAB_OUTPUTS_CANCEL).into_string();
    assert!(!has(&shown, &cancel), "Close, not Cancel, after a run");
    let selected = t!(i18n, ids::FAB_OUTPUTS_SELECTED, "count" => 5usize).into_string();
    assert!(!has(&shown, &selected), "the footer gives the time instead");
}
