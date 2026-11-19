//! View state of the Appearance and Editing pages (#144, #147): the help
//! texts opened with More…, the preview that fills the page, and the pitch
//! labels on the snap preview. None of it is a preference, so none of it
//! marks the dialog dirty; the shell resets it when Preferences opens.

use iced::Task;
use iced::widget::Id;
use signex_i18n::I18n;
use signex_types::coord::Unit;
use signex_types::text_increment::{STEP_MAX, STEP_MIN};
use signex_widgets::LengthFormat;
use signex_widgets::dimension_label::{
    DimensionLabel, DimensionLabelMessage, DimensionLabels, DragHandle, ValueLimits,
};

use crate::canvas::grid::{
    FAST_GRIDS_MM, fast_grid_field_text, parse_fast_grid_field, pitch_mm_exact,
};

pub(in crate::preferences) mod controls;
pub(in crate::preferences) mod key_catcher;

/// Drawing pixels per millimetre on the snap preview: twice the 12 px/mm
/// zoom, so a 0.635 mm grid still reads.
pub(in crate::preferences) const PREVIEW_PX_PER_MM: f32 = 24.0;
/// Decimals a pitch label shows.
const PITCH_DECIMALS: usize = 6;
/// A dragged pitch keeps to this step and never goes below it.
const DRAG_STEP_MM: f64 = 0.01;

/// The text field a pitch label opens into; only one is open at a time.
const SNAP_FIELD_ID: Id = Id::new("signex.preferences.snap_pitch");

/// A help text that shows a summary until More… is pressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpTopic {
    KeepWiresConnected,
    SelectByDragDirection,
    PasteDesignators,
    Autosave,
    AgentConnections,
}

/// A preview that can fill the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewKind {
    /// The grid styles on the Appearance page.
    Grid,
    /// The fast grids on the Editing page.
    Snap,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PagesViewMsg {
    /// More… or Less was pressed under a help text.
    HelpExpandToggled(HelpTopic),
    /// A preview's expand or shrink button was pressed, or Esc while it
    /// fills the page.
    PreviewExpandToggled(PreviewKind),
    /// The Increment step − (`-1`) or + (`1`) button was pressed.
    IncrementStepNudged(i32),
    /// A pitch label on the snap preview: index 0 is Fast grid 1.
    SnapLabel(DimensionLabelMessage),
}

#[derive(Debug, Clone, Default)]
pub struct PagesViewState {
    expanded_help: Vec<HelpTopic>,
    expanded_preview: Option<PreviewKind>,
    snap_labels: DimensionLabels,
}

/// The view state compares by what the user sees: `DimensionLabels` holds a
/// widget id that differs between two fresh states.
impl PartialEq for PagesViewState {
    fn eq(&self, other: &Self) -> bool {
        let (a, b) = (&self.snap_labels, &other.snap_labels);
        self.expanded_help == other.expanded_help
            && self.expanded_preview == other.expanded_preview
            && a.editing_index() == b.editing_index()
            && a.draft() == b.draft()
            && a.dragging_index() == b.dragging_index()
    }
}

/// What a pitch label event asks of the handler.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct SnapLabelStep {
    /// A pitch set on the drawing: the fast-grid index and its field text.
    pub(crate) committed: Option<(usize, String)>,
    /// A label opened its field, which wants the keyboard.
    pub(crate) focus_field: bool,
}

impl PagesViewState {
    pub fn is_help_expanded(&self, topic: HelpTopic) -> bool {
        self.expanded_help.contains(&topic)
    }

    pub fn expanded_preview(&self) -> Option<PreviewKind> {
        self.expanded_preview
    }

    /// The fast grid whose pitch label has its field open.
    pub fn snap_label_editing(&self) -> Option<usize> {
        self.snap_labels.editing_index()
    }

    pub(in crate::preferences) fn snap_labels(&self) -> &DimensionLabels {
        &self.snap_labels
    }

    pub(crate) fn toggle_help(&mut self, topic: HelpTopic) {
        if let Some(at) = self.expanded_help.iter().position(|&t| t == topic) {
            self.expanded_help.remove(at);
        } else {
            self.expanded_help.push(topic);
        }
    }

    pub(crate) fn toggle_preview(&mut self, kind: PreviewKind) {
        self.expanded_preview = if self.expanded_preview == Some(kind) {
            None
        } else {
            Some(kind)
        };
    }

    /// The Increment step after a − or + press: one step on, skipping 0
    /// (which would stamp the same text forever), within the step range.
    pub fn nudged_increment_step(current: i32, delta: i32) -> i32 {
        let mut next = current.saturating_add(delta);
        if next == 0 {
            next = next.saturating_add(delta.signum());
        }
        next.clamp(STEP_MIN, STEP_MAX)
    }

    /// Run a pitch label event against the labels of `drafts`. A value set
    /// on the drawing comes back as field text, so the handler writes it
    /// through the field's own draft message.
    pub(crate) fn snap_label_event(
        &mut self,
        message: DimensionLabelMessage,
        drafts: &[String; 2],
        i18n: &I18n,
    ) -> SnapLabelStep {
        let labels = [
            pitch_label(0, &drafts[0], i18n),
            pitch_label(1, &drafts[1], i18n),
        ];
        let outcome = self.snap_labels.update(message, &labels);
        SnapLabelStep {
            committed: outcome
                .committed
                .map(|c| (c.index, fast_grid_field_text(c.value as f32))),
            focus_field: outcome.focus_input,
        }
    }

    /// Focus the open pitch field and select its text.
    pub(crate) fn focus_snap_field<T: Send + 'static>() -> Task<T> {
        Task::batch([
            iced::widget::operation::focus(SNAP_FIELD_ID),
            iced::widget::operation::text_input::select_all(SNAP_FIELD_ID),
        ])
    }
}

/// Id of the open pitch field, for the view.
pub(in crate::preferences) fn snap_field_id() -> Id {
    SNAP_FIELD_ID
}

/// What the pitch label of fast grid `index` stands for, at the drawing's
/// origin; the snap preview moves it and its handle onto the drawing. A
/// field Save would refuse keeps a label to type into, but no handle; its
/// value is the typed number, or the default grid when there is none.
pub(in crate::preferences) fn pitch_label<'a>(
    index: usize,
    text: &str,
    i18n: &'a I18n,
) -> DimensionLabel<'a> {
    let format = LengthFormat::new(i18n, Unit::Mm);
    let limits = ValueLimits {
        min: Some(DRAG_STEP_MM),
        max: None,
        step: DRAG_STEP_MM,
    };
    let fallback = || {
        let typed = text.trim().parse::<f64>().ok().filter(|v| v.is_finite());
        typed.unwrap_or_else(|| pitch_mm_exact(FAST_GRIDS_MM[index.min(1)]))
    };
    let label = |value: f64| {
        DimensionLabel::new(value, format, iced::Point::ORIGIN)
            .decimals(PITCH_DECIMALS)
            .limits(limits)
    };
    match parse_fast_grid_field(text) {
        Ok(mm) => label(pitch_mm_exact(mm)).handle(DragHandle {
            at: iced::Point::ORIGIN,
            axis: iced::Vector::new(1.0, 0.0),
            value_per_px: 1.0 / f64::from(PREVIEW_PX_PER_MM),
        }),
        Err(_) => label(fallback()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn more_opens_a_help_text_and_less_closes_it_again() {
        let mut state = PagesViewState::default();
        state.toggle_help(HelpTopic::PasteDesignators);
        assert!(state.is_help_expanded(HelpTopic::PasteDesignators));
        assert!(!state.is_help_expanded(HelpTopic::Autosave));
        state.toggle_help(HelpTopic::PasteDesignators);
        assert!(!state.is_help_expanded(HelpTopic::PasteDesignators));
        assert_eq!(state, PagesViewState::default());
    }

    #[test]
    fn one_preview_fills_the_page_at_a_time() {
        let mut state = PagesViewState::default();
        state.toggle_preview(PreviewKind::Grid);
        assert_eq!(state.expanded_preview(), Some(PreviewKind::Grid));
        state.toggle_preview(PreviewKind::Snap);
        assert_eq!(state.expanded_preview(), Some(PreviewKind::Snap));
        state.toggle_preview(PreviewKind::Snap);
        assert_eq!(state.expanded_preview(), None);
    }

    #[test]
    fn the_increment_stepper_skips_zero_and_keeps_to_the_range() {
        let nudge = PagesViewState::nudged_increment_step;
        assert_eq!(nudge(1, -1), -1);
        assert_eq!(nudge(-1, 1), 1);
        assert_eq!(nudge(5, 1), 6);
        assert_eq!(nudge(STEP_MAX, 1), STEP_MAX);
        assert_eq!(nudge(STEP_MIN, -1), STEP_MIN);
    }

    #[test]
    fn a_valid_pitch_label_has_a_handle_and_a_refused_one_does_not() {
        let en = I18n::en_us();
        let good = pitch_label(0, "1.27", &en);
        assert_eq!(good.value, 1.27);
        assert!(good.handle.is_some());
        let tiny = pitch_label(1, "0.0000001", &en);
        assert!(tiny.handle.is_none());
        assert_eq!(tiny.value, 0.0000001);
        let words = pitch_label(1, "abc", &en);
        assert_eq!(words.value, 2.54, "the default grid stands in");
    }

    #[test]
    fn typing_a_pitch_on_the_drawing_commits_its_field_text() {
        let en = I18n::en_us();
        let drafts = ["1.27".to_string(), "2.54".to_string()];
        let mut state = PagesViewState::default();
        let opened = state.snap_label_event(DimensionLabelMessage::LabelPressed(1), &drafts, &en);
        assert!(opened.focus_field);
        assert_eq!(state.snap_label_editing(), Some(1));
        let _ = state.snap_label_event(
            DimensionLabelMessage::DraftEdited("0.635".into()),
            &drafts,
            &en,
        );
        let step = state.snap_label_event(DimensionLabelMessage::DraftSubmitted, &drafts, &en);
        assert_eq!(step.committed, Some((1, "0.635".to_string())));
        assert_eq!(state.snap_label_editing(), None);
    }
}
