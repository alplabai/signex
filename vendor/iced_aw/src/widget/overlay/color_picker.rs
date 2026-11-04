//! Use a color picker as an input element for picking colors.
//!
//! *This API requires the following crate features to be activated: `color_picker`*

use crate::{
    color_picker,
    core::{
        color::{HexString, Hsv},
        overlay::Position,
    },
    style::{self, Status, color_picker::Style, style_state::StyleState},
};

use crate::iced_aw_font::advanced_text::{cancel, ok};
use iced_core::{
    Border, Color, Element, Event, Layout, Length, Overlay, Padding, Pixels, Point, Rectangle,
    Renderer as _, Shell, Size, Text, Vector, Widget,
    alignment::{Horizontal, Vertical},
    event, keyboard,
    layout::Limits,
    mouse::{self, Cursor},
    overlay, renderer, shell,
    text::{self, Renderer as _, Wrapping},
    touch,
    widget::{self, Meta, tree::Tree},
};
use iced_widget::{
    Button, Column, Renderer, Row,
    canvas::{self, LineCap, Path, Stroke},
    graphics::geometry::Renderer as _,
};
use std::collections::HashMap;

/// The padding around the elements.
const PADDING: Padding = Padding::new(10.0);
/// The spacing between the element.
const SPACING: Pixels = Pixels(15.0);
/// The spacing between the buttons.
const BUTTON_SPACING: Pixels = Pixels(5.0);

/// The step value of the keyboard change of the sat/value color values.
const SAT_VALUE_STEP: f32 = 0.005;
/// The step value of the keyboard change of the hue color value.
const HUE_STEP: i32 = 1;
/// The step value of the keyboard change of the RGBA color values.
const RGBA_STEP: i16 = 1;

/// The overlay of the [`ColorPicker`](crate::widget::ColorPicker).
#[allow(missing_debug_implementations)]
pub struct ColorPickerOverlay<'a, 'b, Message, Theme>
where
    Message: Clone,
    Theme: style::color_picker::Catalog + iced_widget::button::Catalog + iced_widget::text::Catalog,
    'b: 'a,
{
    /// The state of the [`ColorPickerOverlay`].
    state: &'a mut State,
    /// The cancel button of the [`ColorPickerOverlay`].
    cancel_button: Button<'a, Message, widget::Text<'a, Theme>, Theme>,
    /// The submit button of the [`ColorPickerOverlay`].
    submit_button: Button<'a, Message, widget::Text<'a, Theme>, Theme>,
    /// The function that produces a message when the submit button of the [`ColorPickerOverlay`].
    on_submit: &'a dyn Fn(Color) -> Message,
    /// Optional function that produces a message when the color changes during selection (real-time updates).
    on_color_change: Option<&'a dyn Fn(Color) -> Message>,
    /// The style of the [`ColorPickerOverlay`].
    class: &'a <Theme as style::color_picker::Catalog>::Class<'b>,
    /// The reference to the tree holding the state of this overlay.
    tree: &'a mut Tree,
    /// The viewport of the [`ColorPickerOverlay`].
    viewport: Rectangle,
    /// The size of the window the overlay is displayed in.
    window: Size,
    /// The computed layout of all the elements of the [`ColorPickerOverlay`].
    geometry: Geometry,
}

/// The layout of a single row (label, bar and value) of the RGBA area.
#[derive(Debug, Clone, Copy, Default)]
struct RgbaRowGeometry {
    /// The bounds of the whole row.
    row: Rectangle,
    /// The bounds of the label.
    label: Rectangle,
    /// The bounds of the color bar.
    bar: Rectangle,
    /// The bounds of the value text.
    value: Rectangle,
}

/// The absolute layout of all the elements of the [`ColorPickerOverlay`].
#[derive(Debug, Clone, Copy)]
struct Geometry {
    /// The layout of the whole overlay.
    overlay: Layout,
    /// The bounds of the saturation/value area.
    sat_value: Rectangle,
    /// The bounds of the hue area.
    hue: Rectangle,
    /// The red, green, blue and alpha rows.
    rgba: [RgbaRowGeometry; 4],
    /// The bounds of the hex text.
    hex: Rectangle,
    /// The layout of the cancel button.
    cancel: Layout,
    /// The layout of the submit button.
    submit: Layout,
}

impl<'a, 'b, Message, Theme> ColorPickerOverlay<'a, 'b, Message, Theme>
where
    Message: 'static + Clone,
    Theme: 'a
        + style::color_picker::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
    'b: 'a,
{
    /// Creates a new [`ColorPickerOverlay`] on the given position.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        state: &'a mut color_picker::State,
        on_cancel: Message,
        on_submit: &'a dyn Fn(Color) -> Message,
        on_color_change: Option<&'a dyn Fn(Color) -> Message>,
        position: Point,
        class: &'a <Theme as style::color_picker::Catalog>::Class<'b>,
        tree: &'a mut Tree,
        viewport: Rectangle,
        renderer: &Renderer,
        window: Size,
    ) -> Self {
        let color_picker::State { overlay_state, .. } = state;

        let (cancel_content, cancel_font, _cancel_shaping) = cancel();
        let (submit_content, submit_font, _submit_shaping) = ok();

        let mut cancel_button = Button::new(
            widget::Text::new(cancel_content)
                .align_x(Horizontal::Center)
                .width(Length::Fill)
                .font(cancel_font),
        )
        .width(Length::Fill)
        .on_press(on_cancel.clone());
        let mut submit_button = Button::new(
            widget::Text::new(submit_content)
                .align_x(Horizontal::Center)
                .width(Length::Fill)
                .font(submit_font),
        )
        .width(Length::Fill)
        .on_press(on_cancel); // Sending a fake message

        // The layout is not part of the `Overlay` trait anymore, so it is
        // computed once, when the overlay is created.
        let geometry = compute_geometry(
            &mut cancel_button,
            &mut submit_button,
            tree,
            position,
            renderer,
            window,
        );

        ColorPickerOverlay {
            state: overlay_state,
            cancel_button,
            submit_button,
            on_submit,
            on_color_change,
            class,
            tree,
            viewport,
            window,
            geometry,
        }
    }

    /// Turn this [`ColorPickerOverlay`] into an overlay [`Element`](overlay::Element).
    #[must_use]
    pub fn overlay(self) -> overlay::Element<'a, Message, Theme, Renderer> {
        overlay::Element::new(Box::new(self))
    }

    /// Force redraw all components if the internal state was changed
    fn clear_cache(&self) {
        self.state.clear_cache();
    }

    /// The event handling for the HSV color area.
    fn on_event_hsv_color(
        &mut self,
        event: &Event,
        cursor: Cursor,
        shell: &mut Shell<'_, Message>,
    ) -> event::Status {
        let hsv_color: Hsv = self.state.color.into();
        let mut color_changed = false;

        let sat_value_bounds = self.geometry.sat_value;
        let hue_bounds = self.geometry.hue;

        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => match delta {
                mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {
                    let move_value =
                        |value: u16, y: f32| ((i32::from(value) + y as i32).rem_euclid(360)) as u16;

                    if cursor.is_over(hue_bounds) {
                        self.state.color = Color {
                            a: self.state.color.a,
                            ..Hsv {
                                hue: move_value(hsv_color.hue, *y),
                                ..hsv_color
                            }
                            .into()
                        };
                        color_changed = true;
                    }
                }
            },
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if cursor.is_over(sat_value_bounds) {
                    self.state.color_bar_dragged = ColorBarDragged::SatValue;
                    self.state.focus = Focus::SatValue;
                }
                if cursor.is_over(hue_bounds) {
                    self.state.color_bar_dragged = ColorBarDragged::Hue;
                    self.state.focus = Focus::Hue;
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. }) => {
                self.state.color_bar_dragged = ColorBarDragged::None;
            }
            _ => {}
        }

        let calc_percentage_sat =
            |cursor_position: Point| (cursor_position.x.max(0.0) / sat_value_bounds.width).min(1.0);

        let calc_percentage_value = |cursor_position: Point| {
            (cursor_position.y.max(0.0) / sat_value_bounds.height).min(1.0)
        };

        let calc_hue = |cursor_position: Point| {
            ((cursor_position.x.max(0.0) / hue_bounds.width).min(1.0) * 360.0) as u16
        };

        match self.state.color_bar_dragged {
            ColorBarDragged::SatValue => {
                self.state.color = Color {
                    a: self.state.color.a,
                    ..Hsv {
                        saturation: cursor
                            .position_in(sat_value_bounds)
                            .map(calc_percentage_sat)
                            .unwrap_or_default(),
                        value: cursor
                            .position_in(sat_value_bounds)
                            .map(calc_percentage_value)
                            .unwrap_or_default(),
                        ..hsv_color
                    }
                    .into()
                };
                color_changed = true;
            }
            ColorBarDragged::Hue => {
                self.state.color = Color {
                    a: self.state.color.a,
                    ..Hsv {
                        hue: cursor
                            .position_in(hue_bounds)
                            .map(calc_hue)
                            .unwrap_or_default(),
                        ..hsv_color
                    }
                    .into()
                };
                color_changed = true;
            }
            _ => {}
        }

        if color_changed {
            // Call on_color_change callback for real-time updates
            if let Some(on_color_change) = self.on_color_change {
                shell.publish(on_color_change(self.state.color));
            }
            event::Status::Captured
        } else {
            event::Status::Ignored
        }
    }

    /// The event handling for the RGBA color area.
    #[allow(clippy::too_many_lines)]
    fn on_event_rgba_color(
        &mut self,
        event: &Event,
        cursor: Cursor,
        shell: &mut Shell<'_, Message>,
    ) -> event::Status {
        let mut color_changed = false;

        let red_bar_bounds = self.geometry.rgba[0].bar;
        let green_bar_bounds = self.geometry.rgba[1].bar;
        let blue_bar_bounds = self.geometry.rgba[2].bar;
        let alpha_bar_bounds = self.geometry.rgba[3].bar;

        match event {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => match delta {
                mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {
                    let move_value =
                        //|value: f32, y: f32| (value * 255.0 + y).clamp(0.0, 255.0) / 255.0;
                        |value: f32, y: f32| value.mul_add(255.0, y).clamp(0.0, 255.0) / 255.0;

                    if cursor.is_over(red_bar_bounds) {
                        self.state.color = Color {
                            r: move_value(self.state.color.r, *y),
                            ..self.state.color
                        };
                        color_changed = true;
                    }
                    if cursor.is_over(green_bar_bounds) {
                        self.state.color = Color {
                            g: move_value(self.state.color.g, *y),
                            ..self.state.color
                        };
                        color_changed = true;
                    }
                    if cursor.is_over(blue_bar_bounds) {
                        self.state.color = Color {
                            b: move_value(self.state.color.b, *y),
                            ..self.state.color
                        };
                        color_changed = true;
                    }
                    if cursor.is_over(alpha_bar_bounds) {
                        self.state.color = Color {
                            a: move_value(self.state.color.a, *y),
                            ..self.state.color
                        };
                        color_changed = true;
                    }
                }
            },
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if cursor.is_over(red_bar_bounds) {
                    self.state.color_bar_dragged = ColorBarDragged::Red;
                    self.state.focus = Focus::Red;
                }
                if cursor.is_over(green_bar_bounds) {
                    self.state.color_bar_dragged = ColorBarDragged::Green;
                    self.state.focus = Focus::Green;
                }
                if cursor.is_over(blue_bar_bounds) {
                    self.state.color_bar_dragged = ColorBarDragged::Blue;
                    self.state.focus = Focus::Blue;
                }
                if cursor.is_over(alpha_bar_bounds) {
                    self.state.color_bar_dragged = ColorBarDragged::Alpha;
                    self.state.focus = Focus::Alpha;
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerLifted { .. } | touch::Event::FingerLost { .. }) => {
                self.state.color_bar_dragged = ColorBarDragged::None;
            }
            _ => {}
        }

        let calc_percentage = |bounds: Rectangle, cursor_position: Point| {
            (cursor_position.x.max(0.0) / bounds.width).min(1.0)
        };

        match self.state.color_bar_dragged {
            ColorBarDragged::Red => {
                self.state.color = Color {
                    r: cursor
                        .position_in(red_bar_bounds)
                        .map(|position| calc_percentage(red_bar_bounds, position))
                        .unwrap_or_default(),
                    ..self.state.color
                };
                color_changed = true;
            }
            ColorBarDragged::Green => {
                self.state.color = Color {
                    g: cursor
                        .position_in(green_bar_bounds)
                        .map(|position| calc_percentage(green_bar_bounds, position))
                        .unwrap_or_default(),
                    ..self.state.color
                };
                color_changed = true;
            }
            ColorBarDragged::Blue => {
                self.state.color = Color {
                    b: cursor
                        .position_in(blue_bar_bounds)
                        .map(|position| calc_percentage(blue_bar_bounds, position))
                        .unwrap_or_default(),
                    ..self.state.color
                };
                color_changed = true;
            }
            ColorBarDragged::Alpha => {
                self.state.color = Color {
                    a: cursor
                        .position_in(alpha_bar_bounds)
                        .map(|position| calc_percentage(alpha_bar_bounds, position))
                        .unwrap_or_default(),
                    ..self.state.color
                };
                color_changed = true;
            }
            _ => {}
        }

        if color_changed {
            // Call on_color_change callback for real-time updates
            if let Some(on_color_change) = self.on_color_change {
                shell.publish(on_color_change(self.state.color));
            }
            event::Status::Captured
        } else {
            event::Status::Ignored
        }
    }

    /// The even handling for the keyboard input.
    fn on_event_keyboard(
        &mut self,
        event: &Event,
        shell: &mut Shell<'_, Message>,
    ) -> event::Status {
        if self.state.focus == Focus::None {
            return event::Status::Ignored;
        }

        if let Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event {
            let mut status = event::Status::Ignored;

            if matches!(key, keyboard::Key::Named(keyboard::key::Named::Tab)) {
                if self.state.keyboard_modifiers.shift() {
                    self.state.focus = self.state.focus.previous();
                } else {
                    self.state.focus = self.state.focus.next();
                }
                // TODO: maybe place this better
                self.clear_cache();
            } else {
                let sat_value_handle = |key_code: &keyboard::Key, color: &mut Color| {
                    let mut hsv_color: Hsv = (*color).into();
                    let mut status = event::Status::Ignored;

                    match key_code {
                        keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => {
                            hsv_color.saturation -= SAT_VALUE_STEP;
                            status = event::Status::Captured;
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowRight) => {
                            hsv_color.saturation += SAT_VALUE_STEP;
                            status = event::Status::Captured;
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => {
                            hsv_color.value -= SAT_VALUE_STEP;
                            status = event::Status::Captured;
                        }
                        keyboard::Key::Named(keyboard::key::Named::ArrowDown) => {
                            hsv_color.value += SAT_VALUE_STEP;
                            status = event::Status::Captured;
                        }
                        _ => {}
                    }

                    hsv_color.saturation = hsv_color.saturation.clamp(0.0, 1.0);
                    hsv_color.value = hsv_color.value.clamp(0.0, 1.0);

                    *color = Color {
                        a: color.a,
                        ..hsv_color.into()
                    };
                    status
                };

                let hue_handle = |key_code: &keyboard::Key, color: &mut Color| {
                    let mut hsv_color: Hsv = (*color).into();
                    let mut status = event::Status::Ignored;

                    let mut value = i32::from(hsv_color.hue);

                    match key_code {
                        keyboard::Key::Named(
                            keyboard::key::Named::ArrowLeft | keyboard::key::Named::ArrowDown,
                        ) => {
                            value -= HUE_STEP;
                            status = event::Status::Captured;
                        }
                        keyboard::Key::Named(
                            keyboard::key::Named::ArrowRight | keyboard::key::Named::ArrowUp,
                        ) => {
                            value += HUE_STEP;
                            status = event::Status::Captured;
                        }
                        _ => {}
                    }

                    hsv_color.hue = value.rem_euclid(360) as u16;

                    *color = Color {
                        a: color.a,
                        ..hsv_color.into()
                    };

                    status
                };

                let rgba_bar_handle = |key_code: &keyboard::Key, value: &mut f32| {
                    let mut byte_value = (*value * 255.0) as i16;
                    let mut status = event::Status::Captured;

                    match key_code {
                        keyboard::Key::Named(
                            keyboard::key::Named::ArrowLeft | keyboard::key::Named::ArrowDown,
                        ) => {
                            byte_value -= RGBA_STEP;
                            status = event::Status::Captured;
                        }
                        keyboard::Key::Named(
                            keyboard::key::Named::ArrowRight | keyboard::key::Named::ArrowUp,
                        ) => {
                            byte_value += RGBA_STEP;
                            status = event::Status::Captured;
                        }
                        _ => {}
                    }
                    *value = f32::from(byte_value.clamp(0, 255)) / 255.0;

                    status
                };

                match self.state.focus {
                    Focus::SatValue => status = sat_value_handle(key, &mut self.state.color),
                    Focus::Hue => status = hue_handle(key, &mut self.state.color),
                    Focus::Red => status = rgba_bar_handle(key, &mut self.state.color.r),
                    Focus::Green => status = rgba_bar_handle(key, &mut self.state.color.g),
                    Focus::Blue => status = rgba_bar_handle(key, &mut self.state.color.b),
                    Focus::Alpha => status = rgba_bar_handle(key, &mut self.state.color.a),
                    _ => {}
                }

                // If color changed via keyboard, call on_color_change callback
                if status == event::Status::Captured
                    && let Some(on_color_change) = self.on_color_change
                {
                    shell.publish(on_color_change(self.state.color));
                }
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

impl<'a, Message, Theme> Overlay<Message, Theme, Renderer>
    for ColorPickerOverlay<'a, '_, Message, Theme>
where
    Message: 'static + Clone,
    Theme: 'a
        + style::color_picker::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
{
    fn update(
        &mut self,
        event: &Event,
        cursor: Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
    ) {
        if event::Status::Captured == self.on_event_keyboard(event, shell) {
            self.clear_cache();
            shell.capture_event();
            shell.request_redraw();
            return;
        }

        // ----------- Block 1 ----------------------
        let hsv_color_status = self.on_event_hsv_color(event, cursor, shell);
        // ----------- Block 1 end ------------------

        // ----------- Block 2 ----------------------
        // ----------- RGB Color -----------------------
        let rgba_color_status = self.on_event_rgba_color(event, cursor, shell);

        // ----------- Text input ----------------------
        // (nothing to handle for the hex text)

        // ----------- Buttons -------------------------
        let viewport = self.geometry.overlay.bounds();

        self.cancel_button.update(
            &mut self.tree.children[0],
            event,
            self.geometry.cancel,
            cursor,
            renderer,
            shell,
            &viewport,
        );

        let mut fake_messages: shell::Bus<Message> = shell::Bus::new();
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
            shell.publish((self.on_submit)(self.state.color));
            shell.capture_event();
            shell.request_redraw();
        }
        // ----------- Block 2 end ------------------

        if hsv_color_status == event::Status::Captured
            || rgba_color_status == event::Status::Captured
        {
            self.clear_cache();
            shell.capture_event();
            shell.request_redraw();
        }
    }

    fn mouse_interaction(&self, cursor: mouse::Cursor, renderer: &Renderer) -> mouse::Interaction {
        let mouse_interaction = mouse::Interaction::default();

        // Block 1
        let mut block1_mouse_interaction = mouse::Interaction::default();
        // HSV color
        if cursor.is_over(self.geometry.sat_value) {
            block1_mouse_interaction = block1_mouse_interaction.max(mouse::Interaction::Pointer);
        }
        if cursor.is_over(self.geometry.hue) {
            block1_mouse_interaction = block1_mouse_interaction.max(mouse::Interaction::Pointer);
        }

        // Block 2
        let mut block2_mouse_interaction = mouse::Interaction::default();
        // RGBA color
        for row in &self.geometry.rgba {
            if cursor.is_over(row.bar) {
                block2_mouse_interaction =
                    block2_mouse_interaction.max(mouse::Interaction::ResizingHorizontally);
            }
        }

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
            .max(block1_mouse_interaction)
            .max(block2_mouse_interaction)
            .max(cancel_mouse_interaction)
            .max(submit_mouse_interaction)
    }

    fn operate(&mut self, renderer: &Renderer, operation: &mut dyn widget::Operation) {
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
            style::color_picker::Catalog::style(theme, self.class, Status::Active),
        );
        let _ = style_sheet.insert(
            StyleState::Selected,
            style::color_picker::Catalog::style(theme, self.class, Status::Selected),
        );
        let _ = style_sheet.insert(
            StyleState::Hovered,
            style::color_picker::Catalog::style(theme, self.class, Status::Hovered),
        );
        let _ = style_sheet.insert(
            StyleState::Focused,
            style::color_picker::Catalog::style(theme, self.class, Status::Focused),
        );

        let mut style_state = StyleState::Active;
        if self.state.focus == Focus::Overlay {
            style_state = style_state.max(StyleState::Focused);
        }
        if cursor.is_over(bounds) {
            style_state = style_state.max(StyleState::Hovered);
        }

        renderer.with_layer(Rectangle::with_size(self.window), |renderer| {
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

            // ----------- Block 1 ----------------------
            block1(renderer, self, cursor, &style_sheet);

            // ----------- Block 2 ----------------------
            block2(renderer, self, cursor, theme, style, &bounds, &style_sheet);
        });
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

/// Moves the given [`Rectangle`] by the given [`Vector`].
fn translate(rectangle: Rectangle, offset: Vector) -> Rectangle {
    Rectangle {
        x: rectangle.x + offset.x,
        y: rectangle.y + offset.y,
        ..rectangle
    }
}

/// Moves the given [`Layout`] by the given [`Vector`].
fn translate_layout(layout: Layout, offset: Vector) -> Layout {
    layout.move_to(layout.position() + offset)
}

/// Computes the layout of the whole color picker overlay.
///
/// The layout is made of two blocks: the first one contains the HSV part, the
/// second one the RGBA part, the hex text and the buttons.
#[allow(clippy::too_many_lines)]
fn compute_geometry<'a, Message, Theme>(
    cancel_button: &mut Button<'a, Message, widget::Text<'a, Theme>, Theme>,
    submit_button: &mut Button<'a, Message, widget::Text<'a, Theme>, Theme>,
    tree: &mut Tree,
    position: Point,
    renderer: &Renderer,
    window: Size,
) -> Geometry
where
    Message: 'static + Clone,
    Theme: 'a + iced_widget::button::Catalog + iced_widget::text::Catalog,
{
    let landscape = window.width > window.height;

    let (max_width, max_height) = if landscape {
        (600.0, 300.0)
    } else {
        (300.0, 600.0)
    };

    // The space available for the two blocks
    let available = Size::new(
        (window.width - PADDING.x()).max(0.0).min(max_width),
        (window.height - PADDING.y()).max(0.0).min(max_height),
    );

    // Divide the available space in two equally sized parts
    let (block1_bounds, block2_bounds) = if landscape {
        let width = ((available.width - SPACING.0) / 2.0).max(0.0);

        (
            Rectangle::new(Point::ORIGIN, Size::new(width, available.height)),
            Rectangle::new(
                Point::new(width + SPACING.0, 0.0),
                Size::new(width, available.height),
            ),
        )
    } else {
        let height = ((available.height - SPACING.0) / 2.0).max(0.0);

        (
            Rectangle::new(Point::ORIGIN, Size::new(available.width, height)),
            Rectangle::new(
                Point::new(0.0, height + SPACING.0),
                Size::new(available.width, height),
            ),
        )
    };

    // ----------- Block 1 ----------------------
    let block1_size = block1_bounds.size();
    let block1_spacing = PADDING.y() / 2.0; // Average vertical padding
    let block1_free_height = (block1_size.height - block1_spacing).max(0.0);
    let sat_value_height = block1_free_height * 7.0 / 8.0;
    let hue_height = block1_free_height / 8.0;

    let block1_origin = Point::new(
        block1_bounds.x + PADDING.left,
        block1_bounds.y + PADDING.top,
    );

    let sat_value = Rectangle::new(
        block1_origin,
        Size::new(block1_size.width, sat_value_height),
    );
    let hue = Rectangle::new(
        Point::new(
            block1_origin.x,
            block1_origin.y + sat_value_height + block1_spacing,
        ),
        Size::new(block1_size.width, hue_height),
    );

    // ----------- Block 2 ----------------------
    sync_child(tree, 0, cancel_button);
    sync_child(tree, 1, submit_button);

    let block2_limits = Limits::new(Size::ZERO, block2_bounds.size());

    // Pre-Buttons TODO: get rid of it
    cancel_button.layout(&mut tree.children[0], renderer, &block2_limits);
    let pre_cancel_height = tree.children[0].size.height;

    let hex_height = (renderer.text_size().0 + PADDING.y()).min(block2_bounds.height);
    let hex_width = block2_bounds.width;

    let block2_limits = block2_limits.shrink(Size::new(
        0.0,
        pre_cancel_height + hex_height + 2.0 * SPACING.0,
    ));

    // RGBA Colors
    let mut rgba_colors: Column<Element<'_, Message, Theme, Renderer>> = Column::new();

    for _ in 0..4 {
        let row: Row<Element<'_, Message, Theme, Renderer>> = Row::new()
            .align_y(Vertical::Center)
            .spacing(SPACING)
            .padding(PADDING)
            .height(Length::Fill)
            .push(boxed(
                widget::Text::new("X:")
                    .align_x(Horizontal::Center)
                    .align_y(Vertical::Center),
            ))
            .push(boxed(
                Row::<Element<'_, Message, Theme, Renderer>>::new()
                    .width(Length::FillPortion(5))
                    .height(Length::Fill),
            ))
            .push(boxed(
                widget::Text::new("XXX")
                    .align_x(Horizontal::Center)
                    .align_y(Vertical::Center),
            ));

        rgba_colors = rgba_colors.push(boxed(row));
    }

    sync_child(tree, 2, &mut rgba_colors);
    let rgba_tree = &mut tree.children[2];
    rgba_colors.layout(rgba_tree, renderer, &block2_limits);

    let rgba_size = rgba_tree.size;

    // Buttons
    let button_limits = {
        let mut limits = block2_limits;
        limits.max.width = ((rgba_size.width / 2.0) - BUTTON_SPACING.0)
            .max(0.0)
            .min(limits.max.width)
            .max(limits.min.width);
        limits
    };

    cancel_button.layout(&mut tree.children[0], renderer, &button_limits);
    let cancel_size = tree.children[0].size;

    submit_button.layout(&mut tree.children[1], renderer, &button_limits);
    let submit_size = tree.children[1].size;

    let block2_origin = block2_bounds.position();

    // RGBA rows
    let rgba_layout = Layout::new(rgba_size).move_to(Point::new(
        block2_origin.x + PADDING.left,
        block2_origin.y + PADDING.top,
    ));

    let mut rgba = [RgbaRowGeometry::default(); 4];

    for (geometry, (row_layout, row_tree)) in rgba
        .iter_mut()
        .zip(rgba_layout.iter(&tree.children[2].children))
    {
        let mut cells = row_layout.iter(&row_tree.children);

        let mut next_bounds = || cells.next().map(|(layout, _)| layout.bounds());

        *geometry = RgbaRowGeometry {
            row: row_layout.bounds(),
            label: next_bounds().unwrap_or_default(),
            bar: next_bounds().unwrap_or_default(),
            value: next_bounds().unwrap_or_default(),
        };
    }

    // Hex text
    let hex = Rectangle::new(
        Point::new(
            block2_origin.x + PADDING.left,
            block2_origin.y + rgba_size.height + PADDING.top + SPACING.0,
        ),
        Size::new(hex_width, hex_height),
    );

    // Buttons
    let buttons_y = block2_origin.y + rgba_size.height + hex_height + PADDING.top + 2.0 * SPACING.0;

    let cancel =
        Layout::new(cancel_size).move_to(Point::new(block2_origin.x + PADDING.left, buttons_y));
    let submit = Layout::new(submit_size).move_to(Point::new(
        block2_origin.x + rgba_size.width - submit_size.width + PADDING.left,
        buttons_y,
    ));

    let block2_size = Size::new(
        rgba_size.width + PADDING.x(),
        rgba_size.height + hex_height + cancel_size.height + PADDING.y() + (2.0 * SPACING.0),
    );

    // ----------- The whole overlay ------------
    let size = if landscape {
        Size::new(
            block1_size.width + block2_size.width + SPACING.0,
            block2_size.height,
        )
    } else {
        Size::new(
            block2_size.width,
            block1_size.height + block2_size.height + SPACING.0,
        )
    };

    let mut overlay = Layout::new(size);
    overlay.center_and_bounce(position, window);

    // Everything above is relative to the overlay, make it absolute
    let offset = Vector::new(overlay.position().x, overlay.position().y);

    for row in &mut rgba {
        row.row = translate(row.row, offset);
        row.label = translate(row.label, offset);
        row.bar = translate(row.bar, offset);
        row.value = translate(row.value, offset);
    }

    Geometry {
        overlay,
        sat_value: translate(sat_value, offset),
        hue: translate(hue, offset),
        rgba,
        hex: translate(hex, offset),
        cancel: translate_layout(cancel, offset),
        submit: translate_layout(submit, offset),
    }
}

/// Draws the 1. block of the color picker containing the HSV part.
fn block1<Message, Theme>(
    renderer: &mut Renderer,
    color_picker: &ColorPickerOverlay<'_, '_, Message, Theme>,
    cursor: Cursor,
    style_sheet: &HashMap<StyleState, Style>,
) where
    Message: Clone,
    Theme: style::color_picker::Catalog + iced_widget::button::Catalog + iced_widget::text::Catalog,
{
    // ----------- Block 1 ----------------------
    // ----------- HSV Color ----------------------
    hsv_color(renderer, color_picker, cursor, style_sheet);

    // ----------- Block 1 end ------------------
}

/// Draws the 2. block of the color picker containing the RGBA part, Hex and buttons.
#[allow(clippy::too_many_arguments)]
fn block2<Message, Theme>(
    renderer: &mut Renderer,
    color_picker: &ColorPickerOverlay<'_, '_, Message, Theme>,
    cursor: Cursor,
    theme: &Theme,
    style: &renderer::Style,
    viewport: &Rectangle,
    style_sheet: &HashMap<StyleState, Style>,
) where
    Message: Clone,
    Theme: style::color_picker::Catalog + iced_widget::button::Catalog + iced_widget::text::Catalog,
{
    // ----------- Block 2 ----------------------
    // ----------- RGBA Color ----------------------
    rgba_color(
        renderer,
        &color_picker.geometry.rgba,
        &color_picker.state.color,
        cursor,
        style,
        style_sheet,
        color_picker.state.focus,
    );

    // ----------- Hex text ----------------------
    hex_text(
        renderer,
        color_picker.geometry.hex,
        &color_picker.state.color,
        cursor,
        style,
        style_sheet,
        color_picker.state.focus,
    );

    // ----------- Buttons -------------------------
    let cancel_button_layout = color_picker.geometry.cancel;

    color_picker.cancel_button.draw(
        &color_picker.tree.children[0],
        renderer,
        theme,
        style,
        cancel_button_layout,
        cursor,
        viewport,
    );

    let submit_button_layout = color_picker.geometry.submit;

    color_picker.submit_button.draw(
        &color_picker.tree.children[1],
        renderer,
        theme,
        style,
        submit_button_layout,
        cursor,
        viewport,
    );

    // Buttons are not focusable right now...
    if color_picker.state.focus == Focus::Cancel {
        let bounds = cancel_button_layout.bounds();
        if (bounds.width > 0.) && (bounds.height > 0.) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
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

    if color_picker.state.focus == Focus::Submit {
        let bounds = submit_button_layout.bounds();
        if (bounds.width > 0.) && (bounds.height > 0.) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
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
    // ----------- Block 2 end ------------------
}

/// Draws the HSV color area.
#[allow(clippy::too_many_lines)]
fn hsv_color<Message, Theme>(
    renderer: &mut Renderer,
    color_picker: &ColorPickerOverlay<'_, '_, Message, Theme>,
    cursor: Cursor,
    style_sheet: &HashMap<StyleState, Style>,
) where
    Message: Clone,
    Theme: style::color_picker::Catalog + iced_widget::button::Catalog + iced_widget::text::Catalog,
{
    let hsv_color: Hsv = color_picker.state.color.into();

    let sat_value_bounds = color_picker.geometry.sat_value;
    let mut sat_value_style_state = StyleState::Active;
    if color_picker.state.focus == Focus::SatValue {
        sat_value_style_state = sat_value_style_state.max(StyleState::Focused);
    }
    if cursor.is_over(sat_value_bounds) {
        sat_value_style_state = sat_value_style_state.max(StyleState::Hovered);
    }

    let geometry = color_picker.state.sat_value_canvas_cache.draw(
        renderer,
        sat_value_bounds.size(),
        |frame| {
            let column_count = frame.width() as u16;
            let row_count = frame.height() as u16;

            for column in 0..column_count {
                for row in 0..row_count {
                    let saturation = f32::from(column) / frame.width();
                    let value = f32::from(row) / frame.height();

                    frame.fill_rectangle(
                        Point::new(f32::from(column), f32::from(row)),
                        Size::new(1.0, 1.0),
                        Color::from(Hsv::from_hsv(hsv_color.hue, saturation, value)),
                    );
                }
            }

            let stroke = Stroke {
                style: canvas::Style::Solid(
                    Hsv {
                        hue: 0,
                        saturation: 0.0,
                        value: 1.0 - hsv_color.value,
                    }
                    .into(),
                ),
                width: 3.0,
                line_cap: LineCap::Round,
                ..Stroke::default()
            };

            let saturation = hsv_color.saturation * frame.width();
            let value = hsv_color.value * frame.height();

            frame.stroke(
                &Path::line(
                    Point::new(saturation, 0.0),
                    Point::new(saturation, frame.height()),
                ),
                stroke,
            );

            frame.stroke(
                &Path::line(Point::new(0.0, value), Point::new(frame.width(), value)),
                stroke,
            );

            let stroke = Stroke {
                style: canvas::Style::Solid(
                    style_sheet
                        .get(&sat_value_style_state)
                        .expect("Style Sheet not found.")
                        .bar_border_color,
                ),
                width: 2.0,
                line_cap: LineCap::Round,
                ..Stroke::default()
            };

            frame.stroke(
                &Path::rectangle(
                    Point::new(0.0, 0.0),
                    Size::new(frame.size().width - 0.0, frame.size().height - 0.0),
                ),
                stroke,
            );
        },
    );

    let translation = Vector::new(sat_value_bounds.x, sat_value_bounds.y);
    renderer.with_translation(translation, |renderer| {
        renderer.draw_geometry(geometry);
    });

    let hue_bounds = color_picker.geometry.hue;
    let mut hue_style_state = StyleState::Active;
    if color_picker.state.focus == Focus::Hue {
        hue_style_state = hue_style_state.max(StyleState::Focused);
    }
    if cursor.is_over(hue_bounds) {
        hue_style_state = hue_style_state.max(StyleState::Hovered);
    }

    let geometry = color_picker
        .state
        .hue_canvas_cache
        .draw(renderer, hue_bounds.size(), |frame| {
            let column_count = frame.width() as u16;

            for column in 0..column_count {
                let hue = (f32::from(column) * 360.0 / frame.width()) as u16;

                let hsv_color = Hsv::from_hsv(hue, 1.0, 1.0);
                let stroke = Stroke {
                    style: canvas::Style::Solid(hsv_color.into()),
                    width: 1.0,
                    line_cap: LineCap::Round,
                    ..Stroke::default()
                };

                frame.stroke(
                    &Path::line(
                        Point::new(f32::from(column), 0.0),
                        Point::new(f32::from(column), frame.height()),
                    ),
                    stroke,
                );
            }

            let stroke = Stroke {
                style: canvas::Style::Solid(Color::BLACK),
                width: 3.0,
                line_cap: LineCap::Round,
                ..Stroke::default()
            };

            let column = f32::from(hsv_color.hue) * frame.width() / 360.0;

            frame.stroke(
                &Path::line(Point::new(column, 0.0), Point::new(column, frame.height())),
                stroke,
            );

            let stroke = Stroke {
                style: canvas::Style::Solid(
                    style_sheet
                        .get(&hue_style_state)
                        .expect("Style Sheet not found.")
                        .bar_border_color,
                ),
                width: 2.0,
                line_cap: LineCap::Round,
                ..Stroke::default()
            };

            frame.stroke(
                &Path::rectangle(
                    Point::new(0.0, 0.0),
                    Size::new(frame.size().width, frame.size().height),
                ),
                stroke,
            );
        });

    let translation = Vector::new(hue_bounds.x, hue_bounds.y);
    renderer.with_translation(translation, |renderer| {
        renderer.draw_geometry(geometry);
    });
}

/// Draws the RGBA color area.
#[allow(clippy::too_many_lines)]
fn rgba_color(
    renderer: &mut Renderer,
    rows: &[RgbaRowGeometry; 4],
    color: &Color,
    cursor: Cursor,
    style: &renderer::Style,
    style_sheet: &HashMap<StyleState, Style>,
    focus: Focus,
) {
    let f = |renderer: &mut Renderer,
             row: &RgbaRowGeometry,
             label: &str,
             color: Color,
             value: f32,
             cursor: Cursor,
             target: Focus| {
        let label_bounds = row.label;
        let value_bounds = row.value;

        // Label
        renderer.fill_text(
            Text {
                content: label.to_owned(),
                bounds: Size::new(label_bounds.width, label_bounds.height),
                size: renderer.text_size(),
                font: renderer.font(),
                align_x: text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: text::LineHeight::Relative(1.3),
                shaping: text::Shaping::Basic,
                wrapping: Wrapping::None,
                ellipsis: text::Ellipsis::None,
                hint_factor: renderer.hint_factor(),
            },
            Point::new(label_bounds.center_x(), label_bounds.center_y()),
            style.text_color,
            label_bounds,
        );

        let bar_bounds = row.bar;

        let bar_style_state = if cursor.is_over(bar_bounds) {
            StyleState::Hovered
        } else {
            StyleState::Active
        };

        // Bar background
        let background_bounds = Rectangle {
            x: bar_bounds.x,
            y: bar_bounds.y,
            width: bar_bounds.width * value,
            height: bar_bounds.height,
        };
        if (background_bounds.width > 0.) && (background_bounds.height > 0.) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: background_bounds,
                    border: Border {
                        radius: style_sheet
                            .get(&bar_style_state)
                            .expect("Style Sheet not found.")
                            .bar_border_radius
                            .into(),
                        width: style_sheet
                            .get(&bar_style_state)
                            .expect("Style Sheet not found.")
                            .bar_border_width,
                        color: Color::TRANSPARENT,
                        ..Border::default()
                    },
                    ..renderer::Quad::default()
                },
                color,
            );
        }

        // Bar
        if (bar_bounds.width > 0.) && (bar_bounds.height > 0.) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: bar_bounds,
                    border: Border {
                        radius: style_sheet
                            .get(&bar_style_state)
                            .expect("Style Sheet not found.")
                            .bar_border_radius
                            .into(),
                        width: style_sheet
                            .get(&bar_style_state)
                            .expect("Style Sheet not found.")
                            .bar_border_width,
                        color: style_sheet
                            .get(&bar_style_state)
                            .expect("Style Sheet not found.")
                            .bar_border_color,
                        ..Border::default()
                    },
                    ..renderer::Quad::default()
                },
                Color::TRANSPARENT,
            );
        }

        // Value
        renderer.fill_text(
            Text {
                content: format!("{}", (255.0 * value) as u8),
                bounds: Size::new(value_bounds.width, value_bounds.height),
                size: renderer.text_size(),
                font: renderer.font(),
                align_x: text::Alignment::Center,
                align_y: Vertical::Center,
                line_height: text::LineHeight::Relative(1.3),
                shaping: text::Shaping::Basic,
                wrapping: Wrapping::None,
                ellipsis: text::Ellipsis::None,
                hint_factor: renderer.hint_factor(),
            },
            Point::new(value_bounds.center_x(), value_bounds.center_y()),
            style.text_color,
            value_bounds,
        );

        let bounds = row.row;
        if (focus == target) && (bounds.width > 0.) && (bounds.height > 0.) {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: Border {
                        radius: style_sheet
                            .get(&StyleState::Focused)
                            .expect("Style Sheet not found.")
                            .border_radius
                            .into(),
                        width: style_sheet
                            .get(&StyleState::Focused)
                            .expect("Style Sheet not found.")
                            .border_width,
                        color: style_sheet
                            .get(&StyleState::Focused)
                            .expect("Style Sheet not found.")
                            .border_color,
                        ..Border::default()
                    },
                    ..renderer::Quad::default()
                },
                Color::TRANSPARENT,
            );
        }
    };

    // Red

    f(
        renderer,
        &rows[0],
        "R",
        Color::from_rgb(color.r, 0.0, 0.0),
        color.r,
        cursor,
        Focus::Red,
    );

    // Green

    f(
        renderer,
        &rows[1],
        "G",
        Color::from_rgb(0.0, color.g, 0.0),
        color.g,
        cursor,
        Focus::Green,
    );

    // Blue

    f(
        renderer,
        &rows[2],
        "B",
        Color::from_rgb(0.0, 0.0, color.b),
        color.b,
        cursor,
        Focus::Blue,
    );

    // Alpha

    f(
        renderer,
        &rows[3],
        "A",
        Color::from_rgba(0.0, 0.0, 0.0, color.a),
        color.a,
        cursor,
        Focus::Alpha,
    );
}

/// Draws the hex text representation of the color.
fn hex_text(
    renderer: &mut Renderer,
    bounds: Rectangle,
    color: &Color,
    cursor: Cursor,
    _style: &renderer::Style,
    style_sheet: &HashMap<StyleState, Style>,
    _focus: Focus,
) {
    let hsv: Hsv = (*color).into();

    let hex_text_style_state = if cursor.is_over(bounds) {
        StyleState::Hovered
    } else {
        StyleState::Active
    };

    if (bounds.width > 0.) && (bounds.height > 0.) {
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border {
                    radius: style_sheet[&hex_text_style_state].bar_border_radius.into(),
                    width: style_sheet[&hex_text_style_state].bar_border_width,
                    color: style_sheet[&hex_text_style_state].bar_border_color,
                    ..Border::default()
                },
                ..renderer::Quad::default()
            },
            *color,
        );
    }

    renderer.fill_text(
        Text {
            content: color.as_hex_string(),
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
        Color {
            a: 1.0,
            ..Hsv {
                hue: 0,
                saturation: 0.0,
                value: if hsv.value < 0.5 { 1.0 } else { 0.0 },
            }
            .into()
        },
        bounds,
    );
}

/// The state of the [`ColorPickerOverlay`].
#[derive(Debug)]
pub struct State {
    /// The selected color of the [`ColorPickerOverlay`].
    pub(crate) color: Color,
    /// The color used to initialize [`ColorPickerOverlay`].
    pub(crate) initial_color: Color,
    /// The cache of the sat/value canvas of the [`ColorPickerOverlay`].
    pub(crate) sat_value_canvas_cache: canvas::Cache,
    /// The cache of the hue canvas of the [`ColorPickerOverlay`].
    pub(crate) hue_canvas_cache: canvas::Cache,
    /// The dragged color bar of the [`ColorPickerOverlay`].
    pub(crate) color_bar_dragged: ColorBarDragged,
    /// the focus of the [`ColorPickerOverlay`].
    pub(crate) focus: Focus,
    /// The previously pressed keyboard modifiers.
    pub(crate) keyboard_modifiers: keyboard::Modifiers,
}

impl State {
    /// Creates a new State with the given color.
    #[must_use]
    pub fn new(color: Color) -> Self {
        Self {
            color,
            initial_color: color,
            ..Self::default()
        }
    }

    /// Reset cached canvas when internal state is modified.
    ///
    /// If the color has changed, empty all canvas caches
    /// as they (unfortunately) do not depend on the picker state.
    fn clear_cache(&self) {
        self.sat_value_canvas_cache.clear();
        self.hue_canvas_cache.clear();
    }

    /// Synchronize the color with an externally provided value.
    pub(crate) fn force_synchronize(&mut self, color: Color) {
        self.initial_color = color;
        self.color = color;
        self.clear_cache();
    }
}

impl Default for State {
    fn default() -> Self {
        let default_color = Color::from_rgb(0.5, 0.25, 0.25);
        Self {
            color: default_color,
            initial_color: default_color,
            sat_value_canvas_cache: canvas::Cache::default(),
            hue_canvas_cache: canvas::Cache::default(),
            color_bar_dragged: ColorBarDragged::None,
            focus: Focus::default(),
            keyboard_modifiers: keyboard::Modifiers::default(),
        }
    }
}

/// Just a workaround to pass the button states from the tree to the overlay
#[allow(missing_debug_implementations)]
pub struct ColorPickerOverlayButtons<'a, Message, Theme>
where
    Message: Clone,
    Theme: style::color_picker::Catalog + iced_widget::button::Catalog,
{
    /// The cancel button of the [`ColorPickerOverlay`].
    cancel_button: Element<'a, Message, Theme, Renderer>,
    /// The submit button of the [`ColorPickerOverlay`].
    submit_button: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme> Default for ColorPickerOverlayButtons<'a, Message, Theme>
where
    Message: 'a + Clone,
    Theme: 'a
        + style::color_picker::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
{
    fn default() -> Self {
        let (cancel_content, cancel_font, _cancel_shaping) = cancel();
        let (submit_content, submit_font, _submit_shaping) = ok();

        Self {
            cancel_button: boxed(Button::new(
                widget::Text::new(cancel_content).font(cancel_font),
            )),
            submit_button: boxed(Button::new(
                widget::Text::new(submit_content).font(submit_font),
            )),
        }
    }
}

impl<Message, Theme> Meta for ColorPickerOverlayButtons<'_, Message, Theme>
where
    Message: Clone,
    Theme: style::color_picker::Catalog + iced_widget::button::Catalog,
{
}

#[allow(clippy::unimplemented)]
impl<Message, Theme> Widget<Message, Theme, Renderer>
    for ColorPickerOverlayButtons<'_, Message, Theme>
where
    Message: Clone,
    Theme: style::color_picker::Catalog + iced_widget::button::Catalog + iced_widget::text::Catalog,
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

impl<'a, Message, Theme> From<ColorPickerOverlayButtons<'a, Message, Theme>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a
        + style::color_picker::Catalog
        + iced_widget::button::Catalog
        + iced_widget::text::Catalog,
{
    fn from(overlay: ColorPickerOverlayButtons<'a, Message, Theme>) -> Self {
        boxed(overlay)
    }
}

/// The state of the currently dragged area.
#[derive(Copy, Clone, Debug, Default)]
pub enum ColorBarDragged {
    /// No area is focussed.
    #[default]
    None,

    /// The saturation/value area is focussed.
    SatValue,

    /// The hue area is focussed.
    Hue,

    /// The red area is focussed.
    Red,

    /// The green area is focussed.
    Green,

    /// The blue area is focussed.
    Blue,

    /// The alpha area is focussed.
    Alpha,
}

/// An enumeration of all focusable element of the [`ColorPickerOverlay`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Focus {
    /// Nothing is in focus.
    #[default]
    None,

    /// The overlay itself is in focus.
    Overlay,

    /// The saturation and value area is in focus.
    SatValue,

    /// The hue bar is in focus.
    Hue,

    /// The red bar is in focus.
    Red,

    /// The green bar is in focus.
    Green,

    /// The blue bar is in focus.
    Blue,

    /// The alpha bar is in focus.
    Alpha,

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
            Self::Overlay => Self::SatValue,
            Self::SatValue => Self::Hue,
            Self::Hue => Self::Red,
            Self::Red => Self::Green,
            Self::Green => Self::Blue,
            Self::Blue => Self::Alpha,
            Self::Alpha => Self::Cancel,
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
            Self::SatValue => Self::Overlay,
            Self::Hue => Self::SatValue,
            Self::Red => Self::Hue,
            Self::Green => Self::Red,
            Self::Blue => Self::Green,
            Self::Alpha => Self::Blue,
            Self::Cancel => Self::Alpha,
            Self::Submit => Self::Cancel,
        }
    }
}
