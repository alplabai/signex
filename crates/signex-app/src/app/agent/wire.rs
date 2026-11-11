//! The JSON Lines wire format of the agent channel: one request object per
//! line in, one reply object per line out. The reply is the envelope the
//! headless runner prints (`signex-cli`), with the command id first.
//! Fields and limits: `docs/AGENT_SOCKET.md`.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::reply::{MessageLine, Status};

/// The longest request line, in bytes, newline excluded. A line over it is
/// refused and the connection closed, as the bytes after it cannot be framed.
pub(crate) const MAX_REQUEST_BYTES: usize = 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRequest {
    token: String,
    id: String,
    #[serde(default)]
    args: Map<String, Value>,
    /// The client's name, e.g. `signex-cli`; journaled, never trusted.
    #[serde(default)]
    client: Option<String>,
    /// What the client says it may do (`read`, `edit`, `save`, `allow-path:..`);
    /// journaled so a person can see under which rights an agent worked.
    #[serde(default)]
    grants: Vec<String>,
}

/// A request that is well formed; its token is not checked yet.
#[derive(Debug)]
pub(super) struct Request {
    pub(super) token: String,
    pub(super) id: String,
    /// `name`, `text` pairs, the text being what `CommandArgs::parse_operands`
    /// reads: the same forms `signex-cli` sends.
    pub(super) operands: Vec<(String, String)>,
    pub(super) client: Option<String>,
    pub(super) grants: Vec<String>,
}

/// One input line: `{"token": "...", "id": "...", "args": {...}}`. Unknown
/// fields and arguments of an unsupported shape are refused with a reason.
pub(super) fn parse_request(line: &str) -> Result<Request, String> {
    let wire: WireRequest =
        serde_json::from_str(line).map_err(|error| format!("malformed request: {error}"))?;
    let operands = wire
        .args
        .iter()
        .map(|(name, value)| Ok((name.clone(), crate::headless::operand_text(name, value)?)))
        .collect::<Result<_, String>>()?;
    Ok(Request {
        token: wire.token,
        id: wire.id,
        operands,
        client: wire.client,
        grants: wire.grants,
    })
}

#[derive(Serialize)]
struct WireReply<'a> {
    id: Option<&'a str>,
    status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<&'a Value>,
    messages: &'a [MessageLine],
    changed: &'a [String],
}

/// One reply line, without its newline.
pub(super) fn reply_line(
    id: Option<&str>,
    status: Status,
    reason: Option<&str>,
    data: Option<&Value>,
    messages: &[MessageLine],
    changed: &[String],
) -> String {
    let reply = WireReply {
        id,
        status,
        reason,
        data,
        messages,
        changed,
    };
    serde_json::to_string(&reply).unwrap_or_else(|error| {
        format!(r#"{{"id":null,"status":"failed","reason":"reply not encodable: {error}","messages":[],"changed":[]}}"#)
    })
}
