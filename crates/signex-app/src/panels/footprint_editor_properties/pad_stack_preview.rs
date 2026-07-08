//! Pad-stack 3D preview (CPU iso-projected) + tab strip + Choice
//! enums (PadShapeChoice / HoleShapeChoice / ExpansionMode) used by
//! the pick_list rows in `pad_form`.

use iced::widget::{
    Column, Space, button, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Background, Border, Color, Element, Length, Theme};

use super::super::PanelMsg;
use super::pad_form::PadFormValues;

/// v0.20 — Pad Stack preview. CPU-side iso-projected 3D rendering of
/// the pad: copper top face (red), solder mask outset (blue) at the
/// board surface, and a hole punched through both for THT pads.
/// Uses a 60° camera tilt so the viewer sees the top face plus the
/// stack thickness as in Altium's PCB Library preview.
///
/// Mirrors the projection helper in `preview3d.rs` but for a single
/// centred pad — no body / courtyard / bbox math.
pub(super) fn pad_stack_preview<'a>(values: &PadFormValues) -> iced::Element<'a, PanelMsg> {
    use iced::widget::canvas;

    #[derive(Debug)]
    struct Preview {
        size_x_mm: f64,
        size_y_mm: f64,
        shape: signex_library::PadShape,
        drill_diameter_mm: Option<f64>,
    }

    impl<Message> canvas::Program<Message> for Preview {
        type State = ();
        fn draw(
            &self,
            _state: &Self::State,
            renderer: &iced::Renderer,
            _theme: &iced::Theme,
            bounds: iced::Rectangle,
            _cursor: iced::mouse::Cursor,
        ) -> Vec<canvas::Geometry> {
            let mut frame = canvas::Frame::new(renderer, bounds.size());
            let bg = iced::Color::from_rgba8(0x18, 0x1B, 0x21, 1.0);
            frame.fill_rectangle(iced::Point::ORIGIN, bounds.size(), canvas::Fill::from(bg));

            // Geometry constants.
            let pad_w = self.size_x_mm.max(0.001);
            let pad_h = self.size_y_mm.max(0.001);
            // v0.27 — restore a small mask outset so the blue
            // soldermask reads as a visible ring around the copper.
            // The v0.25 UAT change set this to 0 (mask flush with
            // copper) which left only the front side-wall sliver
            // visible. Real soldermask expansion is ~0.075 mm
            // (Altium default); using 8% of pad width keeps it
            // proportional across pad sizes.
            let mask_outset_mm: f64 = pad_w.max(pad_h) * 0.08;
            let mask_w = pad_w + 2.0 * mask_outset_mm;
            let mask_h = pad_h + 2.0 * mask_outset_mm;
            // v0.27 — tighter stack ratios. The v0.25 substrate_gap
            // = 25% of pad width made the stack look like two
            // separate floating layers rather than a cohesive pad.
            // Reduce gap to 8% so the mask + copper read as a
            // single pad-stack volume, matching Altium's compact
            // preview chrome.
            let copper_thickness_mm = pad_w.max(pad_h) * 0.06;
            let mask_thickness_mm = pad_w.max(pad_h) * 0.06;
            let substrate_gap_mm = pad_w.max(pad_h) * 0.08;

            // 30° isometric projection — matches `preview3d.rs`. Both
            // X+ and Y+ rotate to screen-up directions, Z+ is screen-up.
            //   sx = (x - y) * cos30
            //   sy = -((x + y) * sin30 + z)
            // This makes the XY plane look tilted as a diamond, with
            // the pad's thickness extruded upward — same as Altium's
            // PCB Library preview tilt.
            let cos30 = 0.866_025_4_f32;
            let sin30 = 0.500_f32;

            // Fit projected bbox into 75% of frame. The iso bbox in
            // screen units: width = (mask_w + mask_h) * cos30, height
            // = (mask_w + mask_h) * sin30 + total_thickness.
            let proj_w = ((mask_w + mask_h) as f32) * cos30;
            let proj_h = ((mask_w + mask_h) as f32) * sin30
                + (copper_thickness_mm + mask_thickness_mm + substrate_gap_mm) as f32;
            let scale =
                ((bounds.width * 0.75 / proj_w).min(bounds.height * 0.75 / proj_h)).max(2.0);
            let cx = bounds.width / 2.0;
            let cy = bounds.height / 2.0 + bounds.height * 0.10; // shift down slightly

            // Project (x, y, z) world → screen.
            let project = move |x: f32, y: f32, z: f32| -> iced::Point {
                let sx = (x - y) * cos30 * scale;
                let sy = -((x + y) * sin30 + z) * scale;
                iced::Point::new(cx + sx, cy + sy)
            };

            let mask_color = iced::Color::from_rgba8(0x2E, 0x6B, 0xD9, 1.0);
            let mask_dark = iced::Color::from_rgba8(0x1F, 0x49, 0x97, 1.0);
            let copper_color = iced::Color::from_rgba8(0xD9, 0x3D, 0x3D, 1.0);
            let copper_dark = iced::Color::from_rgba8(0x99, 0x2A, 0x2A, 1.0);
            let hole_color = iced::Color::from_rgba8(0x70, 0x70, 0x70, 1.0);
            let hole_dark = iced::Color::from_rgba8(0x40, 0x40, 0x40, 1.0);

            let is_round = matches!(
                self.shape,
                signex_library::PadShape::Round | signex_library::PadShape::Oval
            );

            // v0.20 — generate the pad / mask outline in world-space
            // CCW order, then project each at the requested Z. The
            // shape determines how the perimeter is sampled:
            //   Round/Oval     → N evenly-spaced ellipse points
            //   Rect           → 4 corners
            //   RoundRect      → 4 straight edges + 4 quarter arcs
            //   Chamfered      → 4 corners with optional 45° cuts
            //   Custom / etc.  → fallback to rect corners
            let shape_for_outline = self.shape.clone();
            let perimeter_world = move |hw: f32, hh: f32, segments: usize| -> Vec<(f32, f32)> {
                use signex_library::PadShape as PS;
                use std::f32::consts::{FRAC_PI_2, PI, TAU};
                match &shape_for_outline {
                    PS::Round | PS::Oval => (0..segments)
                        .map(|i| {
                            let t = i as f32 / segments as f32 * TAU;
                            (hw * t.cos(), hh * t.sin())
                        })
                        .collect(),
                    PS::RoundRect { radius_ratio } => {
                        let r = (hw.min(hh) * (*radius_ratio as f32 * 2.0))
                            .max(0.1)
                            .min(hw.min(hh));
                        let inner_w = hw - r;
                        let inner_h = hh - r;
                        // Distribute samples roughly equally across
                        // 4 arcs + 4 sides. A power-of-2-ish split
                        // (8 + 2) keeps the curvature smooth without
                        // exploding the vertex count.
                        let arc_n = 8;
                        let mut pts: Vec<(f32, f32)> = Vec::new();
                        // Walk CCW starting at south-edge midpoint.
                        // South edge: (-inner_w, -hh) → (inner_w, -hh)
                        pts.push((-inner_w, -hh));
                        pts.push((inner_w, -hh));
                        // SE arc: center (inner_w, -inner_h), -π/2 → 0
                        for i in 1..=arc_n {
                            let t = -FRAC_PI_2 + (i as f32 / arc_n as f32) * FRAC_PI_2;
                            pts.push((inner_w + r * t.cos(), -inner_h + r * t.sin()));
                        }
                        // East edge: (hw, -inner_h) → (hw, inner_h)
                        pts.push((hw, inner_h));
                        // NE arc: center (inner_w, inner_h), 0 → π/2
                        for i in 1..=arc_n {
                            let t = (i as f32 / arc_n as f32) * FRAC_PI_2;
                            pts.push((inner_w + r * t.cos(), inner_h + r * t.sin()));
                        }
                        // North edge: (inner_w, hh) → (-inner_w, hh)
                        pts.push((-inner_w, hh));
                        // NW arc: center (-inner_w, inner_h), π/2 → π
                        for i in 1..=arc_n {
                            let t = FRAC_PI_2 + (i as f32 / arc_n as f32) * FRAC_PI_2;
                            pts.push((-inner_w + r * t.cos(), inner_h + r * t.sin()));
                        }
                        // West edge: (-hw, inner_h) → (-hw, -inner_h)
                        pts.push((-hw, -inner_h));
                        // SW arc: center (-inner_w, -inner_h), π → 3π/2
                        for i in 1..=arc_n {
                            let t = PI + (i as f32 / arc_n as f32) * FRAC_PI_2;
                            pts.push((-inner_w + r * t.cos(), -inner_h + r * t.sin()));
                        }
                        let _ = segments;
                        pts
                    }
                    PS::Chamfered {
                        chamfer_ratio,
                        corners,
                    } => {
                        let c = (hw.min(hh) * (*chamfer_ratio as f32 * 2.0))
                            .max(0.1)
                            .min(hw.min(hh));
                        let mut pts: Vec<(f32, f32)> = Vec::new();
                        // CCW from south edge.
                        // SE corner.
                        if corners.bottom_right {
                            pts.push((hw - c, -hh));
                            pts.push((hw, -hh + c));
                        } else {
                            pts.push((hw, -hh));
                        }
                        // NE corner.
                        if corners.top_right {
                            pts.push((hw, hh - c));
                            pts.push((hw - c, hh));
                        } else {
                            pts.push((hw, hh));
                        }
                        // NW corner.
                        if corners.top_left {
                            pts.push((-hw + c, hh));
                            pts.push((-hw, hh - c));
                        } else {
                            pts.push((-hw, hh));
                        }
                        // SW corner.
                        if corners.bottom_left {
                            pts.push((-hw, -hh + c));
                            pts.push((-hw + c, -hh));
                        } else {
                            pts.push((-hw, -hh));
                        }
                        let _ = segments;
                        pts
                    }
                    _ => vec![(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)],
                }
            };

            let perimeter_pts = |hw: f32, hh: f32, z: f32, segments: usize| -> Vec<iced::Point> {
                perimeter_world(hw, hh, segments)
                    .into_iter()
                    .map(|(x, y)| project(x, y, z))
                    .collect()
            };

            let segments = 40;
            // v0.25 — copper sits ABOVE the mask with a substrate gap
            // between. Stacking from bottom to top:
            //   z = 0 .................. mask_z_bot
            //   z = mask_thickness ..... mask_z_top
            //   z = mask_thickness + substrate_gap ... copper_z_bot
            //   z = mask_thickness + substrate_gap + copper_thickness . copper_z_top
            let mask_z_bot = 0.0_f32;
            let mask_z_top = mask_thickness_mm as f32;
            let copper_z_bot = (mask_thickness_mm + substrate_gap_mm) as f32;
            let copper_z_top = (mask_thickness_mm + substrate_gap_mm + copper_thickness_mm) as f32;

            let pad_hw = (pad_w / 2.0) as f32;
            let pad_hh = (pad_h / 2.0) as f32;
            let mask_hw = (mask_w / 2.0) as f32;
            let mask_hh = (mask_h / 2.0) as f32;

            // Helper: fill a polygon path from points.
            let fill_poly = |frame: &mut canvas::Frame, pts: &[iced::Point], color: iced::Color| {
                if pts.len() < 3 {
                    return;
                }
                let path = canvas::Path::new(|b| {
                    b.move_to(pts[0]);
                    for p in &pts[1..] {
                        b.line_to(*p);
                    }
                    b.close();
                });
                frame.fill(&path, canvas::Fill::from(color));
            };

            // v0.20 — generic visibility test for any CCW perimeter:
            // edge i→j has world delta (dx, dy); outward normal
            // = (dy, -dx). For 30° iso looking from NE+up, the
            // edge is visible iff outward_x + outward_y < 0
            // → dy - dx < 0 → dy < dx. Works uniformly for round,
            // rect, round-rect, and chamfered perimeters.
            let strip_visible_world = |world_pts: &[(f32, f32)], i: usize| -> bool {
                let j = (i + 1) % world_pts.len();
                let (xi, yi) = world_pts[i];
                let (xj, yj) = world_pts[j];
                let dx = xj - xi;
                let dy = yj - yi;
                dy < dx
            };

            // ── Mask: bottom face is hidden by board; draw the side
            //    walls + top face.
            let mask_world = perimeter_world(mask_hw, mask_hh, segments);
            let mask_top_pts: Vec<iced::Point> = mask_world
                .iter()
                .map(|(x, y)| project(*x, *y, mask_z_top))
                .collect();
            let mask_bot_pts: Vec<iced::Point> = mask_world
                .iter()
                .map(|(x, y)| project(*x, *y, mask_z_bot))
                .collect();
            for i in 0..mask_top_pts.len() {
                if !strip_visible_world(&mask_world, i) {
                    continue;
                }
                let j = (i + 1) % mask_top_pts.len();
                let quad = [
                    mask_bot_pts[i],
                    mask_bot_pts[j],
                    mask_top_pts[j],
                    mask_top_pts[i],
                ];
                fill_poly(&mut frame, &quad, mask_dark);
            }
            // v0.25 — same ring-tile pattern as the copper top so
            // the mask's hole goes through too. Without this, the
            // mask top would block the view of the silver inner
            // wall AND the copper''s hole would look like a
            // step-up onto a solid blue disc.
            if let Some(d) = self.drill_diameter_mm.filter(|d| *d > f32::EPSILON as f64) {
                let hr = (d / 2.0) as f32;
                let n = mask_top_pts.len();
                let inner_mask_top_pts: Vec<iced::Point> = (0..n)
                    .map(|i| {
                        let t = i as f32 / n as f32 * std::f32::consts::TAU;
                        project(hr * t.cos(), hr * t.sin(), mask_z_top)
                    })
                    .collect();
                for i in 0..n {
                    let j = (i + 1) % n;
                    let quad = [
                        mask_top_pts[i],
                        mask_top_pts[j],
                        inner_mask_top_pts[j],
                        inner_mask_top_pts[i],
                    ];
                    fill_poly(&mut frame, &quad, mask_color);
                }
            } else {
                fill_poly(&mut frame, &mask_top_pts, mask_color);
            }

            // ── Copper: side walls + top face.
            let cu_world = perimeter_world(pad_hw, pad_hh, segments);
            let cu_top_pts: Vec<iced::Point> = cu_world
                .iter()
                .map(|(x, y)| project(*x, *y, copper_z_top))
                .collect();
            let cu_bot_pts: Vec<iced::Point> = cu_world
                .iter()
                .map(|(x, y)| project(*x, *y, copper_z_bot))
                .collect();
            for i in 0..cu_top_pts.len() {
                if !strip_visible_world(&cu_world, i) {
                    continue;
                }
                let j = (i + 1) % cu_top_pts.len();
                let quad = [cu_bot_pts[i], cu_bot_pts[j], cu_top_pts[j], cu_top_pts[i]];
                fill_poly(&mut frame, &quad, copper_dark);
            }
            // v0.25 — for THT pads, render the copper top as a RING
            // (donut) so the user sees the punched-through hole.
            // iced''s fill_poly has no hole support, so we triangulate
            // the ring as quads between outer + inner perimeters
            // sampled at the same theta angles. SMD pads (no drill)
            // keep the simple solid-disc fill.
            if let Some(d) = self.drill_diameter_mm.filter(|d| *d > f32::EPSILON as f64) {
                let hr = (d / 2.0) as f32;
                let n = cu_top_pts.len();
                // Sample the hole perimeter at the SAME N angular
                // steps as the outer perimeter so each (outer[i],
                // inner[i]) pair lines up. The ring quad
                // (outer[i], outer[j], inner[j], inner[i]) tiles
                // between them.
                let inner_top_pts: Vec<iced::Point> = (0..n)
                    .map(|i| {
                        let t = i as f32 / n as f32 * std::f32::consts::TAU;
                        project(hr * t.cos(), hr * t.sin(), copper_z_top)
                    })
                    .collect();
                for i in 0..n {
                    let j = (i + 1) % n;
                    let quad = [
                        cu_top_pts[i],
                        cu_top_pts[j],
                        inner_top_pts[j],
                        inner_top_pts[i],
                    ];
                    fill_poly(&mut frame, &quad, copper_color);
                }
            } else {
                fill_poly(&mut frame, &cu_top_pts, copper_color);
            }

            // ── Hole (THT only): textbook iso through-hole construction.
            //
            // Per the engineering-drawing convention (sample sources:
            // technologystudent.com/designpro/isomet2.htm,
            // educale.com/3d isometric cylinder lessons), a cylindrical
            // hole through a solid is rendered as:
            //   1. Top rim ellipse (full — visible looking down at the rim).
            //   2. Bottom rim ellipse OFFSET DOWN by the hole depth.
            //   3. Two vertical tangent lines connecting the leftmost +
            //      rightmost extents of the two ellipses.
            //   4. The visible INNER WALL surface is the closed region
            //      bounded by:
            //        - top: BACK arc of the top ellipse
            //          (θ ∈ [-π/4, 3π/4], where cos θ + sin θ ≥ 0)
            //        - left + right: vertical tangents at sx = ±√2·hr·cos30
            //        - bottom: BACK arc of the bottom ellipse, reversed
            //      Fill SILVER.
            //   5. The visible BOTTOM (front-half of the bottom disc) is
            //      the FRONT arc of the bottom ellipse + reverse back arc;
            //      filled DARK to read as the through-void.
            //
            // Strategy here: paint full bottom disc DARK first, then
            // paint silver racetrack on top. The racetrack covers the
            // back half of the disc, leaving only the front half
            // visible as dark — matches Altium''s pad-stack preview
            // (silver-dominated cylinder, dark crescent at the bottom).
            if let Some(d) = self.drill_diameter_mm.filter(|d| *d > f32::EPSILON as f64) {
                let hr = (d / 2.0) as f32;
                let void_color = iced::Color::from_rgba8(0x14, 0x14, 0x14, 1.0);
                let wall_silver = iced::Color::from_rgba8(0xC8, 0xC8, 0xC8, 1.0);

                // (1) Full bottom ellipse → DARK.
                let hole_bot_pts: Vec<iced::Point> = (0..segments)
                    .map(|i| {
                        let t = i as f32 / segments as f32 * std::f32::consts::TAU;
                        project(hr * t.cos(), hr * t.sin(), mask_z_bot)
                    })
                    .collect();
                fill_poly(&mut frame, &hole_bot_pts, void_color);

                // (2) Silver racetrack: top BACK arc → bottom BACK arc reversed.
                // θ from -π/4 to 3π/4 spans the back half of the cylinder
                // (positive cos θ + sin θ — the camera-facing inner wall).
                let arc_segments = 30;
                let mut wall_poly: Vec<iced::Point> = Vec::with_capacity(2 * (arc_segments + 1));
                // Top BACK arc (z = copper_z_top), forward order:
                for i in 0..=arc_segments {
                    let t = -std::f32::consts::FRAC_PI_4
                        + (i as f32 / arc_segments as f32) * std::f32::consts::PI;
                    wall_poly.push(project(hr * t.cos(), hr * t.sin(), copper_z_top));
                }
                // Bottom BACK arc (z = mask_z_bot), REVERSE order so the
                // polygon closes as a racetrack with implicit vertical
                // tangents at the left + right extents.
                for i in 0..=arc_segments {
                    let t = 3.0 * std::f32::consts::FRAC_PI_4
                        - (i as f32 / arc_segments as f32) * std::f32::consts::PI;
                    wall_poly.push(project(hr * t.cos(), hr * t.sin(), mask_z_bot));
                }
                fill_poly(&mut frame, &wall_poly, wall_silver);

                let _ = hole_color;
                let _ = hole_dark;
            }
            let _ = (is_round, perimeter_pts); // tidied below; suppress unused warnings

            vec![frame.into_geometry()]
        }
    }

    let preview = Preview {
        size_x_mm: values.size_x_mm,
        size_y_mm: values.size_y_mm,
        shape: values.shape.clone(),
        drill_diameter_mm: values.drill_diameter_mm,
    };
    container(
        canvas(preview)
            .width(Length::Fill)
            .height(Length::Fixed(160.0)),
    )
    .padding([4, 8])
    .width(Length::Fill)
    .into()
}

/// v0.20 — Pad Stack tab strip (Simple / Top-Middle-Bottom / Full
/// Stack). UI-only structure today; per-layer overrides require a
/// v0.21 schema follow-up so the body stays the same across tabs.
pub(super) fn pad_stack_tab_strip<'a>(
    values: &PadFormValues,
    primary: Color,
    muted: Color,
    border_c: Color,
) -> iced::Element<'a, PanelMsg> {
    use crate::library::editor::footprint::state::PadStackTab;
    let current = values.pad_stack_tab;
    let chip_border = border_c;
    let active_bg = Color::from_rgba8(0x2E, 0x33, 0x45, 1.0);
    let inactive_bg = Color::from_rgba8(0x1A, 0x1D, 0x28, 1.0);
    let mk = move |label: &'static str, target: PadStackTab| -> iced::Element<'a, PanelMsg> {
        let active = current == target;
        iced::widget::button(
            text(label)
                .size(10)
                .color(if active { primary } else { muted })
                .align_x(iced::alignment::Horizontal::Center),
        )
        .padding([4, 10])
        .width(Length::FillPortion(1))
        .on_press(PanelMsg::FpEditorSetPadStackTab(target))
        .style(move |_: &Theme, status: iced::widget::button::Status| {
            let bg = match status {
                iced::widget::button::Status::Hovered => Some(iced::Background::Color(
                    Color::from_rgba(1.0, 1.0, 1.0, 0.06),
                )),
                _ => Some(iced::Background::Color(if active {
                    active_bg
                } else {
                    inactive_bg
                })),
            };
            iced::widget::button::Style {
                background: bg,
                border: iced::Border {
                    width: 1.0,
                    radius: 2.0.into(),
                    color: chip_border,
                },
                ..iced::widget::button::Style::default()
            }
        })
        .into()
    };
    container(
        row![
            mk("Simple", PadStackTab::Simple),
            mk("Top-Middle-Bottom", PadStackTab::TopMiddleBottom),
            mk("Full Stack", PadStackTab::FullStack),
        ]
        .spacing(2)
        .align_y(iced::Alignment::Center),
    )
    .padding([4, 8])
    .width(Length::Fill)
    .into()
}

/// v0.20 — pick_list-friendly proxy for `signex_library::PadShape`.
/// Mirrors Altium's COPPER → Shape dropdown verbatim minus
/// "Custom Shape" (sketch mode owns freeform geometry):
///   Round / Rectangular / Octagonal / Rounded Rectangle /
///   Chamfered Rectangle / Donut.
/// Schema-mapping notes:
///   - Octagonal / Donut have no native variant on
///     `signex_library::PadShape` yet; both fall back to Round at
///     bake. Round trip preserves the picker selection across
///     sessions once we add schema variants in v0.21.
///   - Chamfered Rectangle uses the existing `Chamfered` variant
///     with sensible defaults (25% chamfer, all corners).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PadShapeChoice {
    Round,
    Rectangular,
    Octagonal,
    RoundedRectangle,
    ChamferedRectangle,
    Donut,
}

impl PadShapeChoice {
    pub(super) const ALL: &'static [PadShapeChoice] = &[
        PadShapeChoice::Round,
        PadShapeChoice::Rectangular,
        PadShapeChoice::Octagonal,
        PadShapeChoice::RoundedRectangle,
        PadShapeChoice::ChamferedRectangle,
        PadShapeChoice::Donut,
    ];

    pub(super) fn from_lib(s: &signex_library::PadShape) -> Self {
        match s {
            signex_library::PadShape::Round => PadShapeChoice::Round,
            signex_library::PadShape::Rect => PadShapeChoice::Rectangular,
            signex_library::PadShape::RoundRect { .. } => PadShapeChoice::RoundedRectangle,
            signex_library::PadShape::Chamfered { .. } => PadShapeChoice::ChamferedRectangle,
            // Oval / Custom / Octagonal / Donut have no 1:1 schema
            // home today; collapse to Round so the picker stays
            // consistent. Custom Shape is intentionally absent — use
            // sketch mode for freeform geometry.
            _ => PadShapeChoice::Round,
        }
    }
    pub(super) fn to_lib(self) -> signex_library::PadShape {
        use signex_library::primitive::footprint::ChamferedCorners;
        match self {
            PadShapeChoice::Round => signex_library::PadShape::Round,
            PadShapeChoice::Rectangular => signex_library::PadShape::Rect,
            PadShapeChoice::RoundedRectangle => {
                signex_library::PadShape::RoundRect { radius_ratio: 0.25 }
            }
            PadShapeChoice::ChamferedRectangle => signex_library::PadShape::Chamfered {
                chamfer_ratio: 0.25,
                corners: ChamferedCorners::all(),
            },
            // v0.21 schema follow-up: native Octagonal + Donut. Until
            // then Round is the closest mappable shape (Donut's
            // hole comes from the drill anyway).
            PadShapeChoice::Octagonal => signex_library::PadShape::Round,
            PadShapeChoice::Donut => signex_library::PadShape::Round,
        }
    }
}

impl std::fmt::Display for PadShapeChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            PadShapeChoice::Round => "Round",
            PadShapeChoice::Rectangular => "Rectangular",
            PadShapeChoice::Octagonal => "Octagonal",
            PadShapeChoice::RoundedRectangle => "Rounded Rectangle",
            PadShapeChoice::ChamferedRectangle => "Chamfered Rectangle",
            PadShapeChoice::Donut => "Donut",
        })
    }
}

/// v0.20 — Altium-parity HOLE → Shape dropdown. Round / Slot today;
/// Rectangular hole deferred until the schema gains it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HoleShapeChoice {
    Round,
    Slot,
}

impl HoleShapeChoice {
    pub(super) const ALL: &'static [HoleShapeChoice] =
        &[HoleShapeChoice::Round, HoleShapeChoice::Slot];
}

impl std::fmt::Display for HoleShapeChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            HoleShapeChoice::Round => "Round",
            HoleShapeChoice::Slot => "Slot",
        })
    }
}

/// v0.21 — Altium-parity PASTE / SOLDER expansion mode picker.
/// `Rule` defers to the per-board design rule (Solder Mask Expansion
/// / Paste Mask Expansion). `Manual` overrides with an explicit
/// per-pad value (consumed by the matching expansion column).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExpansionMode {
    Rule,
    Manual,
}

impl ExpansionMode {
    pub(super) const ALL: &'static [ExpansionMode] = &[ExpansionMode::Rule, ExpansionMode::Manual];
}

impl std::fmt::Display for ExpansionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ExpansionMode::Rule => "Rule Expansion",
            ExpansionMode::Manual => "Manual",
        })
    }
}
