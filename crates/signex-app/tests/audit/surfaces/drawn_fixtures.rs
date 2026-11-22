//! Fixtures for states that need more than the bundled sample holds: parts
//! linked to a stock row with alternates, a project with five net classes,
//! and a repository with a merged branch.

use super::helpers_d::{self, reached};
use super::helpers_f as h;
use super::states;
use crate::fixture::{self, Fixture};
use crate::render;
use crate::settle_support;
use signex_app::app::{
    AssignFootprintsMsg, FileMsg, Message, OverlayMsg, ProjectId, ProjectMsg, RulesTabMsg, Signex,
};
use signex_app::dock::{DockMessage, PanelPosition};
use signex_app::panels::PanelKind;
use signex_drc_dsl::table::matrix::CellKey;
use signex_types::format::SnxSchematic;
use signex_types::net_class::NetClass;
use signex_widgets::constraint_grid::{ClassPair, MatrixMessage};
use uuid::Uuid;

/// The stock library and its `STOCK-R` row, which offers seven chip sizes.
const STOCK_LIBRARY: Uuid = Uuid::from_u128(0x019df45d_af5e_7a00_8c3b_dd3bf89b40ae);
const STOCK_RESISTOR: Uuid = Uuid::from_u128(0x01a18461_b555_75c3_995a_0a8a51e590a4);

/// Assign Footprints with R1 to R3 ticked and set to the row's 0402.
pub(super) fn open_assign_ticked(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    states::edit_sample_file(fx, |sheet| {
        let Some(at) = sheet.symbols.iter().position(|s| s.reference == "R1") else {
            return;
        };
        let part = &mut sheet.symbols[at];
        part.library_id = Some(STOCK_LIBRARY);
        part.row_id = Some(STOCK_RESISTOR);
        part.footprint = "Chip-0805".to_string();
        let first = part.clone();
        for (n, reference) in [(1, "R2"), (2, "R3")] {
            let mut copy = first.clone();
            copy.uuid = Uuid::now_v7();
            copy.reference = reference.to_string();
            copy.position.y += 12.7 * f64::from(n);
            for instance in &mut copy.instances {
                instance.reference = reference.to_string();
            }
            sheet.symbols.push(copy);
        }
    })?;
    fixture::open_sample(app, r, fx);
    fixture::mount_stock(app, fx)?;
    helpers_d::sample_project(app, fx);
    helpers_d::command(app, "assign_footprints")?;
    reached(
        app.ui_state.assign_footprints.is_some(),
        "assign_footprints opened no dialog",
    )?;
    h::settle(app, Message::from(AssignFootprintsMsg::AllToggled(true)))?;
    let (binding, first_ticked) = {
        let state = app
            .ui_state
            .assign_footprints
            .as_ref()
            .ok_or("the dialog closed")?;
        let offer = state
            .shared_offers()
            .into_iter()
            .find(|o| o.label == "Chip-0402")
            .ok_or("the ticked parts share no Chip-0402")?;
        let first = state
            .parts()
            .iter()
            .position(|p| p.designator == "R1")
            .ok_or("R1 is not listed")?;
        (offer.binding, first)
    };
    for msg in [
        AssignFootprintsMsg::TickedPicked(binding),
        AssignFootprintsMsg::RowPreviewed(first_ticked),
    ] {
        h::settle(app, Message::from(msg))?;
    }
    let changed = app
        .ui_state
        .assign_footprints
        .as_ref()
        .map_or(0, |state| state.changes().len());
    reached(changed == 3, "the three ticked parts did not change")
}

/// The net classes of the clearance matrix in table order, each with one
/// net of the board: a class with no net on the board has no fallback.
const CLASSES: [(&str, &str); 5] = [
    ("Default", "LED_A"),
    ("PWR", "VIN"),
    ("HV", "+12V"),
    ("USB", "+5V"),
    ("DIFF", "GND"),
];

/// The written cells, as (row class, column class, mm).
const CLEARANCES: [(usize, usize, &str); 9] = [
    (1, 0, "0.2"),
    (2, 0, "0.8"),
    (2, 1, "0.8"),
    (2, 2, "1.2"),
    (3, 1, "0.2"),
    (3, 2, "0.8"),
    (3, 3, "0.127"),
    (4, 2, "0.8"),
    (4, 4, "0.1"),
];

fn rules(project_id: ProjectId, msg: MatrixMessage) -> Message {
    Message::Project(ProjectMsg::Rules(RulesTabMsg::MatrixActed {
        project_id,
        msg,
    }))
}

/// The sample with its board open and five net classes, on the Rules tab.
pub(super) fn open_rules_with_classes(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<ProjectId, String> {
    let project = helpers_d::sample_with_board(app, r, fx)?;
    if let Some(loaded) = helpers_d::project_mut(app, project) {
        loaded.data.net_classes = CLASSES
            .into_iter()
            .map(|(name, net)| NetClass {
                clearance: 0.15,
                nets: vec![net.to_string()],
                ..NetClass::named(name)
            })
            .collect();
    }
    let board = fx.sample.with_extension("snxpcb");
    settle_support::settle(app, Message::File(FileMsg::Opened(Some(board))));
    helpers_d::sample_project(app, fx);
    settle_support::settle(
        app,
        Message::Project(ProjectMsg::Rules(RulesTabMsg::OpenRequested {
            project: Some(project),
        })),
    );
    helpers_d::rules_tab_active(app)?;
    Ok(project)
}

/// Type the drawn clearances into the matrix, then open the HV x PWR cell
/// with a Track - Pad value of its own.
pub(super) fn fill_clearance_matrix(app: &mut Signex, project: ProjectId) -> Result<(), String> {
    for (row, col, value) in CLEARANCES {
        for msg in [
            MatrixMessage::CellPressed(ClassPair::new(row, col)),
            MatrixMessage::EditTextChanged(value.to_string()),
            MatrixMessage::EditSubmitted,
        ] {
            settle_support::settle(app, rules(project, msg));
        }
    }
    let track_pad = CellKey {
        pair: Some(["track".to_string(), "pad".to_string()]),
        ..CellKey::new("HV", "PWR")
    };
    settle_support::settle(
        app,
        Message::Project(ProjectMsg::Rules(RulesTabMsg::MatrixPairTyped {
            project_id: project,
            key: track_pad,
            text: "1.0".to_string(),
        })),
    );
    settle_support::settle(
        app,
        Message::Project(ProjectMsg::Rules(RulesTabMsg::MatrixCellCommitted {
            project_id: project,
        })),
    );
    for msg in [
        MatrixMessage::CellPressed(ClassPair::new(2, 1)),
        MatrixMessage::EditCancelled,
    ] {
        settle_support::settle(app, rules(project, msg));
    }
    let tab = app.document_state.project_tabs.get(&project);
    reached(
        tab.is_some_and(|t| t.rules.matrix.pinned() == Some(ClassPair::new(2, 1))),
        "the HV x PWR cell did not stay pinned",
    )?;
    // The tab's full width, as drawn. The returned task is what saves the
    // layout; dropping it keeps this row's docks out of the next row.
    for position in [
        PanelPosition::Left,
        PanelPosition::Right,
        PanelPosition::Bottom,
    ] {
        let _ = app.update(Message::Dock(DockMessage::ToggleCollapse(position)));
    }
    Ok(())
}

/// One commit of the drawn history: subject, author and age in minutes.
struct Step {
    subject: &'static str,
    author: &'static str,
    minutes_ago: i64,
}

const MAIN_STEPS: [Step; 5] = [
    Step {
        subject: "Initial import",
        author: "j.berg",
        minutes_ago: 43_200,
    },
    Step {
        subject: "Split power stage into its own sheet",
        author: "m.demir",
        minutes_ago: 7_200,
    },
    Step {
        subject: "Add soft-start cap C7",
        author: "j.berg",
        minutes_ago: 2_880,
    },
    Step {
        subject: "Swap Q2 to a lower-Rds(on) part",
        author: "a.yilmaz",
        minutes_ago: 300,
    },
    Step {
        subject: "Add output cap C12",
        author: "a.yilmaz",
        minutes_ago: 180,
    },
];

const BRANCH_STEP: Step = Step {
    subject: "Raise L1 to 4.7 uH for ripple margin",
    author: "j.berg",
    minutes_ago: 60,
};

const MERGE_STEP: Step = Step {
    subject: "Merge ripple-fix into main",
    author: "j.berg",
    minutes_ago: 12,
};

/// Writes commits of the sample sheet straight to the object store.
struct HistoryRepo<'a> {
    repo: &'a git2::Repository,
    base: git2::Oid,
    sheet: &'a str,
    now: i64,
}

impl HistoryRepo<'_> {
    fn commit(
        &self,
        step: &Step,
        text: &str,
        parents: &[git2::Oid],
    ) -> Result<git2::Oid, git2::Error> {
        let blob = self.repo.blob(text.as_bytes())?;
        let base = self.repo.find_tree(self.base)?;
        let mut tree = self.repo.treebuilder(Some(&base))?;
        tree.insert(self.sheet, blob, 0o100_644)?;
        let tree = self.repo.find_tree(tree.write()?)?;
        let when = git2::Time::new(self.now - step.minutes_ago * 60, 0);
        let email = format!("{}@example.invalid", step.author);
        let who = git2::Signature::new(step.author, &email, &when)?;
        let parents = parents
            .iter()
            .map(|id| self.repo.find_commit(*id))
            .collect::<Result<Vec<_>, _>>()?;
        let parents: Vec<&git2::Commit<'_>> = parents.iter().collect();
        self.repo
            .commit(None, &who, &who, step.subject, &tree, &parents)
    }
}

/// The sheet text with R1 at `value`: a real, parseable older version.
fn sheet_version(text: &str, value: &str) -> Result<String, String> {
    let mut sheet = SnxSchematic::parse(text)
        .map_err(|e| format!("parse sample: {e:?}"))?
        .sheet;
    if let Some(part) = sheet.symbols.iter_mut().find(|s| s.reference == "R1") {
        part.value = value.to_string();
    }
    SnxSchematic::new(sheet)
        .write_string()
        .map_err(|e| format!("encode sheet: {e:?}"))
}

/// A `main` of five commits, a `ripple-fix` branch off the fourth, and the
/// merge of it as `HEAD`, whose sheet is the sample as shipped.
fn commit_branched_history(fx: &Fixture) -> Result<(), String> {
    let git = |e: git2::Error| format!("git: {e}");
    let path = fx.sample.with_extension("snxsch");
    let dir = fx.sample.parent().ok_or("the sample has no directory")?;
    let sheet = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("the sample sheet has no name")?;
    let shipped = std::fs::read_to_string(&path).map_err(|e| format!("read sample: {e}"))?;
    let repo = git2::Repository::init(dir).map_err(git)?;
    let mut index = repo.index().map_err(git)?;
    index
        .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
        .map_err(git)?;
    index.write().map_err(git)?;
    let base = index.write_tree().map_err(git)?;
    let now = chrono::Utc::now().timestamp();
    let history = HistoryRepo {
        repo: &repo,
        base,
        sheet,
        now,
    };
    let mut tip = None;
    let mut fork = None;
    for (at, step) in MAIN_STEPS.iter().enumerate() {
        let text = sheet_version(&shipped, &format!("{}R", 100 + at))?;
        let parents: Vec<git2::Oid> = tip.into_iter().collect();
        let id = history.commit(step, &text, &parents).map_err(git)?;
        if at == 3 {
            fork = Some(id);
        }
        tip = Some(id);
    }
    let main = tip.ok_or("no commit on main")?;
    let fork = fork.ok_or("no commit to branch from")?;
    let text = sheet_version(&shipped, "220R")?;
    let branch = history.commit(&BRANCH_STEP, &text, &[fork]).map_err(git)?;
    let merge = history
        .commit(&MERGE_STEP, &shipped, &[main, branch])
        .map_err(git)?;
    repo.reference("refs/heads/ripple-fix", branch, true, "branch")
        .map_err(git)?;
    repo.reference("refs/heads/main", merge, true, "merge")
        .map_err(git)?;
    repo.set_head("refs/heads/main").map_err(git)?;
    let head = repo.find_object(merge, None).map_err(git)?;
    repo.reset(&head, git2::ResetType::Hard, None).map_err(git)
}

/// The History panel on a repository with a merged branch, moved to the
/// right dock where the panel has the height to list the commits.
pub(super) fn open_history_branches(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    commit_branched_history(fx)?;
    fixture::open_sample(app, r, fx);
    settle_support::settle(
        app,
        Message::Overlay(OverlayMsg::OpenPanel(PanelKind::History)),
    );
    let at = app
        .document_state
        .dock
        .panel_kinds(PanelPosition::Bottom)
        .iter()
        .position(|k| *k == PanelKind::History)
        .ok_or("no History tab in the bottom dock")?;
    // The returned tasks save the layout; dropping them keeps this row's
    // docks out of the next row.
    let _ = app.update(Message::Dock(DockMessage::UndockPanel(
        PanelPosition::Bottom,
        at,
    )));
    let floating = app
        .document_state
        .dock
        .floating
        .len()
        .checked_sub(1)
        .ok_or("the History tab did not undock")?;
    let _ = app.update(Message::Dock(DockMessage::DockFloatingTo(
        floating,
        PanelPosition::Right,
    )));
    fixture::select_panel(app, PanelKind::History)?;
    let _ = render::draw_main(r, app);
    reached(
        app.document_state.panel_ctx.history.entries.len() == 7,
        "the History panel did not list the seven commits",
    )
}
