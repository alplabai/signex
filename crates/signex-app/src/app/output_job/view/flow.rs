//! The run as a flow (mockup row 114, "Running (flow)" and "Step failed"):
//! the run's head and progress, Validate, one node an output by group,
//! Package and Done, each lit by its state; the question Validate asks, or
//! the banner of a stopped run; and the log of the picked node.

use iced::Widget as _;
use iced::widget::{Column, Row, column, row};
use iced::{Alignment, Element};
use signex_i18n::{UserText, ids, t};
use signex_types::output_job::{OnErrors, OnWarnings, OutputGroup};

use super::outputs::group_label;
use super::{Frame, Look, SP, kit, page};
use crate::app::Message;
use crate::app::output_job::pipeline::{FlowNode, JobRun, NodeState};
use crate::app::output_job::tab::{JobTabMsg, TabPage};
use crate::styles::PillTone;

/// The chip of a node's state.
fn state_chip<'a>(look: Look<'a>, state: NodeState, files: usize) -> Element<'a, Message> {
    let (tone, words) = match state {
        NodeState::Pending => (PillTone::Neutral, look.t(ids::OUTPUT_JOB_NODE_PENDING)),
        NodeState::Running => (PillTone::Accent, look.t(ids::OUTPUT_JOB_NODE_RUNNING)),
        NodeState::Done => (
            PillTone::Ok,
            t!(look.i18n, ids::OUTPUT_JOB_NODE_DONE, "files" => files),
        ),
        NodeState::Warning => (PillTone::Warn, look.t(ids::OUTPUT_JOB_NODE_WARNING)),
        NodeState::Failed => (PillTone::Err, look.t(ids::OUTPUT_JOB_NODE_FAILED)),
    };
    kit::chip(look.tokens, tone, words)
}

pub(super) fn view<'a>(frame: &Frame<'a>) -> Element<'a, Message> {
    let look = frame.look;
    let job = frame.job_path();
    let back = kit::ghost(
        look.tokens,
        look.t(ids::OUTPUT_JOB_BACK_TO_SETTINGS),
        Some(Message::from(JobTabMsg::PagePicked {
            job: job.clone(),
            page: TabPage::Settings,
        })),
    );
    let Some(run) = frame.run else {
        return page(vec![back, kit::line(look, look.t(ids::OUTPUT_JOB_NO_RUN))]);
    };
    let (done, total) = run.progress();
    let (outputs_done, outputs) = run.outputs_done();
    let head = column![
        kit::line(
            look,
            t!(
                look.i18n,
                ids::OUTPUT_JOB_FLOW_HEAD,
                "profile" => run.profile.clone().unwrap_or_default(),
                "variant" => run.variant.as_str(),
                "started" => run.started.as_str()
            )
        ),
        kit::quiet(
            look,
            t!(
                look.i18n,
                ids::OUTPUT_JOB_FLOW_PROGRESS,
                "done" => done,
                "total" => total,
                "outputs_done" => outputs_done,
                "outputs" => outputs
            )
        ),
        kit::quiet(look, policy_line(look, run)),
    ]
    .spacing(SP.xs);
    let mut parts = vec![back, head.boxed()];
    if let Some(banner) = banner(frame, run) {
        parts.push(banner);
    }
    parts.push(nodes(frame, run));
    parts.push(log(frame, run));
    page(parts)
}

fn policy_line(look: Look<'_>, run: &JobRun) -> signex_i18n::Localized {
    let errors = match run.on_errors {
        OnErrors::Stop => look.t(ids::OUTPUT_JOB_POLICY_STOP),
        OnErrors::Ask => look.t(ids::OUTPUT_JOB_POLICY_ASK),
    };
    let warnings = match run.on_warnings {
        OnWarnings::Continue => look.t(ids::OUTPUT_JOB_POLICY_CONTINUE),
        OnWarnings::Ask => look.t(ids::OUTPUT_JOB_POLICY_ASK),
        OnWarnings::Stop => look.t(ids::OUTPUT_JOB_POLICY_STOP),
    };
    t!(
        look.i18n,
        ids::OUTPUT_JOB_FLOW_POLICY,
        "errors" => errors.into_string(),
        "warnings" => warnings.into_string()
    )
}

/// The question Validate asks, or why the run stopped with its actions.
fn banner<'a>(frame: &Frame<'a>, run: &'a JobRun) -> Option<Element<'a, Message>> {
    let look = frame.look;
    let job = frame.job_path();
    if run.is_asking() {
        let summary = run.checks.as_ref().map(|c| c.summary()).unwrap_or_default();
        let col = column![
            kit::line(
                look,
                t!(look.i18n, ids::OUTPUT_JOB_ASK, "summary" => summary)
            ),
            row![
                kit::primary(
                    look.tokens,
                    look.t(ids::OUTPUT_JOB_ASK_CONTINUE),
                    Some(Message::from(JobTabMsg::AnswerPressed {
                        job: job.clone(),
                        go_on: true
                    }))
                ),
                kit::ghost(
                    look.tokens,
                    look.t(ids::OUTPUT_JOB_ASK_STOP),
                    Some(Message::from(JobTabMsg::AnswerPressed {
                        job,
                        go_on: false
                    }))
                ),
            ]
            .spacing(SP.s),
        ]
        .spacing(SP.s);
        return Some(kit::card(look.tokens, col.boxed()));
    }
    let stopped = run.stopped.as_ref()?;
    let mut actions: Row<Element<'a, Message>> = Row::new().spacing(SP.s);
    if stopped.at == "validate" {
        actions = actions.push(kit::primary(
            look.tokens,
            look.t(ids::OUTPUT_JOB_SHOW_IN_CHECK),
            Some(Message::from(JobTabMsg::ShowInCheckPressed {
                job: job.clone(),
            })),
        ));
    }
    actions = actions.push(kit::ghost(
        look.tokens,
        look.t(ids::OUTPUT_JOB_RUN_AGAIN),
        Some(Message::from(JobTabMsg::RunPressed {
            job: job.clone(),
            selected: false,
        })),
    ));
    if stopped.at == "validate" && run.on_errors == OnErrors::Stop {
        actions = actions.push(kit::ghost(
            look.tokens,
            look.t(ids::OUTPUT_JOB_ASK_NEXT_TIME),
            Some(Message::from(JobTabMsg::AskNextTimePressed { job })),
        ));
    }
    let col = column![
        kit::line(
            look,
            t!(
                look.i18n,
                ids::OUTPUT_JOB_STOPPED_BANNER,
                "at" => stopped.at.as_str(),
                "reason" => stopped.reason.as_str()
            )
        ),
        actions,
    ]
    .spacing(SP.s);
    Some(kit::card(look.tokens, col.boxed()))
}

/// Validate, the outputs by group, Package and Done.
fn nodes<'a>(frame: &Frame<'a>, run: &'a JobRun) -> Element<'a, Message> {
    let look = frame.look;
    let mut lanes: Row<Element<'a, Message>> = Row::new().spacing(SP.l);
    if let Some(validate) = &run.validate {
        let col = column![
            kit::head(look, ids::OUTPUT_JOB_STEP_VALIDATE),
            node_button(
                frame,
                None,
                look.t(ids::OUTPUT_JOB_STEP_VALIDATE_LONG).into_string(),
                validate
            ),
        ]
        .spacing(SP.xs);
        lanes = lanes.push_boxed(col);
    }
    for group in OutputGroup::ALL {
        let members: Vec<(usize, &FlowNode)> = run
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| node.group() == group)
            .collect();
        if members.is_empty() {
            continue;
        }
        let mut col: Column<Element<'a, Message>> =
            column![kit::head(look, group_label(group))].spacing(SP.xs);
        for (index, node) in members {
            col = col.push(node_button(frame, Some(index), node.name.clone(), node));
        }
        lanes = lanes.push_boxed(col);
    }
    let package = column![
        kit::head(look, ids::OUTPUT_JOB_STEP_PACKAGE),
        state_chip(look, run.package, run.report.file_count()),
    ]
    .spacing(SP.xs);
    lanes = lanes.push_boxed(package);
    lanes.align_y(Alignment::Start).boxed()
}

fn node_button<'a>(
    frame: &Frame<'a>,
    index: Option<usize>,
    name: String,
    node: &'a FlowNode,
) -> Element<'a, Message> {
    let look = frame.look;
    let picked = frame.ui.flow_node == index;
    let msg = Message::from(JobTabMsg::FlowNodePicked {
        job: frame.job_path(),
        node: index,
    });
    let label = match picked {
        true => kit::primary(look.tokens, UserText::new(name), Some(msg)),
        false => kit::ghost(look.tokens, UserText::new(name), Some(msg)),
    };
    row![label, state_chip(look, node.state, node.files)]
        .spacing(SP.xs)
        .align_y(Alignment::Center)
        .boxed()
}

/// The log of the picked node (Validate when none is picked).
fn log<'a>(frame: &Frame<'a>, run: &'a JobRun) -> Element<'a, Message> {
    let look = frame.look;
    let node = match frame.ui.flow_node {
        Some(index) => run.nodes.get(index),
        None => run.validate.as_ref(),
    };
    let mut col: Column<Element<'a, Message>> =
        column![kit::head(look, ids::OUTPUT_JOB_LOG)].spacing(SP.xs);
    match node {
        Some(node) if !node.log.is_empty() => {
            for line in &node.log {
                col = col.push(kit::quiet(look, UserText::new(line.clone())));
            }
        }
        _ => col = col.push(kit::quiet(look, look.t(ids::OUTPUT_JOB_LOG_EMPTY))),
    }
    kit::card(look.tokens, col.boxed())
}
