//! `update` for the constraint grid (#2530): the domain tabs, the grid's
//! own messages, the find box, the filter chips and the drawing of the
//! selected cell. A cell commit, a reset and a drawing commit each write
//! one rule through `edit_rules`, as one undo step on the project's rule
//! history.

use iced::Task;
use signex_drc_dsl::table::catalog::Domain;
use signex_drc_dsl::table::grid as cells;
use signex_drc_dsl::table::{self, LimitField, RuleField, RuleTable};
use signex_widgets::constraint_grid::{CellRef, GridMessage, RowId};

use super::drawing::{self, PreviewMsg, same_subject};
use super::grid::{
    Cells, RowRef, RowSource, RulesGrid, mm_format, mm_text, scoped_on, typed_value,
};
use super::matrix::matrix_preview;
use super::search::FindHit;
use super::{RulesPanel, RulesTabMsg};
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, Signex, TabKind};

impl Signex {
    /// The Constraint sets button: the set cards (the Sets panel) shown or
    /// hidden; the Presets panel gives way to them and its open preview goes
    /// (#2771).
    pub(super) fn toggle_rules_sets(&mut self, id: ProjectId) {
        let tab = self.rules_tab_mut(id);
        tab.panel = match tab.panel {
            Some(RulesPanel::Sets) => None,
            _ => Some(RulesPanel::Sets),
        };
        tab.preview = None;
    }

    /// Overridden rows only: lay the rows out again.
    pub(super) fn toggle_rules_overridden_only(&mut self, id: ProjectId) {
        let tab = self.rules_tab_mut(id);
        tab.overridden_only = !tab.overridden_only;
        self.sync_rules_grid(id);
    }

    /// Show `domain`'s grid; the cell selection and the drawing start over.
    pub(super) fn pick_rules_domain(&mut self, id: ProjectId, domain: Domain) {
        let tab = self.rules_tab_mut(id);
        if tab.grid.domain() != domain {
            tab.grid = RulesGrid::new(domain);
            tab.drawing = Default::default();
        }
        tab.edit = None;
        self.sync_rules_grid(id);
    }

    /// The stored rule text's table, as the tab's analysis holds it.
    fn rules_table(&self, id: ProjectId) -> RuleTable {
        let analysed = self
            .document_state
            .project_tabs
            .get(&id)
            .and_then(|tab| tab.rules.analysis.as_ref())
            .map(|a| a.table.clone());
        analysed.unwrap_or_else(|| {
            let text = self.rules_text_of(id).map(|(t, _)| t).unwrap_or_default();
            table::table(&text)
        })
    }

    fn rules_sets(
        &self,
        id: ProjectId,
    ) -> std::collections::BTreeMap<String, signex_types::project::NamedRuleSet> {
        self.rules_drc_of(id)
            .map(|drc| drc.sets)
            .unwrap_or_default()
    }

    /// What the grid's rows are built from: the project's classes, the
    /// board's nets and copper layers, the rows the text writes rules for.
    fn rules_row_source(&self, id: ProjectId, table: &RuleTable) -> RowSource {
        let classes = self
            .document_state
            .project_by_id(id)
            .map(|p| p.data.net_classes.clone())
            .unwrap_or_default();
        let board = self.rules_board(id).map(|(_, board)| board);
        let nets = board
            .map(|b| {
                b.nets
                    .iter()
                    .filter(|n| !n.name.is_empty())
                    .map(|n| {
                        let class =
                            signex_types::net_class::first_matching_class(&classes, &n.name)
                                .map(|c| c.name.clone());
                        (n.name.clone(), class)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let layers = board
            .map(|b| {
                signex_types::pcb_stack::board_copper_layers(&b.layers)
                    .into_iter()
                    .map(|layer| {
                        signex_types::format::pcb_layer_token(
                            signex_types::pcb_layer::PcbLayer::from(layer),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        RowSource {
            classes: classes.iter().map(|c| c.name.clone()).collect(),
            nets,
            layers,
            written: cells::written_scopes(table),
            overridden: None,
        }
    }

    /// Lay the grid's rows out again when what they are built from
    /// changed. Called from `update` (never from `view`).
    pub(super) fn sync_rules_grid(&mut self, id: ProjectId) {
        let table = self.rules_table(id);
        let sets = self.rules_sets(id);
        let mut source = self.rules_row_source(id, &table);
        let i18n = &self.ui_state.i18n;
        let Some(tab) = self.document_state.project_tabs.get_mut(&id) else {
            return;
        };
        let state = &mut tab.rules;
        let domain = state.grid.domain();
        let scoped = scoped_on(&table, domain);
        if state.overridden_only {
            // The rows with a value of their own, from the full layout.
            let mut full = RulesGrid::new(domain);
            full.build_rows(source.clone(), scoped.clone(), i18n);
            let cells = Cells {
                grid: &full,
                table: &table,
                sets: &sets,
            };
            let own = (0..full.rows().len())
                .map(|i| RowId(i as u32))
                .filter(|row| cells.has_own(*row))
                .filter_map(|row| full.row(row).cloned())
                .chain(std::iter::once(RowRef::All))
                .collect();
            source.overridden = Some(own);
        }
        if state.grid.is_built_from(&source, &scoped) {
            return;
        }
        let Some(reselect) = state.grid.build_rows(source, scoped, i18n) else {
            return;
        };
        // Select it again with the real values, so the editor opens with
        // the cell's own text and a reset still reads as a change.
        self.update_grid_view(id, GridMessage::CellPressed(reselect.cell));
        let after = match reselect.typed {
            Some(text) => GridMessage::EditTextChanged(text),
            None => GridMessage::EditCancelled,
        };
        self.update_grid_view(id, after);
    }

    /// Apply a view-state message to the grid widget.
    fn update_grid_view(&mut self, id: ProjectId, msg: GridMessage) {
        let table = self.rules_table(id);
        let sets = self.rules_sets(id);
        let i18n = &self.ui_state.i18n;
        let Some(tab) = self.document_state.project_tabs.get_mut(&id) else {
            return;
        };
        let grid = &mut tab.rules.grid;
        let mut view = std::mem::take(&mut grid.view);
        {
            let cells = Cells {
                grid,
                table: &table,
                sets: &sets,
            };
            // Value changes come back only for the messages handled below.
            let _ = view.update(msg, &cells, mm_format(i18n));
        }
        grid.view = view;
    }

    pub(super) fn act_on_rules_grid(&mut self, id: ProjectId, msg: GridMessage) -> Task<Message> {
        match msg {
            GridMessage::CellPressed(cell) => self.press_rules_cell(id, cell),
            GridMessage::EditSubmitted => {
                self.submit_rules_cell(id);
                Task::none()
            }
            GridMessage::CellReset(cell) => {
                self.reset_rules_cell(id, cell);
                Task::none()
            }
            GridMessage::ColumnQueryChanged(query) => {
                let i18n = &self.ui_state.i18n;
                if let Some(tab) = self.document_state.project_tabs.get_mut(&id) {
                    tab.rules.find.typed(query, i18n);
                }
                self.jump_to_found(id)
            }
            GridMessage::ColumnQuerySubmitted => {
                self.rules_tab_mut(id).find.step();
                self.jump_to_found(id)
            }
            // No checked column is a choice or a flag yet: those cells are
            // inert, so a pick or a toggle writes nothing.
            GridMessage::ChoicePicked(..) | GridMessage::FlagToggled(..) => Task::none(),
            msg @ (GridMessage::RowToggled(_)
            | GridMessage::GroupToggled(_)
            | GridMessage::EditTextChanged(_)
            | GridMessage::EditCancelled) => {
                self.update_grid_view(id, msg);
                Task::none()
            }
        }
    }

    /// Select `cell`: an editable one opens for typing (focused), an inert
    /// or hatched one is selected only. The detail shows the rule that
    /// governs it.
    fn press_rules_cell(&mut self, id: ProjectId, cell: CellRef) -> Task<Message> {
        let table = self.rules_table(id);
        let sets = self.rules_sets(id);
        let tab = self.rules_tab_mut(id);
        let cells = Cells {
            grid: &tab.grid,
            table: &table,
            sets: &sets,
        };
        let show = cells.show(cell);
        let governing = match tab.grid.row(cell.row) {
            Some(RowRef::Scoped(name)) => Some(name.clone()),
            _ => match &show {
                super::grid::CellShow::Value { source, .. } => source.rule().map(str::to_string),
                _ => None,
            },
        };
        let editable = show.is_editable();
        let moved = tab.grid.view.selected() != Some(cell);
        // The detail names the cell's rule, or none: never one the cell
        // does not show.
        tab.selected = governing;
        tab.edit = None;
        if moved {
            tab.drawing = Default::default();
        }
        self.update_grid_view(id, GridMessage::CellPressed(cell));
        if !editable {
            self.update_grid_view(id, GridMessage::EditCancelled);
            return Task::none();
        }
        iced::widget::operation::focus(signex_widgets::constraint_grid::EDITOR_ID)
    }

    /// Select written rule `rule`, as the DRC panel's Rule link asks: the
    /// cell of the first of its limits with a column on this grid, on its
    /// Scoped row, else on its scope's row, so the drawing and "Which value
    /// wins" are the rule's. A rule with no such cell on the domain is
    /// named in the detail only.
    pub(crate) fn select_rules_rule(&mut self, id: ProjectId, rule: String) -> Task<Message> {
        let table = self.rules_table(id);
        let grid = &self.rules_tab_mut(id).grid;
        let cell = table.row(&rule).and_then(|row| {
            let column = row
                .limits
                .iter()
                .find_map(|limit| column_of(grid, limit.field))?;
            let at = grid.row_of(&RowRef::Scoped(rule.clone())).or_else(|| {
                cells::GridScope::of(&row.scope, row.layer.as_deref())
                    .and_then(|scope| grid.row_of_scope(&scope))
            })?;
            Some(CellRef::new(at, column))
        });
        match cell {
            Some(cell) => {
                // Selected, not opened for typing: no editor to focus.
                drop(self.press_rules_cell(id, cell));
                self.update_grid_view(id, GridMessage::EditCancelled);
                Task::none()
            }
            None => {
                self.rules_tab_mut(id).selected = Some(rule);
                Task::none()
            }
        }
    }

    /// Enter in a cell: write it into the rule text, or say why not and
    /// keep it open.
    fn submit_rules_cell(&mut self, id: ProjectId) {
        let Some(edit) = self.rules_tab_mut(id).grid.view.edit().cloned() else {
            return;
        };
        if edit.text.trim() == edit.opened.trim() {
            self.update_grid_view(id, GridMessage::EditCancelled);
            return;
        }
        if self.write_rules_cell(id, edit.cell, &typed_value(&edit.text)) {
            self.update_grid_view(id, GridMessage::EditCancelled);
        }
    }

    /// Set `cell` to `value` (rule text; empty resets it) as one undo step.
    /// Whether it was stored.
    pub(super) fn write_rules_cell(&mut self, id: ProjectId, cell: CellRef, value: &str) -> bool {
        let grid = &self.rules_tab_mut(id).grid;
        let (Some(row), Some(field)) = (
            grid.row(cell.row).cloned(),
            grid.column(cell.column).and_then(|c| c.field),
        ) else {
            return false;
        };
        let value = value.to_string();
        match row {
            RowRef::Scoped(rule) => self.edit_rules(id, &[], |src| {
                table::set_field(src, &rule, RuleField::Limit(field), &value)
            }),
            row => {
                let Some(scope) = row.scope() else {
                    return false;
                };
                self.edit_rules(id, &[], |src| {
                    cells::set_cell(src, &table::table(src), &scope, field, &value)
                })
            }
        }
    }

    /// ↺ on a cell: drop the row's own value of that column only.
    fn reset_rules_cell(&mut self, id: ProjectId, cell: CellRef) {
        let grid = &self.rules_tab_mut(id).grid;
        let (Some(row), Some(field)) = (
            grid.row(cell.row).cloned(),
            grid.column(cell.column).and_then(|c| c.field),
        ) else {
            return;
        };
        let reset = match row {
            RowRef::Scoped(rule) => self.edit_rules(id, &[], |src| {
                table::set_field(src, &rule, RuleField::Limit(field), "")
            }),
            row => match row.scope() {
                Some(scope) => self.edit_rules(id, &[], |src| {
                    cells::reset_cell(src, &table::table(src), &scope, field)
                }),
                None => false,
            },
        };
        let editing = self
            .rules_tab_mut(id)
            .grid
            .view
            .edit()
            .is_some_and(|e| e.cell == cell);
        if reset && editing {
            self.update_grid_view(id, GridMessage::EditCancelled);
        }
    }

    /// Show the find box's current match: its domain, its group open, a
    /// cell of its column selected and scrolled into view.
    fn jump_to_found(&mut self, id: ProjectId) -> Task<Message> {
        let Some(hit) = self.rules_tab_mut(id).find.current() else {
            return Task::none();
        };
        self.pick_rules_domain(id, hit.domain);
        let grid = &self.rules_tab_mut(id).grid;
        let Some(column) = grid.column_named(hit.column.id) else {
            return Task::none();
        };
        if let Some(group) = grid.group_of(column)
            && grid.view.is_group_collapsed(group)
        {
            self.update_grid_view(id, GridMessage::GroupToggled(group));
        }
        let row = self.found_row(id, hit);
        if let Some(row) = row {
            self.update_grid_view(id, GridMessage::CellPressed(CellRef::new(row, column)));
            self.update_grid_view(id, GridMessage::EditCancelled);
        }
        let x = super::view::column_left(&self.rules_tab_mut(id).grid, column);
        iced::widget::operation::scrollable::scroll_to(
            super::view::grid_scroll_id(),
            iced::widget::scrollable::AbsoluteOffset {
                x: Some(x),
                y: None,
            },
            iced::widget::operation::Animation::Instant,
        )
    }

    /// The row a find jump selects: the selected row when the column
    /// applies to it, else All.
    fn found_row(&mut self, id: ProjectId, hit: FindHit) -> Option<RowId> {
        let table = self.rules_table(id);
        let grid = &self.rules_tab_mut(id).grid;
        let selected = grid.view.selected().map(|c| c.row);
        let applies = |row: RowId| {
            grid.row(row).is_some_and(|r| {
                hit.column
                    .field
                    .is_none_or(|f| super::grid::applies(&table, r, f))
            })
        };
        selected
            .filter(|row| applies(*row))
            .or_else(|| grid.row_of(&RowRef::All))
    }

    /// A drawing message: a committed label or drag writes the selected
    /// cell's row (or the pinned matrix cell) as one undo step.
    pub(super) fn act_on_rules_drawing(&mut self, id: ProjectId, msg: PreviewMsg) -> Task<Message> {
        let tab = self.rules_tab_mut(id);
        let Some(target) = tab.drawing_target.clone() else {
            return Task::none();
        };
        let (commit, task) = drawing::update(&mut tab.drawing, &target, msg);
        let task = task.map(move |msg| {
            Message::Project(ProjectMsg::Rules(RulesTabMsg::PreviewActed {
                project_id: id,
                msg,
            }))
        });
        let Some(commit) = commit else {
            return task;
        };
        if self.rules_tab_mut(id).is_matrix_shown() {
            return Task::batch([
                task,
                super::update_matrix::preview_committed(self, id, commit.value_mm),
            ]);
        }
        let grid = &self.rules_tab_mut(id).grid;
        let Some(selected) = grid.view.selected() else {
            return task;
        };
        // A pair gap writes the selected column; a value of a column this
        // grid does not show writes nothing.
        let column = match commit.field {
            None => selected.column,
            Some(field) => match column_of(grid, field) {
                Some(column) => column,
                None => return task,
            },
        };
        let value = mm_text(commit.value_mm);
        self.write_rules_cell(id, CellRef::new(selected.row, column), &value);
        task
    }

    /// What the drawing shows: in the grid, the selected cell's rule kind
    /// with the values its row resolves to; in the matrix, the hovered or
    /// pinned pair at the value it shows. A new subject closes the
    /// drawing's open label.
    pub(super) fn refresh_rules_drawing(&mut self, id: ProjectId) {
        let sets = self.rules_sets(id);
        let i18n = &self.ui_state.i18n;
        let Some(tab) = self.document_state.project_tabs.get_mut(&id) else {
            return;
        };
        let state = &mut tab.rules;
        let Some(analysis) = state.analysis.as_ref() else {
            return;
        };
        let target = if state.is_matrix_shown() {
            matrix_preview(state, &analysis.table, &analysis.defaults)
                .and_then(|preview| preview.drawing_target())
        } else {
            let cells = Cells {
                grid: &state.grid,
                table: &analysis.table,
                sets: &sets,
            };
            state
                .grid
                .view
                .selected()
                .and_then(|at| drawing_target(&cells, at, i18n))
        };
        if !same_subject(state.drawing_target.as_ref(), target.as_ref()) {
            state.drawing.close_labels();
        }
        state.drawing_target = target;
    }

    /// The drawing's key (Tab, Shift+Tab, Esc on a label being typed or a
    /// handle held), while `window` shows a Rules tab: the main window's
    /// active tab, or the tab an undocked window carries.
    pub(crate) fn rules_drawing_key(
        &self,
        window: iced::window::Id,
        event: &iced::keyboard::Event,
    ) -> Option<Message> {
        let iced::keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
            return None;
        };
        let docs = &self.document_state;
        let shown = docs.tab_shown_in_window(window, &self.ui_state)?;
        let TabKind::Rules(id) = shown.kind.clone() else {
            return None;
        };
        let tab = docs.project_tabs.get(&id)?;
        drawing::key_message(&tab.rules.drawing, key, *modifiers).map(|msg| {
            Message::Project(ProjectMsg::Rules(RulesTabMsg::PreviewActed {
                project_id: id,
                msg,
            }))
        })
    }
}

/// The drawing of cell `at`: its column's rule kind, with every value of
/// that kind the row resolves to, captioned with the row and rule type.
/// `None` for a column the check does not read yet.
pub(super) fn drawing_target(
    cells: &Cells,
    at: CellRef,
    i18n: &signex_i18n::I18n,
) -> Option<drawing::PreviewTarget> {
    let grid = cells.grid;
    let field = grid.column(at.column)?.field?;
    let kind = field.kind();
    let mut values = Vec::new();
    for (c, column) in grid
        .groups()
        .iter()
        .flat_map(|g| g.columns.iter())
        .enumerate()
    {
        let Some(f) = column.field.filter(|f| f.kind() == kind) else {
            continue;
        };
        let at = CellRef::new(at.row, signex_widgets::constraint_grid::ColumnId(c as u32));
        if let super::grid::CellShow::Value { mm: Some(mm), .. } = cells.show(at) {
            values.push((f, mm));
        }
    }
    let row = grid.row(at.row)?;
    let name = match row {
        RowRef::All | RowRef::Heading(_) => {
            signex_i18n::t!(i18n, signex_i18n::ids::RULES_GRID_ROW_ALL).into_string()
        }
        other => other.name().to_string(),
    };
    let group = grid
        .groups()
        .iter()
        .find(|g| g.columns.iter().any(|c| c.field == Some(field)))?;
    let caption = signex_i18n::t!(
        i18n,
        signex_i18n::ids::RULES_GRID_PREVIEW_CAPTION,
        "row" => name,
        "type" => signex_i18n::t!(i18n, super::search::type_label(group.id)).into_string()
    )
    .into_string();
    Some(drawing::PreviewTarget::Rule {
        kind,
        values,
        caption,
    })
}

/// The grid column that shows `field`.
fn column_of(
    grid: &RulesGrid,
    field: LimitField,
) -> Option<signex_widgets::constraint_grid::ColumnId> {
    grid.groups()
        .iter()
        .flat_map(|g| g.columns.iter())
        .find(|c| c.field == Some(field))
        .and_then(|c| grid.column_named(c.id))
}
