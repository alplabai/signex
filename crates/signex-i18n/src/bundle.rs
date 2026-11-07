//! One Fluent bundle per locale, and rendering a message through it.

use std::borrow::Cow;

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

use crate::catalog::{self, EN_XA};
use crate::numbers::format_value;
use crate::types::Arg;

pub(crate) type Bundle = FluentBundle<FluentResource>;

fn pseudolocalize(text: &str) -> Cow<'_, str> {
    fluent_pseudo::transform(text, false, true)
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

/// `id` rendered in `bundle`, or `None` when the bundle has no such message.
pub(crate) fn render(bundle: &Bundle, id: &str, args: &[(&str, Arg)]) -> Option<String> {
    let pattern = bundle.get_message(id)?.value()?;
    let mut fluent_args = FluentArgs::new();
    for (key, arg) in args {
        let value = match arg {
            Arg::Text(text) => FluentValue::from(text.clone()),
            Arg::Number(number) => FluentValue::from(*number),
        };
        fluent_args.set(*key, value);
    }
    let mut errors = Vec::new();
    let text = bundle.format_pattern(pattern, Some(&fluent_args), &mut errors);
    if !errors.is_empty() {
        log::warn!("Message {id} did not format cleanly: {errors:?}");
    }
    Some(text.into_owned())
}
