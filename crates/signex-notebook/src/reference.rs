//! References from a notebook to the design, and the argument lists they
//! and the annotation block are written with.

/// A part, net or pin the notebook names. The file stores the calls
/// `#component("R7")`, `#pin("U3", "3")` and `#net("+3V3")` (in code, such
/// as a block's `target:`, without the `#`); a pin may also be written as
/// one dotted name, `#pin("U3.3")`. Typing `@` in the editor only opens the
/// reference picker: `@` is a label reference in Typst, so the file never
/// stores it. Parameter access after a component (`.value`,
/// `.param("Vref")`) still names that component.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Reference {
    /// A part, by designator.
    Component(String),
    /// A net, by name.
    Net(String),
    /// A pin of a part, by designator and pin number or name.
    Pin { component: String, pin: String },
}

impl Reference {
    /// The Typst function that writes this kind of reference: `component`,
    /// `pin` or `net`.
    pub fn function(&self) -> &'static str {
        match self {
            Reference::Component(_) => "component",
            Reference::Net(_) => "net",
            Reference::Pin { .. } => "pin",
        }
    }

    /// The call as code, the form a block argument holds:
    /// `component("R7")`, `pin("U3", "3")`, `net("+3V3")`.
    pub fn to_code(&self) -> String {
        let args = match self {
            Reference::Component(name) | Reference::Net(name) => quote(name),
            Reference::Pin { component, pin } => format!("{}, {}", quote(component), quote(pin)),
        };
        format!("{}({args})", self.function())
    }

    /// The call as markup, the form the editor inserts into running text:
    /// `#component("R7")`, `#pin("U3", "3")`, `#net("+3V3")`.
    pub fn to_markup(&self) -> String {
        format!("#{}", self.to_code())
    }

    /// The pin a dotted name (`U3.3`) names, cut at its first dot; `None`
    /// when either side is empty.
    pub fn dotted_pin(text: &str) -> Option<Self> {
        let (component, pin) = text.split_once('.')?;
        let (component, pin) = (component.trim(), pin.trim());
        (!component.is_empty() && !pin.is_empty()).then(|| Reference::Pin {
            component: component.to_string(),
            pin: pin.to_string(),
        })
    }

    /// The text a reference reads as: `U3`, `VBUS`, `U1.3`.
    pub fn name(&self) -> String {
        match self {
            Reference::Component(name) | Reference::Net(name) => name.clone(),
            Reference::Pin { component, pin } => format!("{component}.{pin}"),
        }
    }

    /// The part a reference sits on: a component, or a pin's component.
    pub fn component(&self) -> Option<&str> {
        match self {
            Reference::Component(name) => Some(name),
            Reference::Pin { component, .. } => Some(component),
            Reference::Net(_) => None,
        }
    }

    /// The reference a call named `function` with `args` (the text between
    /// its parentheses) makes. `None` for another function; `Some(Err(()))`
    /// for a reference function whose arguments are not the quoted names it
    /// takes.
    pub(crate) fn from_call(function: &str, args: &str) -> Option<Result<Self, ()>> {
        let texts: Option<Vec<String>> = parse_args(args)
            .into_iter()
            .map(|arg| match (arg.name, arg.value) {
                (None, Value::Text(text)) if !text.trim().is_empty() => Some(text),
                _ => None,
            })
            .collect();
        let made = match (function, texts.as_deref()) {
            ("component", Some([name])) => Ok(Reference::Component(name.clone())),
            ("net", Some([name])) => Ok(Reference::Net(name.clone())),
            ("pin", Some([component, pin])) => Ok(Reference::Pin {
                component: component.clone(),
                pin: pin.clone(),
            }),
            ("pin", Some([dotted])) => Reference::dotted_pin(dotted).ok_or(()),
            ("component" | "net" | "pin", _) => Err(()),
            _ => return None,
        };
        Some(made)
    }
}

/// Written as `{"kind": "component", "name": "R7"}`, a pin adding its
/// `component` and `pin`, in a JSON result.
impl serde::Serialize for Reference {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("kind", self.function())?;
        map.serialize_entry("name", &self.name())?;
        if let Reference::Pin { component, pin } = self {
            map.serialize_entry("component", component)?;
            map.serialize_entry("pin", pin)?;
        }
        map.end()
    }
}

/// `text` as a Typst string literal, `"` and `\` escaped and control
/// characters written as `\u{..}`.
pub fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if c.is_control() => out.push_str(&format!("\\u{{{:x}}}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// One argument of a call: `name: value` or a positional `value`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Arg {
    pub name: Option<String>,
    pub value: Value,
}

/// An argument's value: a string literal (unescaped) or anything else, as
/// written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Value {
    Text(String),
    Other(String),
}

/// The arguments of `text`, the inside of a call's parentheses, split at the
/// commas outside strings and brackets. Empty pieces (a trailing comma) are
/// dropped.
pub(crate) fn parse_args(text: &str) -> Vec<Arg> {
    split_top_level(text)
        .into_iter()
        .map(str::trim)
        .filter(|piece| !piece.is_empty())
        .map(|piece| {
            let (name, value) = match named_split(piece) {
                Some((name, value)) => (Some(name.to_string()), value.trim()),
                None => (None, piece),
            };
            let value = match unquote(value) {
                Some(text) => Value::Text(text),
                None => Value::Other(value.to_string()),
            };
            Arg { name, value }
        })
        .collect()
}

/// `piece` cut at each comma that is outside a string and every bracket.
fn split_top_level(text: &str) -> Vec<&str> {
    let mut pieces = Vec::new();
    let (mut depth, mut start, mut in_string, mut escaped) = (0usize, 0usize, false, false);
    for (i, c) in text.char_indices() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                pieces.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    pieces.push(&text[start..]);
    pieces
}

/// `name: value` split at its colon when `piece` starts with an identifier
/// followed by one.
fn named_split(piece: &str) -> Option<(&str, &str)> {
    let end = piece
        .char_indices()
        .find(|&(_, c)| !(c.is_alphanumeric() || c == '_' || c == '-'))
        .map_or(piece.len(), |(i, _)| i);
    if end == 0 {
        return None;
    }
    let rest = piece[end..].trim_start();
    rest.strip_prefix(':').map(|value| (&piece[..end], value))
}

/// The text of a string literal, escapes resolved; `None` when `value` is
/// not exactly one string literal.
pub(crate) fn unquote(value: &str) -> Option<String> {
    let inner = value.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return None,
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                'r' => out.push('\r'),
                'u' => out.push(unicode_escape(&mut chars)?),
                other => out.push(other),
            },
            other => out.push(other),
        }
    }
    Some(out)
}

/// The character of a `\u{...}` escape, its `u` already read.
fn unicode_escape(chars: &mut std::str::Chars<'_>) -> Option<char> {
    if chars.next()? != '{' {
        return None;
    }
    let mut hex = String::new();
    for c in chars.by_ref() {
        if c == '}' {
            return u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32);
        }
        hex.push(c);
    }
    None
}
