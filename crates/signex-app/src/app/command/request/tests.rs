//! #2495: one request path. Each surface's request comes back as one typed
//! outcome, and `if_revision` refuses a request whose document moved.

use serde_json::json;

use super::{AgentGatePassed, CommandRequest, RefusalCause, RequestOutcome, RequestSurface};
use crate::app::Signex;
use crate::app::agent::{Status, outcome_verdict};
use crate::app::command::queries::tests::{app_with, path};
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::keymap::AppCommandId;

const GUI: RequestSurface = RequestSurface::Gui { window: None };

fn id(name: &str) -> AppCommandId {
    AppCommandId::new(name).expect("a valid id")
}

fn request(name: &str, operands: &[&str]) -> CommandRequest {
    let command = id(name);
    let args = if operands.is_empty() {
        CommandArgs::none()
    } else {
        CommandArgs::parse_operands(&command, operands).expect("operands parse")
    };
    CommandRequest::new(command, args)
}

fn agent() -> RequestSurface {
    RequestSurface::Agent(AgentGatePassed::after_gate())
}

fn document_a() -> String {
    format!("document={}", path("a.snxsch").display())
}

fn revision_a(app: &Signex) -> u64 {
    app.document_state.engines[&path("a.snxsch")].revision()
}

fn cause(outcome: &RequestOutcome) -> Option<RefusalCause> {
    match outcome {
        RequestOutcome::Refused { cause, .. } => Some(*cause),
        _ => None,
    }
}

#[test]
fn every_dispatch_outcome_has_one_typed_reading() {
    assert_eq!(
        RequestOutcome::of("x", CommandOutcome::Ran),
        RequestOutcome::Ran
    );
    let value = json!({"count": 1});
    assert_eq!(
        RequestOutcome::of("x", CommandOutcome::Answered(value.clone())),
        RequestOutcome::Answered(value)
    );
    let unknown = RequestOutcome::of("x", CommandOutcome::UnknownId);
    assert_eq!(cause(&unknown), Some(RefusalCause::NoSuchCommand));
    let arm = RequestOutcome::of("x", CommandOutcome::NoBridgeArm);
    assert_eq!(cause(&arm), Some(RefusalCause::NoSuchCommand));
    assert_eq!(
        RequestOutcome::of("x", CommandOutcome::disabled("why")),
        RequestOutcome::Refused {
            reason: "why".into(),
            cause: RefusalCause::NotAvailable
        }
    );
    assert_eq!(
        RequestOutcome::of("x", CommandOutcome::Deferred { ticket: 7 }),
        RequestOutcome::Waiting { ticket: 7 }
    );
    assert!(matches!(
        RequestOutcome::of("x", CommandOutcome::NotHeadless),
        RequestOutcome::Failed(_)
    ));
}

#[test]
fn an_error_notice_fails_a_ran_or_an_explained_refusal_and_nothing_else() {
    let notice = || Some("Open failed: no file".to_string());
    let failed = RequestOutcome::Failed("Open failed: no file".into());
    assert_eq!(RequestOutcome::Ran.after_notice(notice()), failed);
    let disabled = RequestOutcome::of("x", CommandOutcome::disabled("made no document"));
    assert_eq!(disabled.after_notice(notice()), failed);
    let unknown = RequestOutcome::of("x", CommandOutcome::UnknownId);
    assert_eq!(unknown.clone().after_notice(notice()), unknown);
    let moved = RequestOutcome::refused("moved", RefusalCause::Revision);
    assert_eq!(moved.clone().after_notice(notice()), moved);
    let answered = RequestOutcome::Answered(json!(1));
    assert_eq!(answered.clone().after_notice(notice()), answered);
    assert_eq!(RequestOutcome::Ran.after_notice(None), RequestOutcome::Ran);
}

/// The reply's reading is this one: the envelope and the typed outcome
/// never disagree.
#[test]
fn the_verdict_is_the_typed_outcome_as_a_reply() {
    let cases = [
        CommandOutcome::Ran,
        CommandOutcome::UnknownId,
        CommandOutcome::NoBridgeArm,
        CommandOutcome::disabled("why"),
        CommandOutcome::NotHeadless,
        CommandOutcome::Answered(json!({"a": 1})),
        CommandOutcome::Deferred { ticket: 3 },
    ];
    for outcome in cases {
        for notice in [None, Some("a notice".to_string())] {
            let typed = RequestOutcome::of("plce_wire", outcome.clone())
                .after_notice(notice.clone())
                .into_verdict();
            assert_eq!(
                typed,
                outcome_verdict("plce_wire", outcome.clone(), notice),
                "{outcome:?}"
            );
        }
    }
    let waiting = RequestOutcome::Waiting { ticket: 3 }.into_verdict();
    assert_eq!(waiting.0, Status::Failed);
}

#[test]
fn a_gui_request_runs_is_refused_or_names_no_command() {
    let (mut app, _task) = Signex::new();
    let (ran, _task) = app.run_command_request(GUI, request("cycle_selection_mode", &[]));
    assert_eq!(ran, RequestOutcome::Ran);

    let (refused, task) = app.run_command_request(GUI, request("place_wire", &[]));
    assert_eq!(
        refused,
        RequestOutcome::Refused {
            reason: "No schematic is open".into(),
            cause: RefusalCause::NotAvailable
        }
    );
    assert_eq!(task.units(), 0);

    let (unknown, _task) = app.run_command_request(GUI, request("not_a_catalog_command", &[]));
    assert_eq!(cause(&unknown), Some(RefusalCause::NoSuchCommand));
}

#[test]
fn a_headless_request_on_an_app_not_booted_headless_failed() {
    let (mut app, _task) = Signex::new();
    let (outcome, _task) =
        app.run_command_request(RequestSurface::Headless, request("list_documents", &[]));
    assert_eq!(
        outcome,
        RequestOutcome::Failed("unexpected outcome NotHeadless".into())
    );
}

#[test]
fn an_agent_query_is_answered() {
    let mut app = app_with(&[("a.snxsch", 100)]);
    let (outcome, _task) = app.run_command_request(agent(), request("list_documents", &[]));
    assert!(
        matches!(outcome, RequestOutcome::Answered(_)),
        "{outcome:?}"
    );
}

#[test]
fn a_request_at_its_documents_revision_runs() {
    // The agent surface sees only project documents (#1924): a.snxsch is a
    // sheet of the agent tests' project.
    let mut app = crate::app::agent::tests::app_at_work();
    let at = revision_a(&app);
    let document = document_a();
    let request = request("document_info", &[document.as_str()]).with_if_revision(Some(at));
    let (outcome, _task) = app.run_command_request(agent(), request);
    assert!(
        matches!(outcome, RequestOutcome::Answered(_)),
        "{outcome:?}"
    );
}

#[test]
fn a_request_whose_document_moved_is_refused_on_every_surface() {
    let mut app = app_with(&[("a.snxsch", 100)]);
    let at = revision_a(&app);
    let stale = at + 1;
    let document = document_a();
    for surface in [GUI, RequestSurface::Headless, agent()] {
        let request = request("document_info", &[document.as_str()]).with_if_revision(Some(stale));
        let (outcome, task) = app.run_command_request(surface, request);
        let RequestOutcome::Refused { reason, cause } = outcome else {
            panic!("{surface:?} ran a request whose document moved: {outcome:?}");
        };
        assert_eq!(cause, RefusalCause::Revision);
        assert!(
            reason.contains(&format!("changed since revision {stale} (it is at {at})")),
            "{reason}"
        );
        assert_eq!(task.units(), 0, "nothing ran on {surface:?}");
        assert_eq!(revision_a(&app), at);
    }
}

#[test]
fn if_revision_without_a_document_is_refused() {
    let mut app = app_with(&[("a.snxsch", 100)]);
    let request = request("list_documents", &[]).with_if_revision(Some(0));
    let (outcome, _task) = app.run_command_request(agent(), request);
    assert_eq!(
        outcome,
        RequestOutcome::refused(
            "`if_revision` needs the operand `document`",
            RefusalCause::Revision
        )
    );
}
