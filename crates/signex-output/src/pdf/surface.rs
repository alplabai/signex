//! `PdfSurface` — the second render target for the schematic scene graph.
//!
//! Will implement the same draw-primitive interface as `signex-render`'s Iced
//! Canvas target so the existing scene walker can emit either path. That
//! walker isn't wired yet — this module is a placeholder for the surface
//! type that arrives with the pdf-writer integration PR.
