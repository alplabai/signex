//! The (id, form) parity table of ADR-0006 slice C (#2504): every route by
//! which a catalog id reaches its handler or its implicit target, generated
//! from the routing code, with the owner of the route and what it reads
//! besides its operands. `testdata/parity.golden.tsv` pins it.
//!
//! Implicit inputs come from three places: what the form reads by itself
//! ([`forms::Form::implied`]), the [`super::CommandContext`] facts the
//! enablement gate in front of the route reads (probed, [`inputs::read_by`]),
//! and the hand annotations ([`annotations::ANNOTATIONS`]) for what a
//! handler reads past its gate.

mod annotations;
mod forms;
mod inputs;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use forms::{Form, Owner};
use inputs::Input;

use super::{CommandArgs, bridge, enablement, needs, operand_handlers};
use crate::keymap::{AppCommandId, all_command_ids, metadata_for};

/// One `(id, form)` pair.
struct Row {
    id: &'static str,
    form: Form,
    route: String,
    owner: Owner,
    inputs: BTreeSet<Input>,
    /// The inputs are known to be complete: annotated by hand, or the form
    /// reads nothing but its operands.
    reviewed: bool,
}

impl Row {
    /// The row has an implicit-inputs value: some input, or a review that
    /// says it has none.
    fn has_value(&self) -> bool {
        !self.inputs.is_empty() || self.reviewed
    }

    fn inputs_cell(&self) -> String {
        if self.inputs.is_empty() {
            return "none".into();
        }
        let words: Vec<&str> = self.inputs.iter().map(|input| input.word()).collect();
        words.join(",")
    }
}

/// Every row, sorted by id then form.
fn rows() -> Vec<Row> {
    let mut gates: Vec<(needs::Needs, BTreeSet<Input>)> = Vec::new();
    let mut rows = Vec::new();
    for id in all_command_ids() {
        let command = AppCommandId::new(id).expect("catalog id is valid");
        let gate_inputs = needs::needs(&command).map(|gate| {
            if let Some((_, read)) = gates.iter().find(|(known, _)| *known == gate) {
                return read.clone();
            }
            let read = inputs::read_by(|ctx| gate.is_met(ctx));
            gates.push((gate, read.clone()));
            read
        });
        let arms = bridge::resolve(&command, &CommandArgs::none())
            .is_some_and(|message| forms::arms_a_tool(&message));
        for (form, route) in forms::routes_of(&command) {
            rows.push(row(&command, form, route, gate_inputs.as_ref(), arms));
        }
    }
    rows.sort_by(|a, b| (a.id, a.form).cmp(&(b.id, b.form)));
    rows
}

fn row(
    command: &AppCommandId,
    form: Form,
    route: String,
    gate_inputs: Option<&BTreeSet<Input>>,
    arms: bool,
) -> Row {
    let id = metadata_for(command).expect("catalog id has metadata").id;
    let mut inputs: BTreeSet<Input> = form.implied().iter().copied().collect();
    let mut reviewed = form.reads_only_operands();
    if form.is_gated() {
        inputs.extend(gate_inputs.into_iter().flatten());
    }
    match form {
        Form::Operand if operand_handlers::names_its_document(command) => reviewed = true,
        // Without a `document` operand the handler acts on the active tab,
        // behind its gate.
        Form::Operand => {
            inputs.extend(gate_inputs.into_iter().flatten());
            if takes_a_document(command) {
                inputs.insert(Input::ActiveDocument);
            }
        }
        Form::StillRunsRefusal => {
            inputs.extend(inputs::read_by(|ctx| {
                enablement::explains_refusal_in(command, ctx)
            }));
        }
        _ => {}
    }
    if let Some(annotated) = annotations::of(id, form) {
        inputs.extend(annotated.iter().copied());
        reviewed = true;
    }
    Row {
        id,
        form,
        route,
        owner: forms::owner(command, form, arms),
        inputs,
        reviewed,
    }
}

/// `command` takes a `document` operand.
fn takes_a_document(command: &AppCommandId) -> bool {
    metadata_for(command).is_some_and(|meta| meta.operands.iter().any(|op| op.name == "document"))
}

const HEADER: &str = "# id\tform\troute\towner\timplicit_inputs";

/// The table as the golden spells it.
fn table_text(rows: &[Row]) -> String {
    let mut out = format!("{HEADER}\n");
    for row in rows {
        let cells = [
            row.id,
            row.form.name(),
            &row.route,
            row.owner.name(),
            &row.inputs_cell(),
        ];
        out.push_str(&cells.join("\t"));
        out.push('\n');
    }
    out
}
