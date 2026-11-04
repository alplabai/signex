//! Use a time picker as an input element for picking times.
//!
//! *This API requires the following crate features to be activated: `time_picker`*

use crate::iced_aw_font::advanced_text::{cancel, down_open, ok, up_open};
use crate::{
    core::clock::{
        HOUR_RADIUS_PERCENTAGE, HOUR_RADIUS_PERCENTAGE_NO_SECONDS, MINUTE_RADIUS_PERCENTAGE,
        MINUTE_RADIUS_PERCENTAGE_NO_SECONDS, NearestRadius, PERIOD_PERCENTAGE,
        SECOND_RADIUS_PERCENTAGE,
    },
    core::{clock, overlay::Position, time::Period},
    style::{
        Status,
        style_state::StyleState,
        time_picker::{Catalog, Style},
    },
    time_picker::{self, Time},
};
use chrono::{Duration, Local, NaiveTime, Timelike};
use iced_core::{
    Border, Color, Element, Event, Layout, Length, Overlay, Padding, Pixels, Point, Rectangle,
    Renderer as _, Shell, Size, Text, Vector, Widget,
    alignment::{Horizontal, Vertical},
    event, keyboard,
    layout::Limits,
    mouse::{self, Cursor},
    overlay, renderer, shell,
    text::Renderer as _,
    touch,
    widget::{self, Meta, tree::Tree},
};
use iced_widget::{
    Button, Column, Container, Renderer, Row, button,
    canvas::{self, LineCap, Path, Stroke, Text as CanvasText},
    container,
    graphics::geometry::Renderer as _,
    text::{self, Wrapping},
};
use std::collections::HashMap;

/// The padding around the elements.
const PADDING: Padding = Padding::new(10.0);
/// The spacing between the elements.
const SPACING: Pixels = Pixels(15.0);
/// The spacing between the buttons.
const BUTTON_SPACING: Pixels = Pixels(5.0);
/// The percentage size of the numbers.
const NUMBER_SIZE_PERCENTAGE: f32 = 0.15;
/// The percentage size of the period.
const PERIOD_SIZE_PERCENTAGE: f32 = 0.2;

/// The overlay of the [`TimePicker`](crate::widget::TimePicker).
#[allow(missing_debug_implementations)]
pub struct TimePickerOverlay<'a, 'b, Message, Theme>
where
    Message: Clone,
    Theme: Catalog + button::Catalog + text::Catalog,
    'b: 'a,
{
    /// The state of the [`TimePickerOverlay`].
    state: &'a mut State,
    /// The cancel button of the [`TimePickerOverlay`].
    cancel_button: Button<'a, Message, widget::Text<'a, Theme>, Theme>,
    /// The submit button of the [`TimePickerOverlay`].
    submit_button: Button<'a, Message, widget::Text<'a, Theme>, Theme>,
    /// The function that produces a message when the submit button of the [`TimePickerOverlay`] is pressed.
    on_submit: &'a dyn Fn(Time) -> Message,
    /// The style of the [`TimePickerOverlay`].
    class: &'a <Theme as Catalog>::Class<'b>,
    /// The reference to the tree holding the state of this overlay.
    tree: &'a mut Tree,
    /// The viewport of the [`TimePickerOverlay`].
    viewport: Rectangle,
    /// The size of the window the overlay is displayed in.
    window: Size,
    /// The computed layout of all the elements of the [`TimePickerOverlay`].
    geometry: Geometry,
}

/// The layout of a single column (up arrow, value and down arrow) of the
/// digital clock.
#[derive(Debug, Clone, Copy, Default)]
struct DigitalColumn {
    /// The bounds of the whole column.
    bounds: Rectangle,
    /// The bounds of the up arrow.
    up: Rectangle,
    /// The bounds of the value.
    center: Rectangle,
    /// The bounds of the down arrow.
    down: Rectangle,
}

/// The absolute layout of all the elements of the [`TimePickerOverlay`].
#[derive(Debug, Clone, Copy)]
struct Geometry {
    /// The layout of the whole overlay.
    overlay: Layout,
    /// The layout of the analog clock.
    clock: Layout,
    /// The layout of the container of the digital clock.
    digital: Layout,
    /// The hour column of the digital clock.
    hour: DigitalColumn,
    /// The bounds of the separator between hours and minutes.
    hour_minute_separator: Rectangle,
    /// The minute column of the digital clock.
    minute: DigitalColumn,
    /// The bounds of the separator between minutes and seconds.
    minute_second_separator: Option<Rectangle>,
    /// The second column of the digital clock.
    second: Option<DigitalColumn>,
    /// The bounds of the period (AM/PM) of the digital clock.
    period: Option<Rectangle>,
    /// The layout of the cancel button.
    cancel: Layout,
    /// The layout of the submit button.
    submit: Layout,
}

impl<'a, 'b, Message, Theme> TimePickerOverlay<'a, 'b, Message, Theme>
where
    Message: 'static + Clone,
    Theme: 'a + Catalog + button::Catalog + text::Catalog + container::Catalog,
    'b: 'a,
{
    /// Creates a new [`TimePickerOverlay`] on the given position.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        state: &'a mut time_picker::State,
        on_cancel: Message,
        on_submit: &'a dyn Fn(Time) -> Message,
        position: Point,
        class: &'a <Theme as Catalog>::Class<'b>,
        tree: &'a mut Tree,
        viewport: Rectangle,
        renderer: &Renderer,
        window: Size,
    ) -> Self {
        let time_picker::State { overlay_state } = state;
        let (cancel_content, cancel_font, _cancel_shaping) = cancel();
        let (submit_content, submit_font, _submit_shaping) = ok();

        let mut cancel_button = Button::new(
            widget::Text::new(cancel_content)
                .font(cancel_font)
                .align_x(Horizontal::Center)
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .on_press(on_cancel.clone());
        let mut submit_button = Button::new(
            widget::Text::new(submit_content)
                .font(submit_font)
                .align_x(Horizontal::Center)
                .width(Length::Fill),
        )
        .width(Length::Fill)
        .on_press(on_cancel); // Sending a fake message

        // The layout is not part of the `Overlay` trait anymore, so it is
        // computed once, when the overlay is created.
        let geometry = compute_geometry(
            overlay_state,
            &mut cancel_button,
            &mut submit_button,
            tree,
            position,
            renderer,
            window,
        );

        TimePickerOverlay {
            state: overlay_state,
            cancel_button,
            submit_button,
            on_submit,
            class,
            tree,
            viewport,
            window,
            geometry,
        }
    }

    /// Turn this [`TimePickerOverlay`] into an overlay [`Element`](overlay::Element).
    #[must_use]
    pub fn overlay(self) -> overlay::Element<'a, Message, Theme, Renderer> {
        overlay::Element::new(Box::new(self))
    }

    /// The event handling for the clock.
    #[allow(clippy::too_many_lines)]
    fn on_event_clock(&mut self, event: &Event, layout: Layout, cursor: Cursor) -> event::Status {
        if cursor.is_over(layout.bounds()) {
            self.state.clock_cache_needs_clearance = true;
            self.state.clock_cache.clear();
        } else if self.state.clock_cache_needs_clearance {
            self.state.clock_cache.clear();
            self.state.clock_cache_needs_clearance = false;
        }

        let clock_bounds = layout.bounds();
        if cursor.is_over(clock_bounds) {
            let center = clock_bounds.center();
            let radius = clock_bounds.width.min(clock_bounds.height) * 0.5;

            let period_radius = radius * PERIOD_PERCENTAGE;

            let (hour_radius, minute_radius, second_radius) = if self.state.show_seconds {
                (
                    radius * HOUR_RADIUS_PERCENTAGE,
                    radius * MINUTE_RADIUS_PERCENTAGE,
                    radius * SECOND_RADIUS_PERCENTAGE,
                )
            } else {
                (
                    radius * HOUR_RADIUS_PERCENTAGE_NO_SECONDS,
                    radius * MINUTE_RADIUS_PERCENTAGE_NO_SECONDS,
                    f32::MAX,
                )
            };

            let nearest_radius = crate::core::clock::nearest_radius(
                &if self.state.show_seconds {
                    vec![
                        (period_radius, NearestRadius::Period),
                        (hour_radius, NearestRadius::Hour),
                        (minute_radius, NearestRadius::Minute),
                        (second_radius, NearestRadius::Second),
                    ]
                } else {
                    vec![
                        (period_radius, NearestRadius::Period),
                        (hour_radius, NearestRadius::Hour),
                        (minute_radius, NearestRadius::Minute),
                    ]
                },
                cursor.position().unwrap_or_default(),
                center,
            );

            let clock_clicked_status = match event {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                | Event::Touch(touch::Event::FingerPressed { .. }) => match nearest_radius {
                    NearestRadius::Period => {
                        let (pm, hour) = self.state.time.hour12();
                        let hour = if hour == 12 {
                            if pm { 12 } else { 0 }
                        } else {
                            hour
                        };

                        self.state.time = self
                            .state
                            .time
                            .with_hour(if pm && hour != 12 { hour } else { hour + 12 } % 24)
                            .expect("New time with hour should be valid");
                        event::Status::Captured
                    }
                    NearestRadius::Hour => {
                        self.state.focus = Focus::DigitalHour;
                        self.state.clock_dragged = ClockDragged::Hour;
                        event::Status::Captured
                    }
                    NearestRadius::Minute => {
                        self.state.focus = Focus::DigitalMinute;
                        self.state.clock_dragged = ClockDragged::Minute;
                        event::Status::Captured
                    }
                    NearestRadius::Second => {
                        self.state.focus = Focus::DigitalSecond;
                        self.state.clock_dragged = ClockDragged::Second;
                        event::Status::Captured
                    }
                    NearestRadius::None => event::Status::Ignored,
                },
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                | Event::Touch(
                    touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. },
                ) => {
                    self.state.clock_dragged = ClockDragged::None;
                    event::Status::Captured
                }
                _ => event::Status::Ignored,
            };

            let clock_dragged_status = match self.state.clock_dragged {
                ClockDragged::Hour => {
                    let hour_points = crate::core::clock::circle_points(hour_radius, center, 12);
                    let nearest_point = crate::core::clock::nearest_point(
                        &hour_points,
                        cursor.position().unwrap_or_default(),
                    );

                    let (pm, _) = self.state.time.hour12();

                    self.state.time = self
                        .state
                        .time
                        .with_hour((nearest_point as u32 + if pm { 12 } else { 0 }) % 24)
                        .expect("New time with hour should be valid");
                    event::Status::Captured
                }
                ClockDragged::Minute => {
                    let minute_points =
                        crate::core::clock::circle_points(minute_radius, center, 60);
                    let nearest_point = crate::core::clock::nearest_point(
                        &minute_points,
                        cursor.position().unwrap_or_default(),
                    );

                    self.state.time = self
                        .state
                        .time
                        .with_minute(nearest_point as u32)
                        .expect("New time with minute should be valid");
                    event::Status::Captured
                }
                ClockDragged::Second => {
                    let second_points =
                        crate::core::clock::circle_points(second_radius, center, 60);
                    let nearest_point = crate::core::clock::nearest_point(
                        &second_points,
                        cursor.position().unwrap_or_default(),
                    );

                    self.state.time = self
                        .state
                        .time
                        .with_second(nearest_point as u32)
                        .expect("New time with second should be valid");
                    event::Status::Captured
                }
                ClockDragged::None => event::Status::Ignored,
            };

            clock_clicked_status.merge(clock_dragged_status)
        } else {
            match event {
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                | Event::Touch(
                    touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. },
                ) => {
                    self.state.clock_dragged = ClockDragged::None;
                    event::Status::Captured
                }
                _ => event::Status::Ignored,
            }
        }
    }

    /// The event handling for the digital clock.
    fn on_event_digital_clock(&mut self, event: &Event, cursor: Cursor) -> event::Status {
        let calculate_time = |time: &mut NaiveTime,
                              up_arrow: Rectangle,
                              down_arrow: Rectangle,
                              duration: Duration| {
            if cursor.is_over(up_arrow) {
                *time += duration;
                event::Status::Captured
            } else if cursor.is_over(down_arrow) {
                *time -= duration;
                event::Status::Captured
            } else {
                event::Status::Ignored
            }
        };

        let hour = self.geometry.hour;
        let minute = self.geometry.minute;

        let digital_clock_status = match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if cursor.is_over(hour.bounds) {
                    self.state.focus = Focus::DigitalHour;

                    calculate_time(&mut self.state.time, hour.up, hour.down, Duration::hours(1))
                } else if cursor.is_over(minute.bounds) {
                    self.state.focus = Focus::DigitalMinute;

                    calculate_time(
                        &mut self.state.time,
                        minute.up,
                        minute.down,
                        Duration::minutes(1),
                    )
                } else {
                    event::Status::Ignored
                }
            }
            _ => event::Status::Ignored,
        };

        let second_status = if let Some(second) = self.geometry.second {
            match event {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                | Event::Touch(touch::Event::FingerPressed { .. }) => {
                    if cursor.is_over(second.bounds) {
                        self.state.focus = Focus::DigitalSecond;

                        calculate_time(
                            &mut self.state.time,
                            second.up,
                            second.down,
                            Duration::seconds(1),
                        )
                    } else {
                        event::Status::Ignored
                    }
                }
                _ => event::Status::Ignored,
            }
        } else {
            event::Status::Ignored
        };

        let digital_clock_status = digital_clock_status.merge(second_status);

        if digital_clock_status == event::Status::Captured {
            self.state.clock_cache.clear();
        }

        digital_clock_status
    }

    /// The event handling for the keyboard input.
    fn on_event_keyboard(&mut self, event: &Event) -> event::Status {
        if self.state.focus == Focus::None {
            return event::Status::Ignored;
        }

        if let Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event {
            let mut status = event::Status::Ignored;

            if matches!(key, keyboard::Key::Named(keyboard::key::Named::Tab)) {
                if self.state.keyboard_modifiers.shift() {
                    self.state.focus = self.state.focus.previous(self.state.show_seconds);
                } else {
                    self.state.focus = self.state.focus.next(self.state.show_seconds);
                }
            } else {
                let mut keyboard_handle =
                    |key_code: &keyboard::Key, time: &mut NaiveTime, duration: Duration| {
                        match key_code {
                            keyboard::Key::Named(
                                keyboard::key::Named::ArrowLeft | keyboard::key::Named::ArrowDown,
                            ) => {
                                *time -= duration;
                                status = event::Status::Captured;
                            }
                            keyboard::Key::Named(
                                keyboard::key::Named::ArrowRight | keyboard::key::Named::ArrowUp,
                            ) => {
                                *time += duration;
                                status = event::Status::Captured;
                            }
                            _ => {}
                        }
                    };

                match self.state.focus {
                    Focus::DigitalHour => {
                        keyboard_handle(key, &mut self.state.time, Duration::hours(1));
                    }
                    Focus::DigitalMinute => {
                        keyboard_handle(key, &mut self.state.time, Duration::minutes(1));
                    }
                    Focus::DigitalSecond => {
                        keyboard_handle(key, &mut self.state.time, Duration::seconds(1));
                    }
                    _ => {}
                }
            }

            if status == event::Status::Captured {
                self.state.clock_cache.clear();
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
    for TimePickerOverlay<'a, 'b, Message, Theme>
where
    Message: 'static + Clone,
    Theme: 'a + Catalog + button::Catalog + text::Catalog + container::Catalog,
    'b: 'a,
{
    fn update(
        &mut self,
        event: &Event,
        cursor: Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        let mut status = self.on_event_keyboard(event);

        // Clock canvas
        let clock_status = self.on_event_clock(event, self.geometry.clock, cursor);

        // ----------- Digital clock ------------------
        let digital_clock_status = self.on_event_digital_clock(event, cursor);

        if digital_clock_status == event::Status::Captured
            || clock_status == event::Status::Captured
        {
            status = event::Status::Captured;
        }

        // ----------- Buttons --------------
        let viewport = self.geometry.overlay.bounds();

        let mut fake_messages: shell::Bus<Message> = shell::Bus::new();
        {
            let mut fake_shell = shell.local(&mut fake_messages);

            self.cancel_button.update(
                &mut self.tree.children[0],
                event,
                self.geometry.cancel,
                cursor,
                renderer,
                &mut fake_shell,
                &viewport,
            );
        }

        for (message, _receipt) in fake_messages.drain() {
            shell.publish(message);
            status = event::Status::Captured;
        }

        {
            let mut fake_shell = shell.local(&mut fake_messages);

            self.submit_button.update(
                &mut self.tree.children[1],
                event,
                self.geometry.submit,
                cursor,
                renderer,
                &mut fake_shell,
                &viewport,
            );
        }

        if !fake_messages.is_empty() {
            let (hour, period) = if self.state.use_24h {
                (self.state.time.hour(), Period::H24)
            } else {
                let (period, hour) = self.state.time.hour12();
                (hour, if period { Period::Pm } else { Period::Am })
            };

            let time = if self.state.show_seconds {
                Time::Hms {
                    hour,
                    minute: self.state.time.minute(),
                    second: self.state.time.second(),
                    period,
                }
            } else {
                Time::Hm {
                    hour,
                    minute: self.state.time.minute(),
                    period,
                }
            };

            shell.publish((self.on_submit)(time));
            status = event::Status::Captured;
        }

        if status == event::Status::Captured {
            shell.capture_event();
            shell.request_redraw();
        }
    }

    fn mouse_interaction(&self, cursor: mouse::Cursor, renderer: &Renderer) -> mouse::Interaction {
        let mouse_interaction = mouse::Interaction::default();

        // Clock canvas
        let clock_mouse_interaction = if cursor.is_over(self.geometry.clock.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        };

        // Digital clock
        let f = |column: DigitalColumn| {
            let mut mouse_interaction = mouse::Interaction::default();

            let up_arrow_hovered = cursor.is_over(column.up);
            let down_arrow_hovered = cursor.is_over(column.down);

            if up_arrow_hovered || down_arrow_hovered {
                mouse_interaction = mouse_interaction.max(mouse::Interaction::Pointer);
            }

            mouse_interaction
        };

        let hour_mouse_interaction = f(self.geometry.hour);
        let minute_mouse_interaction = f(self.geometry.minute);
        let second_mouse_interaction = self
            .geometry
            .second
            .map_or_else(mouse::Interaction::default, f);

        // Buttons
        let cancel_mouse_interaction = self.cancel_button.mouse_interaction(
            &self.tree.children[0],
            self.geometry.cancel,
            cursor,
            &self.viewport,
            renderer,
        );

        let submit_mouse_interaction = self.submit_button.mouse_interaction(
            &self.tree.children[1],
            self.geometry.submit,
            cursor,
            &self.viewport,
            renderer,
        );

        mouse_interaction
            .max(clock_mouse_interaction)
            .max(hour_mouse_interaction)
            .max(minute_mouse_interaction)
            .max(second_mouse_interaction)
            .max(cancel_mouse_interaction)
            .max(submit_mouse_interaction)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        cursor: Cursor,
    ) {
        let bounds = self.geometry.overlay.bounds();

        let mut style_sheet: HashMap<StyleState, Style> = HashMap::new();
        let _ = style_sheet.insert(
            StyleState::Active,
            Catalog::style(theme, self.class, Status::Active),
        );
        let _ = style_sheet.insert(
            StyleState::Selected,
            Catalog::style(theme, self.class, Status::Selected),
        );
        let _ = style_sheet.insert(
            StyleState::Hovered,
            Catalog::style(theme, self.class, Status::Hovered),
        );
        let _ = style_sheet.insert(
            StyleState::Focused,
            Catalog::style(theme, self.class, Status::Focused),
        );

        let mut style_state = StyleState::Active;
        if self.state.focus == Focus::Overlay {
            style_state = style_state.max(StyleState::Focused);
        }
        if cursor.is_over(bounds) {
            style_state = style_state.max(StyleState::Hovered);
        }

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

        // ----------- Clock canvas --------------------
        draw_clock(renderer, self, self.geometry.clock, cursor, &style_sheet);

        // ----------- Digital clock ------------------
        draw_digital_clock(renderer, self, cursor, &style_sheet);

        // ----------- Buttons ------------------------
        self.cancel_button.draw(
            &self.tree.children[0],
            renderer,
            theme,
            style,
            self.geometry.cancel,
            cursor,
            &bounds,
        );

        self.submit_button.draw(
            &self.tree.children[1],
            renderer,
            theme,
            style,
            self.geometry.submit,
            cursor,
            &bounds,
        );

        // Buttons are not focusable right now...
        let cancel_button_bounds = self.geometry.cancel.bounds();
        if (self.state.focus == Focus::Cancel)
            && (cancel_button_bounds.width > 0.)
            && (cancel_button_bounds.height > 0.)
        {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: cancel_button_bounds,
                    border: Border {
                        radius: style_sheet[&StyleState::Focused].border_radius.into(),
                        width: style_sheet[&StyleState::Focused].border_width,
                        color: style_sheet[&StyleState::Focused].border_color,
                        ..Border::default()
                    },
                    ..renderer::Quad::default()
                },
                Color::TRANSPARENT,
            );
        }

        let submit_button_bounds = self.geometry.submit.bounds();
        if (self.state.focus == Focus::Submit)
            && (submit_button_bounds.width > 0.)
            && (submit_button_bounds.height > 0.)
        {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: submit_button_bounds,
                    border: Border {
                        radius: style_sheet[&StyleState::Focused].border_radius.into(),
                        width: style_sheet[&StyleState::Focused].border_width,
                        color: style_sheet[&StyleState::Focused].border_color,
                        ..Border::default()
                    },
                    ..renderer::Quad::default()
                },
                Color::TRANSPARENT,
            );
        }
    }

    fn operate(&mut self, renderer: &Renderer, operation: &mut dyn widget::Operation) {
        // Skip the clock canvas (analog display) - it has no text to expose

        // Operate on the digital clock element stored in tree.children[2]
        // This recursively exposes all text elements: hour, minute, second, separators, period
        if let Some(digital_clock_tree) = self.tree.children.get_mut(2) {
            // Need to recreate the digital clock element to call operate on it
            // This is the same structure created when the layout is computed
            let mut element = digital_clock_element::<Message, Theme>(self.state, renderer, false);

            element.operate(
                digital_clock_tree,
                self.geometry.digital,
                &self.viewport,
                renderer,
                operation,
            );
        }

        // Operate on cancel button
        Widget::operate(
            &mut self.cancel_button,
            &mut self.tree.children[0],
            self.geometry.cancel,
            &self.viewport,
            renderer,
            operation,
        );

        // Operate on submit button
        Widget::operate(
            &mut self.submit_button,
            &mut self.tree.children[1],
            self.geometry.submit,
            &self.viewport,
            renderer,
            operation,
        );
    }
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

/// Boxes the given widget into an [`Element`].
fn boxed<'a, Message, Theme>(
    widget: impl Widget<Message, Theme, Renderer> + 'a,
) -> Element<'a, Message, Theme, Renderer> {
    Widget::<Message, Theme, Renderer>::_boxed(widget)
}

/// Builds the element of the digital clock of the time picker.
///
/// If `placeholder` is set, the element only contains placeholders that have
/// the same size as the final content. This is what is used to compute the
/// layout.
fn digital_clock_element<'a, Message, Theme>(
    state: &State,
    renderer: &Renderer,
    placeholder: bool,
) -> Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a + text::Catalog + container::Catalog,
{
    let arrow_size = renderer.text_size().0;
    let font_size = 1.2 * renderer.text_size().0;

    let (up_content, up_font, _up_shaping) = up_open();
    let (down_content, down_font, _down_shaping) = down_open();

    let arrow = |content: &str, font| {
        boxed(
            widget::Text::<'a, Theme>::new(if placeholder {
                String::new()
            } else {
                content.to_owned()
            })
            .font(font)
            .size(arrow_size)
            .width(Length::Fixed(arrow_size))
            .height(Length::Fixed(arrow_size)),
        )
    };

    let label = |content: String| {
        boxed(
            Column::<Element<'a, Message, Theme, Renderer>>::new()
                .height(Length::Fit)
                .push(boxed(
                    widget::Text::<'a, Theme>::new(content).size(font_size),
                )),
        )
    };

    let value_column = |value: u32| {
        boxed(
            Column::<Element<'a, Message, Theme, Renderer>>::new()
                .align_x(Horizontal::Center)
                .height(Length::Fit)
                .push(arrow(&up_content, up_font))
                .push(boxed(
                    widget::Text::<'a, Theme>::new(format!("{value:02}")).size(font_size),
                ))
                .push(arrow(&down_content, down_font)),
        )
    };

    let period_text = if !placeholder && state.time.hour12().0 {
        "PM"
    } else {
        "AM"
    };

    let mut digital_clock_row = Row::<Element<'a, Message, Theme, Renderer>>::new()
        .align_y(Vertical::Center)
        .height(Length::Fit)
        .width(Length::Fit)
        .spacing(1);

    if !state.use_24h {
        digital_clock_row = digital_clock_row.push(label(period_text.to_owned()));
    }

    // The placeholders always use the hour as content
    let hour = if state.use_24h || placeholder {
        state.time.hour()
    } else {
        state.time.hour12().1
    };
    let minute = if placeholder {
        state.time.hour()
    } else {
        state.time.minute()
    };
    let second = if placeholder {
        state.time.hour()
    } else {
        state.time.second()
    };

    digital_clock_row = digital_clock_row
        .push(value_column(hour))
        .push(label(":".to_owned()))
        .push(value_column(minute));

    if state.show_seconds {
        digital_clock_row = digital_clock_row
            .push(label(":".to_owned()))
            .push(value_column(second));
    }

    if !state.use_24h {
        digital_clock_row = digital_clock_row.push(label(period_text.to_owned()));
    }

    boxed(
        Container::new(digital_clock_row)
            .width(Length::Fill)
            .height(Length::Fit)
            .center_x(Length::Fill)
            .center_y(Length::Fit),
    )
}

/// Applies a maximum width and height to the given [`Limits`].
fn max_size(mut limits: Limits, max_width: f32, max_height: f32) -> Limits {
    limits.max.width = limits.max.width.min(max_width);
    limits.min.width = limits.min.width.min(limits.max.width);
    limits.max.height = limits.max.height.min(max_height);
    limits.min.height = limits.min.height.min(limits.max.height);
    limits
}

/// Extracts the layout of a column of the digital clock.
fn digital_column((layout, tree): (Layout, &Tree)) -> DigitalColumn {
    let mut cells = layout.iter(&tree.children);

    let mut next_bounds = || cells.next().map(|(layout, _)| layout.bounds());

    DigitalColumn {
        bounds: layout.bounds(),
        up: next_bounds().unwrap_or_default(),
        center: next_bounds().unwrap_or_default(),
        down: next_bounds().unwrap_or_default(),
    }
}

/// Computes the layout of the whole time picker overlay.
#[allow(clippy::too_many_lines)]
fn compute_geometry<'a, Message, Theme>(
    state: &State,
    cancel_button: &mut Button<'a, Message, widget::Text<'a, Theme>, Theme>,
    submit_button: &mut Button<'a, Message, widget::Text<'a, Theme>, Theme>,
    tree: &mut Tree,
    position: Point,
    renderer: &Renderer,
    window: Size,
) -> Geometry
where
    Message: 'a + Clone,
    Theme: 'a + button::Catalog + text::Catalog + container::Catalog,
{
    let limits = max_size(
        Limits::new(Size::ZERO, window)
            .shrink(Size::new(PADDING.x(), PADDING.y()))
            .width(Length::Fill)
            .height(Length::Fill),
        300.0,
        350.0,
    );

    // Digital Clock
    let mut digital_clock = digital_clock_element::<Message, Theme>(state, renderer, true);
    sync_child(tree, 2, &mut digital_clock);
    digital_clock.layout(&mut tree.children[2], renderer, &limits);
    let digital_size = tree.children[2].size;

    // Pre-Buttons TODO: get rid of it
    sync_child(tree, 0, cancel_button);
    sync_child(tree, 1, submit_button);

    cancel_button.layout(&mut tree.children[0], renderer, &limits);
    let pre_cancel_height = tree.children[0].size.height;

    let limits = limits.shrink(Size::new(
        0.0,
        digital_size.height + pre_cancel_height + 2.0 * SPACING.0,
    ));

    // Clock-Canvas
    let clock_size = iced_core::layout::atomic(&limits, Length::Fill, Length::Fill);

    // Buttons
    let button_limits = max_size(
        limits,
        ((clock_size.width / 2.0) - BUTTON_SPACING.0).max(0.0),
        f32::INFINITY,
    );

    cancel_button.layout(&mut tree.children[0], renderer, &button_limits);
    let cancel_size = tree.children[0].size;

    submit_button.layout(&mut tree.children[1], renderer, &button_limits);
    let submit_size = tree.children[1].size;

    // The whole overlay
    let mut overlay = Layout::new(Size::new(
        clock_size.width + PADDING.x(),
        clock_size.height
            + digital_size.height
            + cancel_size.height
            + PADDING.y()
            + 2.0 * SPACING.0,
    ));
    overlay.center_and_bounce(position, window);

    // Everything below is positioned relative to the overlay
    let origin = overlay.position();
    let at = |size: Size, offset: Vector| {
        Layout::new(size).move_to(origin + offset + Vector::new(PADDING.left, PADDING.top))
    };

    let clock = at(clock_size, Vector::ZERO);
    let digital = at(
        digital_size,
        Vector::new(0.0, SPACING.0 + clock_size.height),
    );

    let buttons_y = clock_size.height + digital_size.height + 2.0 * SPACING.0;
    let cancel = at(cancel_size, Vector::new(0.0, buttons_y));
    let submit = at(
        submit_size,
        Vector::new(clock_size.width - submit_size.width, buttons_y),
    );

    // Digital clock: container -> row -> columns
    let digital_tree = &tree.children[2];
    let mut digital_children = digital.iter(&digital_tree.children);
    let (row_layout, row_tree) = digital_children
        .next()
        .expect("widget: Layout should have a digital clock layout");

    let mut columns = row_layout.iter(&row_tree.children);

    if !state.use_24h {
        // Placeholder
        let _ = columns.next();
    }

    let hour = columns.next().map(digital_column).unwrap_or_default();
    let hour_minute_separator = columns
        .next()
        .map(|(layout, _)| layout.bounds())
        .unwrap_or_default();
    let minute = columns.next().map(digital_column).unwrap_or_default();

    let (minute_second_separator, second) = if state.show_seconds {
        (
            columns.next().map(|(layout, _)| layout.bounds()),
            columns.next().map(digital_column),
        )
    } else {
        (None, None)
    };

    let period = if state.use_24h {
        None
    } else {
        columns.next().map(|(layout, _)| layout.bounds())
    };

    Geometry {
        overlay,
        clock,
        digital,
        hour,
        hour_minute_separator,
        minute,
        minute_second_separator,
        second,
        period,
        cancel,
        submit,
    }
}

/// Draws the analog clock.
#[allow(clippy::too_many_lines)]
fn draw_clock<Message, Theme>(
    renderer: &mut Renderer,
    time_picker: &TimePickerOverlay<'_, '_, Message, Theme>,
    layout: Layout,
    cursor: Cursor,
    style: &HashMap<StyleState, Style>,
) where
    Message: 'static + Clone,
    Theme: Catalog + button::Catalog + text::Catalog,
{
    let mut clock_style_state = StyleState::Active;
    if cursor.is_over(layout.bounds()) {
        clock_style_state = clock_style_state.max(StyleState::Hovered);
    }

    let geometry = time_picker
        .state
        .clock_cache
        .draw(renderer, layout.bounds().size(), |frame| {
            let center = frame.center();
            let radius = frame.width().min(frame.height()) * 0.5;
            let period = if time_picker.state.time.hour12().0 {
                clock::Period::PM
            } else {
                clock::Period::AM
            };

            let number_size = radius * NUMBER_SIZE_PERCENTAGE;
            let period_size = radius * PERIOD_SIZE_PERCENTAGE;

            let period_radius = radius * PERIOD_PERCENTAGE;

            let (hour_radius, minute_radius, second_radius) = if time_picker.state.show_seconds {
                (
                    radius * HOUR_RADIUS_PERCENTAGE,
                    radius * MINUTE_RADIUS_PERCENTAGE,
                    radius * SECOND_RADIUS_PERCENTAGE,
                )
            } else {
                (
                    radius * HOUR_RADIUS_PERCENTAGE_NO_SECONDS,
                    radius * MINUTE_RADIUS_PERCENTAGE_NO_SECONDS,
                    f32::MAX,
                )
            };

            let internal_cursor = cursor.position().unwrap_or_default()
                - Vector::new(layout.bounds().x, layout.bounds().y);

            let nearest_radius = if cursor.is_over(layout.bounds()) {
                crate::core::clock::nearest_radius(
                    &if time_picker.state.show_seconds {
                        vec![
                            (period_radius, NearestRadius::Period),
                            (hour_radius, NearestRadius::Hour),
                            (minute_radius, NearestRadius::Minute),
                            (second_radius, NearestRadius::Second),
                        ]
                    } else {
                        vec![
                            (period_radius, NearestRadius::Period),
                            (hour_radius, NearestRadius::Hour),
                            (minute_radius, NearestRadius::Minute),
                        ]
                    },
                    internal_cursor,
                    center,
                )
            } else {
                NearestRadius::None
            };

            let hour_points = crate::core::clock::circle_points(hour_radius, center, 12);
            let minute_points = crate::core::clock::circle_points(minute_radius, center, 60);
            let second_points = crate::core::clock::circle_points(second_radius, center, 60);

            let hand_stroke = Stroke {
                style: canvas::Style::Solid(
                    style
                        .get(&clock_style_state)
                        .expect("Style Sheet not found.")
                        .clock_hand_color,
                ),
                width: style
                    .get(&clock_style_state)
                    .expect("Style Sheet not found.")
                    .clock_hand_width,
                line_cap: LineCap::Round,
                ..Stroke::default()
            };

            match nearest_radius {
                NearestRadius::Period => {
                    frame.fill(
                        &Path::circle(center, period_size),
                        style
                            .get(&StyleState::Hovered)
                            .expect("Style Sheet not found.")
                            .clock_number_background,
                    );
                }
                NearestRadius::Hour => {
                    let nearest_point = hour_points
                        [crate::core::clock::nearest_point(&hour_points, internal_cursor)];

                    frame.fill(
                        &Path::circle(nearest_point, 5.0),
                        style
                            .get(&StyleState::Hovered)
                            .expect("Style Sheet not found.")
                            .clock_number_background,
                    );
                }
                NearestRadius::Minute => {
                    let nearest_point = minute_points
                        [crate::core::clock::nearest_point(&minute_points, internal_cursor)];

                    frame.fill(
                        &Path::circle(nearest_point, 5.0),
                        style
                            .get(&StyleState::Hovered)
                            .expect("Style Sheet not found.")
                            .clock_number_background,
                    );
                }
                NearestRadius::Second => {
                    let nearest_point = second_points
                        [crate::core::clock::nearest_point(&second_points, internal_cursor)];

                    frame.fill(
                        &Path::circle(nearest_point, 5.0),
                        style
                            .get(&StyleState::Hovered)
                            .expect("Style Sheet not found.")
                            .clock_number_background,
                    );
                }
                NearestRadius::None => {}
            }

            let period_text = CanvasText {
                content: format!("{period}"),
                position: center,
                color: style
                    .get(&clock_style_state)
                    .expect("Style Sheet not found.")
                    .clock_number_color,
                size: Pixels(period_size),
                font: renderer.font(),
                align_x: text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: text::LineHeight::Relative(1.3),
                shaping: text::Shaping::Basic,
                max_width: f32::INFINITY,
                wrapping: Wrapping::default(),
                ellipsis: text::Ellipsis::None,
            };
            frame.fill_text(period_text);

            hour_points.iter().enumerate().for_each(|(i, p)| {
                let (pm, selected) = {
                    let (pm, _) = time_picker.state.time.hour12();
                    let hour = time_picker.state.time.hour();
                    (pm, hour % 12 == i as u32)
                };

                let mut style_state = StyleState::Active;
                if selected {
                    frame.stroke(&Path::line(center, *p), hand_stroke);
                    frame.fill(
                        &Path::circle(*p, number_size * 0.8),
                        style
                            .get(&StyleState::Selected)
                            .expect("Style Sheet not found.")
                            .clock_number_background,
                    );
                    style_state = style_state.max(StyleState::Selected);
                }

                let text = CanvasText {
                    content: format!(
                        "{}",
                        if pm && time_picker.state.use_24h {
                            i + 12
                        } else if !time_picker.state.use_24h && i == 0 {
                            12
                        } else {
                            i
                        }
                    ),
                    position: *p,
                    color: style
                        .get(&style_state)
                        .expect("Style Sheet not found.")
                        .clock_number_color,
                    size: Pixels(number_size),
                    font: renderer.font(),
                    align_x: text::Alignment::Center,
                    align_y: Vertical::Center,
                    shaping: text::Shaping::Basic,
                    line_height: text::LineHeight::Relative(1.3),
                    max_width: f32::INFINITY,
                    wrapping: Wrapping::default(),
                    ellipsis: text::Ellipsis::None,
                };

                frame.fill_text(text);
            });

            minute_points.iter().enumerate().for_each(|(i, p)| {
                let selected = time_picker.state.time.minute() == i as u32;

                let mut style_state = StyleState::Active;
                if selected {
                    frame.stroke(&Path::line(center, *p), hand_stroke);
                    frame.fill(
                        &Path::circle(*p, number_size * 0.6),
                        style
                            .get(&StyleState::Selected)
                            .expect("Style Sheet not found.")
                            .clock_number_background,
                    );
                    style_state = style_state.max(StyleState::Selected);
                }

                if i % 5 == 0 {
                    let text = CanvasText {
                        content: format!("{i:02}"),
                        position: *p,
                        color: style
                            .get(&style_state)
                            .expect("Style Sheet not found.")
                            .clock_number_color,
                        size: Pixels(number_size),
                        font: renderer.font(),
                        align_x: text::Alignment::Center,
                        align_y: Vertical::Center,
                        shaping: text::Shaping::Basic,
                        line_height: text::LineHeight::Relative(1.3),
                        max_width: f32::INFINITY,
                        wrapping: Wrapping::default(),
                        ellipsis: text::Ellipsis::None,
                    };

                    frame.fill_text(text);
                } else {
                    let circle = Path::circle(*p, number_size * 0.1);
                    frame.fill(
                        &circle,
                        style
                            .get(&StyleState::Active)
                            .expect("Style Sheet not found.")
                            .clock_dots_color,
                    );
                }
            });

            if time_picker.state.show_seconds {
                second_points.iter().enumerate().for_each(|(i, p)| {
                    let selected = time_picker.state.time.second() == i as u32;

                    let mut style_state = StyleState::Active;
                    if selected {
                        frame.stroke(&Path::line(center, *p), hand_stroke);
                        frame.fill(
                            &Path::circle(*p, number_size * 0.6),
                            style
                                .get(&StyleState::Selected)
                                .expect("Style Sheet not found.")
                                .clock_number_background,
                        );
                        style_state = style_state.max(StyleState::Selected);
                    }

                    if i % 10 == 0 {
                        let text = CanvasText {
                            content: format!("{i:02}"),
                            position: *p,
                            color: style
                                .get(&style_state)
                                .expect("Style Sheet not found.")
                                .clock_number_color,
                            size: Pixels(number_size),
                            font: renderer.font(),
                            align_x: text::Alignment::Center,
                            align_y: Vertical::Center,
                            shaping: text::Shaping::Basic,
                            line_height: text::LineHeight::Relative(1.3),
                            max_width: f32::INFINITY,
                            wrapping: Wrapping::default(),
                            ellipsis: text::Ellipsis::None,
                        };

                        frame.fill_text(text);
                    } else {
                        let circle = Path::circle(*p, number_size * 0.1);
                        frame.fill(
                            &circle,
                            style
                                .get(&StyleState::Active)
                                .expect("Style Sheet not found.")
                                .clock_dots_color,
                        );
                    }
                });
            }
        });

    let translation = Vector::new(layout.bounds().x, layout.bounds().y);
    renderer.with_translation(translation, |renderer| {
        renderer.draw_geometry(geometry);
    });
}

/// Draws the digital clock.
#[allow(clippy::too_many_lines)]
fn draw_digital_clock<Message, Theme>(
    renderer: &mut Renderer,
    time_picker: &TimePickerOverlay<'_, '_, Message, Theme>,
    cursor: Cursor,
    style: &HashMap<StyleState, Style>,
) where
    Message: 'static + Clone,
    Theme: Catalog + button::Catalog + text::Catalog,
{
    let geometry = &time_picker.geometry;

    let draw_text = |renderer: &mut Renderer, content: String, bounds: Rectangle| {
        renderer.fill_text(
            Text {
                content,
                bounds: Size::new(bounds.width, bounds.height),
                size: renderer.text_size(),
                font: renderer.font(),
                align_x: text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: text::LineHeight::Relative(1.3),
                shaping: text::Shaping::Basic,
                wrapping: Wrapping::default(),
                ellipsis: text::Ellipsis::None,
                hint_factor: renderer.hint_factor(),
            },
            Point::new(bounds.center_x(), bounds.center_y()),
            style[&StyleState::Active].text_color,
            bounds,
        );
    };

    let f = |renderer: &mut Renderer, column: DigitalColumn, text: String, target: Focus| {
        let style_state = if time_picker.state.focus == target {
            StyleState::Focused
        } else {
            StyleState::Active
        };

        let up_bounds = column.up;
        let center_bounds = column.center;
        let down_bounds = column.down;

        let up_arrow_hovered = cursor.is_over(up_bounds);
        let down_arrow_hovered = cursor.is_over(down_bounds);

        // Background
        if style_state == StyleState::Focused {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: column.bounds,
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
                    ..renderer::Quad::default()
                },
                style
                    .get(&style_state)
                    .expect("Style Sheet not found.")
                    .background,
            );
        }

        let (up_content, up_font, _up_shaping) = up_open();
        let (down_content, down_font, _down_shaping) = down_open();
        // Caret up
        renderer.fill_text(
            Text {
                content: up_content,
                bounds: Size::new(up_bounds.width, up_bounds.height),
                size: Pixels(renderer.text_size().0 + if up_arrow_hovered { 1.0 } else { 0.0 }),
                font: up_font,
                align_x: text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: text::LineHeight::Relative(1.3),
                shaping: text::Shaping::Basic,
                wrapping: Wrapping::default(),
                ellipsis: text::Ellipsis::None,
                hint_factor: renderer.hint_factor(),
            },
            Point::new(up_bounds.center_x(), up_bounds.center_y()),
            style
                .get(&StyleState::Active)
                .expect("Style Sheet not found.")
                .text_color,
            up_bounds,
        );

        // Text
        draw_text(renderer, text, center_bounds);

        // Down caret
        renderer.fill_text(
            Text {
                content: down_content,
                bounds: Size::new(down_bounds.width, down_bounds.height),
                size: Pixels(renderer.text_size().0 + if down_arrow_hovered { 1.0 } else { 0.0 }),
                font: down_font,
                align_x: text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: text::LineHeight::Relative(1.3),
                shaping: text::Shaping::Basic,
                wrapping: Wrapping::default(),
                ellipsis: text::Ellipsis::None,
                hint_factor: renderer.hint_factor(),
            },
            Point::new(down_bounds.center_x(), down_bounds.center_y()),
            style
                .get(&StyleState::Active)
                .expect("Style Sheet not found.")
                .text_color,
            down_bounds,
        );
    };

    // Draw hours
    f(
        renderer,
        geometry.hour,
        format!(
            "{:02}",
            if time_picker.state.use_24h {
                time_picker.state.time.hour()
            } else {
                time_picker.state.time.hour12().1
            }
        ),
        Focus::DigitalHour,
    );

    // Draw separator between hours and minutes
    draw_text(renderer, ":".to_owned(), geometry.hour_minute_separator);

    // Draw minutes
    f(
        renderer,
        geometry.minute,
        format!("{:02}", time_picker.state.time.minute()),
        Focus::DigitalMinute,
    );

    if time_picker.state.show_seconds {
        // Draw separator between minutes and seconds
        if let Some(separator) = geometry.minute_second_separator {
            draw_text(renderer, ":".to_owned(), separator);
        }

        // Draw seconds
        if let Some(second) = geometry.second {
            f(
                renderer,
                second,
                format!("{:02}", time_picker.state.time.second()),
                Focus::DigitalSecond,
            );
        }
    }

    // Draw period
    if let Some(period) = geometry.period {
        draw_text(
            renderer,
            if time_picker.state.time.hour12().0 {
                "PM".to_owned()
            } else {
                "AM".to_owned()
            },
            period,
        );
    }
}

/// The state of the [`TimePickerOverlay`].
#[derive(Debug)]
pub struct State {
    /// The selected time of the [`TimePickerOverlay`].
    pub(crate) time: NaiveTime,
    /// Toggle if the cache needs to be cleared.
    pub(crate) clock_cache_needs_clearance: bool,
    /// The cache of the clock of the [`TimePickerOverlay`].
    pub(crate) clock_cache: canvas::Cache,
    /// Toggle the use of the 24h clock of the [`TimePickerOverlay`].
    pub(crate) use_24h: bool,
    /// Toggle the use of the seconds of the [`TimePickerOverlay`].
    pub(crate) show_seconds: bool,
    /// The dragged clock element of the [`TimePickerOverlay`].
    pub(crate) clock_dragged: ClockDragged,
    /// The focus of the [`TimePickerOverlay`].
    pub(crate) focus: Focus,
    /// The previously pressed keyboard modifiers.
    pub(crate) keyboard_modifiers: keyboard::Modifiers,
}

impl State {
    /// Creates a new State with the given time.
    #[must_use]
    pub fn new(time: Time, use_24h: bool, show_seconds: bool) -> Self {
        Self {
            use_24h,
            show_seconds,
            time: time.into(),
            ..Self::default()
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            time: Local::now().naive_local().time(),
            clock_cache_needs_clearance: false,
            clock_cache: canvas::Cache::new(),
            use_24h: false,
            show_seconds: false,
            clock_dragged: ClockDragged::None,
            focus: Focus::default(),
            keyboard_modifiers: keyboard::Modifiers::default(),
        }
    }
}

/// Just a workaround to pass the button states from the tree to the overlay
#[allow(missing_debug_implementations)]
pub struct TimePickerOverlayButtons<'a, Message, Theme>
where
    Message: Clone,
    Theme: Catalog + button::Catalog,
{
    /// The cancel button of the [`TimePickerOverlay`].
    cancel_button: Element<'a, Message, Theme, Renderer>,
    /// The submit button of the [`TimePickerOverlay`].
    submit_button: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme> Default for TimePickerOverlayButtons<'a, Message, Theme>
where
    Message: 'a + Clone,
    Theme: 'a + Catalog + button::Catalog + text::Catalog,
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

#[allow(clippy::unimplemented)]
impl<Message, Theme> Meta for TimePickerOverlayButtons<'_, Message, Theme>
where
    Message: Clone,
    Theme: Catalog + button::Catalog,
{
}

#[allow(clippy::unimplemented)]
impl<Message, Theme> Widget<Message, Theme, Renderer>
    for TimePickerOverlayButtons<'_, Message, Theme>
where
    Message: Clone,
    Theme: Catalog + button::Catalog,
{
    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(&mut [&mut self.cancel_button, &mut self.submit_button]);
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

impl<'a, Message, Theme> From<TimePickerOverlayButtons<'a, Message, Theme>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a + Catalog + button::Catalog,
{
    fn from(overlay: TimePickerOverlayButtons<'a, Message, Theme>) -> Self {
        boxed(overlay)
    }
}

/// The state of the currently dragged watch hand.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ClockDragged {
    /// Nothing is dragged.
    None,

    /// The hour hand is dragged.
    Hour,

    /// The minute hand is dragged.
    Minute,

    /// The second hand is dragged.
    Second,
}

/// An enumeration of all focusable elements of the [`TimePickerOverlay`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Focus {
    /// Nothing is in focus.
    #[default]
    None,

    /// The overlay itself is in focus.
    Overlay,

    /// The digital hour is in focus.
    DigitalHour,

    /// The digital minute is in focus.
    DigitalMinute,

    /// The digital second is in focus.
    DigitalSecond,

    /// The cancel button is in focus.
    Cancel,

    /// The submit button is in focus.
    Submit,
}

impl Focus {
    /// Gets the next focusable element.
    #[must_use]
    pub const fn next(self, show_seconds: bool) -> Self {
        match self {
            Self::Overlay => Self::DigitalHour,
            Self::DigitalHour => Self::DigitalMinute,
            Self::DigitalMinute => {
                if show_seconds {
                    Self::DigitalSecond
                } else {
                    Self::Cancel
                }
            }
            Self::DigitalSecond => Self::Cancel,
            Self::Cancel => Self::Submit,
            Self::Submit | Self::None => Self::Overlay,
        }
    }

    /// Gets the previous focusable element.
    #[must_use]
    pub const fn previous(self, show_seconds: bool) -> Self {
        match self {
            Self::None => Self::None,
            Self::Overlay => Self::Submit,
            Self::DigitalHour => Self::Overlay,
            Self::DigitalMinute => Self::DigitalHour,
            Self::DigitalSecond => Self::DigitalMinute,
            Self::Cancel => {
                if show_seconds {
                    Self::DigitalSecond
                } else {
                    Self::DigitalMinute
                }
            }
            Self::Submit => Self::Cancel,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_new_creates_state_with_time() {
        let time = Time::Hm {
            hour: 14,
            minute: 30,
            period: Period::H24,
        };
        let state = State::new(time, true, false);

        assert_eq!(state.time.hour(), 14);
        assert_eq!(state.time.minute(), 30);
        assert!(state.use_24h);
        assert!(!state.show_seconds);
        assert_eq!(state.focus, Focus::None);
        assert_eq!(state.clock_dragged, ClockDragged::None);
    }

    #[test]
    fn state_new_with_seconds() {
        let time = Time::Hms {
            hour: 14,
            minute: 30,
            second: 45,
            period: Period::H24,
        };
        let state = State::new(time, true, true);

        assert_eq!(state.time.hour(), 14);
        assert_eq!(state.time.minute(), 30);
        assert_eq!(state.time.second(), 45);
        assert!(state.show_seconds);
    }

    #[test]
    fn state_default_creates_state() {
        let state = State::default();

        assert!(state.time.hour() < 24);
        assert!(state.time.minute() < 60);
        assert!(!state.use_24h);
        assert!(!state.show_seconds);
        assert_eq!(state.focus, Focus::None);
    }

    #[test]
    fn clock_dragged_enum_variants() {
        let none = ClockDragged::None;
        let hour = ClockDragged::Hour;
        let minute = ClockDragged::Minute;
        let second = ClockDragged::Second;

        // Just verify the enum can be created and compared
        assert!(matches!(none, ClockDragged::None));
        assert!(matches!(hour, ClockDragged::Hour));
        assert!(matches!(minute, ClockDragged::Minute));
        assert!(matches!(second, ClockDragged::Second));
    }

    #[test]
    fn focus_default_is_none() {
        let focus = Focus::default();
        assert_eq!(focus, Focus::None);
    }

    #[test]
    fn focus_next_without_seconds() {
        let show_seconds = false;

        assert_eq!(Focus::None.next(show_seconds), Focus::Overlay);
        assert_eq!(Focus::Overlay.next(show_seconds), Focus::DigitalHour);
        assert_eq!(Focus::DigitalHour.next(show_seconds), Focus::DigitalMinute);
        assert_eq!(Focus::DigitalMinute.next(show_seconds), Focus::Cancel);
        assert_eq!(Focus::Cancel.next(show_seconds), Focus::Submit);
        assert_eq!(Focus::Submit.next(show_seconds), Focus::Overlay);
    }

    #[test]
    fn focus_next_with_seconds() {
        let show_seconds = true;

        assert_eq!(Focus::None.next(show_seconds), Focus::Overlay);
        assert_eq!(Focus::Overlay.next(show_seconds), Focus::DigitalHour);
        assert_eq!(Focus::DigitalHour.next(show_seconds), Focus::DigitalMinute);
        assert_eq!(
            Focus::DigitalMinute.next(show_seconds),
            Focus::DigitalSecond
        );
        assert_eq!(Focus::DigitalSecond.next(show_seconds), Focus::Cancel);
        assert_eq!(Focus::Cancel.next(show_seconds), Focus::Submit);
        assert_eq!(Focus::Submit.next(show_seconds), Focus::Overlay);
    }

    #[test]
    fn focus_previous_without_seconds() {
        let show_seconds = false;

        assert_eq!(Focus::None.previous(show_seconds), Focus::None);
        assert_eq!(Focus::Overlay.previous(show_seconds), Focus::Submit);
        assert_eq!(Focus::DigitalHour.previous(show_seconds), Focus::Overlay);
        assert_eq!(
            Focus::DigitalMinute.previous(show_seconds),
            Focus::DigitalHour
        );
        assert_eq!(Focus::Cancel.previous(show_seconds), Focus::DigitalMinute);
        assert_eq!(Focus::Submit.previous(show_seconds), Focus::Cancel);
    }

    #[test]
    fn focus_previous_with_seconds() {
        let show_seconds = true;

        assert_eq!(Focus::None.previous(show_seconds), Focus::None);
        assert_eq!(Focus::Overlay.previous(show_seconds), Focus::Submit);
        assert_eq!(Focus::DigitalHour.previous(show_seconds), Focus::Overlay);
        assert_eq!(
            Focus::DigitalMinute.previous(show_seconds),
            Focus::DigitalHour
        );
        assert_eq!(
            Focus::DigitalSecond.previous(show_seconds),
            Focus::DigitalMinute
        );
        assert_eq!(Focus::Cancel.previous(show_seconds), Focus::DigitalSecond);
        assert_eq!(Focus::Submit.previous(show_seconds), Focus::Cancel);
    }

    #[test]
    fn focus_navigation_cycle_without_seconds() {
        let show_seconds = false;
        let mut focus = Focus::Overlay;

        // Navigate forward through all elements
        focus = focus.next(show_seconds); // DigitalHour
        focus = focus.next(show_seconds); // DigitalMinute
        focus = focus.next(show_seconds); // Cancel
        focus = focus.next(show_seconds); // Submit
        focus = focus.next(show_seconds); // Back to Overlay

        assert_eq!(focus, Focus::Overlay);

        // Navigate backward through all elements
        focus = focus.previous(show_seconds); // Submit
        focus = focus.previous(show_seconds); // Cancel
        focus = focus.previous(show_seconds); // DigitalMinute
        focus = focus.previous(show_seconds); // DigitalHour
        focus = focus.previous(show_seconds); // Back to Overlay

        assert_eq!(focus, Focus::Overlay);
    }

    #[test]
    fn focus_navigation_cycle_with_seconds() {
        let show_seconds = true;
        let mut focus = Focus::Overlay;

        // Navigate forward through all elements
        focus = focus.next(show_seconds); // DigitalHour
        focus = focus.next(show_seconds); // DigitalMinute
        focus = focus.next(show_seconds); // DigitalSecond
        focus = focus.next(show_seconds); // Cancel
        focus = focus.next(show_seconds); // Submit
        focus = focus.next(show_seconds); // Back to Overlay

        assert_eq!(focus, Focus::Overlay);

        // Navigate backward through all elements
        focus = focus.previous(show_seconds); // Submit
        focus = focus.previous(show_seconds); // Cancel
        focus = focus.previous(show_seconds); // DigitalSecond
        focus = focus.previous(show_seconds); // DigitalMinute
        focus = focus.previous(show_seconds); // DigitalHour
        focus = focus.previous(show_seconds); // Back to Overlay

        assert_eq!(focus, Focus::Overlay);
    }

    #[test]
    fn focus_skips_seconds_when_not_shown() {
        let show_seconds = false;

        // When going forward from DigitalMinute, skip DigitalSecond
        assert_eq!(Focus::DigitalMinute.next(show_seconds), Focus::Cancel);

        // When going backward from Cancel, skip DigitalSecond
        assert_eq!(Focus::Cancel.previous(show_seconds), Focus::DigitalMinute);
    }

    #[test]
    fn focus_includes_seconds_when_shown() {
        let show_seconds = true;

        // When going forward from DigitalMinute, include DigitalSecond
        assert_eq!(
            Focus::DigitalMinute.next(show_seconds),
            Focus::DigitalSecond
        );

        // When going backward from Cancel, include DigitalSecond
        assert_eq!(Focus::Cancel.previous(show_seconds), Focus::DigitalSecond);
    }

    #[test]
    fn state_with_different_times() {
        let test_times = [
            (0, 0, 0),    // Midnight
            (6, 30, 0),   // Morning
            (12, 0, 0),   // Noon
            (18, 45, 30), // Evening with seconds
            (23, 59, 59), // End of day with seconds
        ];

        for (hour, minute, second) in test_times {
            let time = if second > 0 {
                Time::Hms {
                    hour,
                    minute,
                    second,
                    period: Period::H24,
                }
            } else {
                Time::Hm {
                    hour,
                    minute,
                    period: Period::H24,
                }
            };

            let state = State::new(time, true, second > 0);

            assert_eq!(state.time.hour(), hour);
            assert_eq!(state.time.minute(), minute);
            if second > 0 {
                assert_eq!(state.time.second(), second);
            }
        }
    }

    #[test]
    fn state_12h_format() {
        let time = Time::Hm {
            hour: 2,
            minute: 30,
            period: Period::Pm,
        };
        let state = State::new(time, false, false);

        assert_eq!(state.time.hour(), 14); // 2 PM = 14:00 in 24h
        assert!(!state.use_24h);
    }

    #[test]
    fn overlay_buttons_has_two_children() {
        let buttons: TimePickerOverlayButtons<(), iced_widget::Theme> =
            TimePickerOverlayButtons::default();

        let mut buttons = buttons;
        let mut tree = Tree::new(&buttons);
        Widget::<(), iced_widget::Theme, Renderer>::diff(&mut buttons, &mut tree);
        assert_eq!(tree.children.len(), 2);
    }

    #[test]
    fn clock_dragged_is_copy() {
        let dragged = ClockDragged::Hour;
        let copied = dragged;

        // Verify both can be used (Copy trait)
        assert!(matches!(dragged, ClockDragged::Hour));
        assert!(matches!(copied, ClockDragged::Hour));
    }

    #[test]
    fn focus_is_copy_and_eq() {
        let focus = Focus::DigitalHour;
        let copied = focus;

        // Verify both can be used (Copy trait)
        assert_eq!(focus, Focus::DigitalHour);
        assert_eq!(copied, Focus::DigitalHour);

        // Verify equality (Eq trait)
        assert_eq!(focus, copied);
    }

    #[test]
    fn focus_none_stays_none_on_previous() {
        // Focus::None should stay None when calling previous
        assert_eq!(Focus::None.previous(false), Focus::None);
        assert_eq!(Focus::None.previous(true), Focus::None);
    }

    #[test]
    fn focus_all_variants_covered() {
        // Test that all Focus variants can be created
        let variants = [
            Focus::None,
            Focus::Overlay,
            Focus::DigitalHour,
            Focus::DigitalMinute,
            Focus::DigitalSecond,
            Focus::Cancel,
            Focus::Submit,
        ];

        for variant in variants {
            // Just verify they exist and can be compared
            assert!(matches!(
                variant,
                Focus::None
                    | Focus::Overlay
                    | Focus::DigitalHour
                    | Focus::DigitalMinute
                    | Focus::DigitalSecond
                    | Focus::Cancel
                    | Focus::Submit
            ));
        }
    }
}
