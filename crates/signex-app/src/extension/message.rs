//! `Message::Extension`: one root variant for every extension's messages.

use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;

use super::ExtensionId;

/// A message an extension sent to itself, carried by `Message::Extension`.
///
/// The application routes it, by [`ExtensionMsg::extension`], to the one
/// installed extension of that id and to no other. The payload is the
/// extension's own message type; only that extension reads it back, with
/// [`ExtensionMsg::payload`]. Name payload variants as past-tense events, as
/// the rest of the application does (`docs/EXTENSION_API.md`, "Messages").
#[derive(Debug, Clone)]
pub struct ExtensionMsg {
    extension: ExtensionId,
    payload: Arc<dyn Payload>,
}

/// What a payload must be to travel through the update loop: printable for
/// the message log, and safe to send to a background task.
trait Payload: Any + Debug + Send + Sync {}

impl<T: Any + Debug + Send + Sync> Payload for T {}

impl ExtensionMsg {
    /// A message for the extension `extension`, carrying `payload`.
    pub fn new<T: Any + Debug + Send + Sync>(extension: ExtensionId, payload: T) -> Self {
        Self {
            extension,
            payload: Arc::new(payload),
        }
    }

    /// The extension this message is routed to.
    pub fn extension(&self) -> &ExtensionId {
        &self.extension
    }

    /// The payload, if it is a `T`.
    pub fn payload<T: Any>(&self) -> Option<&T> {
        let any: &dyn Any = &*self.payload;
        any.downcast_ref()
    }
}
