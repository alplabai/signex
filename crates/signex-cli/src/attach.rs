//! A run attached to a running Signex (#1924): the commands go to the app that
//! has the project open and run there, inside its update loop, so the person
//! sees the change live and keeps their focus, tab, selection and tool.
//!
//! The run's own policy (capabilities, `--save`, path confinement) is applied
//! to every command before it is sent; the app's agent allowlist and its own
//! refusals apply on top, and come back with the app's reason. Nothing is
//! written to disk from here: what an edit changes stays unsaved in the app.

use std::path::{Path, PathBuf};

use serde_json::Value;
use signex_app::headless::agent_client::{Connection, Reply};

use signex_app::keymap::OperandKind;

use crate::capability::{Capability, capability_of};
use crate::edits::{authorize_edits, named_document_paths};
use crate::policy::{Approved, Policy};
use crate::protocol::{Mode, Request, ResultLine, Status};
use crate::roots::folder_and_roots;
use crate::scrub::{scrub_changed, scrub_data, unknown_id};

pub struct Attached {
    connection: Connection,
    policy: Policy,
    run_id: String,
    project_dir: PathBuf,
    roots: Vec<PathBuf>,
}

impl Attached {
    pub fn new(
        connection: Connection,
        policy: Policy,
        run_id: String,
        project: &Path,
    ) -> Result<Self, String> {
        let (project_dir, roots) = folder_and_roots(project)?;
        Ok(Self {
            connection,
            policy,
            run_id,
            project_dir,
            roots,
        })
    }

    /// A line that is not a command: refused without the app being asked.
    pub fn reject(&self, seq: u64, reason: String) -> ResultLine {
        ResultLine {
            reason: Some(reason),
            ..self.blank(seq, Status::Refused)
        }
    }

    /// Check one command against the run's policy, send it and report the
    /// app's answer in the headless result format.
    pub fn execute(&mut self, seq: u64, request: &Request) -> ResultLine {
        match self.send(request) {
            Ok(reply) => ResultLine {
                id: Some(request.id.clone()),
                reason: reply.reason,
                data: reply.data,
                messages: reply.messages,
                changed: reply.changed,
                ..self.blank(seq, reply.status)
            },
            Err((status, reason)) => ResultLine {
                id: Some(request.id.clone()),
                reason: Some(reason),
                ..self.blank(seq, status)
            },
        }
    }

    fn send(&mut self, request: &Request) -> Result<Reply, (Status, String)> {
        let refused = |reason: String| (Status::Refused, reason);
        let mut approved = self
            .policy
            .authorize(request, &self.project_dir, &self.roots)
            .map_err(refused)?;
        names_its_document(&approved).map_err(refused)?;
        let (policy, roots, connection) = (&self.policy, &self.roots, &mut self.connection);
        // Every `document` operand, an id or a path, in the command and in
        // each edit, is resolved by the app to the file it holds and confined
        // like any path: the app only knows which projects are open, and
        // another open project is not this run's to touch.
        authorize_edits(policy, &self.project_dir, roots, &mut approved, |checked| {
            named_document_paths(policy, roots, checked, true, |text| {
                resolve(connection, text)
            })
            .map(drop)
        })
        .map_err(refused)?;
        policy.recheck(&approved, roots).map_err(refused)?;
        let named = named_document_paths(policy, roots, &approved, true, |text| {
            resolve(connection, text)
        })
        .map_err(refused)?;
        if capability_of(approved.meta).writes_files() {
            policy.check_write_targets(&named, roots).map_err(refused)?;
            check_side_writes(policy, roots, connection, &approved).map_err(refused)?;
        }
        let mut reply = connection
            .ask(approved.command.as_str(), &approved.operands)
            .map_err(|why| (Status::Failed, why))?;
        // The app answers across every project it has open: this run sees only
        // what lies inside its own folders.
        let inside = |text: &str| policy.check_resolved(Path::new(text), roots, false).is_ok();
        reply.changed = scrub_changed(reply.changed, &inside);
        // Log lines are free text that may name any open project's files (a
        // duplicate document id, another thread's line in the shared log), so
        // no command's are shown. The app's own Messages panel keeps them all.
        reply.messages.clear();
        if let Some(data) = reply.data.take() {
            reply.data = match scrub_data(data, &inside) {
                // `save_writes` is for this check alone (made on the raw reply):
                // it can list files outside the run's folders, so it is never printed.
                Some(mut data) => {
                    if let Value::Object(map) = &mut data {
                        map.remove("save_writes");
                    }
                    Some(data)
                }
                // Not revealed to exist: the same answer as for an unknown id.
                None if approved.command.as_str() == "resolve" => {
                    let id = approved
                        .operands
                        .iter()
                        .find(|(name, _)| name == "id")
                        .map_or("", |(_, id)| id.as_str());
                    Some(unknown_id(id))
                }
                None => {
                    return Err(refused(
                        "the answer names files outside the folders this run may use".into(),
                    ));
                }
            };
        }
        Ok(reply)
    }

    fn blank(&self, seq: u64, status: Status) -> ResultLine {
        ResultLine {
            run: self.run_id.clone(),
            seq,
            mode: Mode::Attached,
            id: None,
            status,
            reason: None,
            data: None,
            messages: Vec::new(),
            changed: Vec::new(),
        }
    }
}

/// A command that acts on a document and is not a query must name it: sent
/// without one the app would use the person's active document, which is never
/// resolved here and so never checked against this run's folders.
fn names_its_document(approved: &Approved) -> Result<(), String> {
    let acts_on_a_document = approved
        .meta
        .operands
        .iter()
        .any(|operand| operand.kind == OperandKind::Document);
    let names_one = approved.operands.iter().any(|(name, _)| {
        approved
            .meta
            .operands
            .iter()
            .any(|operand| operand.name == *name && operand.kind == OperandKind::Document)
    });
    if acts_on_a_document && !names_one && capability_of(approved.meta) != Capability::Read {
        return Err(format!(
            "`{}` needs an explicit `document`: name the document; an attached run never acts on the person's active one",
            approved.command.as_str()
        ));
    }
    Ok(())
}

/// A save writes more than the document it names when that is a project file
/// (the libraries it has pending); the headless run lists them with the app's
/// `save_side_writes` and confines each. The running app tells them in the
/// `save_writes` of its `document_info` answer for a project, and each is
/// confined the same way; a sheet's plain save writes nothing else. A project
/// whose answer lacks the list is refused (fail closed). Any other
/// file-writing command is not on the agent channel.
fn check_side_writes(
    policy: &Policy,
    roots: &[PathBuf],
    connection: &mut Connection,
    approved: &Approved,
) -> Result<(), String> {
    for (name, text) in &approved.operands {
        let is_document = approved
            .meta
            .operands
            .iter()
            .any(|operand| operand.name == *name && operand.kind == OperandKind::Document);
        if !is_document {
            continue;
        }
        let reply = connection.ask(
            "document_info",
            &[("document".to_string(), text.to_string())],
        )?;
        let data = reply.data.as_ref();
        match data.and_then(|data| data["kind"].as_str()) {
            Some("project") => {
                let writes = data
                    .and_then(|data| data["save_writes"].as_array())
                    .ok_or_else(|| {
                        format!(
                            "`{}`: the running Signex did not say what saving the project writes",
                            approved.command.as_str()
                        )
                    })?
                    .iter()
                    .map(|file| file.as_str().map(PathBuf::from))
                    .collect::<Option<Vec<_>>>()
                    .ok_or("the running Signex named a file its save writes without a path")?;
                // The refusal names no file: one outside the run's folders may
                // belong to another project, and the agent was never told it.
                for file in &writes {
                    policy
                        .check_write_targets(std::slice::from_ref(file), roots)
                        .map_err(|_| {
                            "a file its save writes is not inside the folders this run may use \
                             (--allow-path adds one)"
                                .to_string()
                        })?;
                }
            }
            // Kinds whose plain save writes nothing but the document; only
            // these are let through without a list.
            Some("schematic" | "pcb" | "symbol" | "footprint" | "component") => {}
            _ => {
                return Err(format!(
                    "`{}` of a document of unknown kind is not available to an attached run",
                    approved.command.as_str()
                ));
            }
        }
    }
    Ok(())
}

/// The file the running app holds the document `text` (an id or a path) as,
/// from its `document_info` answer; the app's reason when it holds none.
fn resolve(connection: &mut Connection, text: &str) -> Result<PathBuf, String> {
    let reply = connection.ask(
        "document_info",
        &[("document".to_string(), text.to_string())],
    )?;
    if reply.status != Status::Answered {
        return Err(reply
            .reason
            .unwrap_or_else(|| "the running Signex does not know it".to_string()));
    }
    reply
        .data
        .as_ref()
        .and_then(|data| data["path"].as_str())
        .map(PathBuf::from)
        .ok_or_else(|| "it has no file".to_string())
}
