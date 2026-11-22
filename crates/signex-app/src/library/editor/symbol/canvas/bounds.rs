//! The canvas's bounding box, which Fit and the background read.

use super::*;

impl SymbolCanvas<'_> {
    /// Body rectangle, when present, derived from the first
    /// `SymbolGraphicKind::Rectangle` in `symbol.graphics`.
    fn body_rect(&self) -> Option<(f64, f64, f64, f64)> {
        for g in &self.symbol.graphics {
            if let SymbolGraphicKind::Rectangle { from, to } = &g.kind {
                return Some((from[0], from[1], to[0], to[1]));
            }
        }
        None
    }

    /// Bounding box around every visible symbol entity.
    ///
    /// If the symbol has no pins and no graphics, return a tiny box
    /// around world origin so Fit keeps the origin marker centered.
    pub(crate) fn bbox(&self) -> (f64, f64, f64, f64) {
        let mut bounds: Option<(f64, f64, f64, f64)> = None;
        let include_rect =
            |bounds: &mut Option<(f64, f64, f64, f64)>, x0: f64, y0: f64, x1: f64, y1: f64| {
                let rx0 = x0.min(x1);
                let ry0 = y0.min(y1);
                let rx1 = x0.max(x1);
                let ry1 = y0.max(y1);
                if let Some((min_x, min_y, max_x, max_y)) = bounds.as_mut() {
                    *min_x = (*min_x).min(rx0);
                    *min_y = (*min_y).min(ry0);
                    *max_x = (*max_x).max(rx1);
                    *max_y = (*max_y).max(ry1);
                } else {
                    *bounds = Some((rx0, ry0, rx1, ry1));
                }
            };

        if let Some((bx0, by0, bx1, by1)) = self.body_rect() {
            include_rect(
                &mut bounds,
                bx0.min(bx1) - 5.08,
                by0.min(by1) - 5.08,
                bx0.max(bx1) + 5.08,
                by0.max(by1) + 5.08,
            );
        }

        for pin in &self.symbol.pins {
            include_rect(
                &mut bounds,
                pin.position[0] - 1.27,
                pin.position[1] - 1.27,
                pin.position[0] + pin.length + 1.27,
                pin.position[1] + 1.27,
            );
        }

        // Include every graphic's extent so Fit doesn't leave shapes
        // off-screen.
        for g in &self.symbol.graphics {
            match &g.kind {
                SymbolGraphicKind::Rectangle { from, to }
                | SymbolGraphicKind::Line { from, to } => {
                    include_rect(&mut bounds, from[0], from[1], to[0], to[1]);
                }
                SymbolGraphicKind::Circle { center, radius }
                | SymbolGraphicKind::Arc { center, radius, .. } => {
                    include_rect(
                        &mut bounds,
                        center[0] - radius,
                        center[1] - radius,
                        center[0] + radius,
                        center[1] + radius,
                    );
                }
                SymbolGraphicKind::Text { position, size, .. } => {
                    include_rect(
                        &mut bounds,
                        position[0] - size,
                        position[1] - size,
                        position[0] + size,
                        position[1] + size,
                    );
                }
                SymbolGraphicKind::Polygon { vertices } => {
                    for v in vertices {
                        include_rect(&mut bounds, v[0], v[1], v[0], v[1]);
                    }
                }
                kind @ (SymbolGraphicKind::Bezier { .. }
                | SymbolGraphicKind::Pie { .. }
                | SymbolGraphicKind::EllipticalArc { .. }
                | SymbolGraphicKind::RoundRectangle { .. }) => {
                    for v in kind.flattened_outline() {
                        include_rect(&mut bounds, v[0], v[1], v[0], v[1]);
                    }
                }
            }
        }

        bounds.unwrap_or((-1.27, -1.27, 1.27, 1.27))
    }
}
