//! Which font families are installed, probed once and cached.
//!
//! A document names a family; whether this machine has it is a render-time
//! question (#510). The probe scans the system font directories, which is
//! slow, so it runs off the update thread ([`scan_in_background`]) and the
//! render path only ever *reads* the cached result ([`installed`]). Until the
//! scan lands, every named family reads as "not known to be installed" and
//! renders in the bundled default, so the first frames are deterministic
//! instead of racing the scan.

use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;

use signex_types::font::BUNDLED_DEFAULT_FAMILY;

/// Families compiled into the binary and registered with iced at startup
/// (`fonts::bundled`). Always installed, on every machine.
pub const BUNDLED_FAMILIES: [&str; 2] = [BUNDLED_DEFAULT_FAMILY, "Roboto"];

/// The set of installed family names.
#[derive(Debug, Clone)]
pub struct FontCatalog {
    /// Display names, sorted, as the fonts spell them.
    names: Vec<String>,
    /// Lower-cased lookup key to the family's canonical spelling: names
    /// compare case-insensitively, and the shaper matches exactly.
    keys: HashMap<String, String>,
}

impl FontCatalog {
    /// A catalog of `families` plus the bundled ones. Hermetic: it asks the
    /// machine nothing, which is what the tests want.
    pub fn from_families<I, S>(families: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let names: BTreeSet<String> = families
            .into_iter()
            .map(|family| family.as_ref().trim().to_string())
            .filter(|family| !family.is_empty())
            .chain(BUNDLED_FAMILIES.iter().map(|family| family.to_string()))
            .collect();
        let mut keys = HashMap::new();
        for name in &names {
            keys.entry(name.to_lowercase())
                .or_insert_with(|| name.clone());
        }
        Self {
            names: names.into_iter().collect(),
            keys,
        }
    }

    /// Scan the system font directories. Slow; never call it on the update
    /// thread.
    pub fn scan_system() -> Self {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        let mut catalog = Self::from_families(
            db.faces()
                .filter_map(|face| face.families.first().map(|(name, _)| name.clone())),
        );
        // A face can be known under several family names (localised names,
        // legacy names); a document may use any of them, so they all count
        // as installed even though the pickers list one name per face.
        for (name, _) in db.faces().flat_map(|face| face.families.iter()) {
            catalog
                .keys
                .entry(name.to_lowercase())
                .or_insert_with(|| name.clone());
        }
        catalog
    }

    /// Whether `family` is installed.
    pub fn has(&self, family: &str) -> bool {
        self.canonical(family).is_some()
    }

    /// The installed spelling of `family`, whatever case it was asked in.
    pub fn canonical(&self, family: &str) -> Option<&str> {
        self.keys
            .get(&family.trim().to_lowercase())
            .map(String::as_str)
    }

    /// Every installed family, sorted.
    pub fn names(&self) -> &[String] {
        &self.names
    }
}

static INSTALLED: OnceLock<FontCatalog> = OnceLock::new();

/// The cached catalog, or `None` while the scan has not finished. Never
/// blocks.
pub fn installed() -> Option<&'static FontCatalog> {
    INSTALLED.get()
}

/// The cached catalog, or just the bundled families while the scan has not
/// finished. Never blocks.
pub fn installed_or_bundled() -> &'static FontCatalog {
    static BUNDLED_ONLY: OnceLock<FontCatalog> = OnceLock::new();
    installed()
        .unwrap_or_else(|| BUNDLED_ONLY.get_or_init(|| FontCatalog::from_families::<_, &str>([])))
}

/// The cached catalog, scanning on the calling thread if nothing has yet.
/// For code that is already allowed to block (a panel opening its font list).
pub fn installed_blocking() -> &'static FontCatalog {
    INSTALLED.get_or_init(FontCatalog::scan_system)
}

/// The scan, as a future for `Task::perform`: the blocking directory walk
/// runs on a `spawn_blocking` worker, never in `update`. Resolves at once
/// when the catalog is already cached.
pub async fn scan_in_background() {
    if INSTALLED.get().is_some() {
        return;
    }
    // A failed join means the scan thread panicked; the catalog then stays
    // unset and every family keeps rendering in the bundled default.
    if let Err(error) = tokio::task::spawn_blocking(installed_blocking).await {
        tracing::error!(
            target: "signex::fonts",
            error = %error,
            "scanning the installed fonts failed; text naming a font that is not bundled \
             will draw in Iosevka"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_ignores_case_and_padding_and_always_knows_the_bundled_families() {
        let catalog = FontCatalog::from_families(["Fira Mono"]);
        assert!(catalog.has("fira mono"));
        assert!(catalog.has("  Fira Mono "));
        assert!(!catalog.has("Comic Sans"));
        assert!(catalog.has("iosevka"), "bundled, so installed everywhere");
        assert!(catalog.has("Roboto"));
    }

    #[test]
    fn canonical_returns_the_installed_spelling() {
        let catalog = FontCatalog::from_families(["Fira Mono"]);
        assert_eq!(catalog.canonical("FIRA mono"), Some("Fira Mono"));
        assert_eq!(catalog.canonical("iosevka"), Some("Iosevka"));
        assert_eq!(catalog.canonical("Nope"), None);
    }

    #[test]
    fn names_are_sorted_and_deduplicated() {
        let catalog = FontCatalog::from_families(["b", "a", "B", "a"]);
        let names: Vec<&str> = catalog.names().iter().map(String::as_str).collect();
        assert_eq!(names, ["B", "Iosevka", "Roboto", "a", "b"]);
    }
}
