//! Signex component library subsystem (v0.9).
//!
//! See `docs/internal/docs/LIBRARY_PLAN.md` for design.

pub mod adapter;
pub mod component;
pub mod diff;
pub mod distributor;
pub mod embed;
pub mod hash;
pub mod identity;
pub mod lifecycle;
pub mod manifest;
pub mod search;
pub mod snxpart;

// Re-exports added as modules are filled in (see Tasks 3–17).

#[cfg(test)]
mod smoke {
    #[test]
    fn crate_compiles() {
        // Smoke test: ensures the crate builds with all module declarations.
    }
}
