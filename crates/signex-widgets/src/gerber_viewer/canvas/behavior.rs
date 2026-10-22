use super::*;

impl GerberCanvas<'_> {
    fn screen_to_world(&self, bounds: Rectangle, screen: Point) -> Option<signex_gerber::Point> {
        let world_bounds = page_bounds(visible_bounds(self.layers), self.page_size)?;
        let (scale, world_center, screen_center) =
            fit_transform(world_bounds, bounds, self.zoom, self.pan);
        Some(screen_to_world_point(
            screen,
            world_center,
            screen_center,
            scale,
            self.mirrored,
        ))
    }

    fn pixels_per_world_unit(&self, bounds: Rectangle) -> Option<f32> {
        let world_bounds = page_bounds(visible_bounds(self.layers), self.page_size)?;
        Some(fit_transform(world_bounds, bounds, self.zoom, self.pan).0)
    }
}

impl canvas::Program<GerberViewerMessage> for GerberCanvas<'_> {
    type State = GerberCanvasState;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<GerberViewerMessage>> {
        match event {
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                gerber_shortcut_message(self.shortcut_resolver, key, *modifiers)
                    .map(|message| canvas::Action::publish(message).and_capture())
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                if !cursor.is_over(bounds) {
                    return None;
                }
                let lines = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / 30.0,
                };
                Some(
                    canvas::Action::publish(GerberViewerMessage::ZoomBy(1.12_f32.powf(lines)))
                        .and_capture(),
                )
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)) => {
                state.drag_start = cursor.position_in(bounds);
                Some(canvas::Action::capture())
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if self.zoom_selection_active =>
            {
                let position = cursor.position_in(bounds)?;
                state.zoom_selection_start = Some(position);
                state.zoom_selection_current = Some(position);
                Some(canvas::Action::capture())
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if self.measurement_active =>
            {
                let position = cursor.position_in(bounds)?;
                let world = self.screen_to_world(bounds, position)?;
                state.measurement_dragging = true;
                Some(
                    canvas::Action::publish(GerberViewerMessage::BeginMeasurement(world))
                        .and_capture(),
                )
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if self.selection_active =>
            {
                let position = cursor.position_in(bounds)?;
                state.item_selection_start = Some(position);
                state.item_selection_current = Some(position);
                Some(canvas::Action::capture())
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                if state.zoom_selection_start.is_some() {
                    state.zoom_selection_current =
                        Some(Point::new(position.x - bounds.x, position.y - bounds.y));
                    Some(canvas::Action::capture())
                } else if state.item_selection_start.is_some() {
                    state.item_selection_current =
                        Some(Point::new(position.x - bounds.x, position.y - bounds.y));
                    Some(canvas::Action::capture())
                } else if state.measurement_dragging {
                    let position = cursor.position_in(bounds)?;
                    let world = self.screen_to_world(bounds, position)?;
                    Some(
                        canvas::Action::publish(GerberViewerMessage::UpdateMeasurement(world))
                            .and_capture(),
                    )
                } else if let Some(previous) = state.drag_start {
                    let current = Point::new(position.x - bounds.x, position.y - bounds.y);
                    state.drag_start = Some(current);
                    Some(
                        canvas::Action::publish(GerberViewerMessage::PanBy(current - previous))
                            .and_capture(),
                    )
                } else {
                    let position = cursor.position_in(bounds)?;
                    Some(canvas::Action::publish(
                        GerberViewerMessage::CursorWorldPositionChanged(
                            self.screen_to_world(bounds, position),
                        ),
                    ))
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Middle)) => {
                state.drag_start = None;
                Some(canvas::Action::capture())
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if state.zoom_selection_start.is_some() =>
            {
                let start = state.zoom_selection_start.take()?;
                let end = state.zoom_selection_current.take().unwrap_or(start);
                let Some(selection) = normalized_screen_rectangle(start, end) else {
                    return Some(canvas::Action::capture());
                };
                let Some(world_start) = self.screen_to_world(
                    bounds,
                    Point::new(selection.x, selection.y + selection.height),
                ) else {
                    return Some(canvas::Action::capture());
                };
                let Some(world_end) = self.screen_to_world(
                    bounds,
                    Point::new(selection.x + selection.width, selection.y),
                ) else {
                    return Some(canvas::Action::capture());
                };
                Some(
                    canvas::Action::publish(GerberViewerMessage::ZoomToSelection {
                        bounds: Bounds {
                            min: signex_gerber::Point {
                                x: world_start.x.min(world_end.x),
                                y: world_start.y.min(world_end.y),
                            },
                            max: signex_gerber::Point {
                                x: world_start.x.max(world_end.x),
                                y: world_start.y.max(world_end.y),
                            },
                        },
                        viewport: bounds,
                    })
                    .and_capture(),
                )
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if state.measurement_dragging =>
            {
                state.measurement_dragging = false;
                let position = cursor.position_in(bounds)?;
                let world = self.screen_to_world(bounds, position)?;
                Some(
                    canvas::Action::publish(GerberViewerMessage::CompleteMeasurement(world))
                        .and_capture(),
                )
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if state.item_selection_start.is_some() =>
            {
                let start = state.item_selection_start.take()?;
                let end = state.item_selection_current.take().unwrap_or(start);
                let selection = normalized_screen_rectangle(start, end);
                if let Some(selection) =
                    selection.filter(|selection| selection.width >= 4.0 || selection.height >= 4.0)
                {
                    let world_first =
                        self.screen_to_world(bounds, Point::new(selection.x, selection.y))?;
                    let world_second = self.screen_to_world(
                        bounds,
                        Point::new(
                            selection.x + selection.width,
                            selection.y + selection.height,
                        ),
                    )?;
                    let selections = hit_test_visible_items_in_bounds(
                        self.layers,
                        Bounds {
                            min: signex_gerber::Point {
                                x: world_first.x.min(world_second.x),
                                y: world_first.y.min(world_second.y),
                            },
                            max: signex_gerber::Point {
                                x: world_first.x.max(world_second.x),
                                y: world_first.y.max(world_second.y),
                            },
                        },
                    );
                    return Some(
                        canvas::Action::publish(GerberViewerMessage::SetRegionSelection(
                            selections,
                        ))
                        .and_capture(),
                    );
                }
                let world = self.screen_to_world(bounds, end)?;
                let scale = self.pixels_per_world_unit(bounds)?;
                let selection = hit_test_visible_item(self.layers, world, 6.0 / f64::from(scale));
                Some(
                    canvas::Action::publish(GerberViewerMessage::SetSelectedItem(selection))
                        .and_capture(),
                )
            }
            Event::Mouse(mouse::Event::CursorLeft) => Some(canvas::Action::publish(
                GerberViewerMessage::CursorWorldPositionChanged(None),
            )),
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        self.draw_view(state, renderer, theme, bounds, cursor)
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.drag_start.is_some() {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) && self.crosshair_mode != GerberCrosshairMode::None {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}
