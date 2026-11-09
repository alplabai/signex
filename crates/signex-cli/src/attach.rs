//! A run attached to a running Signex (#1924): the commands go to the app that
//! has the project open and run there, inside its update loop, so the person
//! sees the change live and keeps their focus, tab, selection and tool.
//!
//! The run's own policy (capabilities, `--save`, path confinement) is applied
//! to every command before it is sent; the app's agent allowlist and its own
//! refusals apply on top, and come back with the app's reason. Nothing is
//! written to disk from here: what an edit changes stays unsaved in the app.

use std::path::{Path, PathBuf};

use signex_app::headless::agent_client::{Connection, Reply};

use crate::edits::authorize_edits;
use crate::policy::Policy;
use crate::protocol::{Mode, Request, ResultLine, Status};
use crate::roots::folder_and_roots;

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
        // A `document` given as an id is resolved by the app, which confines
        // it to the documents of the projects it has open.
        authorize_edits(
            &self.policy,
            &self.project_dir,
            &self.roots,
            &mut approved,
            |_| Ok(()),
        )
        .map_err(refused)?;
        self.policy
            .recheck(&approved, &self.roots)
            .map_err(refused)?;
        self.connection
            .ask(approved.command.as_str(), &approved.operands)
            .map_err(|why| (Status::Failed, why))
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
