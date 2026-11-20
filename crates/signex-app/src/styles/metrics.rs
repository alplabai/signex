//! Fixed heights of the Graphite chrome bands and rows, and the thin
//! scrollbar's thumb and gutter. The values live in
//! `signex_widgets::theme_ext` so widgets read the same heights; they are
//! authored at the default 12 px base and follow it through `chrome_px`.

pub use signex_widgets::theme_ext::ChromeMetrics;
