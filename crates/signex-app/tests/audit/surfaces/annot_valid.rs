//! Mockup group `annot-valid` (rows m75-m92).
//!
//! Every row is mapped.

use super::checks;
use super::helpers_d::{self, command, reached};
use super::states;
use crate::fixture::{self, Fixture};
use crate::surface::{Check, Surface, WindowSel};
use crate::{render, settle_support};
use signex_app::app::{
    AnnotateMsg, AssignFootprintsMsg, ErcMsg, Message, PreferencesMsg, ProjectMsg, RulesTabMsg,
    SheetNumberingMsg, SheetSyncMsg, Signex,
};
use signex_app::library::reprefix_lookup::{ReprefixLookup, RowFacts};
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::PanelKind;
use signex_app::preferences::{PrefMsg, PrefNav, ProfilePageMsg, ProfilesMsg};
use signex_drc_dsl::table::catalog::Domain;
use signex_types::schematic::{
    ChildSheet, FillType, Label, LabelType, Point, SchematicSheet, SelectedItem, SelectedKind,
};
use std::collections::HashMap;
use std::sync::Arc;

pub fn rows() -> Vec<Surface> {
    vec![
        Surface {
            id: "m75",
            title: "Annotate Schematics",
            open: states::open_annotate_hierarchy,
            window: WindowSel::Auto,
            checks: ANNOTATE,
        },
        Surface {
            id: "m76",
            title: "Reset annotation confirm",
            // Annotate > Reset: the confirm over the open Annotate dialog.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_d::sample_project(app, fx);
                settle_support::settle(app, Message::Annotate(AnnotateMsg::OpenDialog));
                settle_support::settle(app, Message::Annotate(AnnotateMsg::OpenResetConfirm));
                reached(
                    app.ui_state.annotate_reset_confirm,
                    "AnnotateMsg::OpenResetConfirm left annotate_reset_confirm false",
                )
            },
            window: WindowSel::Auto,
            checks: RESET_CONFIRM,
        },
        Surface {
            id: "m77",
            title: "Re-prefix to standard confirm",
            open: open_reprefix,
            window: WindowSel::Auto,
            checks: REPREFIX,
        },
        Surface {
            id: "m78",
            title: "Number Schematic Sheets",
            open: states::open_numbering_clash,
            window: WindowSel::Auto,
            checks: NUMBERING,
        },
        Surface {
            id: "m79",
            title: "Synchronize Sheet Entries and Ports",
            open: open_sheet_sync,
            window: WindowSel::Auto,
            checks: SHEET_SYNC,
        },
        Surface {
            id: "m80",
            title: "Assign Footprints",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                fixture::mount_stock(app, fx)?;
                helpers_d::sample_project(app, fx);
                command(app, "assign_footprints")?;
                reached(
                    app.ui_state.assign_footprints.is_some(),
                    "assign_footprints opened no dialog on the sample project",
                )
            },
            window: WindowSel::Auto,
            checks: ASSIGN,
        },
        Surface {
            id: "m81",
            title: "Update PCB (ECO)",
            // From the schematic tab, the project naming the board copy.
            open: |app, r, fx| {
                helpers_d::sample_with_board(app, r, fx)?;
                command(app, "update_pcb_from_schematic")?;
                reached(
                    app.ui_state.update_pcb.is_some(),
                    "update_pcb_from_schematic opened no dialog from the sample sheet",
                )
            },
            window: WindowSel::Auto,
            checks: UPDATE_PCB,
        },
        Surface {
            id: "m82",
            title: "Back Annotate Schematics",
            open: |app, r, fx| {
                helpers_d::sample_with_board(app, r, fx)?;
                command(app, "back_annotate_schematics")?;
                reached(
                    app.ui_state.back_annotate.is_some(),
                    "back_annotate_schematics opened no dialog from the sample sheet",
                )
            },
            window: WindowSel::Auto,
            checks: BACK_ANNOTATE,
        },
        Surface {
            id: "m83",
            title: "Re-Annotate Board",
            open: |app, r, fx| {
                fixture::open_board(app, r, fx);
                command(app, "reannotate_board")?;
                reached(
                    app.ui_state.board_reannotate.is_some(),
                    "reannotate_board opened no dialog on the board",
                )
            },
            window: WindowSel::Auto,
            checks: REANNOTATE,
        },
        Surface {
            id: "m84",
            title: "ERC settings / run dialog",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_d::sample_project(app, fx);
                settle_support::settle(app, Message::Erc(ErcMsg::OpenDialog));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: ERC_DIALOG,
        },
        Surface {
            id: "m85",
            title: "ERC panel (+ waivers)",
            // After a run, so the panel lists results. The waive form and the
            // stale banner of the mockup are other states.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_d::sample_project(app, fx);
                settle_support::settle(app, Message::Erc(ErcMsg::Run));
                settle_support::settle(app, Message::Menu(MenuMessage::OpenErcPanel));
                fixture::select_panel(app, PanelKind::Erc)?;
                let _ = render::draw_main(r, app);
                Ok(())
            },
            window: WindowSel::Auto,
            checks: ERC_PANEL,
        },
        Surface {
            id: "m86",
            title: "Messages panel",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                // Two real log entries, so the Messages tab shows rows and its
                // count.
                signex_app::diagnostics::log_warning("Unconnected input pin TS on U1 (BQ24072)");
                signex_app::diagnostics::log_warning(
                    "Unconnected output pin PGOOD on U1 (BQ24072)",
                );
                let _ = app.update(Message::Preferences(PreferencesMsg::Inner(
                    PrefMsg::DraftTheme(app.ui_state.theme_id),
                )));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: MESSAGES,
        },
        Surface {
            id: "m87",
            title: "DRC panel",
            // Before the first run: the idle state.
            open: |app, r, fx| {
                fixture::open_board(app, r, fx);
                command(app, "open_drc_panel")?;
                fixture::select_panel(app, PanelKind::Drc)
            },
            window: WindowSel::Auto,
            checks: DRC_PANEL,
        },
        Surface {
            id: "m88",
            title: "DRC results",
            open: states::open_drc_findings,
            window: WindowSel::Auto,
            checks: DRC_PANEL,
        },
        Surface {
            id: "m89",
            title: "Rules tab (constraint editor)",
            // The Physical domain, as the mockup shows it; the selected cell
            // and the find box are other states.
            open: |app, r, fx| {
                let project = open_rules(app, r, fx)?;
                settle_support::settle(
                    app,
                    Message::Project(ProjectMsg::Rules(RulesTabMsg::DomainPicked {
                        project_id: project,
                        domain: Domain::Physical,
                    })),
                );
                Ok(())
            },
            window: WindowSel::Auto,
            checks: RULES,
        },
        Surface {
            id: "m90",
            title: "Rules tab: clearance matrix",
            // `ClearanceView` is not exported: the toolbar chip is clicked.
            open: super::states_drawn::open_clearance_matrix,
            window: WindowSel::Auto,
            checks: &[],
        },
        Surface {
            id: "m91",
            title: "Rules tab: presets / text view",
            // The text view with a syntax error. The Presets side panel is
            // `m91-presets`.
            open: states::open_rules_error,
            window: WindowSel::Auto,
            checks: RULES_TEXT,
        },
        Surface {
            id: "m92",
            title: "Rule editor",
            // The shared ERC rule editor, opened from Preferences > Rule
            // Profiles as the mockup shows it.
            open: |app, _, _| {
                settle_support::settle(app, Message::Preferences(PreferencesMsg::Open));
                settle_support::settle(
                    app,
                    Message::Preferences(PreferencesMsg::Nav(PrefNav::RuleProfiles)),
                );
                settle_support::settle(
                    app,
                    Message::Preferences(PreferencesMsg::Inner(PrefMsg::Profiles(
                        ProfilesMsg::Selected("Signex Default".into()),
                    ))),
                );
                // The decided one-rule editor, as a click on a rule opens it.
                settle_support::settle(
                    app,
                    Message::Preferences(PreferencesMsg::Inner(PrefMsg::Profiles(
                        ProfilesMsg::Page(ProfilePageMsg::RuleOpened(
                            signex_erc::RuleKind::UnusedPin,
                        )),
                    ))),
                );
                Ok(())
            },
            window: WindowSel::Auto,
            checks: RULE_EDITOR,
        },
    ]
}

/// The sample with its project under DIN 40719-2 and the stock library
/// mounted, the Annotate dialog open, then Re-prefix pressed. The confirm
/// shows only when some placed part's prefix differs from the standard.
///
/// R1 is linked to a library row whose answer is handed to the planner, as
/// the sample's parts have no library link of their own.
fn open_reprefix(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    const LIBRARY: uuid::Uuid = uuid::Uuid::from_u128(0x7701);
    const ROW: uuid::Uuid = uuid::Uuid::from_u128(0x7702);
    states::edit_sample_file(fx, |sheet| {
        if let Some(part) = sheet.symbols.iter_mut().find(|s| s.reference == "R1") {
            part.library_id = Some(LIBRARY);
            part.row_id = Some(ROW);
        }
    })?;
    fixture::open_sample(app, r, fx);
    fixture::mount_stock(app, fx)?;
    let project = helpers_d::sample_project(app, fx);
    if let Some(p) = helpers_d::project_mut(app, project) {
        p.data.annotation = Some(signex_types::project::AnnotationSettings {
            designator_standard: Some(signex_types::designator::DesignatorStandard::Din40719_2),
            ..Default::default()
        });
    }
    settle_support::settle(app, Message::Annotate(AnnotateMsg::OpenDialog));
    settle_support::settle(app, Message::Annotate(AnnotateMsg::ReprefixPressed));
    let facts = RowFacts {
        internal_pn: "RES-0603".to_string(),
        symbol: (LIBRARY, ROW),
        class: "generic".to_string(),
        prefix_override: Some("Q".to_string()),
    };
    let lookup = ReprefixLookup {
        generation: app.ui_state.reprefix_generation,
        project: helpers_d::project_mut(app, project).map(|p| p.path.clone()),
        libraries: HashMap::from([(LIBRARY, HashMap::from([(ROW, facts)]))]),
        problems: Vec::new(),
    };
    settle_support::settle(
        app,
        Message::Annotate(AnnotateMsg::ReprefixRowsResolved {
            generation: lookup.generation,
            result: Arc::new(Ok(lookup)),
        }),
    );
    reached(
        app.ui_state.reprefix_confirm.is_some(),
        "the planner found no part to re-prefix for R1's linked row",
    )
}

/// A parent sheet placing `sync-child.snxsch` as a sheet symbol with no
/// entries, the child exposing two hierarchical ports; the symbol selected.
fn open_sheet_sync(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let dir = fx.sample.parent().ok_or("the sample has no directory")?;
    let parent = dir.join("sync-parent.snxsch");
    let symbol = uuid::Uuid::now_v7();
    let port = |name: &str, y: f64| Label {
        shape: "input".to_string(),
        ..Label::new(name, Point::new(10.0, y), LabelType::Hierarchical)
    };
    let child = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        labels: vec![port("SDA", 10.0), port("SCL", 15.0)],
        ..Default::default()
    };
    let top = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        child_sheets: vec![ChildSheet {
            uuid: symbol,
            name: "U_Power".to_string(),
            filename: "sync-child.snxsch".to_string(),
            position: Point::new(50.0, 50.0),
            size: (30.0, 30.0),
            stroke_width: 0.0,
            fill: FillType::None,
            stroke_color: None,
            fill_color: None,
            fields_autoplaced: false,
            pins: Vec::new(),
            instances: Vec::new(),
        }],
        ..Default::default()
    };
    for (path, sheet) in [
        (dir.join("sync-child.snxsch"), child),
        (parent.clone(), top),
    ] {
        let text = signex_types::format::SnxSchematic::new(sheet)
            .write_string()
            .map_err(|e| format!("encode sheet: {e:?}"))?;
        std::fs::write(&path, text).map_err(|e| format!("write sheet: {e}"))?;
    }
    settle_support::settle(
        app,
        Message::File(signex_app::app::FileMsg::Opened(Some(parent))),
    );
    let _ = render::draw_main(r, app);
    app.interaction_state.active_canvas().fit_to_sheet();
    app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(symbol, SelectedKind::ChildSheet)];
    command(app, "sync_sheet_entries")?;
    reached(
        app.ui_state.sheet_sync.is_some(),
        "sync_sheet_entries opened no dialog for the selected sheet symbol",
    )
}

/// The sample's project with its Rules tab open and in front.
pub(super) fn open_rules(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<signex_app::app::ProjectId, String> {
    fixture::open_sample(app, r, fx);
    let project = helpers_d::sample_project(app, fx);
    settle_support::settle(
        app,
        Message::Project(ProjectMsg::Rules(RulesTabMsg::OpenRequested {
            project: Some(project),
        })),
    );
    helpers_d::rules_tab_active(app)?;
    Ok(project)
}

const ANNOTATE: &[Check] = &[("the Annotate dialog is open", annotate_open)];

const RESET_CONFIRM: &[Check] = &[
    ("the reset confirm is open", reset_confirm_open),
    ("Cancel closes the reset confirm", reset_confirm_cancels),
];

const REPREFIX: &[Check] = &[("Cancel closes the re-prefix confirm", reprefix_cancels)];

const NUMBERING: &[Check] = &[("Cancel closes Number Schematic Sheets", numbering_cancels)];

const SHEET_SYNC: &[Check] = &[("Cancel closes the sync dialog", sheet_sync_cancels)];

const ASSIGN: &[Check] = &[
    ("Assign Footprints is open", assign_open),
    ("Cancel closes Assign Footprints", assign_cancels),
];

const UPDATE_PCB: &[Check] = &[("the Update PCB dialog is open", update_pcb_open)];

const BACK_ANNOTATE: &[Check] = &[("the Back Annotate dialog is open", back_annotate_open)];

const REANNOTATE: &[Check] = &[
    ("a board tab is active", checks::board_tab_active),
    ("the Re-Annotate dialog is open", reannotate_open),
];

const ERC_DIALOG: &[Check] = &[
    ("the ERC dialog is open", erc_dialog_open),
    ("Close closes the ERC dialog", erc_dialog_closes),
];

const ERC_PANEL: &[Check] = &[("the ERC panel is docked", erc_panel_shown)];

const MESSAGES: &[Check] = &[("a schematic tab is active", checks::schematic_tab_active)];

const DRC_PANEL: &[Check] = &[
    ("a board tab is active", checks::board_tab_active),
    ("the DRC panel is docked", drc_panel_shown),
];

const RULES: &[Check] = &[
    ("the Rules tab is active", helpers_d::rules_tab_active),
    ("a domain pick switches the grid", rules_domain_switches),
];

const RULES_TEXT: &[Check] = &[
    ("the Rules tab is active", helpers_d::rules_tab_active),
    ("the text view is shown", rules_text_shown),
];

const RULE_EDITOR: &[Check] = &[
    ("Preferences is open", preferences_open),
    ("a rule profile is selected", profile_selected),
];

fn annotate_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.annotate_dialog_open {
        Ok(())
    } else {
        Err("annotate_dialog_open is false".into())
    }
}

fn reset_confirm_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.annotate_reset_confirm {
        Ok(())
    } else {
        Err("annotate_reset_confirm is false".into())
    }
}

fn reset_confirm_cancels(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Annotate(AnnotateMsg::CloseResetConfirm));
    if app.ui_state.annotate_reset_confirm {
        Err("CloseResetConfirm left the confirm open".into())
    } else {
        Ok(())
    }
}

fn reprefix_cancels(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Annotate(AnnotateMsg::ReprefixCancelled));
    if app.ui_state.reprefix_confirm.is_some() {
        Err("ReprefixCancelled left the confirm open".into())
    } else {
        Ok(())
    }
}

fn numbering_cancels(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::from(SheetNumberingMsg::CancelRequested));
    if app.ui_state.sheet_numbering.is_some() {
        Err("CancelRequested left Number Schematic Sheets open".into())
    } else {
        Ok(())
    }
}

fn sheet_sync_cancels(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::from(SheetSyncMsg::CancelRequested));
    if app.ui_state.sheet_sync.is_some() {
        Err("CancelRequested left the sync dialog open".into())
    } else {
        Ok(())
    }
}

fn assign_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.assign_footprints.is_some() {
        Ok(())
    } else {
        Err("assign_footprints is None".into())
    }
}

fn assign_cancels(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::from(AssignFootprintsMsg::CancelRequested));
    if app.ui_state.assign_footprints.is_some() {
        Err("CancelRequested left Assign Footprints open".into())
    } else {
        Ok(())
    }
}

fn update_pcb_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.update_pcb.is_some() {
        Ok(())
    } else {
        Err("update_pcb is None".into())
    }
}

fn back_annotate_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.back_annotate.is_some() {
        Ok(())
    } else {
        Err("back_annotate is None".into())
    }
}

fn reannotate_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.board_reannotate.is_some() {
        Ok(())
    } else {
        Err("board_reannotate is None".into())
    }
}

fn erc_dialog_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.erc_dialog_open {
        Ok(())
    } else {
        Err("erc_dialog_open is false".into())
    }
}

fn erc_dialog_closes(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Erc(ErcMsg::CloseDialog));
    if app.ui_state.erc_dialog_open {
        Err("CloseDialog left the ERC dialog open".into())
    } else {
        Ok(())
    }
}

fn erc_panel_shown(app: &mut Signex) -> Result<(), String> {
    helpers_d::panel_shown(app, PanelKind::Erc)
}

fn drc_panel_shown(app: &mut Signex) -> Result<(), String> {
    helpers_d::panel_shown(app, PanelKind::Drc)
}

fn rules_domain_switches(app: &mut Signex) -> Result<(), String> {
    let project = app
        .document_state
        .active_project
        .ok_or("no active project")?;
    settle_support::settle(
        app,
        Message::Project(ProjectMsg::Rules(RulesTabMsg::DomainPicked {
            project_id: project,
            domain: Domain::Clearance,
        })),
    );
    let domain = app
        .document_state
        .project_tabs
        .get(&project)
        .map(|tab| tab.rules.grid.domain());
    if domain == Some(Domain::Clearance) {
        Ok(())
    } else {
        Err(format!("the grid shows {domain:?} after picking Clearance"))
    }
}

fn rules_text_shown(app: &mut Signex) -> Result<(), String> {
    let project = app
        .document_state
        .active_project
        .ok_or("no active project")?;
    let shown = app
        .document_state
        .project_tabs
        .get(&project)
        .is_some_and(|tab| tab.rules.text.is_some());
    if shown {
        Ok(())
    } else {
        Err("the Rules tab shows no text view".into())
    }
}

fn preferences_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.preferences_open {
        Ok(())
    } else {
        Err("preferences_open is false".into())
    }
}

fn profile_selected(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.profile_manager.selected_profile().is_some() {
        Ok(())
    } else {
        Err("no rule profile is selected in Preferences".into())
    }
}
