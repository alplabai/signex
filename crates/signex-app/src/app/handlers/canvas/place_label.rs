//! The Label tool's commit as a plain-value method (#1930), so a command can
//! place a label without a canvas click.

use signex_types::schematic::{HAlign, Label, LabelType, Point, VAlign};

use super::super::super::*;

/// The pin direction a Port (global label) is armed with, and a CLI run uses.
pub(crate) const GLOBAL_PORT_SHAPE: &str = "bidirectional";

/// A label to place: its kind and text, and its look (a click's from the
/// pre-placement form, an operand run's from the project defaults).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LabelSpec {
    pub label_type: LabelType,
    pub shape: String,
    pub text: String,
    pub rotation: f64,
    pub font_size_mm: f64,
    pub justify_h: HAlign,
    pub justify_v: VAlign,
    pub font_family: Option<String>,
}

impl Signex {
    /// What a Label-tool click places now: the pending port kind (a net
    /// label when none is pending) with the pre-placement form's text and
    /// look, and the project's default text size and font (#2026).
    pub(crate) fn label_spec_from_form(&self) -> LabelSpec {
        let (label_type, shape, default_text) = match self.interaction_state.pending_port.clone() {
            Some((kind, shape)) => {
                let default = match kind {
                    LabelType::Global => "PORT",
                    LabelType::Hierarchical => "SHEET",
                    _ => "NET",
                };
                (kind, shape, default)
            }
            None => (LabelType::Net, String::new(), "NET"),
        };
        let pre = self.document_state.panel_ctx.pre_placement.as_ref();
        let text = pre
            .map(|pp| pp.label_text.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| default_text.to_string());
        let spec = self.label_spec_from_defaults(label_type, shape, text);
        match pre {
            Some(pp) => LabelSpec {
                rotation: pp.rotation,
                font_size_mm: pp.font_size_pt as f64 * signex_types::schematic::SCHEMATIC_PT_TO_MM,
                justify_h: pp.justify_h,
                justify_v: pp.justify_v,
                ..spec
            },
            None => spec,
        }
    }

    /// A label of `label_type`, `shape` and `text` in the look the project
    /// defaults give the sheet the edit targets: unrotated, the default text
    /// size and font, left and bottom justified. What an agent or CLI run
    /// places where its operands say nothing; it never reads the form (#2326).
    pub(crate) fn label_spec_from_defaults(
        &self,
        label_type: LabelType,
        shape: String,
        text: String,
    ) -> LabelSpec {
        LabelSpec {
            label_type,
            shape,
            text,
            rotation: 0.0,
            font_size_mm: self
                .document_state
                .active_schematic_defaults()
                .placed_text_mm(),
            justify_h: HAlign::Left,
            justify_v: VAlign::Bottom,
            font_family: self.document_state.placed_font_family(),
        }
    }

    /// Place the label `spec` at `at`, already snapped. Whether it was placed.
    pub(crate) fn place_label_at(&mut self, at: Point, spec: LabelSpec) -> bool {
        self.apply_engine_command(label_command(at, spec), false, false)
    }
}

/// The command that places the label `spec` at `at`; the point check applies
/// it to a copy first.
pub(crate) fn label_command(at: Point, spec: LabelSpec) -> signex_engine::Command {
    let label = Label {
        rotation: spec.rotation,
        shape: spec.shape,
        font_size: spec.font_size_mm,
        justify: spec.justify_h,
        justify_v: spec.justify_v,
        font_family: spec.font_family,
        ..Label::new(spec.text, at, spec.label_type)
    };
    signex_engine::Command::PlaceLabel { label }
}
