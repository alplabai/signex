//! The Interactive Routing options (#2719, step 1) as the catalog command
//! `route_options`: the conflict mode (`cycle_route_mode`), the corner mode
//! (Shift+Space, shared with wires, #2372), the track width and the via
//! size the next leg routes with, the active layer, and while a route is
//! drawn on the board its net, class and lengths. Each value is the state
//! the router reads, so the command and the keys set the same thing. Shape:
//! `docs/CLI.md`, "Routing options". The Properties panel's rows wait for
//! their mockup row (#2719).

use std::path::Path;

use serde_json::{Value, json};
use signex_engine::pcb::draw;
use signex_engine::pcb::route::RouteMode;
use signex_types::corner::{CornerMode, CornerRule};
use signex_types::format::pcb_layer_token;
use signex_types::net_class::first_matching_class;
use signex_types::pcb::{PcbBoard, Point};
use uuid::Uuid;

use super::BoardToolSession;
use crate::app::Signex;

/// What one `route_options` sets; `None` leaves that option as it is.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct RouteOptionsEdit {
    pub conflict_mode: Option<RouteMode>,
    pub corner_mode: Option<CornerMode>,
    pub width_mm: Option<f64>,
    pub via_diameter_mm: Option<f64>,
    pub via_hole_mm: Option<f64>,
}

/// Whether `mm` is a size a track or via can take.
fn is_size(mm: f64) -> bool {
    mm.is_finite() && mm > 0.0
}

impl Signex {
    /// `route_options` on the board at `path`: apply `edit`, then answer
    /// with every option. A size that is not above zero refuses the whole
    /// edit, so nothing changes.
    pub(crate) fn route_options(
        &mut self,
        path: &Path,
        edit: RouteOptionsEdit,
    ) -> Result<Value, String> {
        if self.document_state.pcb_board(path).is_none() {
            return Err(format!("{} is not loaded", path.display()));
        }
        let sizes = [edit.width_mm, edit.via_diameter_mm, edit.via_hole_mm];
        if sizes.into_iter().flatten().any(|mm| !is_size(mm)) {
            return Err("a width or via size must be above zero".into());
        }
        if let Some(mode) = edit.conflict_mode {
            self.set_route_mode(mode);
        }
        if let Some(mode) = edit.corner_mode {
            let rule = self.interaction_state.corner_rule;
            self.set_corner_rule(CornerRule { mode, ..rule });
        }
        let defaults = &mut self.interaction_state.pcb_tool_defaults;
        if let Some(width) = edit.width_mm {
            defaults.track_width_mm = Some(width);
        }
        if let Some(diameter) = edit.via_diameter_mm {
            defaults.via_diameter_mm = Some(diameter);
        }
        if let Some(hole) = edit.via_hole_mm {
            defaults.via_hole_mm = Some(hole);
        }
        self.restyle_route_on(path, edit.width_mm);
        Ok(self.route_options_json(path))
    }

    /// A route being drawn on the board at `path` goes on with a new
    /// `width`, and its next leg is routed again, as a width typed in the
    /// paused form does.
    fn restyle_route_on(&mut self, path: &Path, width: Option<f64>) {
        let Some(session) = self.interaction_state.pcb_tool.as_mut() else {
            return;
        };
        if session.path != path || !session.is_routing() {
            return;
        }
        if let (Some(width), Some(route)) = (width, session.route.as_mut()) {
            route.width_mm = width;
        }
        if let Some(router) = session.router.as_mut() {
            router.preview = None;
        }
        self.refresh_pcb_highlight();
    }

    /// Every routing option for the board at `path` (loaded, checked by
    /// the caller); `null` fields are documented in `docs/CLI.md`.
    fn route_options_json(&self, path: &Path) -> Value {
        let Some(board) = self.document_state.pcb_board(path) else {
            return Value::Null;
        };
        let interaction = &self.interaction_state;
        let session = interaction.pcb_tool.as_ref().filter(|s| s.path == path);
        let defaults = &interaction.pcb_tool_defaults;
        let route = session.and_then(|s| s.route);
        let (width_mm, width_from) = match (route, defaults.track_width_mm) {
            (Some(route), _) => (route.width_mm, "route"),
            (None, Some(width)) => (width, "set"),
            (None, None) => (draw::default_track_width(board), "net_class_or_board"),
        };
        let via = defaults.via(board, Uuid::nil(), Point::new(0.0, 0.0));
        let via_set = defaults.via_diameter_mm.is_some() || defaults.via_hole_mm.is_some();
        let via_from = if via_set { "set" } else { "board" };
        let corner = interaction.corner_rule;
        json!({
            "conflict_mode": interaction.route_mode.token(),
            "conflict_modes": RouteMode::ALL.map(RouteMode::token),
            "corner_mode": corner.mode.token(),
            "corner_modes": CornerMode::ALL.map(CornerMode::token),
            "corner_flipped": corner.is_flipped,
            "layer": pcb_layer_token(self.pcb_layer_view(path).active()),
            "width_mm": width_mm,
            "width_from": width_from,
            "via_diameter_mm": via.diameter,
            "via_hole_mm": via.drill,
            "via_from": via_from,
            "route": session.and_then(|s| self.route_json(board, path, s)),
        })
    }

    /// The route being drawn in `session`: its net, the net's class, the
    /// length the net has on the board and the length of the legs not
    /// landed yet. `None` when no route is being drawn.
    fn route_json(
        &self,
        board: &PcbBoard,
        path: &Path,
        session: &BoardToolSession,
    ) -> Option<Value> {
        let route = session.route?;
        let name = board
            .nets
            .iter()
            .find(|n| n.number == route.net && route.net != 0)
            .map(|n| n.name.clone());
        let class = name.as_deref().and_then(|name| {
            first_matching_class(self.board_net_classes(path), name).map(|c| c.name.clone())
        });
        let draft = session.router.as_ref().map(|r| r.draft.tracks.iter());
        Some(json!({
            "net": route.net,
            "name": name,
            "class": class,
            "layer": pcb_layer_token(signex_types::pcb_layer::PcbLayer::Copper(route.layer)),
            "routed_length_mm": pcb_geom::net_routed_length_mm(board, route.net),
            "draft_length_mm": draft.and_then(pcb_geom::tracks_length_mm),
        }))
    }
}
