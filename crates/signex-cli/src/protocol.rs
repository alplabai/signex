//! The JSON Lines wire format (#1923): one command object per input line,
//! one result object per output line. #1924 sends the same objects.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A command to run: its id and its operands as `name`, `text` pairs, the
/// text being what `CommandArgs::parse_operands` reads (`1.5,2` for a point).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub id: String,
    pub operands: Vec<(String, String)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    id: String,
    #[serde(default)]
    args: Map<String, Value>,
}

/// One input line: `{"id": "...", "args": {...}}`. An argument is a string,
/// a number, a boolean, `[x, y]` for a point, or the list of edit objects of
/// `apply_edits`; anything else is refused.
pub fn parse_request(line: &str) -> Result<Request, String> {
    let wire: Wire =
        serde_json::from_str(line).map_err(|error| format!("malformed command line: {error}"))?;
    let operands = wire
        .args
        .iter()
        .map(|(name, value)| Ok((name.clone(), operand_text(name, value)?)))
        .collect::<Result<_, String>>()?;
    Ok(Request {
        id: wire.id,
        operands,
    })
}

/// `signex-cli run <project> <id> --arg key=value`: the same request without
/// JSON.
pub fn request_from_args(id: &str, args: &[String]) -> Result<Request, String> {
    let operands = args
        .iter()
        .map(|arg| {
            arg.split_once('=')
                .map(|(name, text)| (name.to_string(), text.to_string()))
                .ok_or_else(|| format!("`{arg}` is not key=value"))
        })
        .collect::<Result<_, _>>()?;
    Ok(Request {
        id: id.to_string(),
        operands,
    })
}

/// The text of one argument: the app reads the same forms inside the edits of
/// `apply_edits`, so there is one rule for it.
fn operand_text(name: &str, value: &Value) -> Result<String, String> {
    signex_app::headless::operand_text(name, value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// The command ran and changed or did what it names.
    Ran,
    /// A query ran; its answer is in `data`.
    Answered,
    /// Nothing ran: the runner's policy or the app said no, with a reason.
    Refused,
    /// The command ran and did not succeed, or never finished.
    Failed,
}

impl Status {
    pub const fn is_ok(self) -> bool {
        matches!(self, Self::Ran | Self::Answered)
    }
}

/// One entry of the app's Messages log, as it was logged during a command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MessageLine {
    pub level: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResultLine {
    /// The run this line belongs to (also in the audit log).
    pub run: String,
    /// 1-based position of the command line in the run.
    pub seq: u64,
    /// The command id; `null` when the line was not a command.
    pub id: Option<String>,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    pub messages: Vec<MessageLine>,
    /// Documents whose content or unsaved state differs after the command.
    pub changed: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn operands(line: &str) -> Vec<(String, String)> {
        parse_request(line).unwrap().operands
    }

    fn pair(name: &str, text: &str) -> (String, String) {
        (name.to_string(), text.to_string())
    }

    #[test]
    fn a_command_line_becomes_a_request_with_operand_texts() {
        let request = parse_request(
            r#"{"id":"place_net_label","args":{"at":[10.16,5.08],"text":"VCC","rotation":90,"overwrite":true}}"#,
        )
        .unwrap();
        assert_eq!(request.id, "place_net_label");
        assert_eq!(
            request.operands,
            [
                pair("at", "10.16,5.08"),
                pair("overwrite", "true"),
                pair("rotation", "90"),
                pair("text", "VCC"),
            ]
        );
        assert_eq!(operands(r#"{"id":"undo"}"#), []);
    }

    #[test]
    fn a_line_that_is_not_a_command_is_refused_with_a_reason() {
        for bad in [
            "not json",
            "[]",
            r#"{"args":{}}"#,
            r#"{"id":5}"#,
            r#"{"id":"undo","extra":1}"#,
            r#"{"id":"undo","args":[1]}"#,
        ] {
            let err = parse_request(bad).unwrap_err();
            assert!(err.starts_with("malformed command line"), "{bad}: {err}");
        }
    }

    #[test]
    fn an_argument_of_an_unsupported_shape_names_the_operand() {
        for (args, wanted) in [
            (r#"{"at":[1]}"#, "operand `at` must be [x, y]"),
            (r#"{"at":[1,"2"]}"#, "operand `at` must be [x, y]"),
            (r#"{"text":null}"#, "operand `text` must be a string"),
            (r#"{"text":{"a":1}}"#, "operand `text` must be a string"),
            (r#"{"a=b":"x"}"#, "must not contain `=`"),
        ] {
            let line = format!(r#"{{"id":"x","args":{args}}}"#);
            let err = parse_request(&line).unwrap_err();
            assert!(err.contains(wanted), "{args}: {err}");
        }
    }

    #[test]
    fn deeply_nested_input_is_an_error_not_a_stack_overflow() {
        let line = format!(
            r#"{{"id":"x","args":{{"a":{}1{}}}}}"#,
            "[".repeat(5000),
            "]".repeat(5000)
        );
        assert!(parse_request(&line).is_err());
    }

    #[test]
    fn single_command_arguments_split_on_the_first_equals() {
        let request = request_from_args("place_net_label", &["text=a=b".to_string()]).unwrap();
        assert_eq!(request.operands, [pair("text", "a=b")]);
        let err = request_from_args("x", &["nokey".to_string()]).unwrap_err();
        assert_eq!(err, "`nokey` is not key=value");
    }

    #[test]
    fn a_result_line_has_the_documented_shape() {
        let line = ResultLine {
            run: "r".into(),
            seq: 2,
            id: Some("find".into()),
            status: Status::Answered,
            reason: None,
            data: Some(json!({"n": 1})),
            messages: vec![MessageLine {
                level: "INFO",
                message: "m".into(),
            }],
            changed: vec!["a".into()],
        };
        assert_eq!(
            serde_json::to_value(&line).unwrap(),
            json!({"run":"r","seq":2,"id":"find","status":"answered","data":{"n":1},
                   "messages":[{"level":"INFO","message":"m"}],"changed":["a"]})
        );
        let refused = ResultLine {
            id: None,
            status: Status::Refused,
            reason: Some("why".into()),
            data: None,
            ..line
        };
        let value = serde_json::to_value(&refused).unwrap();
        assert_eq!(value["status"], "refused");
        assert_eq!(value["reason"], "why");
        assert!(value["id"].is_null() && value.get("data").is_none());
        assert!(Status::Ran.is_ok() && Status::Answered.is_ok());
        assert!(!Status::Refused.is_ok() && !Status::Failed.is_ok());
    }
}
