//! Mockup row 193 by id: the commands that select a pad or a plane, flip
//! the board, put the holes on top, highlight a net and show the Open menu,
//! on the GUI path (the palette's toolbar messages through `Signex::update`,
//! then the laid-out tab) and on the headless path (their JSON answers). And
//! the recent-package check failure as a catalog warning in Messages.

use std::path::PathBuf;
use std::sync::Arc;

use iced::Size;
use serde_json::Value;
use signex_gerber::{Package, ReadFile, assemble, read_file};
use signex_i18n::{MessageId, RenderTranslatable, ids};

use super::GerberViewerMsg;
use super::state::OpenPackage;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{Message, Signex};
use crate::keymap::AppCommandId;

/// The size the mockup shows the tab at.
const TAB: Size = Size::new(1200.0, 760.0);

const TOP: &str = "row193-L1.gbr";
const PLANE: &str = "row193-L2.gbr";

/// The top copper: a 1 mm pad on VIN_12V at the origin and one on GND at
/// x 5 mm.
fn top_copper() -> String {
    "%FSLAX46Y46*%\n%MOMM*%\n%TF.FileFunction,Copper,L1,Top*%\n%ADD10C,1.000000*%\n\
     %TO.N,VIN_12V*%\nD10*\nX0Y0D03*\n%TD*%\n%TO.N,GND*%\nX5000000Y0D03*\n%TD*%\nM02*\n"
        .to_string()
}

/// An inner plane on GND: one region from -10 to 10 mm both ways.
fn plane() -> String {
    "%FSLAX46Y46*%\n%MOMM*%\n%TF.FileFunction,Copper,L2,Inr*%\n%TO.N,GND*%\nG01*\nG36*\n\
     X-10000000Y-10000000D02*\nX10000000Y-10000000D01*\nX10000000Y10000000D01*\n\
     X-10000000Y10000000D01*\nX-10000000Y-10000000D01*\nG37*\n%TD*%\nM02*\n"
        .to_string()
}

fn package_of(files: &[(&str, String)]) -> Arc<Package> {
    let files: Vec<ReadFile> = files
        .iter()
        .map(|(name, text)| read_file(name, text.as_bytes()))
        .collect();
    Arc::new(assemble(files))
}

/// An app with the two-layer package open on the viewer tab.
fn app_with_package() -> Signex {
    let (mut app, _task) = Signex::new();
    app.open_gerber_viewer_tab();
    let package = package_of(&[(TOP, top_copper()), (PLANE, plane())]);
    let open = OpenPackage::new(package, vec![PathBuf::from("/fab/row193")]);
    app.document_state.gerber_viewer.open = Some(open);
    app
}

fn id(name: &str) -> AppCommandId {
    AppCommandId::new(name).expect("a catalog id")
}

fn args(name: &str, operands: &[&str]) -> CommandArgs {
    CommandArgs::parse_operands(&id(name), operands).expect("operands parse")
}

/// The headless path: the command's JSON answer.
fn answer(app: &mut Signex, name: &str, operands: &[&str]) -> Value {
    let (outcome, _task) = app.run_headless_command_in_test(&id(name), args(name, operands));
    match outcome {
        CommandOutcome::Answered(value) => value,
        other => panic!("{name} {operands:?}: expected an answer, got {other:?}"),
    }
}

/// The headless path: the refusal's message id.
fn refusal(app: &mut Signex, name: &str, operands: &[&str]) -> Option<MessageId> {
    let (outcome, _task) = app.run_headless_command_in_test(&id(name), args(name, operands));
    match outcome {
        CommandOutcome::Disabled { reason } => reason.id(),
        other => panic!("{name} {operands:?}: expected a refusal, got {other:?}"),
    }
}

fn open(app: &Signex) -> &OpenPackage {
    app.document_state
        .gerber_viewer
        .open
        .as_ref()
        .expect("a package")
}

/// The texts of the laid-out tab.
fn shown(app: &Signex) -> Vec<String> {
    crate::layout_probe::lay_out_texts(app.view_gerber_viewer_tab(), TAB)
        .into_iter()
        .map(|laid| laid.text)
        .collect()
}

fn shows(app: &Signex, text: &str) -> bool {
    shown(app).iter().any(|laid| laid.contains(text))
}

/// The palette, a key or a menu row sends the toolbar's own message.
#[test]
fn the_palette_forms_send_the_toolbar_messages() {
    use crate::app::command::bridge::core_to_message;
    use GerberViewerMsg as M;
    for (name, sent) in [
        ("select_gerber_object", M::SelectionCleared),
        ("flip_gerber_view", M::FlipToggled),
        ("show_gerber_drills_on_top", M::DrillsToggled),
        ("highlight_gerber_net", M::NetHighlightToggled),
        ("show_gerber_open_menu", M::OpenMenuToggled),
    ] {
        let resolved = core_to_message(&id(name), &CommandArgs::none());
        assert_eq!(
            format!("{resolved:?}"),
            format!("{:?}", Some(Message::from(sent))),
            "{name}"
        );
    }
}

/// The GUI path: Flip, Drills and the Open menu from the palette change
/// the tab, and the tab lays out with the menu's rows while it shows.
#[test]
fn the_palette_flips_puts_the_holes_on_top_and_shows_the_open_menu() {
    let mut app = app_with_package();
    for name in ["flip_gerber_view", "show_gerber_drills_on_top"] {
        let (outcome, _task) = app.dispatch_command_reporting(&id(name), CommandArgs::none(), None);
        assert_eq!(outcome, CommandOutcome::Ran, "{name}");
    }
    assert!(open(&app).flipped && open(&app).drills_on_top);
    assert!(!app.document_state.gerber_viewer.open_menu);
    assert!(!shows(&app, "Open folder"));

    let menu = id("show_gerber_open_menu");
    let (outcome, _task) = app.dispatch_command_reporting(&menu, CommandArgs::none(), None);
    assert_eq!(outcome, CommandOutcome::Ran);
    assert!(app.document_state.gerber_viewer.open_menu);
    assert!(shows(&app, "Open folder"), "{:?}", shown(&app));
    let _ = app.update(Message::from(GerberViewerMsg::EscapePressed));
    assert!(!app.document_state.gerber_viewer.open_menu);
}

/// mockup row 193, "Top view, pad selected": a point selects the topmost
/// visible object, as a click does, and Properties shows its net.
#[test]
fn a_pad_selected_by_id_shows_in_properties() {
    let mut app = app_with_package();
    assert!(!shows(&app, "VIN_12V"));
    let answer = answer(&mut app, "select_gerber_object", &["at=0,0"]);
    let selected = &answer["view"]["selected"];
    assert_eq!(selected["layer"], TOP);
    assert_eq!(selected["kind"], "flash");
    assert_eq!(selected["net"], "VIN_12V");
    assert_eq!(selected["object"], 0);
    assert!(shows(&app, "VIN_12V"), "{:?}", shown(&app));
    assert!(shows(&app, "Flash"));
}

/// mockup row 193, "Plane selected (L2)": with `layer`, the object of that
/// layer under the point, though the top pad covers it.
#[test]
fn a_plane_selected_by_id_on_its_layer_under_a_pad() {
    let mut app = app_with_package();
    let at = ["at=0,0", "layer=row193-L2.gbr"];
    let answer = answer(&mut app, "select_gerber_object", &at);
    let selected = &answer["view"]["selected"];
    assert_eq!(selected["layer"], PLANE);
    assert_eq!(selected["kind"], "region");
    assert_eq!(selected["net"], "GND");
    assert!(shows(&app, "Region"), "{:?}", shown(&app));
}

/// `layer` alone picks the layer row; no operands, or a point on nothing,
/// clears the selection; the palette form clears it too.
#[test]
fn a_layer_alone_picks_its_row_and_nothing_clears() {
    let mut app = app_with_package();
    let picked = answer(&mut app, "select_gerber_object", &["layer=row193-L2.gbr"]);
    assert_eq!(picked["view"]["current_layer"], PLANE);
    assert_eq!(picked["view"]["selected"], Value::Null);

    let _ = answer(&mut app, "select_gerber_object", &["at=0,0"]);
    let missed = answer(&mut app, "select_gerber_object", &["at=50,50"]);
    assert_eq!(missed["view"]["selected"], Value::Null);

    let _ = answer(&mut app, "select_gerber_object", &["at=0,0"]);
    let cleared = answer(&mut app, "select_gerber_object", &[]);
    assert_eq!(cleared["view"]["selected"], Value::Null);

    let _ = answer(&mut app, "select_gerber_object", &["at=0,0"]);
    let (outcome, _task) =
        app.dispatch_command_reporting(&id("select_gerber_object"), CommandArgs::none(), None);
    assert_eq!(outcome, CommandOutcome::Ran);
    assert_eq!(open(&app).selected, None);
}

#[test]
fn a_selection_by_id_is_refused_with_its_reason() {
    let (mut empty, _task) = Signex::new();
    let select = "select_gerber_object";
    assert_eq!(
        refusal(&mut empty, select, &["at=0,0"]),
        Some(ids::GERBER_VIEWER_COMMAND_NO_PACKAGE)
    );
    let mut app = app_with_package();
    assert_eq!(
        refusal(&mut app, select, &["layer=nope.gbr"]),
        Some(ids::GERBER_VIEWER_COMMAND_NO_LAYER)
    );
    assert_eq!(
        refusal(&mut app, select, &["at=U1.1"]),
        Some(ids::GERBER_VIEWER_COMMAND_AT_NOT_A_POINT)
    );
    let plane = open(&app).layer_of_file(PLANE).expect("the plane");
    let _ = app.update(Message::from(GerberViewerMsg::LayerVisibilityToggled(
        plane,
    )));
    assert_eq!(
        refusal(&mut app, select, &["at=0,0", "layer=row193-L2.gbr"]),
        Some(ids::GERBER_VIEWER_COMMAND_LAYER_HIDDEN)
    );
}

/// mockup row 193, "Flipped": `on` sets the switch, its absence turns it
/// over; Drills the same.
#[test]
fn flip_and_drills_are_set_by_on_and_turned_over_without_it() {
    let mut app = app_with_package();
    for (name, key) in [
        ("flip_gerber_view", "flipped"),
        ("show_gerber_drills_on_top", "drills_on_top"),
    ] {
        assert_eq!(answer(&mut app, name, &["on=true"])["view"][key], true);
        assert_eq!(answer(&mut app, name, &["on=true"])["view"][key], true);
        assert_eq!(answer(&mut app, name, &[])["view"][key], false);
        assert_eq!(answer(&mut app, name, &[])["view"][key], true);
        assert_eq!(answer(&mut app, name, &["on=false"])["view"][key], false);
    }
    let (mut empty, _task) = Signex::new();
    assert_eq!(
        refusal(&mut empty, "flip_gerber_view", &[]),
        Some(ids::GERBER_VIEWER_COMMAND_NO_PACKAGE)
    );
}

/// mockup row 193, "Net highlighted (VIN_12V)": by name, from the
/// selection, toggled without operands, let go with `on=false`.
#[test]
fn a_net_is_highlighted_by_name_or_from_the_selection() {
    let mut app = app_with_package();
    let net = "highlight_gerber_net";
    assert_eq!(
        refusal(&mut app, net, &[]),
        Some(ids::GERBER_VIEWER_HIGHLIGHT_NET_NO_NET)
    );
    assert_eq!(
        answer(&mut app, net, &["net=VIN_12V"])["view"]["net"],
        "VIN_12V"
    );
    assert_eq!(
        refusal(&mut app, net, &["net=NOPE"]),
        Some(ids::GERBER_VIEWER_COMMAND_NO_SUCH_NET)
    );
    assert_eq!(open(&app).net.as_deref(), Some("VIN_12V"), "kept");
    assert_eq!(answer(&mut app, net, &[])["view"]["net"], Value::Null);

    let _ = answer(&mut app, "select_gerber_object", &["at=5,0"]);
    assert_eq!(answer(&mut app, net, &[])["view"]["net"], "GND");
    assert_eq!(answer(&mut app, net, &["on=true"])["view"]["net"], "GND");
    assert_eq!(
        answer(&mut app, net, &["on=false"])["view"]["net"],
        Value::Null
    );

    let (mut bare, _task) = Signex::new();
    let no_nets = "%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,1.000000*%\nD10*\nX0Y0D03*\nM02*\n";
    let package = package_of(&[("bare.gbr", no_nets.to_string())]);
    bare.document_state.gerber_viewer.open = Some(OpenPackage::new(package, Vec::new()));
    assert_eq!(
        refusal(&mut bare, net, &["on=true"]),
        Some(ids::GERBER_VIEWER_HIGHLIGHT_NET_NO_X2)
    );
}

/// mockup row 193, "Open menu": shown and closed by id, with or without a
/// package; the answer says so.
#[test]
fn the_open_menu_is_shown_and_closed_by_id() {
    let mut app = app_with_package();
    let menu = "show_gerber_open_menu";
    assert_eq!(
        answer(&mut app, menu, &["on=true"])["view"]["open_menu"],
        true
    );
    assert!(shows(&app, "Open folder"), "{:?}", shown(&app));
    assert_eq!(
        answer(&mut app, menu, &["on=false"])["view"]["open_menu"],
        false
    );
    assert!(!shows(&app, "Open folder"));
    assert_eq!(answer(&mut app, menu, &[])["view"]["open_menu"], true);

    let (mut empty, _task) = Signex::new();
    let shown_empty = answer(&mut empty, menu, &["on=true"]);
    assert_eq!(shown_empty["view"]["open_menu"], true);
    assert_eq!(shown_empty["view"]["selected"], Value::Null);
}

/// The answer of every viewer command carries the view, defaults first.
#[test]
fn the_answer_says_how_the_view_is_set() {
    let (mut app, _task) = Signex::new();
    let view = answer(&mut app, "open_gerber_viewer", &[])["view"].clone();
    assert_eq!(view["flipped"], false);
    assert_eq!(view["drills_on_top"], false);
    assert_eq!(view["unit"], "mm");
    assert_eq!(view["net"], Value::Null);
    assert_eq!(view["open_menu"], false);
    assert_eq!(view["current_layer"], Value::Null);
    assert_eq!(view["selected"], Value::Null);
}

/// The recent-package check that cannot run is a Gerber Viewer warning,
/// worded by the catalog; the list and its clock stay as they were.
#[test]
fn a_failed_recent_check_is_a_gerber_viewer_warning_from_the_catalog() {
    let _ = crate::diagnostics::said_on_this_thread();
    let marker = signex_types::object_id::short_id(&uuid::Uuid::now_v7());
    let why = format!("worker gone {marker}");
    let mark = crate::diagnostics::newest_entry_id();
    let (mut app, _task) = Signex::new();
    app.document_state.gerber_viewer.now = Some(5);
    let _ = app.update(Message::from(GerberViewerMsg::RecentCheckFailed(
        why.clone(),
    )));
    assert_eq!(app.document_state.gerber_viewer.now, Some(5));

    if crate::diagnostics::configured_level() < log::LevelFilter::Warn {
        eprintln!("skipped the Messages half: the log filter drops warnings");
        return;
    }
    let expected = signex_i18n::Translatable::new(ids::GERBER_VIEWER_RECENT_CHECK_FAILED)
        .arg("why", why.as_str())
        .render_en_us();
    let rows: Vec<_> = crate::diagnostics::entries_since(mark)
        .entries
        .into_iter()
        .filter(|entry| entry.message.contains(&why))
        .collect();
    assert_eq!(rows.len(), 1, "one Messages row: {rows:#?}");
    assert_eq!(rows[0].level, crate::diagnostics::DiagnosticLevel::Warning);
    assert_eq!(rows[0].message, expected);
    assert_eq!(
        crate::panels::MessageSource::of_code(&rows[0].code),
        crate::panels::MessageSource::GerberViewer
    );
}
