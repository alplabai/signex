//! Use a date picker as an input element for picking dates.
//!
//! *This API requires the following crate features to be activated: `date_picker`*

use crate::iced_aw_font::advanced_text::{cancel, left_open, ok, right_open};
use crate::{
    core::{
        date::{Date, IsInMonth},
        overlay::Position,
    },
    date_picker,
    style::{Status, date_picker::Style, style_state::StyleState},
};
use chrono::{Datelike, Local, NaiveDate};
use iced_core::{
    Border, Color, Element, Event, Font, Layout, Length, Overlay, Padding, Pixels, Point,
    Rectangle, Renderer as _, Shadow, Shell, Size, Vector, Widget,
    alignment::{Horizontal, Vertical},
    event, keyboard,
    layout::Limits,
    mouse::{self, Cursor},
    overlay, renderer, shell,
    text::{self, Renderer as _},
    touch,
    widget::{self, Meta, tree::Tree},
};
use iced_widget::{Button, Column, Container, Renderer, Row};
use std::collections::HashMap;

/// The padding around the elements.
const PADDING: Padding = Padding::new(10.0);
/// The spacing between the elements.
const SPACING: Pixels = Pixels(15.0);
/// The padding of the day cells.
const DAY_CELL_PADDING: Padding = Padding::new(7.0);
/// The spacing between the buttons.
const BUTTON_SPACING: Pixels = Pixels(5.0);

/// The overlay of the [`DatePicker`](crate::widget::DatePicker).
#[allow(missing_debug_implementations)]
pub struct DatePickerOverlay<'a, 'b, Message, Theme>
where
    Message: Clone,
    Theme: 'a
        + crate::style::date_picker::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
    'b: 'a,
{
    /// The state of the [`DatePickerOverlay`].
    state: &'a mut State,
    /// The cancel button of the [`DatePickerOverlay`].
    cancel_button: Button<'a, Message, widget::Text<'a, Theme>, Theme>,
    /// The submit button of the [`DatePickerOverlay`].
    submit_button: Button<'a, Message, widget::Text<'a, Theme>, Theme>,
    /// The function that produces a message when the submit button of the [`DatePickerOverlay`] is pressed.
    on_submit: &'a dyn Fn(Date) -> Message,
    /// The position of the [`DatePickerOverlay`].
    position: Point,
    /// The style of the [`DatePickerOverlay`].
    class: &'a <Theme as crate::style::date_picker::Catalog>::Class<'b>,
    /// The reference to the tree holding the state of this overlay.
    ///
    /// The computed size of the overlay and the sizes and translations of its
    /// children (cancel button, submit button and calendar) are stored in it.
    tree: &'a mut Tree,
    /// The font size of text and icons in the [`DatePickerOverlay`]
    font_size: Pixels,
    viewport: Rectangle,
}

/// The absolute bounds of a month or year selector (left arrow, text, right arrow).
#[derive(Debug, Clone, Copy, Default)]
struct Selector {
    /// The bounds of the whole selector.
    whole: Rectangle,
    /// The bounds of the left arrow.
    left: Rectangle,
    /// The bounds of the center text.
    center: Rectangle,
    /// The bounds of the right arrow.
    right: Rectangle,
}

/// The absolute bounds of all the elements of the [`DatePickerOverlay`].
#[derive(Debug, Clone)]
struct Geometry {
    /// The bounds of the whole overlay.
    overlay: Rectangle,
    /// The layout of the calendar column.
    col: Layout,
    /// The month selector.
    month: Selector,
    /// The year selector.
    year: Selector,
    /// The bounds of the days table.
    days: Rectangle,
    /// The bounds of the day labels.
    labels: Vec<Rectangle>,
    /// The bounds of the day cells, row by row.
    cells: Vec<Vec<Rectangle>>,
    /// The layout of the cancel button.
    cancel: Layout,
    /// The layout of the submit button.
    submit: Layout,
}

impl<'a, 'b, Message, Theme> DatePickerOverlay<'a, 'b, Message, Theme>
where
    Message: 'static + Clone,
    Theme: 'a
        + crate::style::date_picker::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog
        + iced_widget::container::Catalog,
    'b: 'a,
{
    #[allow(clippy::too_many_arguments)]
    /// Creates a new [`DatePickerOverlay`] on the given position.
    pub fn new(
        state: &'a mut date_picker::State,
        on_cancel: Message,
        on_submit: &'a dyn Fn(Date) -> Message,
        position: Point,
        class: &'a <Theme as crate::style::date_picker::Catalog>::Class<'b>,
        tree: &'a mut Tree,
        //button_style: impl Clone +  Into<<Renderer as button::Renderer>::Style>, // clone not satisfied
        font_size: Pixels,
        viewport: Rectangle,
    ) -> Self {
        let date_picker::State { overlay_state } = state;

        let (cancel_content, cancel_font, _cancel_shaping) = cancel();
        let (submit_content, submit_font, _submit_shaping) = ok();
        DatePickerOverlay {
            state: overlay_state,
            cancel_button: Button::new(
                widget::Text::new(cancel_content)
                    .font(cancel_font)
                    .size(font_size)
                    .align_x(Horizontal::Center)
                    .width(Length::Fill),
            )
            .width(Length::Fill)
            .on_press(on_cancel.clone()),
            submit_button: Button::new(
                widget::Text::new(submit_content)
                    .font(submit_font)
                    .size(font_size)
                    .align_x(Horizontal::Center)
                    .width(Length::Fill),
            )
            .width(Length::Fill)
            .on_press(on_cancel), // Sending a fake message
            on_submit,
            position,
            class,
            tree,
            font_size,
            viewport,
        }
    }

    /// Turn this [`DatePickerOverlay`] into an overlay [`Element`](overlay::Element).
    #[must_use]
    pub fn overlay(self) -> overlay::Element<'a, Message, Theme, Renderer> {
        overlay::Element::new(Box::new(self))
    }

    /// String representation of the current month and year.
    fn date_as_string(&self) -> String {
        crate::core::date::date_as_string(self.state.date)
    }

    /// Returns whether the sizes stored in the tree have been computed.
    fn is_laid_out(&self) -> bool {
        self.tree.children.len() >= 3 && self.tree.size.width > 0.0 && self.tree.size.height > 0.0
    }

    /// Lays out the [`DatePickerOverlay`].
    ///
    /// The `Overlay` trait has no `layout` method anymore, so the sizes of the
    /// children are computed here and stored in the tree. The absolute position
    /// of the overlay is derived from them by [`Self::geometry`].
    #[allow(clippy::too_many_lines)]
    pub fn layout(&mut self, renderer: &Renderer) {
        let limits = Limits::new(Size::ZERO, self.viewport.size())
            .shrink(PADDING)
            .width(Length::Fit)
            .height(Length::Fit);

        sync_child(self.tree, 0, &mut self.cancel_button);
        sync_child(self.tree, 1, &mut self.submit_button);

        // Pre-Buttons TODO: get rid of it
        self.cancel_button
            .layout(&mut self.tree.children[0], renderer, &limits);
        let pre_cancel_height = self.tree.children[0].size.height;

        let limits = limits.shrink(Size::new(0.0, pre_cancel_height + SPACING.0));

        // Month/Year and days, measured with the widest possible content
        let mut col = calendar::<Message, Theme>(self.font_size, "September", "9999", |_x, _y| {
            "31".to_owned()
        });

        sync_child(self.tree, 2, &mut col);
        col.layout(&mut self.tree.children[2], renderer, &limits);

        let col_size = self.tree.children[2].size;
        self.tree.children[2].translation = Vector::new(PADDING.left, PADDING.top);

        // Buttons
        let mut button_limits = limits;
        button_limits.max.width = limits
            .max
            .width
            .min(((col_size.width / 2.0) - BUTTON_SPACING.0).max(0.0));
        button_limits.min.width = limits.min.width.min(button_limits.max.width);

        self.cancel_button
            .layout(&mut self.tree.children[0], renderer, &button_limits);
        self.submit_button
            .layout(&mut self.tree.children[1], renderer, &button_limits);

        let cancel_size = self.tree.children[0].size;
        let submit_size = self.tree.children[1].size;

        let buttons_y = col_size.height + PADDING.top + SPACING.0;

        self.tree.children[0].translation = Vector::new(PADDING.left, buttons_y);
        self.tree.children[1].translation =
            Vector::new(col_size.width - submit_size.width + PADDING.left, buttons_y);

        self.tree.size = Size::new(
            col_size.width + PADDING.x(),
            col_size.height + cancel_size.height + PADDING.y() + SPACING.0,
        );
    }

    /// Computes the absolute geometry of the [`DatePickerOverlay`] from the
    /// sizes stored in the tree by [`Self::layout`].
    ///
    /// Returns `None` if the overlay has not been laid out yet.
    fn geometry(&self) -> Option<Geometry> {
        let size = self.tree.size;

        if self.tree.children.len() < 3 || size.width <= 0.0 || size.height <= 0.0 {
            return None;
        }

        let mut overlay = Layout::new(size);
        overlay.center_and_bounce(self.position, self.viewport.size());

        let mut children = overlay.iter(&self.tree.children);
        let (cancel, _) = children.next()?;
        let (submit, _) = children.next()?;
        let (col, col_tree) = children.next()?;

        let mut col_children = col.iter(&col_tree.children);
        let (month_year, month_year_tree) = col_children.next()?;
        let (days_parent, days_parent_tree) = col_children.next()?;

        let selector = |layout: Layout, tree: &Tree| -> Option<Selector> {
            let mut children = layout.iter(&tree.children);

            Some(Selector {
                whole: layout.bounds(),
                left: children.next()?.0.bounds(),
                center: children.next()?.0.bounds(),
                right: children.next()?.0.bounds(),
            })
        };

        let mut selectors = month_year.iter(&month_year_tree.children);
        let (month_layout, month_tree) = selectors.next()?;
        let (year_layout, year_tree) = selectors.next()?;

        let (days, days_tree) = days_parent.iter(&days_parent_tree.children).next()?;

        let mut rows = days.iter(&days_tree.children);
        let (labels_layout, labels_tree) = rows.next()?;

        Some(Geometry {
            overlay: overlay.bounds(),
            col,
            month: selector(month_layout, month_tree)?,
            year: selector(year_layout, year_tree)?,
            days: days.bounds(),
            labels: labels_layout
                .iter(&labels_tree.children)
                .map(|(layout, _)| layout.bounds())
                .collect(),
            cells: rows
                .map(|(row, tree)| {
                    row.iter(&tree.children)
                        .map(|(layout, _)| layout.bounds())
                        .collect()
                })
                .collect(),
            cancel,
            submit,
        })
    }

    /// The event handling for the month / year bar.
    fn on_event_month_year(
        &mut self,
        event: &Event,
        geometry: &Geometry,
        cursor: Cursor,
        shell: &mut Shell<'_, Message>,
    ) {
        // ----------- Month ----------------------
        let month = geometry.month;

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if cursor.is_over(month.whole) {
                    self.state.focus = Focus::Month;
                    shell.request_redraw();
                }

                if cursor.is_over(month.left) {
                    self.state.date = crate::core::date::pred_month(self.state.date);
                    shell.capture_event();
                    shell.request_redraw();
                } else if cursor.is_over(month.right) {
                    self.state.date = crate::core::date::succ_month(self.state.date);
                    shell.capture_event();
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if month.whole.contains(*position)
                    || month.left.contains(*position)
                    || month.right.contains(*position)
                {
                    self.state.day_mouse_over = None;
                    shell.request_redraw();
                }
            }
            _ => {}
        }

        // ----------- Year -----------------------
        let year = geometry.year;

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if cursor.is_over(year.whole) {
                    self.state.focus = Focus::Year;
                    shell.request_redraw();
                }

                if cursor.is_over(year.left) {
                    self.state.date = crate::core::date::pred_year(self.state.date);
                    shell.capture_event();
                    shell.request_redraw();
                } else if cursor.is_over(year.right) {
                    self.state.date = crate::core::date::succ_year(self.state.date);
                    shell.capture_event();
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if year.whole.contains(*position)
                    || year.left.contains(*position)
                    || year.right.contains(*position)
                {
                    self.state.day_mouse_over = None;
                    shell.request_redraw();
                }
            }
            _ => {}
        }
    }

    /// The event handling for the calendar days.
    fn on_event_days(
        &mut self,
        event: &Event,
        geometry: &Geometry,
        cursor: Cursor,
        shell: &mut Shell<'_, Message>,
    ) {
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if cursor.is_over(geometry.days) {
                    self.state.focus = Focus::Day;
                    shell.request_redraw();
                }

                'outer: for (y, row) in geometry.cells.iter().enumerate() {
                    for (x, bounds) in row.iter().enumerate() {
                        if cursor.is_over(*bounds) {
                            let (day, is_in_month) = crate::core::date::position_to_day(
                                x,
                                y,
                                self.state.date.year(),
                                self.state.date.month(),
                            );

                            self.state.date = match is_in_month {
                                IsInMonth::Previous => {
                                    crate::core::date::pred_month(self.state.date)
                                        .with_day(day as u32)
                                        .expect("Previous month with day should be valid")
                                }
                                IsInMonth::Same => self
                                    .state
                                    .date
                                    .with_day(day as u32)
                                    .expect("Same month with day should be valid"),
                                IsInMonth::Next => crate::core::date::succ_month(self.state.date)
                                    .with_day(day as u32)
                                    .expect("Succeeding month with day should be valid"),
                            };

                            shell.capture_event();
                            shell.request_redraw();
                            break 'outer;
                        }
                    }
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if cursor.is_over(geometry.days) {
                    self.state.focus = Focus::Day;
                    shell.request_redraw();
                }

                self.state.day_mouse_over = None;

                'outer: for (y, row) in geometry.cells.iter().enumerate() {
                    for (x, bounds) in row.iter().enumerate() {
                        if bounds.contains(*position) {
                            self.state.day_mouse_over = Some((y, x));
                            shell.capture_event();
                            shell.request_redraw();
                            break 'outer;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// The event handling for the keyboard input.
    fn on_event_keyboard(&mut self, event: &Event) -> event::Status {
        if self.state.focus == Focus::None {
            return event::Status::Ignored;
        }

        if let Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event {
            let mut status = event::Status::Ignored;

            match key.as_ref() {
                keyboard::Key::Named(keyboard::key::Named::Tab) => {
                    if self.state.keyboard_modifiers.shift() {
                        self.state.focus = self.state.focus.previous();
                    } else {
                        self.state.focus = self.state.focus.next();
                    }
                }
                keyboard::Key::Named(k) => match self.state.focus {
                    Focus::Month => match k {
                        keyboard::key::Named::ArrowLeft => {
                            self.state.date = crate::core::date::pred_month(self.state.date);
                            status = event::Status::Captured;
                        }
                        keyboard::key::Named::ArrowRight => {
                            self.state.date = crate::core::date::succ_month(self.state.date);
                            status = event::Status::Captured;
                        }
                        _ => {}
                    },
                    Focus::Year => match k {
                        keyboard::key::Named::ArrowLeft => {
                            self.state.date = crate::core::date::pred_year(self.state.date);
                            status = event::Status::Captured;
                        }
                        keyboard::key::Named::ArrowRight => {
                            self.state.date = crate::core::date::succ_year(self.state.date);
                            status = event::Status::Captured;
                        }
                        _ => {}
                    },
                    Focus::Day => match k {
                        keyboard::key::Named::ArrowLeft => {
                            self.state.date = crate::core::date::pred_day(self.state.date);
                            status = event::Status::Captured;
                        }
                        keyboard::key::Named::ArrowRight => {
                            self.state.date = crate::core::date::succ_day(self.state.date);
                            status = event::Status::Captured;
                        }
                        keyboard::key::Named::ArrowUp => {
                            self.state.date = crate::core::date::pred_week(self.state.date);
                            status = event::Status::Captured;
                        }
                        keyboard::key::Named::ArrowDown => {
                            self.state.date = crate::core::date::succ_week(self.state.date);
                            status = event::Status::Captured;
                        }
                        _ => {}
                    },
                    _ => {}
                },
                _ => {}
            }

            status
        } else if let Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) = event {
            self.state.keyboard_modifiers = *modifiers;
            event::Status::Ignored
        } else {
            event::Status::Ignored
        }
    }
}

impl<'a, 'b, Message, Theme> Overlay<Message, Theme, Renderer>
    for DatePickerOverlay<'a, 'b, Message, Theme>
where
    Message: 'static + Clone,
    Theme: 'a
        + crate::style::date_picker::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog
        + iced_widget::container::Catalog,
    'b: 'a,
{
    fn update(
        &mut self,
        event: &Event,
        cursor: Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        // Laying out is expensive: it is only done when needed, or on window events
        // (redraws, resizes) so that changes of the font size are picked up.
        if !self.is_laid_out() || matches!(event, Event::Window(_)) {
            self.layout(renderer);
        }

        if event::Status::Captured == self.on_event_keyboard(event) {
            shell.capture_event();
            shell.request_redraw();
            return;
        }

        let Some(geometry) = self.geometry() else {
            return;
        };

        // ----------- Year/Month----------------------
        self.on_event_month_year(event, &geometry, cursor, shell);

        // ----------- Days ----------------------
        self.on_event_days(event, &geometry, cursor, shell);

        // ----------- Buttons ------------------------
        let viewport = geometry.overlay;

        if cursor.is_over(geometry.cancel.bounds()) {
            self.state.day_mouse_over = None;
        }

        self.cancel_button.update(
            &mut self.tree.children[0],
            event,
            geometry.cancel,
            cursor,
            renderer,
            shell,
            &viewport,
        );

        if cursor.is_over(geometry.submit.bounds()) {
            self.state.day_mouse_over = None;
        }

        let mut fake_messages: shell::Bus<Message> = shell::Bus::new();

        {
            let mut fake_shell = shell.local(&mut fake_messages);

            self.submit_button.update(
                &mut self.tree.children[1],
                event,
                geometry.submit,
                cursor,
                renderer,
                &mut fake_shell,
                &viewport,
            );
        }

        if !fake_messages.is_empty() {
            shell.publish((self.on_submit)(self.state.date.into()));
            shell.capture_event();
            shell.request_redraw();
        }
    }

    fn mouse_interaction(&self, cursor: mouse::Cursor, renderer: &Renderer) -> mouse::Interaction {
        let mouse_interaction = mouse::Interaction::default();

        let Some(geometry) = self.geometry() else {
            return mouse_interaction;
        };

        // Month and year mouse interaction
        let f = |selector: &Selector| {
            if cursor.is_over(selector.left) || cursor.is_over(selector.right) {
                mouse::Interaction::Pointer
            } else {
                mouse::Interaction::default()
            }
        };

        let month_mouse_interaction = f(&geometry.month);
        let year_mouse_interaction = f(&geometry.year);

        // Days
        let mut table_mouse_interaction = mouse::Interaction::default();

        for row in &geometry.cells {
            for bounds in row {
                if cursor.is_over(*bounds) {
                    table_mouse_interaction =
                        table_mouse_interaction.max(mouse::Interaction::Pointer);
                }
            }
        }

        // Buttons
        let cancel_button_mouse_interaction = self.cancel_button.mouse_interaction(
            &self.tree.children[0],
            geometry.cancel,
            cursor,
            &self.viewport,
            renderer,
        );

        let submit_button_mouse_interaction = self.submit_button.mouse_interaction(
            &self.tree.children[1],
            geometry.submit,
            cursor,
            &self.viewport,
            renderer,
        );

        mouse_interaction
            .max(month_mouse_interaction)
            .max(year_mouse_interaction)
            .max(table_mouse_interaction)
            .max(cancel_button_mouse_interaction)
            .max(submit_button_mouse_interaction)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        cursor: Cursor,
    ) {
        let Some(geometry) = self.geometry() else {
            return;
        };

        let bounds = geometry.overlay;

        renderer.with_layer(bounds, |renderer| {
            let cursor_position = cursor.position().unwrap_or_default();
            let mut style_sheet: HashMap<StyleState, Style> = HashMap::new();
            let _ = style_sheet.insert(
                StyleState::Active,
                crate::style::date_picker::Catalog::style(theme, self.class, Status::Active),
            );
            let _ = style_sheet.insert(
                StyleState::Selected,
                crate::style::date_picker::Catalog::style(theme, self.class, Status::Selected),
            );
            let _ = style_sheet.insert(
                StyleState::Hovered,
                crate::style::date_picker::Catalog::style(theme, self.class, Status::Hovered),
            );
            let _ = style_sheet.insert(
                StyleState::Focused,
                crate::style::date_picker::Catalog::style(theme, self.class, Status::Focused),
            );

            let style_state = if self.state.focus == Focus::Overlay {
                StyleState::Focused
            } else if cursor.is_over(bounds) {
                StyleState::Hovered
            } else {
                StyleState::Active
            };

            // Background
            if (bounds.width > 0.) && (bounds.height > 0.) {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds,
                        border: Border {
                            radius: style_sheet[&style_state].border_radius.into(),
                            width: style_sheet[&style_state].border_width,
                            color: style_sheet[&style_state].border_color,
                            ..Border::default()
                        },
                        ..renderer::Quad::default()
                    },
                    style_sheet[&style_state].background,
                );
            }

            // ----------- Year/Month----------------------
            month_year(
                renderer,
                &geometry,
                &self.date_as_string(),
                cursor_position,
                &style_sheet,
                self.state.focus,
                self.font_size,
            );

            // ----------- Days ---------------------------
            days(
                renderer,
                &geometry,
                self.state.date,
                &style_sheet,
                self.state.focus,
                self.font_size,
                self.state.day_mouse_over,
            );

            // ----------- Buttons ------------------------
            self.cancel_button.draw(
                &self.tree.children[0],
                renderer,
                theme,
                style,
                geometry.cancel,
                cursor,
                &bounds,
            );

            self.submit_button.draw(
                &self.tree.children[1],
                renderer,
                theme,
                style,
                geometry.submit,
                cursor,
                &bounds,
            );

            let border = Border {
                radius: style_sheet[&StyleState::Focused].border_radius.into(),
                width: style_sheet[&StyleState::Focused].border_width,
                color: style_sheet[&StyleState::Focused].border_color,
                ..Border::default()
            };

            // Buttons are not focusable right now...
            if self.state.focus == Focus::Cancel {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: geometry.cancel.bounds(),
                        border,
                        ..renderer::Quad::default()
                    },
                    Color::TRANSPARENT,
                );
            }

            if self.state.focus == Focus::Submit {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: geometry.submit.bounds(),
                        border,
                        ..renderer::Quad::default()
                    },
                    Color::TRANSPARENT,
                );
            }
        });
    }

    fn operate(&mut self, renderer: &Renderer, operation: &mut dyn widget::Operation) {
        if !self.is_laid_out() {
            self.layout(renderer);
        }

        let Some(geometry) = self.geometry() else {
            return;
        };

        // Operate on the calendar column element stored in tree.children[2].
        // This recursively exposes all text: month, year, arrows, day labels, day numbers.
        // The element is recreated here with the same structure than the one
        // created in `layout()`, but with the actual content.
        let date_string = crate::core::date::date_as_string(self.state.date);
        let (year, month) = date_string.split_once(' ').unwrap_or(("", ""));
        let date = self.state.date;

        let mut col = calendar::<Message, Theme>(self.font_size, month, year, |x, y| {
            let (day, _) = crate::core::date::position_to_day(x, y, date.year(), date.month());

            format!("{day:02}")
        });

        col.operate(
            &mut self.tree.children[2],
            geometry.col,
            &self.viewport,
            renderer,
            operation,
        );

        // Operate on cancel button
        Widget::operate(
            &mut self.cancel_button,
            &mut self.tree.children[0],
            geometry.cancel,
            &self.viewport,
            renderer,
            operation,
        );

        // Operate on submit button
        Widget::operate(
            &mut self.submit_button,
            &mut self.tree.children[1],
            geometry.submit,
            &self.viewport,
            renderer,
            operation,
        );
    }
}

/// The state of the [`DatePickerOverlay`].
#[derive(Debug)]
pub struct State {
    /// The selected date of the [`DatePickerOverlay`].
    pub(crate) date: NaiveDate,
    /// The focus of the [`DatePickerOverlay`].
    pub(crate) focus: Focus,
    /// The previously pressed keyboard modifiers.
    pub(crate) keyboard_modifiers: keyboard::Modifiers,
    /// ID of the day the mouse is over.
    pub(crate) day_mouse_over: Option<(usize, usize)>,
}

impl State {
    /// Creates a new State with the given date.
    #[must_use]
    pub fn new(date: NaiveDate) -> Self {
        Self {
            date,
            ..Self::default()
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            date: Local::now().naive_local().date(),
            focus: Focus::default(),
            keyboard_modifiers: keyboard::Modifiers::default(),
            day_mouse_over: None,
        }
    }
}

/// Just a workaround to pass the button states from the tree to the overlay
#[allow(missing_debug_implementations)]
pub struct DatePickerOverlayButtons<'a, Message, Theme>
where
    Message: Clone,
    Theme: crate::style::date_picker::Catalog + iced_widget::button::Catalog,
{
    /// The cancel button of the [`DatePickerOverlay`].
    cancel_button: Element<'a, Message, Theme, Renderer>,
    /// The submit button of the [`DatePickerOverlay`].
    submit_button: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme> Default for DatePickerOverlayButtons<'a, Message, Theme>
where
    Message: 'a + Clone,
    Theme: 'a
        + crate::style::date_picker::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog
        + iced_widget::container::Catalog,
{
    fn default() -> Self {
        let (cancel_content, cancel_font, _cancel_shaping) = cancel();
        let (submit_content, submit_font, _submit_shaping) = ok();

        Self {
            cancel_button: boxed(Button::new(
                widget::Text::new(cancel_content)
                    .font(cancel_font)
                    .align_x(Horizontal::Center)
                    .width(Length::Fill),
            )),
            submit_button: boxed(Button::new(
                widget::Text::new(submit_content)
                    .font(submit_font)
                    .align_x(Horizontal::Center)
                    .width(Length::Fill),
            )),
        }
    }
}

impl<Message, Theme> Meta for DatePickerOverlayButtons<'_, Message, Theme>
where
    Message: Clone,
    Theme: crate::style::date_picker::Catalog + iced_widget::button::Catalog,
{
}

#[allow(clippy::unimplemented)]
impl<Message, Theme> Widget<Message, Theme, Renderer>
    for DatePickerOverlayButtons<'_, Message, Theme>
where
    Message: Clone,
    Theme: crate::style::date_picker::Catalog
        + iced_widget::button::Catalog
        + iced_widget::container::Catalog,
{
    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children_custom(
            &mut [&mut self.cancel_button, &mut self.submit_button],
            |tree, element| tree.diff(&mut **element),
            |element| Tree::new(&**element),
        );
    }

    fn size(&self) -> Size<Length> {
        unimplemented!("This should never be reached!")
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, _limits: &Limits) {
        unimplemented!("This should never be reached!")
    }

    fn draw(
        &self,
        _state: &Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout,
        _cursor: Cursor,
        _viewport: &Rectangle,
    ) {
        unimplemented!("This should never be reached!")
    }
}

impl<'a, Message, Theme> From<DatePickerOverlayButtons<'a, Message, Theme>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a
        + crate::style::date_picker::Catalog
        + iced_widget::button::Catalog
        + iced_widget::container::Catalog,
{
    fn from(overlay: DatePickerOverlayButtons<'a, Message, Theme>) -> Self {
        boxed(overlay)
    }
}

/// Wraps a [`Widget`] into an [`Element`].
fn boxed<'a, Message, Theme>(
    widget: impl Widget<Message, Theme, Renderer> + 'a,
) -> Element<'a, Message, Theme, Renderer> {
    Widget::<Message, Theme, Renderer>::_boxed(widget)
}

/// Reconciles the child tree at the given index with the given widget.
fn sync_child<Message, Theme>(
    tree: &mut Tree,
    index: usize,
    widget: &mut impl Widget<Message, Theme, Renderer>,
) {
    if let Some(child) = tree.children.get_mut(index) {
        child.diff(widget);
    } else {
        let mut child = Tree::new(&*widget);
        child.diff(widget);
        tree.children.push(child);
    }
}

/// Builds the calendar column: the month/year selectors and the table of days.
///
/// `day` produces the content of the day cell at the given `(x, y)` position.
fn calendar<'a, Message, Theme>(
    font_size: Pixels,
    month: &str,
    year: &str,
    day: impl Fn(usize, usize) -> String,
) -> Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a + iced_widget::text::Catalog + iced_widget::container::Catalog,
{
    let (left_content, left_font, _left_shaping) = left_open();
    let (right_content, right_font, _right_shaping) = right_open();

    let arrow = |content: &str, font: Font| -> Element<'a, Message, Theme, Renderer> {
        boxed(
            Container::new(
                widget::Text::new(content.to_owned())
                    .size(font_size.0 + 1.0)
                    .font(font),
            )
            .height(Length::Fit)
            .width(Length::Fit),
        )
    };

    let selector = |text: &str| -> Element<'a, Message, Theme, Renderer> {
        boxed(
            Row::<Element<'a, Message, Theme, Renderer>>::new()
                .width(Length::Fit)
                .spacing(SPACING)
                .align_y(Vertical::Center)
                // Left arrow
                .push(arrow(&left_content, left_font))
                // Text
                .push(boxed(
                    widget::Text::new(text.to_owned())
                        .size(font_size)
                        .width(Length::Fit),
                ))
                // Right arrow
                .push(arrow(&right_content, right_font)),
        )
    };

    let month_year = boxed(
        Row::<Element<'a, Message, Theme, Renderer>>::new()
            .width(Length::Fit)
            .spacing(SPACING)
            .push(selector(month))
            .push(selector(year)),
    );

    let days = (0..7).fold(
        Column::<Element<'a, Message, Theme, Renderer>>::new()
            .width(Length::Fit)
            .height(Length::Fit),
        |column, y| {
            column.push(boxed(
                (0..7).fold(
                    Row::<Element<'a, Message, Theme, Renderer>>::new()
                        .height(Length::Fit)
                        .width(Length::Fit)
                        .spacing(SPACING),
                    |row, x| {
                        row.push(boxed(
                            Container::new(
                                Row::<Element<'a, Message, Theme, Renderer>>::new()
                                    .push(boxed(widget::Text::new(day(x, y)).size(font_size))),
                            )
                            .width(Length::Fit)
                            .height(Length::Fit)
                            .padding(DAY_CELL_PADDING),
                        ))
                    },
                ),
            ))
        },
    );

    let days = boxed(
        Container::new(days)
            .width(Length::Fit)
            .height(Length::Fit)
            .center_y(Length::Fit),
    );

    boxed(
        Column::<Element<'a, Message, Theme, Renderer>>::new()
            .spacing(SPACING)
            .align_x(Horizontal::Center)
            .push(month_year)
            .push(days),
    )
}

/// An enumeration of all focusable elements of the [`DatePickerOverlay`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Focus {
    /// Nothing is in focus.
    #[default]
    None,

    /// The overlay itself is in focus.
    Overlay,

    /// The month area is in focus.
    Month,

    /// The year area is in focus.
    Year,

    /// The day area is in focus.
    Day,

    /// The cancel button is in focus.
    Cancel,

    /// The submit button is in focus.
    Submit,
}

impl Focus {
    /// Gets the next focusable element.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Overlay => Self::Month,
            Self::Month => Self::Year,
            Self::Year => Self::Day,
            Self::Day => Self::Cancel,
            Self::Cancel => Self::Submit,
            Self::Submit | Self::None => Self::Overlay,
        }
    }

    /// Gets the previous focusable element.
    #[must_use]
    pub const fn previous(self) -> Self {
        match self {
            Self::None => Self::None,
            Self::Overlay => Self::Submit,
            Self::Month => Self::Overlay,
            Self::Year => Self::Month,
            Self::Day => Self::Year,
            Self::Cancel => Self::Day,
            Self::Submit => Self::Cancel,
        }
    }
}

/// Fills the given text centered in the given bounds
fn fill_centered_text(
    renderer: &mut Renderer,
    content: String,
    bounds: Rectangle,
    size: Pixels,
    font: Font,
    shaping: text::Shaping,
    color: Color,
) {
    let hint_factor = renderer.hint_factor();

    renderer.fill_text(
        iced_core::Text {
            content,
            bounds: bounds.size(),
            size,
            line_height: text::LineHeight::Relative(1.3),
            font,
            align_x: text::Alignment::Center,
            align_y: Vertical::Center,
            shaping,
            wrapping: text::Wrapping::default(),
            ellipsis: text::Ellipsis::None,
            hint_factor,
        },
        Point::new(bounds.center_x(), bounds.center_y()),
        color,
        bounds,
    );
}

/// Draws the month/year row
fn month_year(
    renderer: &mut Renderer,
    geometry: &Geometry,
    date: &str,
    cursor: Point,
    //style: &Style,
    style: &HashMap<StyleState, Style>,
    focus: Focus,
    font_size: Pixels,
) {
    let mut f = |selector: &Selector, text: &str, target: Focus, font_size: Pixels| {
        let style_state = if focus == target {
            StyleState::Focused
        } else {
            StyleState::Active
        };

        let left_bounds = selector.left;
        let center_bounds = selector.center;
        let right_bounds = selector.right;

        let left_arrow_hovered = left_bounds.contains(cursor);
        let right_arrow_hovered = right_bounds.contains(cursor);

        let bounds = selector.whole;
        if (style_state == StyleState::Focused) && (bounds.width > 0.) && (bounds.height > 0.) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        radius: style
                            .get(&style_state)
                            .expect("Style Sheet not found.")
                            .border_radius
                            .into(),
                        width: style
                            .get(&style_state)
                            .expect("Style Sheet not found.")
                            .border_width,
                        color: style
                            .get(&style_state)
                            .expect("Style Sheet not found.")
                            .border_color,
                        ..Border::default()
                    },
                    shadow: Shadow::default(),
                    snap: false,
                },
                style
                    .get(&style_state)
                    .expect("Style Sheet not found.")
                    .background,
            );
        }

        let text_color = style
            .get(&style_state)
            .expect("Style Sheet not found.")
            .text_color;

        let (left_content, left_font, _left_shaping) = left_open();
        let (right_content, right_font, _right_shaping) = right_open();

        // Left caret
        fill_centered_text(
            renderer,
            left_content,
            left_bounds,
            Pixels(font_size.0 + if left_arrow_hovered { 1.0 } else { 0.0 }),
            left_font,
            text::Shaping::Advanced,
            text_color,
        );

        // Text
        let default_font = renderer.font();

        fill_centered_text(
            renderer,
            text.to_owned(),
            center_bounds,
            font_size,
            default_font,
            text::Shaping::Basic,
            text_color,
        );

        // Right caret
        fill_centered_text(
            renderer,
            right_content,
            right_bounds,
            Pixels(font_size.0 + if right_arrow_hovered { 1.0 } else { 0.0 }),
            right_font,
            text::Shaping::Advanced,
            text_color,
        );
    };

    let (year, month) = date.split_once(' ').expect("Date must contain space");

    // Draw month
    f(&geometry.month, month, Focus::Month, font_size);

    // Draw year
    f(&geometry.year, year, Focus::Year, font_size);
}

/// Draws the days
#[allow(clippy::too_many_arguments)]
fn days(
    renderer: &mut Renderer,
    geometry: &Geometry,
    date: chrono::NaiveDate,
    style: &HashMap<StyleState, Style>,
    focus: Focus,
    font_size: Pixels,
    mouse_over: Option<(usize, usize)>,
) {
    day_labels(renderer, &geometry.labels, style, focus, font_size);

    day_table(
        renderer,
        &geometry.cells,
        date,
        style,
        focus,
        font_size,
        mouse_over,
    );
}

/// Draws the day labels
fn day_labels(
    renderer: &mut Renderer,
    labels: &[Rectangle],
    style: &HashMap<StyleState, Style>,
    _focus: Focus,
    font_size: Pixels,
) {
    for (i, bounds) in labels.iter().enumerate() {
        let default_font = renderer.font();

        fill_centered_text(
            renderer,
            crate::core::date::WEEKDAY_LABELS[i].clone(),
            *bounds,
            font_size,
            default_font,
            text::Shaping::Basic,
            style
                .get(&StyleState::Active)
                .expect("Style Sheet not found.")
                .text_color,
        );
    }
}

#[allow(clippy::too_many_arguments)]
/// Draws the day table
fn day_table(
    renderer: &mut Renderer,
    rows: &[Vec<Rectangle>],
    date: chrono::NaiveDate,
    style: &HashMap<StyleState, Style>,
    focus: Focus,
    font_size: Pixels,
    day_mouse_over: Option<(usize, usize)>,
) {
    for (y, row) in rows.iter().enumerate() {
        for (x, bounds) in row.iter().enumerate() {
            let bounds = *bounds;
            let (number, is_in_month) =
                crate::core::date::position_to_day(x, y, date.year(), date.month());

            let mouse_over = if let Some((day_y, day_x)) = day_mouse_over {
                day_y == y && day_x == x
            } else {
                false
            };

            let selected = date.day() == number as u32 && is_in_month == IsInMonth::Same;

            let style_state = if selected {
                StyleState::Selected
            } else if mouse_over {
                StyleState::Hovered
            } else {
                StyleState::Active
            };

            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        radius: (bounds.height / 2.0).into(),
                        width: 0.0,
                        color: Color::TRANSPARENT,
                        ..Border::default()
                    },
                    shadow: Shadow::default(),
                    snap: false,
                },
                style
                    .get(&style_state)
                    .expect("Style Sheet not found.")
                    .day_background,
            );

            if focus == Focus::Day && selected {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds,
                        border: Border {
                            radius: style
                                .get(&StyleState::Focused)
                                .expect("Style Sheet not found.")
                                .border_radius
                                .into(),
                            width: style
                                .get(&StyleState::Focused)
                                .expect("Style Sheet not found.")
                                .border_width,
                            color: style
                                .get(&StyleState::Focused)
                                .expect("Style Sheet not found.")
                                .border_color,
                            ..Border::default()
                        },
                        shadow: Shadow::default(),
                        snap: false,
                    },
                    Color::TRANSPARENT,
                );
            }

            let default_font = renderer.font();

            fill_centered_text(
                renderer,
                format!("{number:02}"), // Todo: is there some way of static format as this has a fixed size?
                bounds,
                font_size,
                default_font,
                text::Shaping::Basic,
                if is_in_month == IsInMonth::Same {
                    style
                        .get(&style_state)
                        .expect("Style Sheet not found.")
                        .text_color
                } else {
                    style
                        .get(&style_state)
                        .expect("Style Sheet not found.")
                        .text_attenuated_color
                },
            );
        }
    }
}
