//! The shaper the geometry measures text with (#510).
//!
//! One `TextMeasurer` for the process, built on the startup scan's worker
//! thread beside the installed-font catalog so nothing on the update thread
//! ever builds a font system. [`shaped_advance_em`] returns `None` until it
//! exists. Results are cached per (family, text): hit-testing asks for the
//! same label many times a second.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use signex_gfx::pipeline::text_measure::TextMeasurer;

static MEASURER: OnceLock<Mutex<TextMeasurer>> = OnceLock::new();
static CAP_HEIGHTS: Mutex<Option<HashMap<String, Option<f32>>>> = Mutex::new(None);
static CACHE: Mutex<Option<HashMap<(String, String), f32>>> = Mutex::new(None);

/// Most entries kept before the cache starts over.
const CACHE_CAP: usize = 8192;

/// Build the measurer with the bundled faces and the machine's fonts, the
/// set the renderer shapes with. Slow; run it off the update thread.
pub fn install_blocking() {
    MEASURER.get_or_init(|| Mutex::new(TextMeasurer::new(&super::bundled::BUNDLED_FONTS, true)));
    signex_types::font::set_cap_height_resolver(cap_height_per_em);
}

/// Cap height over em of an installed family, read once per family from the
/// measurer's font database (#1797). `None` before the measurer exists (not
/// cached, so the next call retries) and for a family with no usable face.
pub fn cap_height_per_em(family: &str) -> Option<f32> {
    let measurer = MEASURER.get()?;
    let mut cache = CAP_HEIGHTS.lock().unwrap_or_else(|e| e.into_inner());
    let map = cache.get_or_insert_with(HashMap::new);
    if let Some(&ratio) = map.get(family) {
        return ratio;
    }
    let ratio = measurer
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .cap_height_per_em(family);
    map.insert(family.to_string(), ratio);
    ratio
}

/// Whether the measurer exists yet.
pub fn installed() -> bool {
    MEASURER.get().is_some()
}

/// Width of `text` in em, shaped in `family`, or `None` before the measurer
/// is installed.
pub fn shaped_advance_em(text: &str, family: &str) -> Option<f32> {
    let measurer = MEASURER.get()?;
    let key = (family.to_string(), text.to_string());
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let map = cache.get_or_insert_with(HashMap::new);
    if let Some(&em) = map.get(&key) {
        return Some(em);
    }
    let em = measurer
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .advance_em(text, family);
    if map.len() >= CACHE_CAP {
        map.clear();
    }
    map.insert(key, em);
    Some(em)
}

/// The bundled faces only, so tests do not depend on the machine's fonts.
#[cfg(test)]
pub fn install_hermetic() {
    MEASURER.get_or_init(|| Mutex::new(TextMeasurer::new(&super::bundled::BUNDLED_FONTS, false)));
    signex_types::font::set_cap_height_resolver(cap_height_per_em);
}
