//! Hit-test internals — the spatial-hash builder and per-primitive
//! distance / containment helpers used by the public
//! [`super::hit_test_point`] / [`super::hit_test_box`] entries.
//!
//! Filled in by orchestrator in Wave 4 once every primitive's renderer
//! is in place (so this file can borrow each primitive's bbox helper
//! and stay consistent with the visible shape).

// Wave 4 fills this in.

#[cfg(test)]
mod tests {
    // Wave 4 populates: bucket build / lookup, primitive-level hit
    // helpers, and selection-mode tests.
}
