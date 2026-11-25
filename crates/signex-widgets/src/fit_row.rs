//! A row of cells that never overlap: each keeps at least its natural width
//! (or a floor the caller names), spare width is shared, and when the cells
//! do not fit they fold into an even grid, down to one per line. Segmented
//! controls and paired fields are built on it.

use iced::advanced::layout::{Layout, Limits};
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Shell, overlay, renderer};
use iced::mouse::{self, Cursor};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

/// Cells laid out by [`plan_lines`].
pub struct FitRow<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    cells: Vec<Element<'a, Message, Theme, Renderer>>,
    spacing: f32,
    is_fill: bool,
    cell_floor: Option<f32>,
    max_columns: Option<usize>,
}

impl<'a, Message, Theme, Renderer> FitRow<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    /// `cells` hugging their content, with no gap.
    pub fn new(cells: Vec<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            cells,
            spacing: 0.0,
            is_fill: false,
            cell_floor: None,
            max_columns: None,
        }
    }

    /// The gap between cells, across and between lines.
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    /// Take the full width and share what the cells leave spare.
    pub fn fill(mut self, is_fill: bool) -> Self {
        self.is_fill = is_fill;
        self
    }

    /// Treat every cell as `floor` wide when deciding the fold, for cells
    /// that fill their width and so have no natural width to measure.
    pub fn cell_floor(mut self, floor: f32) -> Self {
        self.cell_floor = Some(floor);
        self
    }

    /// Never put more than `columns` cells on a line.
    pub fn max_columns(mut self, columns: usize) -> Self {
        self.max_columns = Some(columns);
        self
    }

    fn width(&self) -> Length {
        if self.is_fill {
            Length::Fill
        } else {
            Length::Shrink
        }
    }
}

/// Less than this over the width is float rounding, not overflow: a parent
/// sized to the cells' measured sum hands back the sum less a few ulps.
const ROUNDING: f32 = 0.01;

/// The widths of each line for cells of `naturals` in `available`: the most
/// columns (up to `max_columns`) whose every line fits, one column at worst.
/// With `is_fill` each line takes the whole width; a cell wider than the
/// width is cut to it.
pub fn plan_lines(
    naturals: &[f32],
    available: f32,
    gap: f32,
    is_fill: bool,
    max_columns: Option<usize>,
) -> Vec<Vec<f32>> {
    if naturals.is_empty() {
        return Vec::new();
    }
    let most = max_columns
        .unwrap_or(naturals.len())
        .clamp(1, naturals.len());
    let columns = (1..=most)
        .rev()
        .find(|&c| {
            naturals
                .chunks(c)
                .all(|line| line_width(line, gap) <= available + ROUNDING)
        })
        .unwrap_or(1);
    let is_fill = is_fill && available.is_finite();
    naturals
        .chunks(columns)
        .map(|line| {
            if is_fill {
                shared_widths(line, available, gap)
            } else {
                line.iter().map(|w| w.min(available)).collect()
            }
        })
        .collect()
}

fn line_width(line: &[f32], gap: f32) -> f32 {
    line.iter().sum::<f32>() + gap * line.len().saturating_sub(1) as f32
}

/// `line` grown to fill `available`: the narrowest cells rise to a common
/// width first, so cells are equal whenever the widest allows it.
fn shared_widths(line: &[f32], available: f32, gap: f32) -> Vec<f32> {
    let room = (available - gap * line.len().saturating_sub(1) as f32).max(0.0);
    let total: f32 = line.iter().sum();
    if total >= room {
        return line.iter().map(|w| w.min(room)).collect();
    }
    let mut sorted = line.to_vec();
    sorted.sort_by(f32::total_cmp);
    let mut rest = total;
    let mut level = 0.0;
    for (k, width) in sorted.iter().enumerate() {
        rest -= width;
        level = (room - rest) / (k + 1) as f32;
        if sorted.get(k + 1).is_none_or(|next| level <= *next) {
            break;
        }
    }
    line.iter().map(|w| w.max(level)).collect()
}

impl<Message, Theme, Renderer> widget::Meta for FitRow<'_, Message, Theme, Renderer> where
    Renderer: iced::advanced::Renderer
{
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for FitRow<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(&mut self.cells);
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width(),
            height: Length::Shrink,
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &Limits) {
        let width = self.width();
        let limits = limits.width(width).height(Length::Shrink);
        let available = limits.bounds().width;
        let is_stretched = self.is_fill && !limits.compression.width;

        let naturals: Vec<f32> = match self.cell_floor {
            Some(floor) => vec![floor; self.cells.len()],
            None => {
                let measure = limits.loose();
                self.cells
                    .iter_mut()
                    .zip(&mut tree.children)
                    .map(|(cell, child)| {
                        cell.layout(child, renderer, &measure);
                        child.size.width
                    })
                    .collect()
            }
        };

        let lines = plan_lines(
            &naturals,
            available,
            self.spacing,
            is_stretched,
            self.max_columns,
        );
        let mut index = 0;
        let mut y = 0.0;
        let mut widest: f32 = 0.0;
        for (n, line) in lines.iter().enumerate() {
            if n > 0 {
                y += self.spacing;
            }
            let mut x = 0.0;
            let mut height: f32 = 0.0;
            for &cell_width in line {
                let tight = Limits::with_flags(
                    Size::new(cell_width, 0.0),
                    Size::new(cell_width, limits.max.height),
                    Size::new(false, true),
                    Size::new(false, limits.infinite.height),
                );
                let child = &mut tree.children[index];
                self.cells[index].layout(child, renderer, &tight);
                child.translation = Vector::new(x, y);
                height = height.max(child.size.height);
                x += cell_width + self.spacing;
                index += 1;
            }
            widest = widest.max(x - self.spacing);
            y += height;
        }
        tree.size = limits.resolve(width, Length::Shrink, Size::new(widest, y));
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        operation.container(None, layout.bounds(), viewport);
        operation.traverse(&mut |operation| {
            self.cells
                .iter_mut()
                .zip(layout.iter_mut(&mut tree.children))
                .for_each(|(cell, (layout, state))| {
                    cell.operate(state, layout, viewport, renderer, operation);
                });
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        for (cell, (layout, tree)) in self
            .cells
            .iter_mut()
            .zip(layout.iter_mut(&mut tree.children))
        {
            cell.update(tree, event, layout, cursor, renderer, shell, viewport);
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.cells
            .iter()
            .zip(layout.iter(&tree.children))
            .map(|(cell, (layout, tree))| {
                cell.mouse_interaction(tree, layout, cursor, viewport, renderer)
            })
            .max()
            .unwrap_or_default()
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        for (cell, (layout, tree)) in self
            .cells
            .iter()
            .zip(layout.iter(&tree.children))
            .filter(|(_, (layout, _))| layout.bounds().intersects(viewport))
        {
            cell.draw(tree, renderer, theme, style, layout, cursor, viewport);
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'b, Message, Theme, Renderer>> {
        overlay::from_children(
            &mut self.cells,
            tree,
            layout,
            renderer,
            viewport,
            translation,
            window,
        )
    }
}

impl<'a, Message, Theme, Renderer> From<FitRow<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(row: FitRow<'a, Message, Theme, Renderer>) -> Self {
        iced::advanced::Widget::_boxed(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_that_fit_share_one_line_equally() {
        let lines = plan_lines(&[30.0, 40.0, 20.0], 200.0, 2.0, true, None);
        assert_eq!(lines.len(), 1);
        let want = (200.0 - 4.0) / 3.0;
        assert!(
            lines[0].iter().all(|w| (w - want).abs() < 1e-3),
            "{lines:?}"
        );
    }

    #[test]
    fn a_wide_label_keeps_its_width_and_the_others_share_the_rest() {
        let lines = plan_lines(&[20.0, 120.0, 20.0], 200.0, 0.0, true, None);
        assert_eq!(lines, [vec![40.0, 120.0, 40.0]]);
    }

    #[test]
    fn cells_that_do_not_fit_fold_into_an_even_grid() {
        let lines = plan_lines(&[60.0; 4], 130.0, 2.0, true, None);
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|line| line.len() == 2), "{lines:?}");
    }

    #[test]
    fn a_cell_wider_than_the_row_gets_a_line_cut_to_the_row() {
        let lines = plan_lines(&[300.0, 20.0], 100.0, 2.0, true, None);
        assert_eq!(lines, [vec![100.0], vec![100.0]]);
    }

    #[test]
    fn hugging_cells_keep_their_natural_widths() {
        let lines = plan_lines(&[30.0, 40.0], 200.0, 2.0, false, None);
        assert_eq!(lines, [vec![30.0, 40.0]]);
    }

    #[test]
    fn max_columns_caps_a_line() {
        let lines = plan_lines(&[10.0; 4], 500.0, 0.0, false, Some(2));
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn cells_a_rounding_error_over_the_width_still_share_one_line() {
        let lines = plan_lines(&[150.0, 132.0], 283.999, 2.0, true, None);
        assert_eq!(lines.len(), 1, "{lines:?}");
        let lines = plan_lines(&[150.0, 132.0], 283.5, 2.0, true, None);
        assert_eq!(lines.len(), 2, "half a pixel short is overflow: {lines:?}");
    }

    #[test]
    fn an_unbounded_row_stays_on_one_natural_line() {
        let lines = plan_lines(&[30.0, 40.0], f32::INFINITY, 2.0, true, None);
        assert_eq!(lines, [vec![30.0, 40.0]]);
    }
}
