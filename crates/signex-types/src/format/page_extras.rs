//! `[extras.sheet.page]` (#2034): the sheet's paper colour and coordinate
//! origin, plus every key this build cannot read, kept raw so a save writes
//! it back unchanged: a key a newer build added, or a `color` / `origin`
//! value it wrote in a form this build does not know. Such a value shows as
//! the default until the user picks a new one, which replaces it.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

use super::unknown::UnknownFields;
use crate::schematic::{PageOrigin, SheetPageSettings};
use crate::theme::Color;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub(in crate::format) struct PageExtras {
    #[serde(flatten)]
    pub(in crate::format) settings: SheetPageSettings,
    #[serde(flatten)]
    pub(in crate::format) unknown: UnknownFields,
}

impl PageExtras {
    pub(in crate::format) fn is_default(&self) -> bool {
        self.settings.is_default() && self.unknown.is_empty()
    }

    /// The table for `settings` and the raw keys kept from the file. A key
    /// the settings write now is never written twice.
    pub(in crate::format) fn of(settings: SheetPageSettings, kept: &UnknownFields) -> Self {
        let mut unknown = kept.clone();
        if settings.color.is_some() {
            unknown.remove("color");
        }
        if settings.origin != PageOrigin::default() {
            unknown.remove("origin");
        }
        Self { settings, unknown }
    }
}

impl<'de> Deserialize<'de> for PageExtras {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut out = Self::default();
        for (key, value) in BTreeMap::<String, toml::Value>::deserialize(d)? {
            let read = match key.as_str() {
                "color" => value
                    .as_str()
                    .and_then(Color::parse_hex)
                    .map(|color| out.settings.color = Some(color))
                    .is_some(),
                "origin" => value
                    .clone()
                    .try_into::<PageOrigin>()
                    .map(|origin| out.settings.origin = origin)
                    .is_ok(),
                _ => false,
            };
            if !read {
                out.unknown.insert_value(&key, value);
            }
        }
        Ok(out)
    }
}
