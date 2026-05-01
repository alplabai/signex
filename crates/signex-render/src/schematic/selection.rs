//! Selection-overlay rendering — outlines around selected items,
//! ghost previews during drag, hover highlights, ERC marker pins.
//!
//! Selection lives in the *overlay* cache layer
//! ([`super::RenderLayers::overlay`]) so changing selection or hovering
//! never invalidates the (more expensive) content layer.
//!
//! Filled in by orchestrator in Wave 5.

// Wave 5 fills this in.

#[cfg(test)]
mod tests {
    // Wave 5 populates: smoke for each SelectedKind + a ghost-preview
    // smoke.
}
