//! Focusing a net-level ERC finding: select every item on its net and fit the
//! viewport to the net's bounding box, instead of centring on one point.

use iced::Task;
use signex_types::schematic::{SchematicSheet, SelectedItem, SelectedKind};

use super::super::super::*;

/// Smallest side of the fitted box, so a one-pin net does not zoom to a dot.
const MIN_FIT_SIDE_MM: f32 = 16.0;
/// Breathing room around the net's extent.
const FIT_MARGIN_MM: f32 = 2.0;

/// The viewport rectangle that frames `[min_x, min_y, max_x, max_y]`.
pub(super) fn fit_rect(bounds: [f64; 4]) -> iced::Rectangle {
    let [x0, y0, x1, y1] = bounds.map(|v| v as f32);
    let (w, h) = (
        (x1 - x0 + 2.0 * FIT_MARGIN_MM).max(MIN_FIT_SIDE_MM),
        (y1 - y0 + 2.0 * FIT_MARGIN_MM).max(MIN_FIT_SIDE_MM),
    );
    iced::Rectangle {
        x: (x0 + x1) / 2.0 - w / 2.0,
        y: (y0 + y1) / 2.0 - h / 2.0,
        width: w,
        height: h,
    }
}

/// Everything a net-level finding highlights: the items to select and the box
/// that frames them (`[min_x, min_y, max_x, max_y]`).
pub(super) fn plan(
    sheet: &SchematicSheet,
    net: &signex_erc::NetFinding,
) -> Option<(Vec<SelectedItem>, [f64; 4])> {
    let anchor = net.representative()?.position;
    let extent = signex_net::net_extent_at(sheet, &anchor);
    let mut selected: Vec<SelectedItem> = Vec::new();
    selected.extend(
        extent
            .wires
            .iter()
            .map(|u| SelectedItem::new(*u, SelectedKind::Wire)),
    );
    selected.extend(
        extent
            .junctions
            .iter()
            .map(|u| SelectedItem::new(*u, SelectedKind::Junction)),
    );
    selected.extend(
        extent
            .labels
            .iter()
            .map(|u| SelectedItem::new(*u, SelectedKind::Label)),
    );
    let mut bounds = extent.bounds;
    for pin in &net.pins {
        let item = SelectedItem::new(pin.symbol, SelectedKind::Symbol);
        if !selected.contains(&item) {
            selected.push(item);
        }
        let p = pin.position;
        let b = bounds.get_or_insert([p.x, p.y, p.x, p.y]);
        b[0] = b[0].min(p.x);
        b[1] = b[1].min(p.y);
        b[2] = b[2].max(p.x);
        b[3] = b[3].max(p.y);
    }
    bounds.map(|b| (selected, b))
}

impl Signex {
    /// Row click on an ERC entry: a net-level finding highlights its whole net,
    /// any other finding centres on its point.
    pub(crate) fn focus_erc_entry(
        &mut self,
        target: &crate::panels::ErcDiagnosticEntry,
    ) -> Task<Message> {
        match &target.net {
            Some(net) => self.handle_focus_net(net),
            None => self.handle_focus_at(target.world_x, target.world_y, target.select),
        }
    }

    /// Select the net's items on the active sheet and stage a viewport fit
    /// around all of them.
    fn handle_focus_net(&mut self, net: &signex_erc::NetFinding) -> Task<Message> {
        let Some((selected, bounds)) = self
            .active_render_snapshot()
            .and_then(|sheet| plan(sheet, net))
        else {
            return Task::none();
        };
        let canvas = self.interaction_state.active_canvas_mut();
        canvas.selected = selected;
        canvas.clear_overlay_cache();
        self.update_selection_info();
        self.interaction_state
            .canvas
            .pending_fit
            .set(Some(fit_rect(bounds)));
        Task::none()
    }
}
