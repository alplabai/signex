//! Parsing and checking a command's operands from text (#1934): the
//! `key=value` pairs a CLI or an agent passes, validated against the
//! command's descriptor in the catalog. Pure: nothing here runs a command.

use std::collections::BTreeMap;
use std::path::Path;

use signex_types::format::mm_as_stored_nm;

use crate::keymap::{AppCommandId, Operand, OperandKind, metadata_for};

use super::CommandArgs;
use super::args::Operands;

/// One operand's value, in the unit its [`OperandKind`] names.
#[derive(Debug, Clone, PartialEq)]
pub enum OperandValue {
    /// `x`, `y` in mm.
    Point(f64, f64),
    /// Degrees.
    Angle(f64),
    Text(String),
    /// An absolute path on the host running the command (`C:\...` on Windows).
    Path(String),
    Bool(bool),
    /// An opaque colour: red, green, blue.
    Color(u8, u8, u8),
}

/// Why operands were refused; every variant names the operand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperandError {
    /// An argument that is not `key=value`.
    Malformed(String),
    /// The command takes no operand of that name.
    Unknown { command: String, operand: String },
    /// A required operand was not given.
    Missing { command: String, operand: String },
    /// The same operand was given twice.
    Duplicate(String),
    /// The value is not of the operand's kind.
    Invalid {
        operand: String,
        expected: &'static str,
        got: String,
    },
}

impl std::fmt::Display for OperandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed(arg) => write!(f, "`{arg}` is not key=value"),
            Self::Unknown { command, operand } => {
                write!(f, "`{command}` takes no operand `{operand}`")
            }
            Self::Missing { command, operand } => {
                write!(f, "`{command}` needs the operand `{operand}`")
            }
            Self::Duplicate(operand) => write!(f, "operand `{operand}` is given twice"),
            Self::Invalid {
                operand,
                expected,
                got,
            } => write!(f, "operand `{operand}` must be {expected}, got `{got}`"),
        }
    }
}

impl std::error::Error for OperandError {}

impl CommandArgs {
    /// The operands of `command` from `key=value` arguments, checked against
    /// the catalog's descriptor: an unknown name, a missing required one, a
    /// repeated one or a value of the wrong form is an error naming it.
    pub fn parse_operands(command: &AppCommandId, args: &[&str]) -> Result<Self, OperandError> {
        let schema = metadata_for(command).map_or(&[][..], |meta| meta.operands);
        let mut values = BTreeMap::new();
        for arg in args {
            let (name, text) = arg
                .split_once('=')
                .ok_or_else(|| OperandError::Malformed((*arg).to_string()))?;
            let operand =
                schema
                    .iter()
                    .find(|op| op.name == name)
                    .ok_or_else(|| OperandError::Unknown {
                        command: command.as_str().to_string(),
                        operand: name.to_string(),
                    })?;
            let value = parse_value(operand, text)?;
            if values.insert(name.to_string(), value).is_some() {
                return Err(OperandError::Duplicate(name.to_string()));
            }
        }
        if let Some(op) = schema
            .iter()
            .find(|op| op.required && !values.contains_key(op.name))
        {
            return Err(OperandError::Missing {
                command: command.as_str().to_string(),
                operand: op.name.to_string(),
            });
        }
        Ok(Self {
            operands: Operands { values },
            ..Self::none()
        })
    }

    /// The operand `name` as parsed, `None` when it was not given.
    pub fn operand(&self, name: &str) -> Option<&OperandValue> {
        self.operands.values.get(name)
    }

    /// Whether any operand was given.
    pub(crate) fn has_operands(&self) -> bool {
        !self.operands.values.is_empty()
    }
}

fn parse_value(operand: &Operand, text: &str) -> Result<OperandValue, OperandError> {
    let invalid = |expected: &'static str| OperandError::Invalid {
        operand: operand.name.to_string(),
        expected,
        got: text.to_string(),
    };
    // `+ 0.0` turns -0 into 0.
    let number = |s: &str| {
        s.trim()
            .parse::<f64>()
            .ok()
            .filter(|n| n.is_finite())
            .map(|n| n + 0.0)
    };
    match operand.kind {
        OperandKind::Point => {
            let (x, y) = text.split_once(',').ok_or_else(|| invalid("x,y in mm"))?;
            // A coordinate the document cannot store as nm would make it unsaveable.
            let storable = |n: Option<f64>| n.filter(|mm| mm_as_stored_nm(*mm).is_some());
            match (storable(number(x)), storable(number(y))) {
                (Some(x), Some(y)) => Ok(OperandValue::Point(x, y)),
                _ => Err(invalid("x,y in mm within the drawing range")),
            }
        }
        // The pre-placement form offers 0, 90, 180 and 270 only, so a right
        // angle is stored in that form (-90 reads as 270, 360 as 0).
        OperandKind::Angle => number(text)
            .filter(|deg| deg % 90.0 == 0.0 && deg.abs() <= 360.0)
            .map(|deg| OperandValue::Angle(deg.rem_euclid(360.0) + 0.0))
            .ok_or_else(|| invalid("a multiple of 90 degrees")),
        OperandKind::Text if text.chars().any(char::is_control) => {
            Err(invalid("text without control characters"))
        }
        OperandKind::Text if text.trim().is_empty() && operand.required => {
            Err(invalid("non-empty text"))
        }
        OperandKind::Text => Ok(OperandValue::Text(text.to_string())),
        OperandKind::Path if text.chars().any(char::is_control) => {
            Err(invalid("a path without control characters"))
        }
        OperandKind::Path if Path::new(text).is_absolute() => {
            Ok(OperandValue::Path(text.to_string()))
        }
        OperandKind::Path => Err(invalid("an absolute path")),
        OperandKind::Bool => match text {
            "true" => Ok(OperandValue::Bool(true)),
            "false" => Ok(OperandValue::Bool(false)),
            _ => Err(invalid("true or false")),
        },
        OperandKind::Color => parse_color(text).ok_or_else(|| invalid("#rrggbb")),
    }
}

/// `#rrggbb` or `rrggbb`: six hex digits, nothing else.
fn parse_color(text: &str) -> Option<OperandValue> {
    let digits = text.strip_prefix('#').unwrap_or(text);
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |at: usize| u8::from_str_radix(&digits[at..at + 2], 16).ok();
    Some(OperandValue::Color(channel(0)?, channel(2)?, channel(4)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::{CommandMetadata, OperandKind, all_command_ids, metadata_for_str};

    fn parse(id: &str, args: &[&str]) -> Result<CommandArgs, OperandError> {
        CommandArgs::parse_operands(&AppCommandId::new(id).unwrap(), args)
    }

    #[test]
    fn a_label_parses_its_operands() {
        let args = parse(
            "place_net_label",
            &["at=10.16,5.08", "text=VCC", "rotation=90"],
        )
        .unwrap();
        assert_eq!(args.operand("at"), Some(&OperandValue::Point(10.16, 5.08)));
        assert_eq!(
            args.operand("text"),
            Some(&OperandValue::Text("VCC".into()))
        );
        assert_eq!(args.operand("rotation"), Some(&OperandValue::Angle(90.0)));
    }

    #[test]
    fn an_optional_operand_may_be_left_out() {
        let args = parse("place_net_label", &["at=0,0", "text=A"]).unwrap();
        assert_eq!(args.operand("rotation"), None);
    }

    #[test]
    fn an_unknown_operand_is_named() {
        let err = parse("place_junction", &["at=0,0", "colour=red"]).unwrap_err();
        assert_eq!(
            err.to_string(),
            "`place_junction` takes no operand `colour`"
        );
        let err = parse("undo", &["at=0,0"]).unwrap_err();
        assert!(matches!(err, OperandError::Unknown { .. }));
    }

    #[test]
    fn a_missing_required_operand_is_named() {
        let err = parse("place_net_label", &["at=0,0"]).unwrap_err();
        assert_eq!(
            err.to_string(),
            "`place_net_label` needs the operand `text`"
        );
    }

    #[test]
    fn a_repeated_or_malformed_argument_is_refused() {
        assert_eq!(
            parse("place_junction", &["at=0,0", "at=1,1"]).unwrap_err(),
            OperandError::Duplicate("at".into())
        );
        assert!(matches!(
            parse("place_junction", &["at"]).unwrap_err(),
            OperandError::Malformed(_)
        ));
    }

    /// `path=<absolute path in the temp dir>`: absolute on every host.
    fn path_arg(file: &str) -> String {
        format!("path={}", std::env::temp_dir().join(file).display())
    }

    #[test]
    fn a_value_of_the_wrong_form_is_named() {
        let save = path_arg("x.snxsch");
        let nul_path = path_arg("a\0b.snxsch");
        let cases: Vec<(&str, Vec<&str>, &str)> = vec![
            ("place_junction", vec!["at=1"], "at"),
            ("place_junction", vec!["at=a,b"], "at"),
            ("place_junction", vec!["at=1,inf"], "at"),
            ("place_junction", vec!["at=1e300,0"], "at"),
            (
                "place_net_label",
                vec!["at=0,0", "text=A", "rotation=NaN"],
                "rotation",
            ),
            (
                "place_net_label",
                vec!["at=0,0", "text=A", "rotation=45"],
                "rotation",
            ),
            ("place_net_label", vec!["at=0,0", "text="], "text"),
            ("place_net_label", vec!["at=0,0", "text=a\nb"], "text"),
            ("save_document_as", vec!["path=relative/x.snxsch"], "path"),
            ("open_document", vec![&nul_path], "path"),
            (
                "save_document_as",
                vec![&save, "overwrite=yes"],
                "overwrite",
            ),
        ];
        for (id, args, operand) in cases {
            let err = parse(id, &args).unwrap_err();
            assert!(
                matches!(&err, OperandError::Invalid { operand: o, .. } if o == operand),
                "{id} {args:?}: {err}"
            );
        }
    }

    #[test]
    fn a_save_dialog_answer_parses() {
        let save = path_arg("a.snxsch");
        let args = parse("save_document_as", &[&save, "overwrite=true"]).unwrap();
        assert_eq!(args.operand("overwrite"), Some(&OperandValue::Bool(true)));
        let open = path_arg("a.snxprj");
        assert_eq!(
            parse("open_document", &[&open])
                .unwrap()
                .operand("overwrite"),
            None
        );
    }

    #[test]
    fn a_value_splits_on_the_first_equals_only() {
        let arg = path_arg("a=b.snxsch");
        let args = parse("open_document", &[&arg]).unwrap();
        let Some(OperandValue::Path(path)) = args.operand("path") else {
            panic!("no path operand");
        };
        assert!(path.ends_with("a=b.snxsch"), "{path}");
    }

    #[test]
    fn negative_zero_reads_as_zero_and_right_angles_pass() {
        let args = parse("place_net_label", &["at=-0,-0.0", "text=A", "rotation=-0"]).unwrap();
        let Some(OperandValue::Point(x, y)) = args.operand("at") else {
            panic!("no point");
        };
        assert!(x.is_sign_positive() && y.is_sign_positive());
        let Some(OperandValue::Angle(deg)) = args.operand("rotation") else {
            panic!("no angle");
        };
        assert!(deg.is_sign_positive());
        for (given, stored) in [
            ("90", 90.0),
            ("180", 180.0),
            ("270", 270.0),
            ("-90", 270.0),
            ("360", 0.0),
            ("-360", 0.0),
        ] {
            let args = parse(
                "place_net_label",
                &["at=0,0", "text=A", &format!("rotation={given}")],
            )
            .unwrap();
            assert_eq!(
                args.operand("rotation"),
                Some(&OperandValue::Angle(stored)),
                "{given}"
            );
        }
        for refused in [
            ["at=0,0", "text=A", "rotation=1e300"],
            ["at=0,0", "text=A", "rotation=720"],
            ["at=0,0", "text=A\t", "rotation=0"],
            ["at=0,0", "text= ", "rotation=0"],
        ] {
            assert!(parse("place_net_label", &refused).is_err(), "{refused:?}");
        }
    }

    /// The one convention (#1923): every descriptor names operands from the
    /// shared table with that name's one kind, and a `gui_only` row has none.
    #[test]
    fn every_descriptor_follows_the_one_convention() {
        let mut filled = 0;
        for id in all_command_ids() {
            let meta: &CommandMetadata = metadata_for_str(id).unwrap();
            filled += usize::from(!meta.operands.is_empty());
            assert!(
                !(meta.flags.gui_only && !meta.operands.is_empty()),
                "{id} is gui_only but has an operand form"
            );
            let mut seen = Vec::new();
            for op in meta.operands {
                let want = crate::keymap::OPERAND_CONVENTION
                    .iter()
                    .find(|(name, _)| *name == op.name)
                    .map(|(_, kind)| *kind);
                assert_eq!(want, Some(op.kind), "{id}: operand `{}`", op.name);
                assert!(!seen.contains(&op.name), "{id}: `{}` twice", op.name);
                seen.push(op.name);
                assert_eq!(
                    op.kind.unit().is_some(),
                    matches!(op.kind, OperandKind::Point | OperandKind::Angle)
                );
            }
        }
        assert!(filled > 0);
    }
}
