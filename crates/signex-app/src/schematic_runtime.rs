//! Schematic runtime bridge for Milestone F cutover.
//!
//! Centralizes legacy schematic runtime imports so app modules stop
//! depending on `signex_render::schematic` directly.

pub use signex_render::schematic::*;

pub mod hit_test {
    pub use signex_render::schematic::hit_test::*;
}

pub mod label {
    pub use signex_render::schematic::label::*;
}

pub mod selection {
    pub use signex_render::schematic::selection::*;
}

pub mod text {
    pub use signex_render::schematic::text::*;
}
