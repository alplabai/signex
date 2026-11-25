//! One Fluent bundle per locale, and rendering a message through it.

use std::borrow::Cow;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

use crate::catalog::{self, EN_XA};
use crate::numbers::format_value;
use signex_i18n_ids::ArgValue;

use crate::Arg;

pub(crate) type Bundle = FluentBundle<FluentResource>;

fn pseudolocalize(text: &str) -> Cow<'_, str> {
    fluent_pseudo::transform(text, false, true)
}

/// Whether `bundle` is the pseudolocale's. Its whole rendered message is
/// bracketed, so a string drawn without brackets is not from the catalog
/// (#2064). The transform alone cannot: it sees each text run, not the message.
fn is_pseudo(bundle: &Bundle) -> bool {
    bundle.locales.first().is_some_and(|lang| {
        lang.language.as_str() == "en" && lang.region.as_ref().is_some_and(|r| r.as_str() == "XA")
    })
}

/// The bundle of `tag`, built from its embedded sources. A source that does
/// not parse keeps its good messages and reports the rest.
pub(crate) fn build(tag: &str) -> Bundle {
    build_from(tag, catalog::sources(tag))
}

/// [`build`] over explicit sources.
pub(crate) fn build_from(tag: &str, sources: &[&str]) -> Bundle {
    let lang: LanguageIdentifier = tag.parse().expect("a catalog tag is a valid BCP 47 tag");
    let mut bundle = FluentBundle::new_concurrent(vec![lang]);
    bundle.set_formatter(Some(format_value));
    if tag == EN_XA {
        bundle.set_transform(Some(pseudolocalize));
    }
    for source in sources {
        let resource =
            FluentResource::try_new((*source).to_owned()).unwrap_or_else(|(partial, errors)| {
                log::error!("A {tag} message file does not parse: {errors:?}");
                partial
            });
        if let Err(errors) = bundle.add_resource(resource) {
            log::error!("A {tag} message file repeats a message id: {errors:?}");
        }
    }
    bundle
}

/// A rendered message and, when Fluent hit errors on the way, what they were.
pub(crate) struct Rendered {
    pub text: String,
    pub problems: Option<String>,
}

/// `id` rendered in `bundle`, or `None` when the bundle has no such message.
pub(crate) fn render(bundle: &Bundle, id: &str, args: &[(&str, Arg)]) -> Option<Rendered> {
    let pattern = bundle.get_message(id)?.value()?;
    let mut fluent_args = FluentArgs::new();
    for (key, arg) in args {
        let value = match arg.value() {
            ArgValue::Text(text) => FluentValue::from(text.to_owned()),
            ArgValue::Number(number) => FluentValue::from(number),
        };
        fluent_args.set(*key, value);
    }
    let mut errors = Vec::new();
    let text = bundle.format_pattern(pattern, Some(&fluent_args), &mut errors);
    let problems = (!errors.is_empty()).then(|| format!("{errors:?}"));
    let text = if is_pseudo(bundle) {
        format!("[{text}]")
    } else {
        text.into_owned()
    };
    Some(Rendered { text, problems })
}
