//! The forms a command id reaches, read from the routing code itself: one
//! `(form, route)` pair per distinct route to a handler or an implicit target.

use std::collections::BTreeSet;
use std::path::PathBuf;

use super::super::args::InvocationContext;
use super::super::routing::{self, InWindowRoute, WindowFacts};
use super::super::{
    CommandArgs, active_bar, bridge, enablement, needs, opened, operand_handlers, operands,
    queries, translator, window_document,
};
use super::inputs::Input;
use crate::app::document::{DocumentKey, UndoTarget};
use crate::app::state::ProjectId;
use crate::app::{Message, PcbMsg};
use crate::keymap::{AppCommandId, CatalogTextExt, metadata_for};
use crate::toolbar::ToolMessage;

/// A distinct route by which an id reaches its handler or its implicit
/// target (ADR-0006, slice C). The order is the column order of the golden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Form {
    /// A `bridge::resolve` arm, as a key, menu row or the palette sends it.
    Bridge,
    /// A bridge arm whose handler is a GUI translator (`translator.rs`): it
    /// fills `document` and `ids` from the selection and runs the id's
    /// operand handler, the `operand_handler` row.
    Translator,
    /// The same arm from a context menu, which carries the click point or
    /// the project, where that changes the message.
    BridgeContextMenu,
    /// The Active Bar table (`ACTIVE_BAR_COMMANDS`), the bridge's fallthrough.
    ActiveBar,
    /// An operand handler (`HANDLERS`).
    Operand,
    Query,
    /// A query that runs a check (`CHECK_IDS`).
    Check,
    /// Undo / Redo in an undocked sheet window (#1546).
    UndoSheetWindow,
    /// Undo / Redo in a window with a document of its own (#1854).
    UndoOwnWindow,
    /// Undo / Redo while the ERC dialog takes the input.
    UndoErcDialog,
    /// Close Document in the window it was typed in (#1787).
    CloseInWindow,
    /// Save, Save As, Print and the tab stepper in their window (#1794).
    WindowDocument,
    /// A resolved message rewritten for the window it was typed in.
    InWindow,
    /// The enablement gate, `needs::needs`.
    Needs,
    /// Refused without operands on a surface that gives none.
    NeedsOperands,
    /// Disabled, yet its handler runs to say why (`explains_when_unavailable`).
    StillRunsUnavailable,
    /// Disabled on a locked selection or an empty board clipboard, yet run
    /// (`explains_refusal_in`).
    StillRunsRefusal,
    HeadlessRefusal,
    HeadlessMissingOperand,
    HeadlessWithoutOperands,
    HeadlessNewDocument,
    /// Tile and Untile on the agent channel (`run_agent_window_layout`,
    /// #2249), outside `run_operands`.
    AgentWindowLayout,
    /// The one core command a split router makes (#2551): the GUI router
    /// resolves what is in hand, then makes at most one `execute`.
    SplitCore,
}

impl Form {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Bridge => "bridge",
            Self::Translator => "translator",
            Self::BridgeContextMenu => "bridge_context_menu",
            Self::ActiveBar => "active_bar",
            Self::Operand => "operand_handler",
            Self::Query => "query",
            Self::Check => "check",
            Self::UndoSheetWindow => "undo_router_sheet_window",
            Self::UndoOwnWindow => "undo_router_own_window",
            Self::UndoErcDialog => "undo_router_erc_dialog",
            Self::CloseInWindow => "window_close",
            Self::WindowDocument => "window_document",
            Self::InWindow => "in_window_rewrite",
            Self::Needs => "refusal_needs",
            Self::NeedsOperands => "refusal_needs_operands",
            Self::StillRunsUnavailable => "disabled_still_runs",
            Self::StillRunsRefusal => "disabled_still_runs_refusal",
            Self::HeadlessRefusal => "headless_refusal",
            Self::HeadlessMissingOperand => "headless_missing_operand",
            Self::HeadlessWithoutOperands => "headless_without_operands",
            Self::HeadlessNewDocument => "headless_new_document",
            Self::AgentWindowLayout => "agent_window_layout",
            Self::SplitCore => "split_core_command",
        }
    }

    /// The inputs every row of the form reads, whatever the id.
    pub(super) fn implied(self) -> &'static [Input] {
        match self {
            Self::UndoSheetWindow
            | Self::UndoOwnWindow
            | Self::CloseInWindow
            | Self::WindowDocument => &[Input::Window, Input::ActiveDocument],
            Self::UndoErcDialog | Self::InWindow => &[Input::Window],
            Self::HeadlessNewDocument => &[Input::DialogAnswer],
            _ => &[],
        }
    }

    /// Whether the enablement gate stands in front of the form's route, so
    /// what the gate reads is what the route reads.
    pub(super) fn is_gated(self) -> bool {
        matches!(
            self,
            Self::Bridge
                | Self::Translator
                | Self::BridgeContextMenu
                | Self::ActiveBar
                | Self::Needs
                | Self::StillRunsUnavailable
                | Self::StillRunsRefusal
                | Self::AgentWindowLayout
        )
    }

    /// A form the routing code describes whole: a refusal, a router or a
    /// rewrite, with no handler behind it that the row stands for.
    pub(super) fn is_routing_only(self) -> bool {
        matches!(
            self,
            Self::Needs
                | Self::NeedsOperands
                | Self::HeadlessRefusal
                | Self::HeadlessMissingOperand
                | Self::UndoSheetWindow
                | Self::UndoOwnWindow
                | Self::UndoErcDialog
                | Self::WindowDocument
                | Self::InWindow
        )
    }

    /// Reached only with a window id, which a headless caller never has.
    pub(super) fn needs_a_window(self) -> bool {
        matches!(
            self,
            Self::UndoSheetWindow
                | Self::UndoOwnWindow
                | Self::UndoErcDialog
                | Self::CloseInWindow
                | Self::WindowDocument
                | Self::InWindow
                | Self::NeedsOperands
        )
    }
}

/// Who runs a form once the core is split off (ADR-0006), from what the
/// route writes (`owners`, #2551).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Owner {
    Core,
    /// The adapter, GUI or CLI: session state and machine settings.
    Gui,
    /// A GUI router over what is in hand, with one core command behind it
    /// (the id's `split_core_command` row).
    Split,
    /// A maintainer decision is pending.
    Undecided,
}

impl Owner {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Gui => "gui",
            Self::Split => "split",
            Self::Undecided => "undecided",
        }
    }
}

/// Every `(form, route)` the routing code gives `command`.
pub(super) fn routes_of(command: &AppCommandId) -> Vec<(Form, String)> {
    let id = command.as_str();
    let mut routes = Vec::new();
    if let Some(message) = bridge::resolve(command, &CommandArgs::none()) {
        routes.push(bridge_route(id, &message));
        let menu = CommandArgs::in_context(InvocationContext {
            origin: Some((1.0, 2.0)),
            project: Some("proj:1".into()),
        });
        if let Some(in_menu) = bridge::resolve(command, &menu)
            && format!("{in_menu:?}") != format!("{message:?}")
        {
            routes.push((Form::BridgeContextMenu, format!("{in_menu:?}")));
        }
        routes.push((Form::InWindow, in_window_kinds(&message).join("|")));
    }
    if operand_handlers::operand_handler(command).is_some() {
        let route = if operand_handlers::names_its_document(command) {
            "run_operands: named document"
        } else {
            "run_operands: active-tab gate unless a document is named"
        };
        routes.push((Form::Operand, route.into()));
    }
    if queries::is_check(command) {
        routes.push((Form::Check, "run_query: check_run".into()));
    } else if queries::is_query(command) {
        routes.push((Form::Query, "run_query".into()));
    }
    routes.extend(window_routes(command));
    routes.extend(refusal_routes(command));
    routes.extend(headless_routes(command));
    if crate::app::agent::is_window_layout(command) {
        routes.push((Form::AgentWindowLayout, "run_agent_window_layout".into()));
    }
    routes
}

/// The bridge arm, or the Active Bar row the bridge falls through to.
fn bridge_route(id: &str, message: &Message) -> (Form, String) {
    use crate::active_bar::ActiveBarMsg;
    let debug = format!("{message:?}");
    match active_bar::action_for_id(id) {
        Some(action)
            if format!(
                "{:?}",
                Message::ActiveBar(ActiveBarMsg::Action(action.clone()))
            ) == debug =>
        {
            (Form::ActiveBar, format!("{action:?}"))
        }
        _ if translator::is_translated(id) => (Form::Translator, debug),
        _ => (Form::Bridge, debug),
    }
}

/// The in-window rewrites `message` can take, over every window fact.
fn in_window_kinds(message: &Message) -> Vec<&'static str> {
    let mut kinds = BTreeSet::new();
    for bits in 0..16u8 {
        let facts = WindowFacts {
            has_canvas: bits & 1 != 0,
            shows_undocked_board: bits & 2 != 0,
            is_document_window: bits & 4 != 0,
            is_main: bits & 8 != 0,
        };
        for window in [Some(iced::window::Id::unique()), None] {
            kinds.insert(
                match routing::in_window_route(message.clone(), window, facts) {
                    InWindowRoute::CanvasInWindow(..) => "canvas_in_window",
                    InWindowRoute::BoardViewInWindow(..) => "board_view_in_window",
                    InWindowRoute::BoardTurnInWindow(..) => "board_turn_in_window",
                    InWindowRoute::DocumentWindow(..) => "document_window",
                    InWindowRoute::FindFromMain(_) => "find_from_main",
                    InWindowRoute::Plain(_) => continue,
                },
            );
        }
    }
    kinds.into_iter().collect()
}

/// The undo routers and the window-routed document commands.
fn window_routes(command: &AppCommandId) -> Vec<(Form, String)> {
    let mut routes = Vec::new();
    if routing::is_history_step(command) {
        let reached = |decide: fn(&UndoTarget) -> bool| {
            undo_targets()
                .into_iter()
                .filter(|(_, target)| decide(target))
                .map(|(name, _)| name)
                .collect::<Vec<_>>()
                .join("|")
        };
        routes.push((
            Form::UndoSheetWindow,
            reached(routing::steps_through_window_swap),
        ));
        routes.push((Form::UndoOwnWindow, reached(routing::steps_own_document)));
    }
    if let Some(message) = routing::erc_dialog_step(command) {
        routes.push((Form::UndoErcDialog, format!("{message:?}")));
    }
    if routing::closes_in_its_window(command) {
        routes.push((Form::CloseInWindow, "close_document_in_window".into()));
    }
    if window_document::is_window_document_command(command) {
        routes.push((Form::WindowDocument, "window_document_command".into()));
    }
    routes
}

/// One [`UndoTarget`] of each kind, named.
fn undo_targets() -> Vec<(&'static str, UndoTarget)> {
    let path = PathBuf::from("a");
    let row = crate::library::state::EditorAddress::new(
        path.clone(),
        "t".into(),
        signex_library::RowId::new(),
    );
    let project = ProjectId::test_id(1);
    vec![
        (
            "schematic",
            UndoTarget::Document(DocumentKey::Schematic(path.clone())),
        ),
        ("pcb", UndoTarget::Document(DocumentKey::Pcb(path.clone()))),
        (
            "symbol",
            UndoTarget::Document(DocumentKey::Symbol(path.clone())),
        ),
        (
            "footprint",
            UndoTarget::Document(DocumentKey::Footprint(path)),
        ),
        ("library_row", UndoTarget::Document(DocumentKey::Row(row))),
        (
            "project",
            UndoTarget::Document(DocumentKey::Project(project)),
        ),
        ("erc_rules", UndoTarget::ProjectRules(project)),
        ("rules_tab", UndoTarget::RulesTab(project)),
        ("none", UndoTarget::None),
    ]
}

/// The enablement and operand refusals, and the disabled commands that
/// still run.
fn refusal_routes(command: &AppCommandId) -> Vec<(Form, String)> {
    let mut routes = Vec::new();
    if let Some(gate) = needs::needs(command) {
        routes.push((Form::Needs, format!("{gate:?}")));
    }
    if operand_handlers::needs_operands_reason(command).is_some() {
        routes.push((Form::NeedsOperands, "needs_operands_reason".into()));
    }
    if enablement::explains_when_unavailable(command) {
        routes.push((
            Form::StillRunsUnavailable,
            "explains_when_unavailable".into(),
        ));
    }
    if super::inputs::ever(|ctx| enablement::explains_refusal_in(command, ctx)) {
        routes.push((Form::StillRunsRefusal, "explains_refusal_in".into()));
    }
    routes
}

/// The headless path's own refusals and routes.
fn headless_routes(command: &AppCommandId) -> Vec<(Form, String)> {
    let mut routes = Vec::new();
    if metadata_for(command)
        .and_then(|meta| meta.headless_refusal())
        .is_some()
    {
        routes.push((Form::HeadlessRefusal, "headless_refusal".into()));
    }
    if let Some(name) = operands::missing_headless_operand(command) {
        routes.push((Form::HeadlessMissingOperand, format!("needs `{name}`")));
    }
    if operand_handlers::runs_headless_without_operands(command) {
        routes.push((Form::HeadlessWithoutOperands, "run_operands".into()));
    }
    if opened::answers_with_the_new_document(command) {
        routes.push((
            Form::HeadlessNewDocument,
            "settle_and_answer_new_document".into(),
        ));
    }
    routes
}

/// The message only arms a placement tool.
pub(super) fn arms_a_tool(message: &Message) -> bool {
    matches!(
        message,
        Message::Tool(ToolMessage::SelectTool(_)) | Message::Pcb(PcbMsg::PlaceToolPicked(_))
    )
}
