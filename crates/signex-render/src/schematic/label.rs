//! Schematic labels — net, global, hierarchical (power labels are
//! rendered via the parent symbol in [`super::symbol`]).
//!
//! Spec: `docs/RENDERING_RULES.md::sch-labels`. Net labels paint plain
//! text bottom-aligned at the wire endpoint; global / hier labels
//! paint a directional flag polygon with the text inside, with point
//! direction derived from `label.rotation`. Flag dimensions are
//! derived from rendered text height times Signex-tuned constants.
//!
//! Depends on the text mechanics exposed by [`super::text`]. Filled in
//! by Wave 3 sub-agent 10.

use iced::widget::canvas::Frame;
use signex_types::schematic::Label;

use super::RenderContext;

/// Render a single label into the content layer's frame.
pub fn draw_label(_frame: &mut Frame, _label: &Label, _ctx: &RenderContext<'_>) {
    todo!(
        "Wave 3 sub-agent 10 fills this in against signex-types::Label and RENDERING_RULES.md::sch-labels"
    )
}

#[cfg(test)]
mod tests {
    // Sub-agent populates: render_smoke for each LabelType + a rotation
    // edge case.
}
