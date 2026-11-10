//! Who a command comes from (#2253): the person at the window, or an agent on
//! the socket or in a headless run. The app holds it for the length of one
//! command and never threads it through a handler; see `docs/AGENT_EDITS.md`,
//! "Origin".

use crate::app::Signex;

/// The source of the command being run now.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum Origin {
    /// A click, a key, a menu or the palette.
    #[default]
    Person,
    /// An agent: a request on the socket, or a headless run.
    Agent {
        /// The process that asked; `None` when the system did not say.
        pid: Option<u32>,
        /// The client's own name, when its request carries one. The wire has
        /// no such field yet, so it is never filled in.
        client: Option<String>,
    },
}

impl Origin {
    pub(crate) fn agent(pid: Option<u32>) -> Self {
        Self::Agent { pid, client: None }
    }

    pub(crate) fn is_agent(&self) -> bool {
        matches!(self, Self::Agent { .. })
    }

    /// "agent, pid 12" (and the client's name when known); `None` for the
    /// person.
    pub(crate) fn describe(&self) -> Option<String> {
        let Self::Agent { pid, client } = self else {
            return None;
        };
        let pid = pid.map_or_else(|| "pid unknown".to_string(), |pid| format!("pid {pid}"));
        Some(match client {
            Some(client) => format!("agent, {pid}, client {client}"),
            None => format!("agent, {pid}"),
        })
    }
}

impl Signex {
    /// Run `f` with `origin` as the source of every command it runs, and put
    /// the previous one back when it returns, whichever way it ends.
    pub(crate) fn with_origin<R>(&mut self, origin: Origin, f: impl FnOnce(&mut Self) -> R) -> R {
        let previous = std::mem::replace(&mut self.agent.origin, origin);
        let result = f(self);
        self.agent.origin = previous;
        result
    }
}
