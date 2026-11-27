//! Message files an extension ships (#2495): checked before they join a
//! bundle, then added to the active locale's bundle and to the en-US
//! fallback, so they follow the app language and fall back like the
//! built-in messages.

use fluent_bundle::FluentResource;
use fluent_syntax::ast::Entry;

use crate::I18n;
use crate::bundle::{self, Bundle};
use crate::catalog::{EN_US, EN_XA};

/// One `.ftl` file of an extension, for the locale `locale` (a BCP 47 tag
/// such as `en-US`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtensionMessages {
    pub locale: &'static str,
    pub source: &'static str,
}

/// The ids `source` defines: each message's id, and each term's with its
/// leading `-`. An error when the file does not parse, naming the problem.
pub fn ftl_entry_ids(source: &str) -> Result<Vec<String>, String> {
    let resource = FluentResource::try_new(source.to_owned())
        .map_err(|(_, errors)| format!("the message file does not parse: {errors:?}"))?;
    Ok(resource
        .entries()
        .filter_map(|entry| match entry {
            Entry::Message(message) => Some(message.id.name.to_owned()),
            Entry::Term(term) => Some(format!("-{}", term.id.name)),
            _ => None,
        })
        .collect())
}

/// Add `files` to `bundle`: the ones for `tag`.
fn add_to(bundle: &mut Bundle, tag: &str, files: &[ExtensionMessages]) {
    for file in files.iter().filter(|file| file.locale == tag) {
        bundle::add_source(bundle, tag, file.source);
    }
}

impl I18n {
    /// This instance with the extensions' message `files` added: the files
    /// for the active locale to its bundle (the pseudolocale reads the
    /// en-US ones, as it does for the built-in messages), and the en-US
    /// files to the fallback. A message the active locale's files lack
    /// falls back to en-US, as a built-in one does. The files were checked
    /// at install, so none defines an id outside its extension's namespace.
    #[must_use]
    pub fn with_extension_messages(mut self, files: &[ExtensionMessages]) -> Self {
        if files.is_empty() {
            return self;
        }
        let source_tag = if self.locale() == EN_XA {
            EN_US
        } else {
            self.locale()
        };
        add_to(self.primary_mut(), source_tag, files);
        if self.locale() != EN_US {
            add_to(self.fallback_mut(), EN_US, files);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LanguagePref, MessageId};

    const EN: ExtensionMessages = ExtensionMessages {
        locale: "en-US",
        source: "ext-sample-title = Notes\next-sample-only-en = English only\n",
    };

    fn id(text: &'static str) -> MessageId {
        MessageId::extension(text).expect("an extension id")
    }

    #[test]
    fn the_ids_of_a_file_are_its_messages_and_its_terms() {
        let ids = ftl_entry_ids("ext-a-b = B\n-ext-a-term = T\n").expect("parses");
        assert_eq!(ids, ["ext-a-b", "-ext-a-term"]);
        // A message with no `=` is junk to the parser.
        assert!(ftl_entry_ids("ext-a-b\n").is_err());
    }

    #[test]
    fn an_extension_message_renders_in_en_us_and_in_the_pseudolocale() {
        let en =
            I18n::new(&LanguagePref::Locale("en-US".to_owned())).with_extension_messages(&[EN]);
        assert_eq!(
            en.message(id("ext-sample-title"), &[]).into_string(),
            "Notes"
        );

        // The pseudolocale reads the en-US files, transformed like the
        // built-in messages.
        let xa =
            I18n::new(&LanguagePref::Locale("en-XA".to_owned())).with_extension_messages(&[EN]);
        let shown = xa.message(id("ext-sample-title"), &[]).into_string();
        assert!(shown.starts_with('[') && shown != "Notes", "{shown}");

        // Without the files the id shows, as for any missing message.
        let bare = I18n::new(&LanguagePref::Locale("en-US".to_owned()));
        assert_eq!(
            bare.message(id("ext-sample-title"), &[]).into_string(),
            "ext-sample-title"
        );
    }
}
