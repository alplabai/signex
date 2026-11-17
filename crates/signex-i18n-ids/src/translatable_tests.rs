//! Nested message arguments: the renderer fills the inner message first.

use super::*;
use crate::ArgValue;

/// Writes `lang:id(name=value, …)`, so a test reads which renderer filled
/// which message and with what.
struct Fake(&'static str);

impl MessageRenderer for Fake {
    fn message_plain(&self, id: MessageId, args: &[(&str, Arg)]) -> String {
        let args: Vec<String> = args
            .iter()
            .map(|(name, arg)| match arg.value() {
                ArgValue::Text(text) => format!("{name}={text}"),
                ArgValue::Number(number) => format!("{name}={number}"),
            })
            .collect();
        format!("{}:{}({})", self.0, id.as_str(), args.join(", "))
    }
}

const OUTER: MessageId = MessageId::new("outer");
const MIDDLE: MessageId = MessageId::new("middle");
const INNER: MessageId = MessageId::new("inner");

#[test]
fn a_nested_message_is_rendered_before_it_fills_the_outer_one() {
    let text = Translatable::new(OUTER)
        .arg("menu", Translatable::new(INNER))
        .arg("count", 3_u32);
    assert_eq!(
        text.render(&Fake("en")),
        "en:outer(menu=en:inner(), count=3)"
    );
}

#[test]
fn two_levels_of_nesting_render_inside_out() {
    let text = Translatable::new(OUTER).arg(
        "why",
        Translatable::new(MIDDLE).arg("what", Translatable::new(INNER).arg("n", "x")),
    );
    assert_eq!(
        text.render(&Fake("en")),
        "en:outer(why=en:middle(what=en:inner(n=x)))"
    );
}

#[test]
fn a_language_switch_re_renders_the_nested_message_too() {
    let text = Translatable::new(OUTER).arg("menu", Translatable::new(INNER));
    assert_eq!(text.render(&Fake("en")), "en:outer(menu=en:inner())");
    assert_eq!(text.render(&Fake("tr")), "tr:outer(menu=tr:inner())");
}

#[test]
fn a_nested_verbatim_text_passes_through() {
    let text = Translatable::new(OUTER).arg("path", Translatable::from(UserText::new("a/b")));
    assert_eq!(text.render(&Fake("tr")), "tr:outer(path=a/b)");
}

#[test]
fn a_nested_message_keeps_equality_and_clone() {
    let a = Translatable::new(OUTER).arg("menu", Translatable::new(INNER));
    assert_eq!(a.clone(), a);
    assert_ne!(
        a,
        Translatable::new(OUTER).arg("menu", Translatable::new(MIDDLE))
    );
}

#[test]
fn an_unrendered_nested_message_reads_as_its_id() {
    let arg = Arg::from(Translatable::new(INNER));
    assert_eq!(arg.value(), ArgValue::Text("inner"));
}
