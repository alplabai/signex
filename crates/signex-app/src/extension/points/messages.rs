//! The `.ftl` message files an extension ships (#2495). Every id a file
//! defines sits under the owner's namespace, `ext-<name>-` (a term:
//! `-ext-<name>-`), so it can never shadow a built-in message. The files
//! follow the app language and fall back to en-US like the built-in ones.

use signex_i18n::{ExtensionMessages, MessageId};

use super::RegisterError;
use crate::extension::ExtensionId;

/// The message files one extension ships, in the order it added them.
#[derive(Debug)]
#[non_exhaustive]
pub struct Messages {
    owner: ExtensionId,
    files: Vec<ExtensionMessages>,
    pub(super) errors: Vec<RegisterError>,
}

impl Messages {
    pub(super) fn new(owner: ExtensionId) -> Self {
        Self {
            owner,
            files: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// The prefix every message id of the owner starts with:
    /// `ext-<name>-`.
    pub fn prefix(&self) -> String {
        Self::prefix_of(&self.owner)
    }

    /// The message-id prefix of `owner`: `ext-<name>-`.
    pub(super) fn prefix_of(owner: &ExtensionId) -> String {
        format!("{}{}-", signex_i18n::EXTENSION_MESSAGE_PREFIX, owner.name())
    }

    /// Add the `.ftl` file `source` for the locale `locale` (`en-US`, the
    /// one every other locale falls back to, should come first). Refused,
    /// and recorded so install refuses the set, when it does not parse
    /// (`InvalidMessages`) or defines an id outside the owner's namespace
    /// (`NotOwned`).
    pub fn add(&mut self, locale: &'static str, source: &'static str) -> Result<(), RegisterError> {
        let refused = match signex_i18n::ftl_entry_ids(source) {
            Err(why) => Some(RegisterError::InvalidMessages(why)),
            Ok(ids) => {
                let prefix = self.prefix();
                ids.into_iter()
                    .find(|id| !id.trim_start_matches('-').starts_with(&prefix))
                    .map(RegisterError::NotOwned)
            }
        };
        if let Some(error) = refused {
            self.errors.push(error.clone());
            return Err(error);
        }
        self.files.push(ExtensionMessages { locale, source });
        Ok(())
    }

    /// The id of the owner's message `id`, for a panel title, a row label
    /// or a section heading. `None` when `id` is not in the owner's
    /// namespace.
    pub fn id(&self, id: &'static str) -> Option<MessageId> {
        id.starts_with(&self.prefix())
            .then(|| MessageId::extension(id))
            .flatten()
    }

    /// The files added, in order.
    pub fn files(&self) -> &[ExtensionMessages] {
        &self.files
    }

    /// Were no files added?
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}
