//! The (id, form) parity table of ADR-0006 slice C (#2504): every route by
//! which a catalog id reaches its handler or its implicit target, generated
//! from the routing code, with the owner of the route and what it reads
//! besides its operands. `testdata/parity.golden.tsv` pins it.
//!
//! Implicit inputs come from four places: what the form reads by itself
//! ([`forms::Form::implied`]), the [`super::CommandContext`] facts the
//! enablement gate in front of the route reads (probed, [`inputs::read_by`]),
//! what resolving a `document` operand and the agent's one-step wrapping
//! read (the origin, a port operand listed so its GUI-state read stays
//! visible), and the hand annotations ([`annotations::ANNOTATIONS`]) for
//! what a handler reads past its gate. The `basis` column says which:
//! `handler` (the handler was read and annotated), `routing` (the form has
//! no handler of its own to read) or `gate` (only the gate and the routing
//! were read, not the handler); no core row may be `gate`.
//!
//! The owner comes from what the route writes, by the D / S / C / X rule
//! of #2326 ([`owners`], #2551), never from who can call it today.
//!
//! The table follows the synchronous route only: the follow-up messages of
//! a returned `Task` (a save's completion, `ErcFinished` / `DrcFinished`, a
//! requirements run's result) run later against live state and are not rows.

mod annotations;
#[cfg(test)]
mod enablement_tests;
mod forms;
mod inputs;
#[cfg(test)]
mod needs_owner_tests;
mod owners;
mod plain_operands;
#[cfg(test)]
mod probe_tests;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use forms::{Form, Owner};
use inputs::Input;

use super::{CommandArgs, agent_edits, bridge, needs, operand_handlers};
use crate::keymap::{AppCommandId, all_command_ids, metadata_for};

/// How a row's implicit inputs were found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Basis {
    /// The handler was read; its inputs are annotated by hand.
    Handler,
    /// The form is routing only; the routing code was read.
    Routing,
    /// Only the gate and the routing were read, not the handler behind them.
    Gate,
    /// No handler exists yet: the inputs of a split router's core command,
    /// declared from the #2326 decisions (`split_core_command` rows only).
    Declared,
}

impl Basis {
    fn name(self) -> &'static str {
        match self {
            Self::Handler => "handler",
            Self::Routing => "routing",
            Self::Gate => "gate",
            Self::Declared => "declared",
        }
    }
}

/// One `(id, form)` pair.
struct Row {
    id: &'static str,
    form: Form,
    route: String,
    owner: Owner,
    /// How the owner was found; `None` fails `every_row_has_an_owner_basis`.
    owner_basis: Option<owners::OwnerBasis>,
    inputs: BTreeSet<Input>,
    basis: Basis,
}

impl Row {
    /// The row has an implicit-inputs value: some implicit input, or a read
    /// of its handler or routing that found none. A gate-only row that found
    /// nothing implicit says nothing, so it has none; the origin, a port
    /// operand, does not count.
    fn has_value(&self) -> bool {
        self.inputs.iter().any(|input| !input.is_port_operand()) || self.basis != Basis::Gate
    }

    /// The kill criterion of ADR-0006 slice C (#2504): a core route that
    /// reads a person's in-progress work or the render cache.
    fn meets_the_kill_criterion(&self) -> bool {
        self.owner == Owner::Core
            && (self.inputs.contains(&Input::InProgressEdit)
                || self.inputs.contains(&Input::RenderCache))
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
        let mut routes = forms::routes_of(&command);
        let forms: Vec<Form> = routes.iter().map(|(form, _)| *form).collect();
        let id = metadata_for(&command).expect("catalog id has metadata").id;
        if forms
            .iter()
            .any(|form| owners::annotated(id, *form) == Some(owners::Writes::Split))
        {
            routes.push((Form::SplitCore, "the core command after the router".into()));
        }
        let forms: Vec<Form> = routes.iter().map(|(form, _)| *form).collect();
        for (form, route) in routes {
            let owned = owners::owner(id, form, &forms, arms);
            rows.push(row(&command, form, route, gate_inputs.as_ref(), owned));
        }
    }
    rows.sort_by(|a, b| (a.id, a.form).cmp(&(b.id, b.form)));
    derive_apply_edits(&mut rows);
    rows
}

/// `apply_edits` runs each edit's own operand handler
/// (`(edit.handler)(app, &edit.args)` in `run_edits`,
/// command/agent_edits/group_run.rs), and an edit may be any command
/// `runs_as_one_step` accepts. So its row reads what those handlers' rows
/// read. No member reads the render cache: the wire snap reads the edited
/// sheet's engine (`snap_wire_on_edit_sheet`, #2542).
///
/// Every input is kept, the active document included: the active-tab gate is
/// skipped, but kept here as a superset rather than argued away.
fn derive_apply_edits(rows: &mut [Row]) {
    let mut read = BTreeSet::new();
    for row in rows.iter().filter(|row| row.form == Form::Operand) {
        let command = AppCommandId::new(row.id).expect("catalog id is valid");
        if agent_edits::runs_as_one_step(&command) {
            // A member that reads the render cache would put `apply_edits` on
            // the kill list; argue it here first.
            assert!(
                !row.inputs.contains(&Input::RenderCache),
                "{} reads the render cache inside an edit list",
                row.id
            );
            read.extend(row.inputs.iter().copied());
        }
    }
    if let Some(row) = rows
        .iter_mut()
        .find(|row| row.id == "apply_edits" && row.form == Form::Operand)
    {
        row.inputs.extend(read);
        row.basis = Basis::Handler;
    }
}

fn row(
    command: &AppCommandId,
    form: Form,
    route: String,
    gate_inputs: Option<&BTreeSet<Input>>,
    owned: Option<(Owner, owners::OwnerBasis)>,
) -> Row {
    let id = metadata_for(command).expect("catalog id has metadata").id;
    let mut inputs: BTreeSet<Input> = form.implied().iter().copied().collect();
    if form.is_gated() {
        inputs.extend(gate_inputs.into_iter().flatten());
    }
    match form {
        // A named document runs past the gate.
        Form::Operand if operand_handlers::names_its_document(command) => {}
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
                super::enablement::explains_refusal_in(command, ctx)
            }));
        }
        _ => {}
    }
    if reads_the_origin(command, form) {
        inputs.insert(Input::Origin);
    }
    let mut basis = if form.is_routing_only() {
        Basis::Routing
    } else {
        Basis::Gate
    };
    if let Some(annotated) = annotations::of(id, form) {
        inputs.extend(annotated.iter().copied());
        // The core command behind a split router has no handler yet: its
        // inputs are declared from the #2326 decisions, not read.
        basis = if form == Form::SplitCore {
            Basis::Declared
        } else {
            Basis::Handler
        };
    }
    Row {
        id,
        form,
        route,
        owner: owned.map_or(Owner::Undecided, |(owner, _)| owner),
        owner_basis: owned.map(|(_, basis)| basis),
        inputs,
        basis,
    }
}

/// The route reads who calls (today `agent.serving`): a `document` operand
/// resolves among an agent's documents only (`document_entries`),
/// `run_operands` wraps an agent's single edit in one undo step, and the
/// agent channel's window-layout route answers from the agent's view.
fn reads_the_origin(command: &AppCommandId, form: Form) -> bool {
    let resolves_a_document =
        matches!(form, Form::Operand | Form::Query | Form::Check) && takes_a_document(command);
    let one_step = form == Form::Operand && agent_edits::runs_as_one_step(command);
    resolves_a_document || one_step || form == Form::AgentWindowLayout
}

/// `command` takes a `document` operand.
fn takes_a_document(command: &AppCommandId) -> bool {
    metadata_for(command).is_some_and(|meta| meta.operands.iter().any(|op| op.name == "document"))
}

const HEADER: &str = "# id\tform\troute\towner\timplicit_inputs\tbasis";

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
            row.basis.name(),
        ];
        out.push_str(&cells.join("\t"));
        out.push('\n');
    }
    out
}
