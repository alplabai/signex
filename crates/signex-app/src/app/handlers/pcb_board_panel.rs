//! The board Properties panel with nothing selected (#2353). The selection
//! filter and the snap options are editor settings for the session
//! (`UiState`); the snap grid step is the board's own and changes through
//! one `BoardCommand::SetGridSize` undo step, the guides, extra grids and
//! sheet through `pcb_board_options` (#2374).

use iced::Task;
use pcb_geom::{ItemRef, Side};
use signex_engine::pcb::BoardCommand;
use signex_i18n::{ids, t};
use signex_types::measure::parse_length_field_mm;

use crate::app::state::pcb_edit::CheckedStep;
use crate::app::{Message, OverlayMsg, Signex};
use crate::panels::{BoardTab, ComponentSide, PanelKind, PcbBoardMsg, pcb_board_properties};

impl Signex {
    pub(crate) fn handle_pcb_board_panel(&mut self, msg: &PcbBoardMsg) -> Task<Message> {
        let mut task = Task::none();
        let view = &mut self.ui_state.pcb_board_panel;
        match msg {
            // Boards hold no parameters yet: that tab stays disabled.
            PcbBoardMsg::TabPicked(BoardTab::Parameters) => {}
            PcbBoardMsg::TabPicked(tab) => view.tab = *tab,
            PcbBoardMsg::SearchChanged(text) => view.search = text.clone(),
            // The header button opens the Active Bar's filter dropdown (#2383).
            PcbBoardMsg::FilterMenuToggled => {
                let open = &mut self.interaction_state.pcb_bar_menu;
                *open = match *open {
                    Some(crate::app::PcbBarMenu::Filter) => None,
                    _ => Some(crate::app::PcbBarMenu::Filter),
                };
            }
            PcbBoardMsg::GridStepTyped(text) => view.grid_draft = Some(text.clone()),
            PcbBoardMsg::GridStepSubmitted => self.apply_pcb_grid_step(),
            PcbBoardMsg::FilterKindToggled(kind) => task = self.toggle_pcb_filter_kind(*kind),
            PcbBoardMsg::FilterAllToggled => {
                let all = self.ui_state.pcb_pick_filter.is_all_on();
                self.set_all_pcb_filter_kinds(!all);
            }
            PcbBoardMsg::FilterAllSet(on) => self.set_all_pcb_filter_kinds(*on),
            PcbBoardMsg::FilterPresetAdded => task = self.add_pcb_filter_preset(),
            PcbBoardMsg::FilterPresetApplied(idx) => self.click_pcb_filter_preset(*idx),
            PcbBoardMsg::FilterPresetRenamed(idx, name) => {
                task = self.rename_pcb_filter_preset(*idx, name.clone());
            }
            PcbBoardMsg::FilterPresetRemoved(idx) => task = self.remove_pcb_filter_preset(*idx),
            PcbBoardMsg::SnapToGridsToggled => {
                let snap = &mut self.ui_state.pcb_snap;
                snap.to_grids = !snap.to_grids;
            }
            PcbBoardMsg::SnapScopePicked(scope) => self.ui_state.pcb_snap.scope = *scope,
            PcbBoardMsg::SnapObjectToggled(object) => self.ui_state.pcb_snap.toggle_object(*object),
            PcbBoardMsg::SnapDistancePicked(distance) => {
                self.ui_state.pcb_snap.distance = *distance;
            }
            PcbBoardMsg::SnapToGuidesToggled => {
                let snap = &mut self.ui_state.pcb_snap;
                snap.to_guides = !snap.to_guides;
            }
            PcbBoardMsg::SnapToAxesToggled => {
                let snap = &mut self.ui_state.pcb_snap;
                snap.to_axes = !snap.to_axes;
            }
            PcbBoardMsg::FieldTyped(field, text) => {
                view.drafts.insert(*field, text.clone());
            }
            PcbBoardMsg::FieldSubmitted(field) => self.submit_pcb_board_field(*field),
            PcbBoardMsg::GuideAdded(kind) => {
                view.guide_add_open = false;
                self.add_pcb_guide(*kind);
            }
            PcbBoardMsg::GuidePlaceArmed(kind) => {
                view.guide_add_open = false;
                self.arm_pcb_guide_place(*kind);
            }
            PcbBoardMsg::GuideEnabledToggled(id) => self.toggle_pcb_guide(*id),
            PcbBoardMsg::GuideColorStepped(id) => self.step_pcb_guide_color(*id),
            PcbBoardMsg::GuideDeleted(id) => self.delete_pcb_guide(*id),
            PcbBoardMsg::GuideMenuToggled(id) => {
                view.guide_menu = (view.guide_menu != Some(*id)).then_some(*id);
            }
            PcbBoardMsg::GuideAddMenuToggled => view.guide_add_open = !view.guide_add_open,
            PcbBoardMsg::GuidePlaceModeToggled => view.guide_place = !view.guide_place,
            PcbBoardMsg::GridAdded => self.add_pcb_grid(),
            PcbBoardMsg::GridMenuToggled(card) => {
                view.grid_menu = (view.grid_menu != Some(*card)).then_some(*card);
            }
            PcbBoardMsg::GridEditToggled(card) => {
                view.grid_menu = None;
                view.grid_editing = (view.grid_editing != Some(*card)).then_some(*card);
            }
            PcbBoardMsg::GridMoved(id, up) => self.move_pcb_grid(*id, *up),
            PcbBoardMsg::GridDeleted(id) => self.delete_pcb_grid(*id),
            PcbBoardMsg::GridComponentsToggled(id) => self.toggle_pcb_grid_use(*id, true),
            PcbBoardMsg::GridNonComponentsToggled(id) => self.toggle_pcb_grid_use(*id, false),
            PcbBoardMsg::GridColorStepped(id) => self.step_pcb_grid_color(*id),
            PcbBoardMsg::PolygonNamingPicked(naming) => {
                let naming = *naming;
                self.edit_pcb_board_options(|o| {
                    std::mem::replace(&mut o.polygon_naming, naming) != naming
                });
            }
            PcbBoardMsg::DesignatorDisplayPicked(display) => {
                let display = *display;
                self.edit_pcb_board_options(|o| {
                    std::mem::replace(&mut o.designator_display, display) != display
                });
            }
            PcbBoardMsg::SheetFromBoardToggled => self.toggle_pcb_sheet_from_board(),
            // #2410: the Panel section's button runs the menu row's command.
            PcbBoardMsg::PanelRemoveClicked => {
                task = self.run_board_panel_command("remove_board_panel");
            }
            PcbBoardMsg::PanelSeparationPicked(separation) => {
                let separation = *separation;
                self.edit_active_board_panel(|panel| {
                    Ok(std::mem::replace(&mut panel.separation, separation) != separation)
                });
            }
            PcbBoardMsg::FastGridApplied(index) => self.apply_pcb_fast_grid(*index),
            PcbBoardMsg::ComponentsSelected(side) => self.select_pcb_components(*side),
            PcbBoardMsg::DrcResultsOpened => {
                // The check's results are listed in the Messages panel.
                task = Task::done(Message::Overlay(OverlayMsg::OpenPanel(PanelKind::Messages)));
            }
            // The button runs the BOM report's command, as its menu row does.
            PcbBoardMsg::ReportsOpened => {
                task = self.run_board_panel_command("board_report_bom");
            }
            // #2523: a field row or a callout of the board illustration.
            // Only the hover changes: patch the shown summary instead of
            // rebuilding it on every pointer crossing.
            PcbBoardMsg::IllustrationHovered(hover) => {
                view.illustration_hover.update(hover.clone());
                let hover = view.illustration_hover.clone();
                if let Some(board) = self.document_state.panel_ctx.pcb_board.as_mut() {
                    board.view.illustration_hover = hover;
                }
                return task;
            }
            // #2523: the size follows the board shape; edit it in Board
            // sketch, as Design ▸ Board Sketch does.
            PcbBoardMsg::BoardShapeEditPressed => {
                task = self.run_board_panel_command("board_sketch");
            }
            // #2523, mockup row 176: the board's materials live in the Layer
            // Stack.
            PcbBoardMsg::LayerStackOpened => {
                task = self.run_board_panel_command(
                    crate::app::handlers::pcb_layer_strip::LAYER_STACK_COMMAND,
                );
            }
        }
        self.sync_pcb_board_summary();
        task
    }

    /// Shift+E on a board: step Snapping through All Layers, Current Layer
    /// and Off.
    pub(crate) fn cycle_pcb_snapping(&mut self) {
        let snap = &mut self.ui_state.pcb_snap;
        snap.scope = snap.scope.next();
        self.sync_pcb_board_summary();
    }

    /// Select the board's components on `side`, as a Board Information count
    /// asks. Those on a hidden layer are left out.
    fn select_pcb_components(&mut self, side: ComponentSide) {
        let Some(board) = self.active_pcb() else {
            return;
        };
        let items: Vec<ItemRef> = board
            .footprints
            .iter()
            .filter(|fp| match side {
                ComponentSide::All => true,
                ComponentSide::Top => Side::of_layer(&fp.layer) == Side::Top,
                ComponentSide::Bottom => Side::of_layer(&fp.layer) == Side::Bottom,
            })
            .map(|fp| ItemRef::Footprint(fp.uuid))
            .collect();
        let done = self.with_pcb_selection(|sel, board| sel.select_items(board, &items));
        if done.is_some_and(|d| d.changed) {
            self.refresh_pcb_highlight();
        }
    }

    /// Apply the typed grid step, in the panel's unit, as one undo step; a
    /// step that is not a length above zero, or a refused edit, is said in
    /// the Messages panel and changes nothing.
    fn apply_pcb_grid_step(&mut self) {
        let Some(text) = self.ui_state.pcb_board_panel.grid_draft.take() else {
            return;
        };
        let unit = pcb_board_properties::shown_unit(self.ui_state.unit);
        let typed = parse_length_field_mm(&text, unit)
            .filter(|mm| *mm > 0.0 && Self::board_grid_step(*mm).is_some());
        match typed {
            Some(grid_mm) => self.set_pcb_grid_step(grid_mm),
            None => self.say_pcb_grid_refused(t!(self.ui_state.i18n, ids::PCB_BOARD_GRID_INVALID)),
        }
    }

    /// A Fast Grids button: the grid in effect becomes that fast grid.
    fn apply_pcb_fast_grid(&mut self, index: usize) {
        if let Some(mm) = self.ui_state.fast_grids_mm.get(index).copied() {
            self.set_pcb_grid_in_effect(mm);
        }
    }

    /// The board's snap step becomes a preset held as `f32`, without its
    /// float noise (0.1 stays 0.1, not 0.10000000149). One below the
    /// nanometre a board stores is refused, not read as the default step.
    pub(crate) fn set_pcb_grid_preset(&mut self, mm: f32) {
        match Self::board_grid_step(f64::from(mm)) {
            Some(step) => self.set_pcb_grid_step(step),
            None => self.say_pcb_grid_refused(t!(self.ui_state.i18n, ids::PCB_BOARD_GRID_INVALID)),
        }
    }

    /// A status-bar pick or grid key: the extra grid in effect takes it,
    /// else the board's own step. One undo step.
    pub(crate) fn set_pcb_grid_in_effect(&mut self, mm: f32) {
        let owner = self.interaction_state.acting_window;
        let Some((path, board)) = self.active_tab_path().zip(self.active_pcb()) else {
            return;
        };
        let what = self.pcb_grid_use(&path, owner);
        let extra = board
            .options
            .grids
            .iter()
            .find(|grid| grid.applies_to(what))
            .map(|grid| grid.uuid);
        let Some(id) = extra else {
            self.set_pcb_grid_preset(mm);
            return;
        };
        let Some(step) = Self::board_grid_step(f64::from(mm)) else {
            self.say_pcb_grid_refused(t!(self.ui_state.i18n, ids::PCB_BOARD_GRID_INVALID));
            return;
        };
        self.edit_pcb_board_options(|options| {
            match options.grids.iter_mut().find(|grid| grid.uuid == id) {
                Some(grid) if grid.step_mm != step => {
                    grid.step_mm = step;
                    true
                }
                _ => false,
            }
        });
    }

    /// `mm` to the nanometre, or `None` when that leaves no step.
    pub(crate) fn board_grid_step(mm: f64) -> Option<f64> {
        let nm = (mm * 1e6).round();
        (nm.is_finite() && nm >= 1.0).then(|| nm / 1e6)
    }

    fn say_pcb_grid_refused(&self, why: impl std::fmt::Display) {
        let line = t!(self.ui_state.i18n, ids::PCB_BOARD_GRID_REFUSED, "why" => why.to_string());
        crate::diagnostics::log_warning(line.into_string());
    }

    /// Set the board's snap step as one undo step; the step it already has
    /// is no edit. A refusal is said in the Messages panel.
    fn set_pcb_grid_step(&mut self, grid_mm: f64) {
        let Some(path) = self.active_tab_path() else {
            return;
        };
        let Some(engine) = self.document_state.board_engine(&path) else {
            return;
        };
        if signex_engine::pcb::placement::grid_step_mm(engine.board()) == grid_mm {
            return;
        }
        let label = t!(self.ui_state.i18n, ids::PCB_BOARD_GRID_UNDO).into_string();
        let step = CheckedStep {
            label: &label,
            expected_revision: engine.revision(),
        };
        let command = BoardCommand::SetGridSize { grid_mm };
        if let Err(error) = self.execute_board_commands(&path, vec![command], Some(step)) {
            self.say_pcb_grid_refused(error);
        }
    }

    /// Run catalog command `id` as its menu row does. Inside an undocked
    /// window's swap it runs now, on that window's board: a task would land
    /// on the main window.
    fn run_board_panel_command(&mut self, id: &str) -> Task<Message> {
        let Ok(id) = crate::keymap::AppCommandId::new(id) else {
            return Task::none();
        };
        if self.interaction_state.acting_window.is_some() {
            return self.run_menu_command(&id, None);
        }
        Task::done(Message::Menu(crate::menu_bar::MenuMessage::Command(id)))
    }
}

#[cfg(test)]
#[path = "pcb_board_panel_tests.rs"]
mod tests;
