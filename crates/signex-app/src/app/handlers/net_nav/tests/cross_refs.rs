//! Off-sheet cross references (#1273) on the canvas and through
//! `query_cross_references`: root, mid and third each carry the Global label
//! VCC, on pages 1, 2 and 3.

use serde_json::Value;
use signex_output::template::FramePrimitive;
use signex_types::schematic::SelectedKind;

use super::fixture::*;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::{EditMsg, Message};
use crate::keymap::AppCommandId;

/// The cross-reference marks the active canvas draws with its frame.
fn marks(p: &Project) -> Vec<String> {
    p.app
        .interaction_state
        .active_canvas()
        .sheet_frame
        .iter()
        .filter_map(|primitive| match primitive {
            FramePrimitive::Text { text, .. } if text.starts_with('[') => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn query(p: &mut Project, document: &std::path::Path) -> CommandOutcome {
    let command = AppCommandId::new("query_cross_references").expect("a valid id");
    let operand = format!("document={}", document.display());
    let args = CommandArgs::parse_operands(&command, &[operand.as_str()]).expect("operands");
    match p.app.run_query(&command, &args).expect("a query") {
        // Waits for the index build, as a headless run does.
        (CommandOutcome::Deferred { ticket }, task) => p.app.settle_deferred(ticket, task).0,
        (outcome, _) => outcome,
    }
}

fn answered(p: &mut Project, document: &std::path::Path) -> Value {
    match query(p, document) {
        CommandOutcome::Answered(value) => value,
        other => panic!("not answered: {other:?}"),
    }
}

/// The pages each placement of `label` lists in an answer.
fn pages_of(answer: &Value, label: u128) -> Vec<Vec<u64>> {
    let labels = answer["labels"].as_array().expect("labels");
    let entry = labels
        .iter()
        .find(|l| l["id"] == id(label).to_string())
        .unwrap_or_else(|| panic!("label {label} in {answer}"));
    entry["placements"]
        .as_array()
        .expect("placements")
        .iter()
        .map(|placement| {
            placement["targets"]
                .as_array()
                .expect("targets")
                .iter()
                .map(|t| t["page"].as_u64().expect("page"))
                .collect()
        })
        .collect()
}

#[test]
fn the_canvas_draws_the_pages_a_global_label_continues_on() {
    let mut p = project();
    assert!(marks(&p).is_empty(), "nothing before the index lands");

    p.app.settle_net_index_blocking();

    let shown = marks(&p);
    assert_eq!(shown.len(), 1, "{shown:?}");
    assert!(
        shown[0].starts_with("[2/") && shown[0].contains(", 3/"),
        "{shown:?}"
    );
}

/// An edit on another sheet moves the marks once the index is rebuilt.
#[test]
fn deleting_a_partner_takes_its_page_off_the_mark() {
    let mut p = project();
    p.app.settle_net_index_blocking();
    assert!(marks(&p)[0].contains(", 3/"));

    p.focus(&p.third.clone());
    p.select(THIRD_VCC, SelectedKind::Label);
    let _task = p.app.update(Message::Edit(EditMsg::DeleteSelected));
    p.focus(&p.root.clone());
    p.app.settle_net_index_blocking();

    let shown = marks(&p);
    assert_eq!(shown.len(), 1, "{shown:?}");
    assert!(
        shown[0].starts_with("[2/") && !shown[0].contains("3/"),
        "{shown:?}"
    );
}

/// The query waits for the index and answers every Global label of the
/// sheet; one whose net stays on its page lists no placement.
#[test]
fn the_query_answers_each_global_label_with_its_pages() {
    let mut p = project();
    let root = p.root.clone();
    let answer = answered(&mut p, &root);
    assert_eq!(pages_of(&answer, ROOT_VCC), [vec![2, 3]], "{answer}");
    let text = answer["labels"][0]["placements"][0]["cross_reference"]
        .as_str()
        .unwrap_or_default();
    assert!(text.starts_with("[2/"), "{answer}");

    let mid = p.mid.clone();
    let answer = answered(&mut p, &mid);
    assert_eq!(pages_of(&answer, MID_VCC), [vec![1, 3]], "{answer}");
    assert_eq!(pages_of(&answer, ALONE), Vec::<Vec<u64>>::new(), "{answer}");
}

/// The query and the canvas read one table: what the canvas draws is the
/// text the query names.
#[test]
fn the_query_names_the_text_the_canvas_draws() {
    let mut p = project();
    let root = p.root.clone();
    let answer = answered(&mut p, &root);
    let text = answer["labels"][0]["placements"][0]["cross_reference"].clone();
    assert_eq!(marks(&p), [text.as_str().expect("text").to_string()]);
}
