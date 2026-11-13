//! The turn of a board view on screen (#2316). View state like pan and zoom:
//! it lives in the canvas camera (`pcb_canvas::BoardCamera`), so nothing here
//! touches the board, its undo history or its dirty mark.

use iced::Task;
use iced::widget::operation;
use signex_i18n::{ids, t};

use crate::app::contracts::VIEW_ANGLE_INPUT_ID;
use crate::app::{Message, Signex, ViewAngleEntry, ViewRotationMsg};
use crate::status_bar::ViewAngleReadout;

impl Signex {
    /// Apply `event` to the board canvas in the main slot: the main window's,
    /// or an undocked window's while `run_in_window` has it there. `window` is
    /// the window the event came from (`None`: a command, the main window).
    pub(crate) fn apply_view_rotation(
        &mut self,
        window: Option<iced::window::Id>,
        event: ViewRotationMsg,
    ) -> Task<Message> {
        let window = window.or(self.ui_state.main_window_id);
        match event {
            ViewRotationMsg::TurnedBy(degrees) => {
                let now = self.interaction_state.pcb_canvas.view_rotation_deg();
                self.turn_pcb_view(now + degrees);
            }
            ViewRotationMsg::AngleSet(degrees) => self.turn_pcb_view(degrees),
            ViewRotationMsg::AngleEntryToggled => {
                let entry = &mut self.ui_state.pcb_view_angle_entry;
                let open_here = entry.as_ref().map(|e| Some(e.window)) == Some(window);
                *entry = match window {
                    Some(window) if !open_here => Some(ViewAngleEntry {
                        window,
                        text: view_angle_text(
                            self.interaction_state.pcb_canvas.view_rotation_deg(),
                        ),
                    }),
                    _ => None,
                };
                if entry.is_some() {
                    let input = VIEW_ANGLE_INPUT_ID.clone();
                    return Task::batch([
                        operation::focus(input.clone()),
                        operation::text_input::select_all(input),
                    ]);
                }
            }
            ViewRotationMsg::AngleEntryEdited(text) => {
                if let Some(entry) = self.angle_entry_of(window) {
                    entry.text = text;
                }
            }
            ViewRotationMsg::AngleEntrySubmitted => {
                let typed = self
                    .angle_entry_of(window)
                    .map(|e| parse_view_angle(&e.text));
                // Text that is not an angle keeps the field open to correct.
                if let Some(Some(degrees)) = typed {
                    self.ui_state.pcb_view_angle_entry = None;
                    self.turn_pcb_view(degrees);
                }
            }
        }
        Task::none()
    }

    /// Close the status bar's angle field when it is open in `window`
    /// (`None`: the main window). Whether it was: Esc then stops there.
    pub(crate) fn close_view_angle_entry_in(&mut self, window: Option<iced::window::Id>) -> bool {
        let window = window.or(self.ui_state.main_window_id);
        let open_here = self
            .ui_state
            .pcb_view_angle_entry
            .as_ref()
            .is_some_and(|e| Some(e.window) == window);
        if open_here {
            self.ui_state.pcb_view_angle_entry = None;
        }
        open_here
    }

    fn angle_entry_of(&mut self, window: Option<iced::window::Id>) -> Option<&mut ViewAngleEntry> {
        self.ui_state
            .pcb_view_angle_entry
            .as_mut()
            .filter(|e| Some(e.window) == window)
    }

    /// Turn the board view in the main slot to `degrees`; nothing without a
    /// board in view.
    fn turn_pcb_view(&mut self, degrees: f32) {
        if self.has_active_pcb() && self.interaction_state.pcb_canvas.set_view_rotation(degrees) {
            // An open field would show the angle it was opened at.
            self.ui_state.pcb_view_angle_entry = None;
        }
    }

    /// The view angle of the board canvas `window` shows, if it shows one.
    pub(crate) fn pcb_view_angle_of(&self, window: iced::window::Id) -> Option<f32> {
        if self.ui_state.main_window_id == Some(window) {
            return self
                .has_active_pcb()
                .then(|| self.interaction_state.pcb_canvas.view_rotation_deg());
        }
        self.undocked_pcb_path(window)?;
        let canvas = self.interaction_state.pcb_canvases.get(&window)?;
        Some(canvas.view_rotation_deg())
    }

    /// Whether the board view a command from `window` acts on is turned:
    /// an undocked PCB window's own, else the main window's.
    pub(crate) fn pcb_view_turned_in(&self, window: Option<iced::window::Id>) -> bool {
        let window = window
            .filter(|id| self.undocked_pcb_path(*id).is_some())
            .or(self.ui_state.main_window_id);
        let degrees = match window {
            Some(id) => self.pcb_view_angle_of(id),
            None => self
                .has_active_pcb()
                .then(|| self.interaction_state.pcb_canvas.view_rotation_deg()),
        };
        degrees.is_some_and(|d| d != 0.0)
    }

    /// The status bar's angle readout of `window`: shown while it shows a
    /// board, with the value field when it is open there.
    pub(crate) fn status_view_angle(&self, window: iced::window::Id) -> Option<ViewAngleReadout> {
        let degrees = self.pcb_view_angle_of(window)?;
        let i18n = &self.ui_state.i18n;
        let entry = self
            .ui_state
            .pcb_view_angle_entry
            .as_ref()
            .filter(|e| e.window == window)
            .map(|e| e.text.clone());
        Some(ViewAngleReadout {
            caption: t!(i18n, ids::PCB_STATUS_VIEW_ANGLE),
            value: format!("{}°", view_angle_text(degrees)),
            tooltip: t!(i18n, ids::PCB_STATUS_VIEW_ANGLE_TIP),
            placeholder: t!(i18n, ids::PCB_STATUS_VIEW_ANGLE_PLACEHOLDER),
            entry,
        })
    }
}

/// An angle in `[0, 360)` as the readout and its field show it: whole
/// degrees without a fraction, else one decimal; a turn that rounds to 360
/// reads 0.
pub(crate) fn view_angle_text(degrees: f32) -> String {
    let tenths = (degrees * 10.0).round() / 10.0;
    let tenths = if tenths >= 360.0 { 0.0 } else { tenths };
    if tenths.fract() == 0.0 {
        format!("{tenths:.0}")
    } else {
        format!("{tenths:.1}")
    }
}

/// What the angle field holds as degrees: a finite number, a trailing `°`
/// allowed. `None` for anything else.
pub(crate) fn parse_view_angle(text: &str) -> Option<f32> {
    let number = text.trim().trim_end_matches('°').trim();
    number.parse::<f32>().ok().filter(|d| d.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_angle_text_drops_a_whole_fraction() {
        assert_eq!(view_angle_text(90.0), "90");
        assert_eq!(view_angle_text(37.25), "37.3");
        assert_eq!(view_angle_text(359.96), "0");
    }

    #[test]
    fn the_field_reads_a_number_with_or_without_the_degree_sign() {
        assert_eq!(parse_view_angle(" 37 "), Some(37.0));
        assert_eq!(parse_view_angle("-45°"), Some(-45.0));
        assert_eq!(parse_view_angle("12.5 °"), Some(12.5));
        assert_eq!(parse_view_angle("ninety"), None);
        assert_eq!(parse_view_angle("inf"), None);
        assert_eq!(parse_view_angle(""), None);
    }
}
