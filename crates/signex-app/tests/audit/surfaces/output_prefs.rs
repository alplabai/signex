//! Mockup group `output-prefs` (rows m111-m117, m143-m150).

use crate::fixture::{self, Fixture};
use crate::settle_support;
use crate::surface::{Check, Surface, WindowSel};
use signex_app::annotate_undo::AnnotateUndo;
use signex_app::app::{
    BoardOutputDialog, BoardOutputMsg, BomPreviewMsg, FabOutputsMsg, Message, PreferencesMsg,
    PrintPreviewMsg, Signex,
};
use signex_app::menu_bar::MenuMessage;
use signex_app::preferences::{PrefMsg, PrefNav};
use signex_output::{BoardReportKind, BomRule};

pub fn rows() -> Vec<Surface> {
    vec![
        Surface {
            id: "m111",
            title: "Print Preview / Export PDF",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                settle_support::settle(app, Message::PrintPreview(PrintPreviewMsg::Requested));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: PRINT_PREVIEW,
        },
        Surface {
            id: "m112",
            title: "BOM export",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                settle_support::settle(app, Message::Menu(MenuMessage::ExportBom));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: BOM_EXPORT,
        },
        Surface {
            id: "m113",
            title: "BOM Studio",
            open: |_, _, _| {
                Err(
                    "not built: PanelKind::BomStudio is a view_stub panel; Tools > Reports > \
                     Bill of Materials opens the BOM export dialog (m112), and there is no \
                     .snxbom tab"
                        .into(),
                )
            },
            window: WindowSel::Auto,
            checks: &[],
        },
        Surface {
            id: "m114",
            title: "Output Jobs",
            open: |_, _, _| {
                Err(
                    "not built: PanelKind::OutputJobs is a view_stub panel; no File > New > \
                     Output Job and no .snxout tab"
                        .into(),
                )
            },
            window: WindowSel::Auto,
            checks: &[],
        },
        Surface {
            id: "m115",
            title: "Fabrication Outputs",
            open: |app, r, fx| {
                fixture::open_board(app, r, fx);
                settle_support::settle(app, Message::from(FabOutputsMsg::Requested));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: FAB_OUTPUTS,
        },
        Surface {
            id: "m116",
            title: "Board Print / PDF",
            open: |app, r, fx| {
                fixture::open_board(app, r, fx);
                settle_support::settle(app, Message::from(BoardOutputMsg::PrintRequested));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: BOARD_PRINT,
        },
        Surface {
            id: "m117",
            title: "Board Reports",
            // Opened on the BOM table; the other report kinds are switch states.
            open: |app, r, fx| {
                fixture::open_board(app, r, fx);
                settle_support::settle(
                    app,
                    Message::from(BoardOutputMsg::ReportRequested(BoardReportKind::Bom)),
                );
                Ok(())
            },
            window: WindowSel::Auto,
            checks: BOARD_REPORTS,
        },
        Surface {
            id: "m143",
            title: "Preferences shell + nav",
            open: |app, _, fx| open_broken_prefs(app, fx),
            window: WindowSel::Auto,
            checks: PREFERENCES,
        },
        Surface {
            id: "m144",
            title: "Appearance",
            open: |app, _, _| prefs_page(app, PrefNav::Appearance),
            window: WindowSel::Auto,
            checks: PREFERENCES,
        },
        Surface {
            id: "m144a",
            title: "Language",
            open: |app, _, _| prefs_page(app, PrefNav::Language),
            window: WindowSel::Auto,
            checks: LANGUAGE,
        },
        Surface {
            id: "m145",
            title: "Keyboard Shortcuts",
            open: |app, _, _| prefs_page(app, PrefNav::Keyboard),
            window: WindowSel::Auto,
            checks: KEYBOARD,
        },
        Surface {
            id: "m146",
            title: "Annotation",
            open: |app, _, _| prefs_page(app, PrefNav::Annotation),
            window: WindowSel::Auto,
            checks: ANNOTATION,
        },
        Surface {
            id: "m147",
            title: "Editing (+ fast grids)",
            open: |app, _, _| prefs_page(app, PrefNav::Editing),
            window: WindowSel::Auto,
            checks: EDITING,
        },
        Surface {
            id: "m148",
            title: "Rule Profiles",
            open: |app, _, _| prefs_page(app, PrefNav::RuleProfiles),
            window: WindowSel::Auto,
            checks: RULE_PROFILES,
        },
        Surface {
            id: "m149",
            title: "Distributor APIs",
            open: |app, _, _| prefs_page(app, PrefNav::LibraryDistributors),
            window: WindowSel::Auto,
            checks: DISTRIBUTORS,
        },
        Surface {
            id: "m150",
            title: "Default Component Classes",
            open: |app, _, _| prefs_page(app, PrefNav::ComponentClasses),
            window: WindowSel::Auto,
            checks: COMPONENT_CLASSES,
        },
    ]
}

/// Open Preferences on page `nav`. Drafts only, nothing is saved.
fn prefs_page(app: &mut Signex, nav: PrefNav) -> Result<(), String> {
    settle_support::settle(app, Message::Preferences(PreferencesMsg::Open));
    settle_support::settle(app, Message::Preferences(PreferencesMsg::Nav(nav)));
    Ok(())
}

/// The shell as mockup row 143 draws it: the broken-file banner, the nav
/// search in use and an unsaved change on Annotation. The banner carries the
/// loader's own error for a broken copy, so the user's prefs file is not
/// touched; it is set after Open, whose probe reads the real file.
fn open_broken_prefs(app: &mut Signex, fx: &Fixture) -> Result<(), String> {
    let broken = fx.pcb.with_file_name("broken-prefs.json");
    std::fs::write(&broken, "{ \"theme\": ").map_err(|e| format!("write prefs copy: {e}"))?;
    let error = signex_app::fonts::check_prefs_file_at(&broken)
        .err()
        .ok_or("the broken prefs copy loaded")?;
    prefs_page(app, PrefNav::Annotation)?;
    app.ui_state.prefs_load_error = Some(error.to_string());
    for msg in [
        PrefMsg::NavSearchChanged("an".into()),
        PrefMsg::DraftAnnotateUndo(AnnotateUndo::PerSheet),
    ] {
        settle_support::settle(app, inner(msg));
    }
    if app.ui_state.preferences_dirty {
        Ok(())
    } else {
        Err("the Annotate undo change left the dialog clean".into())
    }
}

fn inner(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

const PRINT_PREVIEW: &[Check] = &[("the print preview is open", print_preview_open)];

const BOM_EXPORT: &[Check] = &[
    ("the BOM export dialog is open", bom_preview_open),
    (
        "turning a rule off drops its issues",
        bom_rule_off_drops_issues,
    ),
];

const FAB_OUTPUTS: &[Check] = &[("Fabrication Outputs is open", fab_outputs_open)];

const BOARD_PRINT: &[Check] = &[("Board Print / PDF is open", board_print_open)];

const BOARD_REPORTS: &[Check] = &[
    ("Board Reports is open on the BOM", board_reports_on_bom),
    (
        "picking Net Lengths switches the table",
        board_reports_switch,
    ),
];

const PREFERENCES: &[Check] = &[("Preferences is open", preferences_open)];

const LANGUAGE: &[Check] = &[
    ("Preferences is open", preferences_open),
    ("the Language page is shown", on_language),
];

const KEYBOARD: &[Check] = &[
    ("Preferences is open", preferences_open),
    ("the Keyboard page is shown", on_keyboard),
];

const ANNOTATION: &[Check] = &[
    ("the Annotation page is shown", on_annotation),
    (
        "a scope change marks the page dirty",
        annotation_draft_dirties,
    ),
];

const EDITING: &[Check] = &[
    ("the Editing page is shown", on_editing),
    (
        "a wire-drag change marks the page dirty",
        editing_draft_dirties,
    ),
];

const RULE_PROFILES: &[Check] = &[
    ("Preferences is open", preferences_open),
    ("the Rule Profiles page is shown", on_rule_profiles),
];

const DISTRIBUTORS: &[Check] = &[
    ("Preferences is open", preferences_open),
    ("the Distributor APIs page is shown", on_distributors),
];

const COMPONENT_CLASSES: &[Check] = &[
    ("Preferences is open", preferences_open),
    ("the Component Classes page is shown", on_component_classes),
];

fn flag(on: bool, what: &str) -> Result<(), String> {
    if on {
        Ok(())
    } else {
        Err(format!("{what} is false"))
    }
}

fn print_preview_open(app: &mut Signex) -> Result<(), String> {
    flag(app.document_state.preview.is_some(), "preview.is_some()")
}

fn bom_preview_open(app: &mut Signex) -> Result<(), String> {
    flag(
        app.document_state.bom_preview.is_some(),
        "bom_preview.is_some()",
    )
}

fn bom_rule_off_drops_issues(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::BomPreview(BomPreviewMsg::SetRule(
        BomRule::MissingMpn,
        false,
    )));
    let preview = app
        .document_state
        .bom_preview
        .as_ref()
        .ok_or("the BOM dialog closed")?;
    match preview
        .validation_report
        .issues
        .iter()
        .any(|i| i.rule == BomRule::MissingMpn)
    {
        false => Ok(()),
        true => Err("MissingMpn issues are still listed with the rule off".into()),
    }
}

fn fab_outputs_open(app: &mut Signex) -> Result<(), String> {
    flag(app.ui_state.fab_outputs.is_some(), "fab_outputs.is_some()")
}

fn board_print_open(app: &mut Signex) -> Result<(), String> {
    match app.ui_state.board_output.dialog.as_deref() {
        Some(BoardOutputDialog::Print(_)) => Ok(()),
        Some(BoardOutputDialog::Reports(_)) => Err("Board Reports opened instead".into()),
        None => Err("no board output dialog".into()),
    }
}

fn report_kind(app: &Signex) -> Result<BoardReportKind, String> {
    match app.ui_state.board_output.dialog.as_deref() {
        Some(BoardOutputDialog::Reports(state)) => Ok(state.report.kind),
        Some(BoardOutputDialog::Print(_)) => Err("Board Print opened instead".into()),
        None => Err("no board output dialog".into()),
    }
}

fn board_reports_on_bom(app: &mut Signex) -> Result<(), String> {
    match report_kind(app)? {
        BoardReportKind::Bom => Ok(()),
        other => Err(format!("opened on {other:?}")),
    }
}

fn board_reports_switch(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::from(BoardOutputMsg::ReportPicked(
        BoardReportKind::NetLength,
    )));
    match report_kind(app)? {
        BoardReportKind::NetLength => Ok(()),
        other => Err(format!("still on {other:?}")),
    }
}

fn preferences_open(app: &mut Signex) -> Result<(), String> {
    flag(app.ui_state.preferences_open, "preferences_open")
}

fn page_is(app: &Signex, want: PrefNav) -> Result<(), String> {
    match app.ui_state.preferences_nav {
        nav if nav == want => Ok(()),
        nav => Err(format!("Preferences shows {nav:?}, not {want:?}")),
    }
}

fn on_language(app: &mut Signex) -> Result<(), String> {
    page_is(app, PrefNav::Language)
}

fn on_keyboard(app: &mut Signex) -> Result<(), String> {
    page_is(app, PrefNav::Keyboard)
}

fn on_annotation(app: &mut Signex) -> Result<(), String> {
    page_is(app, PrefNav::Annotation)
}

fn on_editing(app: &mut Signex) -> Result<(), String> {
    page_is(app, PrefNav::Editing)
}

fn on_rule_profiles(app: &mut Signex) -> Result<(), String> {
    page_is(app, PrefNav::RuleProfiles)
}

fn on_distributors(app: &mut Signex) -> Result<(), String> {
    page_is(app, PrefNav::LibraryDistributors)
}

fn on_component_classes(app: &mut Signex) -> Result<(), String> {
    page_is(app, PrefNav::ComponentClasses)
}

fn page_dirty(app: &Signex, page: PrefNav) -> Result<(), String> {
    flag(
        app.ui_state.preferences_pages.dirty_pages.contains(&page),
        &format!("{page:?} in dirty_pages"),
    )
}

fn annotation_draft_dirties(app: &mut Signex) -> Result<(), String> {
    let other = match app.ui_state.annotate_undo {
        AnnotateUndo::Project => AnnotateUndo::PerSheet,
        AnnotateUndo::PerSheet => AnnotateUndo::Project,
    };
    let _ = app.update(inner(PrefMsg::DraftAnnotateUndo(other)));
    page_dirty(app, PrefNav::Annotation)
}

fn editing_draft_dirties(app: &mut Signex) -> Result<(), String> {
    let keeps_wires = app.ui_state.move_keeps_wires;
    let _ = app.update(inner(PrefMsg::DraftMoveKeepsWires(!keeps_wires)));
    page_dirty(app, PrefNav::Editing)
}
