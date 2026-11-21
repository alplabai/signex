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

/// Verbatim text inside a nested message passes through unchanged. It sits
/// one level down because, at the top level, verbatim text and its
/// unrendered fallback read the same; under a catalog message, a skipped
/// render would show `middle` instead.
#[test]
fn a_nested_verbatim_text_passes_through() {
    let text = Translatable::new(OUTER).arg(
        "why",
        Translatable::new(MIDDLE).arg("path", Translatable::from(UserText::new("a/b"))),
    );
    assert_eq!(
        text.render(&Fake("tr")),
        "tr:outer(why=tr:middle(path=a/b))"
    );
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

/// A renderer that reads `value()` without `rendered_with` (a third-party
/// `MessageRenderer`) sees the nested message's id, not a blank.
#[test]
fn an_unrendered_nested_message_reads_as_its_id() {
    let arg = Arg::from(Translatable::new(INNER));
    assert_eq!(arg.value(), ArgValue::Text("inner"));
    assert_eq!(
        arg.rendered_with(&Fake("en")).value(),
        ArgValue::Text("en:inner()")
    );
}

/// #2326 M2: legacy English renders as written in every language, has no
/// id, and an argument does not change it.
#[test]
fn legacy_english_renders_as_written_and_has_no_id() {
    let text = Translatable::legacy_english("Nothing to undo").arg("count", 1_u32);
    assert_eq!(text.render(&Fake("de")), "Nothing to undo");
    assert_eq!(text.id(), None);
    assert_ne!(
        text,
        Translatable::verbatim(UserText::new("Nothing to undo"))
    );
}
