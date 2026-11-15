//! Fast grid 1 / Fast grid 2 preference IO (#1672).

use super::*;
use crate::canvas::grid::FAST_GRIDS_MM;

/// The `prefs.json` keys of Fast grid 1 and Fast grid 2, in mm.
pub const FAST_GRID_PREF_KEYS: [&str; 2] = ["fast_grid_1_mm", "fast_grid_2_mm"];

/// Read the two fast grids (mm). A missing key, a file without them (saved
/// before #1672) or a value the Preferences field would refuse reads as its
/// default in [`FAST_GRIDS_MM`].
pub fn read_fast_grids_mm_pref_from(json: Option<&serde_json::Value>) -> [f32; 2] {
    let mut grids = FAST_GRIDS_MM;
    for (grid, key) in grids.iter_mut().zip(FAST_GRID_PREF_KEYS) {
        let saved = json
            .and_then(|json| json[key].as_f64())
            .and_then(|v| crate::canvas::grid::fast_grid_mm(v).ok());
        if let Some(mm) = saved {
            *grid = mm;
        }
    }
    grids
}

/// Persist both fast grids without clobbering other keys.
pub fn write_fast_grids_mm_pref(grids: [f32; 2]) {
    update_prefs_json(&prefs_path(), "fast_grids_mm", |prefs| {
        for (mm, key) in grids.into_iter().zip(FAST_GRID_PREF_KEYS) {
            prefs.insert(key.to_string(), serde_json::json!(mm));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_broken_fast_grids_read_as_the_defaults() {
        assert_eq!(read_fast_grids_mm_pref_from(None), FAST_GRIDS_MM);
        let partial = serde_json::json!({ "fast_grid_2_mm": 5.08, "fast_grid_1_mm": -1.0 });
        assert_eq!(
            read_fast_grids_mm_pref_from(Some(&partial)),
            [FAST_GRIDS_MM[0], 5.08]
        );
    }
}
