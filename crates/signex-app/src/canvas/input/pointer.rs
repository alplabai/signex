use super::super::*;

impl SchematicCanvas {
    /// Left-press: select, tool action, start box-select, or start drag-move.
    pub(in crate::canvas) fn update_left_pressed(
        &self,
        state: &mut CanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if let Some(cursor_pos) = cursor.position_in(bounds) {
            let world = state.camera.screen_to_world(cursor_pos, bounds);
            let wx = world.x as f64;
            let wy = world.y as f64;

            // Double-click detection (300ms, 3mm threshold)
            let now = std::time::Instant::now();
            if let (Some(last_time), Some(last_pos)) =
                (state.last_click_time, state.last_click_world)
            {
                let dt = now.duration_since(last_time);
                let dist = ((wx - last_pos.0).powi(2) + (wy - last_pos.1).powi(2)).sqrt();
                if dt.as_millis() < 300 && dist < 3.0 {
                    state.last_click_time = None;
                    state.last_click_world = None;
                    state.select_drag_start = None;
                    state.click_on_selected = false;
                    return Some(canvas::Action::publish(Message::CanvasEvent(
                        CanvasEvent::DoubleClicked {
                            world_x: wx,
                            world_y: wy,
                            screen_x: cursor_pos.x,
                            screen_y: cursor_pos.y,
                        },
                    )));
                }
            }
            state.last_click_time = Some(now);
            state.last_click_world = Some((wx, wy));

            // Classify the click target:
            //   - hit an already-selected item  → defer click, prepare drag
            //   - hit an unselected item       → publish click (selects it), prepare drag
            //   - hit empty space              → publish click, start box-select
            // Altium-style: clicking and dragging on an unselected item
            // should immediately select-and-drag in one gesture.
            let (on_selected, on_unselected_item) = if !self.drawing_mode {
                if let Some(snapshot) = self.active_snapshot() {
                    if let Some(hit) =
                        crate::schematic_runtime::hit_test::hit_test(snapshot, wx, wy)
                    {
                        let sel = self.selected.iter().any(|s| s.uuid == hit.uuid);
                        (sel, !sel)
                    } else {
                        (false, false)
                    }
                } else {
                    (false, false)
                }
            } else {
                (false, false)
            };

            if on_selected && !state.ctrl_held {
                // Defer click — prepare for potential drag-to-move
                state.click_on_selected = true;
                state.move_origin = Some((wx, wy));
                state.move_dragging = false;
                state.move_current = None;
                state.select_drag_start = None;
                return Some(canvas::Action::capture());
            }

            if on_unselected_item && !state.ctrl_held {
                // Publish the click (which selects the item via HitAt)
                // AND prepare drag state. If the user crosses the drag
                // threshold before mouse-up, the motion handler promotes
                // this into a move gesture without a second click.
                // `and_capture()` keeps subsequent mouse events flowing
                // through this program so the drag detection stays live.
                state.click_on_selected = true;
                state.move_origin = Some((wx, wy));
                state.move_dragging = false;
                state.move_current = None;
                state.select_drag_start = None;
                return Some(
                    canvas::Action::publish(Message::CanvasEvent(CanvasEvent::Clicked {
                        world_x: wx,
                        world_y: wy,
                    }))
                    .and_capture(),
                );
            }

            // Normal click — publish immediately, start potential box-select
            state.click_on_selected = false;
            state.move_origin = None;
            state.move_dragging = false;
            // Don't track box-select during drawing mode (avoids spurious BoxSelect events)
            if !self.drawing_mode {
                state.select_drag_start = Some((wx, wy));
                state.select_drag_end = None;
            } else {
                state.select_drag_start = None;
                state.select_drag_end = None;
            }
            // Ctrl+Click toggles selection (add if missing, remove if
            // present). Shift+Click adds to selection (Altium-style).
            let evt = if state.ctrl_held || state.shift_held {
                CanvasEvent::CtrlClicked {
                    world_x: wx,
                    world_y: wy,
                }
            } else {
                CanvasEvent::Clicked {
                    world_x: wx,
                    world_y: wy,
                }
            };
            return Some(canvas::Action::publish(Message::CanvasEvent(evt)));
        }
        None
    }

    /// Left-release: finish drag-move, deferred click, or box-select.
    pub(in crate::canvas) fn update_left_released(
        &self,
        state: &mut CanvasState,
    ) -> Option<canvas::Action<Message>> {
        // Case 1: Drag-to-move in progress → commit the move
        if state.move_dragging {
            if let (Some(origin), Some(current)) = (state.move_origin, state.move_current) {
                let dx = current.0 - origin.0;
                let dy = current.1 - origin.1;
                state.move_dragging = false;
                state.move_origin = None;
                state.move_current = None;
                state.click_on_selected = false;
                if dx.abs() > 0.01 || dy.abs() > 0.01 {
                    return Some(canvas::Action::publish(Message::CanvasEvent(
                        CanvasEvent::MoveSelected { dx, dy },
                    )));
                }
            }
            return None;
        }

        // Case 2: Click was on selected item but didn't drag → deferred click
        if state.click_on_selected {
            state.click_on_selected = false;
            if let Some(origin) = state.move_origin.take() {
                return Some(canvas::Action::publish(Message::CanvasEvent(
                    CanvasEvent::Clicked {
                        world_x: origin.0,
                        world_y: origin.1,
                    },
                )));
            }
            return None;
        }

        // Case 3: Box-select drag
        if let (Some(start), Some(end)) =
            (state.select_drag_start.take(), state.select_drag_end.take())
        {
            let dx = (end.0 - start.0).abs();
            let dy = (end.1 - start.1).abs();
            if dx > 2.0 || dy > 2.0 {
                return Some(canvas::Action::publish(Message::CanvasEvent(
                    CanvasEvent::BoxSelect {
                        x1: start.0.min(end.0),
                        y1: start.1.min(end.1),
                        x2: start.0.max(end.0),
                        y2: start.1.max(end.1),
                    },
                )));
            }
        } else {
            state.select_drag_start = None;
        }
        None
    }

    /// Right-press: cancel drag, else start pan or Active Bar dropdown.
    pub(in crate::canvas) fn update_right_pressed(
        &self,
        state: &mut CanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        // Abort an in-progress drag — matches Altium / Esc behavior.
        if state.move_dragging || state.click_on_selected {
            state.move_dragging = false;
            state.click_on_selected = false;
            state.move_origin = None;
            state.move_current = None;
            return Some(canvas::Action::capture());
        }
        if let Some(pos) = cursor.position_in(bounds) {
            // Active Bar zone: top ~46px, centered (bar 36px + 4 top margin + slack)
            if pos.y < 46.0 {
                // Calculate which Active Bar button was right-clicked
                let bar_width: f32 = crate::active_bar::BAR_WIDTH_PX;
                let bar_x = (bounds.width - bar_width) / 2.0;
                let rel_x = pos.x - bar_x;
                if rel_x >= 0.0
                    && rel_x < bar_width
                    && let Some(menu) = active_bar_hit(rel_x)
                {
                    return Some(canvas::Action::publish(Message::ActiveBar(
                        crate::active_bar::ActiveBarMsg::ToggleMenu(menu),
                    )));
                }
                // Prevent panning when right-clicking in the Active Bar zone
                return Some(canvas::Action::capture());
            }
            state.panning = true;
            state.pan_moved = false;
            state.last_pan_pos = Some(pos);
        }
        Some(canvas::Action::capture())
    }

    /// Middle-press: start pan.
    pub(in crate::canvas) fn update_middle_pressed(
        &self,
        state: &mut CanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if let Some(pos) = cursor.position_in(bounds) {
            state.panning = true;
            state.pan_moved = false;
            state.last_pan_pos = Some(pos);
        }
        Some(canvas::Action::capture())
    }

    /// Right-release: stop pan, context menu, or cancel drawing.
    pub(in crate::canvas) fn update_right_released(
        &self,
        state: &mut CanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        let did_pan = state.pan_moved;
        state.panning = false;
        state.pan_moved = false;
        state.last_pan_pos = None;
        if !did_pan {
            if self.drawing_mode {
                // Right-click cancels wire drawing (Altium behavior)
                return Some(canvas::Action::publish(Message::Tool(
                    ToolMessage::CancelDrawing,
                )));
            }
            // Show context menu at screen position
            if let Some(cursor_pos) = cursor.position_in(bounds) {
                let screen_x = bounds.x + cursor_pos.x;
                let screen_y = bounds.y + cursor_pos.y;
                return Some(canvas::Action::publish(Message::ContextMenu(
                    ContextMenuMsg::Show(screen_x, screen_y),
                )));
            }
        }
        Some(canvas::Action::capture())
    }

    /// Middle-release: stop pan.
    pub(in crate::canvas) fn update_middle_released(
        &self,
        state: &mut CanvasState,
    ) -> Option<canvas::Action<Message>> {
        state.panning = false;
        state.last_pan_pos = None;
        Some(canvas::Action::capture())
    }

    /// Cursor motion: pan, track drag-move / box-select, hover reporting.
    pub(in crate::canvas) fn update_cursor_moved(
        &self,
        state: &mut CanvasState,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        if let Some(cursor_pos) = cursor.position_in(bounds) {
            // Pan if right/middle button held
            if state.panning {
                let mut pan_just_started = false;
                if let Some(last) = state.last_pan_pos {
                    let dx = cursor_pos.x - last.x;
                    let dy = cursor_pos.y - last.y;
                    if (dx.abs() > 2.0 || dy.abs() > 2.0) && !state.pan_moved {
                        state.pan_moved = true;
                        pan_just_started = true;
                    }
                    state.camera.pan(dx, dy);
                }
                state.last_pan_pos = Some(cursor_pos);
                // On the very first frame where the pan actually
                // moves, close the context menu (matches Altium:
                // right-drag-to-pan dismisses the menu). Skipping
                // the CursorMoved publish for this one frame is
                // harmless — `camera.pan` above already moved the
                // view and the next cursor tick will publish
                // normally.
                if pan_just_started {
                    return Some(
                        canvas::Action::publish(Message::ContextMenu(ContextMenuMsg::Close))
                            .and_capture(),
                    );
                }
                return Some(
                    canvas::Action::publish(Message::CanvasEvent(CanvasEvent::CursorMoved))
                        .and_capture(),
                );
            }

            // Track drag-to-move (selected items)
            if state.click_on_selected {
                let world = state.camera.screen_to_world(cursor_pos, bounds);
                let wx = world.x as f64;
                let wy = world.y as f64;
                if let Some(origin) = state.move_origin {
                    let dist = ((wx - origin.0).powi(2) + (wy - origin.1).powi(2)).sqrt();
                    if dist > 1.0 {
                        // Exceeded threshold — switch to move mode
                        state.move_dragging = true;
                    }
                }
                if state.move_dragging {
                    // Snap the *delta*, not the absolute cursor. This keeps
                    // the dragged item on-grid during the live preview
                    // (origin may be off-grid, but offset stays a grid
                    // multiple so object_start + offset stays on grid).
                    let (mx, my) = if let (Some(origin), true) = (
                        state.move_origin,
                        self.snap_enabled && self.snap_grid_mm > 0.0,
                    ) {
                        let g = self.snap_grid_mm;
                        let dx = ((wx - origin.0) / g).round() * g;
                        let dy = ((wy - origin.1) / g).round() * g;
                        (origin.0 + dx, origin.1 + dy)
                    } else {
                        (wx, wy)
                    };
                    state.move_current = Some((mx, my));
                }
            }

            // Track drag-to-select
            if state.select_drag_start.is_some() && !state.click_on_selected {
                let world = state.camera.screen_to_world(cursor_pos, bounds);
                state.select_drag_end = Some((world.x as f64, world.y as f64));
            }

            // Regular hover — update cursor position for status bar
            let world = state.camera.screen_to_world(cursor_pos, bounds);
            let zoom_pct = state.camera.zoom_percent();
            return Some(canvas::Action::publish(Message::CanvasEvent(
                CanvasEvent::CursorAt {
                    x: world.x,
                    y: world.y,
                    zoom_pct,
                },
            )));
        }
        None
    }
}
