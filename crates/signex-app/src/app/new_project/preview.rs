//! The live previews of the New Project and New PCB dialogs (mockup rows 21
//! and 22): the board rules drawing's editable values and handles, and the
//! larger view. A value set on the drawing is the same edit as typing it in
//! its field, so the two always agree.

use iced::keyboard::{self, Key, Modifiers, key::Named};
use iced::{Point, Task, Vector};
use signex_i18n::{I18n, MessageId, ids, t};
use signex_types::coord::Unit;
use signex_types::project::{PcbDefaults, PcbField};
use signex_widgets::LengthFormat;
use signex_widgets::dimension_label::{
    DimensionLabel, DimensionLabelMessage, DimensionLabels, DragHandle, LabelAnchor, ValueLimits,
};

use super::{NewProjectMsg, NewProjectState};
use crate::app::state::ModalId;
use crate::app::{DocumentSetupMsg, Message, Signex};

/// Width of the drawing, in view units (a view unit is a pixel unblown).
pub(crate) const VIEW_W: f32 = 264.0;
/// Height of the drawing, in view units.
pub(crate) const VIEW_H: f32 = 198.0;
/// How much larger the drawing is in the larger view.
pub(crate) const EXPANDED_ZOOM: f32 = 2.0;
/// Where the tracks run, and the via's centre.
pub(crate) const TRACK_X: (f32, f32) = (8.0, 156.0);
pub(crate) const VIA_X: f32 = 210.0;
pub(crate) const CENTRE_Y: f32 = 100.0;
/// The two tracks and their gap fill this many view units.
const TRACKS_SPAN: f64 = 96.0;
/// The via fills this many view units.
const VIA_SPAN: f64 = 72.0;
/// View units per millimetre, at most and at least.
const SCALE: (f64, f64) = (60.0, 320.0);
/// Clear space between a drawn edge and its label.
const LABEL_GAP: f32 = 15.0;
/// Where along the tracks the clearance label and its handle sit.
const CLEARANCE_X: f32 = 96.0;
/// Where along the top track its width label and handle sit.
const WIDTH_X: f32 = 40.0;
/// A dragged value snaps to this, in millimetres.
const DRAG_STEP_MM: f64 = 0.005;
/// Decimals a label shows.
const DECIMALS: usize = 3;

/// The rule each label writes, in label order.
pub(crate) const LABEL_FIELDS: [PcbField; 4] = [
    PcbField::TrackWidth,
    PcbField::Clearance,
    PcbField::ViaDrill,
    PcbField::ViaSize,
];

/// The preview shown in the larger view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogPreview {
    BoardRules,
    Sheet,
}

/// The previews' interaction state, held in the dialog's draft.
#[derive(Debug, Clone, Default)]
pub struct SetupPreview {
    pub(crate) labels: DimensionLabels,
    pub(crate) expanded: Option<DialogPreview>,
}

/// An event of a dialog preview.
#[derive(Debug, Clone)]
pub enum SetupPreviewMsg {
    Labels(DimensionLabelMessage),
    /// The larger view opened, or closed (`None`).
    Expanded(Option<DialogPreview>),
}

/// What a key press means to an open preview.
pub(crate) enum SetupPreviewKey {
    Consume(Message),
    /// Enter belongs to the open value's field and must not create.
    Swallow,
}

impl SetupPreview {
    /// Drawing pixels per view unit.
    pub(crate) fn zoom(&self) -> f32 {
        if self.expanded == Some(DialogPreview::BoardRules) {
            EXPANDED_ZOOM
        } else {
            1.0
        }
    }

    /// Tab and Esc on an open value or held handle, then Esc on the larger
    /// view.
    pub(crate) fn key_message(&self, key: &Key, modifiers: Modifiers) -> Option<SetupPreviewMsg> {
        if let Some(message) = self.labels.key_message(key, modifiers) {
            return Some(SetupPreviewMsg::Labels(message));
        }
        (self.expanded.is_some() && matches!(key, Key::Named(Named::Escape)))
            .then_some(SetupPreviewMsg::Expanded(None))
    }

    fn holds_enter(&self, key: &Key) -> bool {
        self.labels.editing_index().is_some() && matches!(key, Key::Named(Named::Enter))
    }

    fn key(
        &self,
        key: &Key,
        modifiers: Modifiers,
        wrap: impl Fn(SetupPreviewMsg) -> Message,
    ) -> Option<SetupPreviewKey> {
        if self.holds_enter(key) {
            return Some(SetupPreviewKey::Swallow);
        }
        self.key_message(key, modifiers)
            .map(|msg| SetupPreviewKey::Consume(wrap(msg)))
    }
}

/// Where the drawing puts the rules, in view units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RuleGeometry {
    /// View units per millimetre.
    pub scale: f32,
    pub track: f32,
    pub gap: f32,
    pub via_r: f32,
    pub drill_r: f32,
}

impl RuleGeometry {
    /// One scale for the whole drawing, so the values read in proportion.
    pub(crate) fn of(pcb: &PcbDefaults) -> Self {
        let span = 2.0 * pcb.track_width_mm + pcb.clearance_mm;
        let scale = (TRACKS_SPAN / span.max(f64::EPSILON))
            .min(VIA_SPAN / pcb.via_diameter_mm.max(f64::EPSILON))
            .clamp(SCALE.0, SCALE.1);
        let units = |mm: f64| (mm * scale) as f32;
        Self {
            scale: scale as f32,
            track: units(pcb.track_width_mm),
            gap: units(pcb.clearance_mm),
            via_r: units(pcb.via_diameter_mm) / 2.0,
            drill_r: units(pcb.via_drill_mm) / 2.0,
        }
    }

    /// Top edge of the upper track.
    pub(crate) fn upper_top(self) -> f32 {
        CENTRE_Y - self.gap / 2.0 - self.track
    }

    /// Top edge of the lower track.
    pub(crate) fn lower_top(self) -> f32 {
        CENTRE_Y + self.gap / 2.0
    }
}

/// One label per rule, in [`LABEL_FIELDS`] order. A drag moves the edge
/// under the pointer, which is half of a width drawn about its centre.
pub(crate) fn rule_labels<'a>(
    pcb: &PcbDefaults,
    zoom: f32,
    i18n: &'a I18n,
) -> Vec<DimensionLabel<'a>> {
    let g = RuleGeometry::of(pcb);
    let format = LengthFormat::new(i18n, Unit::Mm);
    let px = |x: f32, y: f32| Point::new(x * zoom, y * zoom);
    let per_px = 2.0 / f64::from(g.scale * zoom);
    let limits = ValueLimits {
        min: Some(DRAG_STEP_MM),
        max: None,
        step: DRAG_STEP_MM,
    };
    let label = |value: f64, caption: MessageId, at: Point, handle_at: Point, axis: Vector| {
        DimensionLabel::new(value, format, at)
            .caption(t!(i18n, caption))
            .anchor(LabelAnchor::Middle)
            .decimals(DECIMALS)
            .limits(limits)
            .handle(DragHandle {
                at: handle_at,
                axis,
                value_per_px: per_px,
            })
    };
    let up = Vector::new(0.0, -1.0);
    let down = Vector::new(0.0, 1.0);
    let right = Vector::new(1.0, 0.0);
    vec![
        label(
            pcb.track_width_mm,
            ids::DIALOG_NEW_PROJECT_RULE_CAP_WIDTH,
            px(WIDTH_X, g.upper_top() - LABEL_GAP),
            px(TRACK_X.1 - WIDTH_X, g.upper_top()),
            up,
        ),
        label(
            pcb.clearance_mm,
            ids::DIALOG_NEW_PROJECT_RULE_CAP_CLEARANCE,
            px(CLEARANCE_X, CENTRE_Y),
            px(CLEARANCE_X + WIDTH_X, g.lower_top()),
            down,
        ),
        label(
            pcb.via_drill_mm,
            ids::DIALOG_NEW_PROJECT_RULE_CAP_DRILL,
            px(VIA_X, CENTRE_Y - g.via_r - LABEL_GAP),
            px(VIA_X, CENTRE_Y - g.drill_r),
            up,
        ),
        label(
            pcb.via_diameter_mm,
            ids::DIALOG_NEW_PROJECT_RULE_CAP_VIA,
            px(VIA_X, CENTRE_Y + g.via_r + LABEL_GAP),
            px(VIA_X + g.via_r, CENTRE_Y),
            right,
        ),
    ]
}

impl NewProjectState {
    /// A preview event: a value set on the drawing goes through the rule
    /// field's own edit, so a refused one shows at the field too.
    pub(crate) fn act_on_preview(&mut self, msg: SetupPreviewMsg) -> Task<SetupPreviewMsg> {
        match msg {
            SetupPreviewMsg::Expanded(view) => {
                self.preview = SetupPreview {
                    expanded: view,
                    ..SetupPreview::default()
                };
                Task::none()
            }
            SetupPreviewMsg::Labels(message) => {
                // Lengths read `.` in every locale, so the fallback bundle
                // parses exactly what the view shows.
                let en = I18n::en_us();
                let shown = rule_labels(&self.setup.pcb, self.preview.zoom(), &en);
                let outcome = self.preview.labels.update(message, &shown);
                let task = if outcome.focus_input {
                    self.preview.labels.focus_input()
                } else {
                    Task::none()
                };
                if let Some(done) = outcome.committed
                    && let Some(field) = LABEL_FIELDS.get(done.index)
                {
                    self.edit_rule(*field, format!("{}", done.value));
                }
                task
            }
        }
    }
}

impl Signex {
    /// A key press for the preview of the New Project or New PCB dialog in
    /// front. `own_window`: the key came from the New Project window;
    /// `paints_overlays`: from a window that shows the in-window dialogs.
    pub(crate) fn setup_preview_key(
        &self,
        own_window: bool,
        paints_overlays: bool,
        event: &keyboard::Event,
    ) -> Option<SetupPreviewKey> {
        let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
            return None;
        };
        if let Some(state) = self.ui_state.new_project.as_ref() {
            let in_fallback =
                paints_overlays && self.detached_window_of(ModalId::NewProject).is_none();
            if own_window || in_fallback {
                return state.preview.key(key, *modifiers, |msg| {
                    Message::from(NewProjectMsg::Preview(msg))
                });
            }
        }
        let state = self.ui_state.document_setup.as_ref()?;
        if !paints_overlays {
            return None;
        }
        state.draft.preview.key(key, *modifiers, |msg| {
            Message::from(DocumentSetupMsg::Setup(NewProjectMsg::Preview(msg)))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signex_types::project::ProjectSetup;
    use signex_widgets::dimension_label::DimensionLabelMessage as L;

    fn state() -> NewProjectState {
        NewProjectState::of_setup(ProjectSetup::default())
    }

    fn send(state: &mut NewProjectState, message: L) {
        let _ = state.act_on_preview(SetupPreviewMsg::Labels(message));
    }

    /// Mockup row 21: a value typed on the drawing is the field's edit.
    #[test]
    fn a_value_typed_on_the_drawing_lands_in_its_field() {
        let mut state = state();
        send(&mut state, L::LabelPressed(1));
        send(&mut state, L::DraftEdited("0.3".into()));
        send(&mut state, L::DraftSubmitted);
        assert_eq!(state.setup.pcb.clearance_mm, 0.3);
        assert_eq!(state.drafts.text(PcbField::Clearance), "0.3");
    }

    /// Mockup row 22: a drill at the via's size shows as the field's error
    /// and turns Create off.
    #[test]
    fn a_drill_set_on_the_drawing_past_the_via_is_the_fields_error() {
        let mut state = state();
        let via = state.setup.pcb.via_diameter_mm;
        send(&mut state, L::LabelPressed(2));
        send(&mut state, L::DraftEdited(format!("{}", via + 0.05)));
        send(&mut state, L::DraftSubmitted);
        assert!(state.drafts.problem(PcbField::ViaDrill).is_some());
        assert!(!state.rules_valid());
    }

    #[test]
    fn esc_cancels_a_value_and_keeps_the_rule() {
        let mut state = state();
        let before = state.setup.pcb.track_width_mm;
        send(&mut state, L::LabelPressed(0));
        send(&mut state, L::DraftEdited("0.9".into()));
        let esc = state
            .preview
            .key_message(&Key::Named(Named::Escape), Modifiers::empty())
            .expect("Esc belongs to the open value");
        let _ = state.act_on_preview(esc);
        assert_eq!(state.setup.pcb.track_width_mm, before);
        assert!(state.preview.labels.editing_index().is_none());
    }

    /// Enter in an open value commits it and never creates the project.
    #[test]
    fn enter_in_an_open_value_is_not_the_dialogs_enter() {
        let mut state = state();
        let enter = Key::Named(Named::Enter);
        assert!(!state.preview.holds_enter(&enter));
        send(&mut state, L::LabelPressed(0));
        assert!(state.preview.holds_enter(&enter));
    }

    #[test]
    fn esc_closes_the_larger_view() {
        let mut state = state();
        let _ = state.act_on_preview(SetupPreviewMsg::Expanded(Some(DialogPreview::BoardRules)));
        assert_eq!(state.preview.zoom(), EXPANDED_ZOOM);
        let esc = state
            .preview
            .key_message(&Key::Named(Named::Escape), Modifiers::empty())
            .expect("Esc closes it");
        let _ = state.act_on_preview(esc);
        assert_eq!(state.preview.expanded, None);
    }

    #[test]
    fn the_drawing_keeps_every_rule_inside_its_view() {
        for (track, clr, via, drill) in [(0.2, 0.15, 0.5, 0.25), (0.075, 0.075, 0.25, 0.1)] {
            let pcb = PcbDefaults {
                track_width_mm: track,
                clearance_mm: clr,
                via_diameter_mm: via,
                via_drill_mm: drill,
                ..ProjectSetup::default().pcb
            };
            let g = RuleGeometry::of(&pcb);
            assert!(g.upper_top() > 0.0, "{track}");
            assert!(g.lower_top() + g.track < VIEW_H, "{track}");
            assert!(VIA_X + g.via_r < VIEW_W, "{via}");
            assert!(g.drill_r < g.via_r, "{drill}");
        }
    }
}
