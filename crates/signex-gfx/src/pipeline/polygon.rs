//! Polygon pipeline implementation.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use crate::primitive::polygon::GpuPolygon;
use crate::shader;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PolygonVertex {
    pub(crate) position: [f32; 2],
    pub(crate) color: [f32; 4],
    /// Unit direction the shader pushes this vertex along, by the stroke's
    /// half-width — the edge normal, signed for the side of the edge it sits
    /// on. Zero for fill vertices, which therefore never move.
    pub(crate) extrude: [f32; 2],
    /// Half the stroke width in world mm. The shader raises it to the
    /// camera's screen-space floor, so a stroke cannot fade to sub-pixel
    /// zoomed out, and a zero-width stroke draws as a hairline — the CPU
    /// replay's `stroke_px` rule. Doing it in the shader keeps the vertices
    /// zoom-independent; flooring here would re-tessellate on every zoom.
    pub(crate) half_width: f32,
    /// Where the shader puts this corner when the stroke is thicker than its
    /// edge is long: the miter vector at this end of the edge, signed like
    /// `extrude`, so neighbouring edges meet exactly with no gap and no
    /// overlap. Zero for fill vertices. See `polygon.wgsl`.
    pub(crate) miter: [f32; 2],
    /// The shorter of the two edges that meet at this corner, world mm: how
    /// thick a stroke must be, relative to it, before the shader moves to
    /// `miter`. A property of the corner, so both quads that share it agree on
    /// where it lands. Zero for fill vertices.
    pub(crate) edge_len: f32,
}

impl PolygonVertex {
    fn fill(position: [f32; 2], color: [f32; 4]) -> Self {
        Self {
            position,
            color,
            extrude: [0.0, 0.0],
            half_width: 0.0,
            miter: [0.0, 0.0],
            edge_len: 0.0,
        }
    }
}

/// Build the triangle-list vertices for a batch of polygons: an ear-clip fill
/// for every contour, followed by a stroke outline (expanded edge quads) for
/// polygons that carry one. Fill and stroke share this pipeline's
/// `PolygonVertex` format, so they draw in a single pass — the stroke vertices
/// come last and composite on top of the fill under alpha blending, matching
/// the CPU `draw_polygons` order (fill, then stroke).
///
/// Fill uses `signex_sketch::ear_clip` (below), which is exact for concave
/// contours too — the CPU path (`frame.fill`, lyon) tessellates the same
/// contour, so the two agree.
///
/// `pub(crate)` (rather than private) so `scene::scenario_tests`'s full-board
/// scenario can exercise this exact CPU-side vertex generation instead of
/// re-deriving the vertex-count formula independently.
pub(crate) fn triangulate_polygons(polygons: &[GpuPolygon]) -> Vec<PolygonVertex> {
    let mut vertices = Vec::new();

    for polygon in polygons {
        // Skip degenerate contours entirely (no fill, no stroke), as the CPU
        // path does.
        if polygon.vertices.len() < 3 {
            continue;
        }
        append_fill(&mut vertices, polygon);
        append_stroke(&mut vertices, polygon);
    }

    vertices
}

/// The fill triangles the GPU path draws for one polygon (holes excluded),
/// as world-space corner triples. Empty for a contour under 3 vertices. Lets
/// callers above this crate assert on the real fill geometry. Test-only:
/// built for this crate's tests and for dependents that enable the
/// `test-support` feature.
#[cfg(any(test, feature = "test-support"))]
pub fn polygon_fill_triangles(polygon: &GpuPolygon) -> Vec<[[f32; 2]; 3]> {
    if polygon.vertices.len() < 3 {
        return Vec::new();
    }
    let mut vertices = Vec::new();
    append_fill(&mut vertices, polygon);
    vertices
        .chunks_exact(3)
        .map(|t| [t[0].position, t[1].position, t[2].position])
        .collect()
}

/// Ear-clip fill, concave-safe. Assumes `polygon.vertices.len() >= 3`.
///
/// A triangle fan from `points[0]` is exact only for convex contours — a
/// concave copper pour/rule-area (an arbitrary user outline, frequently
/// non-convex) would bridge triangles across the notch and paint copper where
/// the pour has none. `signex_sketch::ear_clip` is the authoritative
/// triangulator for exactly this contract (already driving the sketch
/// overlay's filled-loop renderer), reused here instead of re-derived so the
/// GPU fill partitions the same polygon area the CPU `frame.fill` (lyon)
/// tessellates. Falls back to the fan only for the degenerate cases
/// `ear_clip` refuses (self-intersecting / all-collinear contours) — the CPU
/// path has no defined behaviour for those either, so a fan is a reasonable
/// best-effort rather than drawing nothing.
fn append_fill(vertices: &mut Vec<PolygonVertex>, polygon: &GpuPolygon) {
    if polygon.holes.iter().any(|hole| hole.len() >= 3) && append_fill_with_holes(vertices, polygon)
    {
        return;
    }
    append_solid_fill(vertices, polygon);
}

/// Fill the outer contour alone, ignoring holes.
fn append_solid_fill(vertices: &mut Vec<PolygonVertex>, polygon: &GpuPolygon) {
    let points = &polygon.vertices;
    let points_2d: Vec<signex_sketch::geom::Point2> = points
        .iter()
        .map(|p| signex_sketch::geom::Point2::new(p[0] as f64, p[1] as f64))
        .collect();
    let triangles = signex_sketch::geom::ear_clip(&points_2d);

    if triangles.is_empty() {
        append_fan_fill(vertices, points, polygon.fill_color);
        return;
    }

    for [a, b, c] in triangles {
        for idx in [a, b, c] {
            vertices.push(PolygonVertex::fill(points[idx], polygon.fill_color));
        }
    }
}

/// Fill for a polygon with cut-outs: each hole is bridged into the outer
/// contour (`ear_clip_with_holes`) and the merged ring ear-clipped, so the
/// holes receive no triangles. Holes under 3 vertices are malformed and
/// skipped. Matches the CPU path, which fills the same subpaths even-odd.
///
/// Returns `false`, appending nothing, when the triangles do not add up to
/// the outline minus the holes (degenerate, overlapping or outline-touching
/// holes). The caller then fills the solid outline, so a zone is never left
/// invisible. The area check is a heuristic: on valid input a wrong fill
/// does not pass it, but a self-intersecting outline with the right total
/// area could.
fn append_fill_with_holes(vertices: &mut Vec<PolygonVertex>, polygon: &GpuPolygon) -> bool {
    use signex_sketch::geom::{Point2, signed_area};
    let to_2d = |p: &[f32; 2]| Point2::new(p[0] as f64, p[1] as f64);
    let outer: Vec<_> = polygon.vertices.iter().map(to_2d).collect();
    let holes: Vec<Vec<_>> = polygon
        .holes
        .iter()
        .filter(|hole| hole.len() >= 3)
        .map(|hole| hole.iter().map(to_2d).collect())
        .collect();
    let (merged, triangles) = signex_sketch::geom::ear_clip_with_holes(&outer, &holes);

    let expected =
        signed_area(&outer).abs() - holes.iter().map(|h| signed_area(h).abs()).sum::<f64>();
    let filled: f64 = triangles
        .iter()
        .map(|&[a, b, c]| signed_area(&[merged[a], merged[b], merged[c]]).abs())
        .sum();
    // A non-finite vertex makes every comparison false, so it would pass the
    // area check unnoticed; treat it as a failed fill.
    if triangles.is_empty()
        || !filled.is_finite()
        || !expected.is_finite()
        || (filled - expected).abs() > expected.abs() * 1e-6 + 1e-9
    {
        return false;
    }
    for [a, b, c] in triangles {
        for idx in [a, b, c] {
            let p = merged[idx];
            vertices.push(PolygonVertex::fill(
                [p.x as f32, p.y as f32],
                polygon.fill_color,
            ));
        }
    }
    true
}

/// Triangle-fan fill from the first vertex — exact for convex contours only.
/// Used as the [`append_fill`] fallback when `ear_clip` can't triangulate the
/// contour at all.
fn append_fan_fill(vertices: &mut Vec<PolygonVertex>, points: &[[f32; 2]], fill_color: [f32; 4]) {
    let origin = points[0];
    for idx in 1..(points.len() - 1) {
        vertices.push(PolygonVertex::fill(origin, fill_color));
        vertices.push(PolygonVertex::fill(points[idx], fill_color));
        vertices.push(PolygonVertex::fill(points[idx + 1], fill_color));
    }
}

/// Stroke outline: one width-expanded quad per edge of the closed contour
/// (including the closing edge, `last -> first`), so fill-only-invisible areas
/// such as rule/keepout zones still read as an outline. World-space width in
/// mm — the shader's ortho projection scales it with zoom, mirroring the CPU's
/// `stroke_width * camera.scale` — raised in the shader to the camera's
/// screen-space floor, as the CPU replay's `stroke_px` does. No miter joins
/// (thin strokes only; a stroke thicker than the edges at a corner is mitered
/// there instead).
/// No-op when the polygon has no stroke colour.
fn append_stroke(vertices: &mut Vec<PolygonVertex>, polygon: &GpuPolygon) {
    // `is_stroked()` is the shared definition of "this contour carries an
    // outline". Keeping the guard here in terms of that predicate is what the
    // CPU↔GPU parity test locks against.
    if !polygon.is_stroked() {
        return;
    }
    let stroke_color = polygon
        .stroke_color
        .expect("is_stroked() guarantees a stroke colour");

    let half = polygon.stroke_width.max(0.0) * 0.5;
    let hole_rings = polygon.holes.iter().filter(|hole| hole.len() >= 3);
    for points in std::iter::once(&polygon.vertices).chain(hole_rings) {
        append_ring_stroke(vertices, points, half, stroke_color);
    }
}

/// The longest miter, in half-widths, a corner may have. A sharper turn gets no
/// miter (each edge keeps its own normal there, which leaves the outer notch a
/// thin stroke has anyway) instead of spiking.
const MITER_LIMIT: f32 = 4.0;

/// The left normal of edge `a -> b`, or `None` for a zero-length edge.
fn edge_normal(a: [f32; 2], b: [f32; 2]) -> Option<([f32; 2], f32)> {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let len = (dx * dx + dy * dy).sqrt();
    (len > f32::EPSILON).then(|| ([-dy / len, dx / len], len))
}

/// The miter vector where an edge with normal `own` meets its neighbour with
/// normal `other`: the vector whose multiple by the half-width reaches the
/// point where both edges' offsets cross. Falls back to `own` past
/// [`MITER_LIMIT`]: no bevel triangle is emitted.
fn miter_vector(own: [f32; 2], other: [f32; 2]) -> [f32; 2] {
    let denominator = 1.0 + own[0] * other[0] + own[1] * other[1];
    // |miter| = sqrt(2 / denominator); the limit caps it.
    if denominator < 2.0 / (MITER_LIMIT * MITER_LIMIT) {
        return own;
    }
    [
        (own[0] + other[0]) / denominator,
        (own[1] + other[1]) / denominator,
    ]
}

/// The nearest edge at `from` (its normal and length), stepping by `step`
/// around a ring of `n` edges, skipping zero-length ones.
fn neighbour_edge(
    normals: &[Option<([f32; 2], f32)>],
    from: usize,
    step: isize,
) -> Option<([f32; 2], f32)> {
    let n = normals.len() as isize;
    (1..n).find_map(|offset| {
        let index = (from as isize + step * offset).rem_euclid(n) as usize;
        normals[index]
    })
}

/// A corner of the ring: the miter vector there and the length the shader
/// judges it by, the shorter of the two edges that meet. Both quads at the
/// corner compute the same pair, so they land on the same point.
fn corner_join(own: ([f32; 2], f32), neighbour: Option<([f32; 2], f32)>) -> ([f32; 2], f32) {
    match neighbour {
        Some((normal, len)) => (miter_vector(own.0, normal), own.1.min(len)),
        None => own,
    }
}

/// The stroke quads of one closed ring.
///
/// Each edge is a quad whose four corners sit ON the ring's vertices, each
/// carrying two displacement directions the shader chooses between by the
/// floored half-width: the edge normal (a plain rectangle, right when the
/// stroke is thin next to the edge) and the miter vector (right when it is
/// thick, because neighbouring quads then share their corner exactly, with no
/// gap and no overlap, so a translucent stroke does not bead).
fn append_ring_stroke(
    vertices: &mut Vec<PolygonVertex>,
    points: &[[f32; 2]],
    half: f32,
    color: [f32; 4],
) {
    let normals: Vec<_> = (0..points.len())
        .map(|i| edge_normal(points[i], points[(i + 1) % points.len()]))
        .collect();
    for (i, edge) in normals.iter().enumerate() {
        let Some((normal, len)) = *edge else { continue };
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        let (start, start_len) = corner_join((normal, len), neighbour_edge(&normals, i, -1));
        let (end, end_len) = corner_join((normal, len), neighbour_edge(&normals, i, 1));
        let corner = |position, side: f32, miter: [f32; 2], corner_len: f32| PolygonVertex {
            position,
            color,
            extrude: [normal[0] * side, normal[1] * side],
            half_width: half,
            miter: [miter[0] * side, miter[1] * side],
            edge_len: corner_len,
        };
        for vertex in [
            corner(a, 1.0, start, start_len),
            corner(a, -1.0, start, start_len),
            corner(b, 1.0, end, end_len),
            corner(b, 1.0, end, end_len),
            corner(a, -1.0, start, start_len),
            corner(b, -1.0, end, end_len),
        ] {
            vertices.push(vertex);
        }
    }
}

/// GPU polygon pipeline for filled triangle-list geometry.
pub struct PolygonPipeline {
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: usize,
    vertex_count: u32,
    /// Buffer for the polygons that draw above the stroke buckets (PCB pads);
    /// separate so the GPU can interleave it after circles, before text.
    top_vertex_buffer: wgpu::Buffer,
    top_vertex_capacity: usize,
    top_vertex_count: u32,
    /// Second, independent vertex buffer for overlay geometry — see
    /// `LinePipeline`'s equivalent field doc for why this is a separate
    /// buffer rather than a shared one with a base/overlay split index.
    overlay_vertex_buffer: wgpu::Buffer,
    overlay_vertex_capacity: usize,
    overlay_vertex_count: u32,
}

impl PolygonPipeline {
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("signex_gfx_polygon_shader"),
            source: wgpu::ShaderSource::Wgsl(shader::POLYGON_WGSL.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("signex_gfx_polygon_pipeline_layout"),
            bind_group_layouts: &[camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("signex_gfx_polygon_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<PolygonVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32,
                        },
                        wgpu::VertexAttribute {
                            offset: 36,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 44,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Float32,
                        },
                    ],
                }],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });

        let vertex_capacity = 1usize;
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("signex_gfx_polygon_vertices"),
            size: std::mem::size_of::<PolygonVertex>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let top_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("signex_gfx_polygon_top_vertices"),
            size: std::mem::size_of::<PolygonVertex>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let overlay_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("signex_gfx_polygon_overlay_vertices"),
            size: std::mem::size_of::<PolygonVertex>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            render_pipeline,
            vertex_buffer,
            vertex_capacity,
            vertex_count: 0,
            top_vertex_buffer,
            top_vertex_capacity: vertex_capacity,
            top_vertex_count: 0,
            overlay_vertex_buffer,
            overlay_vertex_capacity: vertex_capacity,
            overlay_vertex_count: 0,
        }
    }

    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, polygons: &[GpuPolygon]) {
        let vertices = triangulate_polygons(polygons);
        Self::upload_into(
            device,
            queue,
            &vertices,
            &mut self.vertex_buffer,
            &mut self.vertex_capacity,
            &mut self.vertex_count,
            "signex_gfx_polygon_vertices",
        );
    }

    /// Upload the polygons that composite above the stroke buckets into
    /// their own buffer, drawn by [`Self::draw_top`] after circles.
    pub fn upload_top(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        polygons: &[GpuPolygon],
    ) {
        let vertices = triangulate_polygons(polygons);
        Self::upload_into(
            device,
            queue,
            &vertices,
            &mut self.top_vertex_buffer,
            &mut self.top_vertex_capacity,
            &mut self.top_vertex_count,
            "signex_gfx_polygon_top_vertices",
        );
    }

    /// Upload overlay polygon geometry into the dedicated overlay buffer,
    /// drawn by [`Self::draw_overlay`] in a separate later pass.
    pub fn upload_overlay(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        polygons: &[GpuPolygon],
    ) {
        let vertices = triangulate_polygons(polygons);
        Self::upload_into(
            device,
            queue,
            &vertices,
            &mut self.overlay_vertex_buffer,
            &mut self.overlay_vertex_capacity,
            &mut self.overlay_vertex_count,
            "signex_gfx_polygon_overlay_vertices",
        );
    }

    fn upload_into(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        vertices: &[PolygonVertex],
        buffer: &mut wgpu::Buffer,
        capacity: &mut usize,
        count: &mut u32,
        label: &'static str,
    ) {
        if vertices.is_empty() {
            *count = 0;
            return;
        }

        let writable = super::growth::ensure_capacity(
            device,
            buffer,
            capacity,
            vertices.len(),
            &super::growth::GrowthParams {
                elem_size: std::mem::size_of::<PolygonVertex>(),
                label,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                max_buffer_size: device.limits().max_buffer_size,
            },
        );
        *count = writable as u32;

        queue.write_buffer(buffer, 0, bytemuck::cast_slice(&vertices[..writable]));
    }

    pub fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        camera_bind_group: &wgpu::BindGroup,
    ) {
        Self::draw_from(
            render_pass,
            camera_bind_group,
            &self.render_pipeline,
            &self.vertex_buffer,
            self.vertex_count,
        );
    }

    /// Draw the polygons uploaded by [`Self::upload_top`]. Callers place this
    /// after the stroke buckets and before text — see
    /// `crate::scene::GPU_SCENE_DRAW_ORDER`.
    pub fn draw_top(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        camera_bind_group: &wgpu::BindGroup,
    ) {
        Self::draw_from(
            render_pass,
            camera_bind_group,
            &self.render_pipeline,
            &self.top_vertex_buffer,
            self.top_vertex_count,
        );
    }

    /// Draw all uploaded overlay polygon geometry. Callers composite this in
    /// a pass strictly after every base bucket, so overlay content always
    /// renders on top — see `crate::scene_shader::ScenePrimitive::draw`.
    pub fn draw_overlay(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        camera_bind_group: &wgpu::BindGroup,
    ) {
        Self::draw_from(
            render_pass,
            camera_bind_group,
            &self.render_pipeline,
            &self.overlay_vertex_buffer,
            self.overlay_vertex_count,
        );
    }

    fn draw_from(
        render_pass: &mut wgpu::RenderPass<'_>,
        camera_bind_group: &wgpu::BindGroup,
        render_pipeline: &wgpu::RenderPipeline,
        vertex_buffer: &wgpu::Buffer,
        vertex_count: u32,
    ) {
        if vertex_count == 0 {
            return;
        }

        render_pass.set_pipeline(render_pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.draw(0..vertex_count, 0..1);
    }

    pub fn vertex_count(&self) -> u32 {
        self.vertex_count
    }
}

#[cfg(test)]
mod holes_tests;

#[cfg(test)]
mod tests {
    use super::triangulate_polygons;
    use crate::primitive::polygon::GpuPolygon;

    /// Shoelace area of a closed contour — the ground truth a correct
    /// triangulation's total triangle area must equal exactly, whether the
    /// contour is convex or concave.
    fn shoelace_area(points: &[[f32; 2]]) -> f64 {
        let mut sum = 0.0f64;
        for i in 0..points.len() {
            let [x0, y0] = points[i].map(f64::from);
            let [x1, y1] = points[(i + 1) % points.len()].map(f64::from);
            sum += x0 * y1 - x1 * y0;
        }
        (sum / 2.0).abs()
    }

    fn triangle_area(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f64 {
        shoelace_area(&[a, b, c])
    }

    #[test]
    fn triangulates_simple_convex_contour() {
        let polygons = vec![GpuPolygon {
            holes: Vec::new(),
            vertices: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
            fill_color: [0.3, 0.4, 0.5, 1.0],
            stroke_color: None,
            stroke_width: 0.0,
        }];

        let vertices = triangulate_polygons(&polygons);
        assert_eq!(vertices.len(), 6);
    }

    #[test]
    fn triangulates_every_contour_and_never_treats_it_as_triangle_soup() {
        // #2 regression: a six-vertex contour must triangulate into (6 - 2) = 4
        // triangles (12 vertices), NOT be passed through as two triangles
        // because its vertex count is divisible by three.
        let polygons = vec![GpuPolygon {
            holes: Vec::new(),
            vertices: vec![
                [0.0, 0.0],
                [2.0, 0.0],
                [3.0, 1.0],
                [2.0, 2.0],
                [0.0, 2.0],
                [-1.0, 1.0],
            ],
            fill_color: [0.9, 0.2, 0.1, 1.0],
            stroke_color: None,
            stroke_width: 0.0,
        }];

        let vertices = triangulate_polygons(&polygons);
        assert_eq!(vertices.len(), 12);
    }

    /// Correctness — a concave contour (e.g. an L-shaped copper pour) must
    /// fill EXACTLY its own area, no more and no less. A triangle-fan from
    /// `points[0]` bridges triangles across the notch and over-fills; ear-clip
    /// partitions the polygon exactly, so the fill vertices' total triangle
    /// area must equal the contour's shoelace area. This is the regression
    /// test for the GPU-paints-copper-where-the-pour-has-none bug.
    #[test]
    fn concave_contour_fills_exactly_its_own_area() {
        // L-shape: a 2x2 square with a 1x1 corner notched out (area = 3).
        let l_shape = vec![
            [0.0, 0.0],
            [2.0, 0.0],
            [2.0, 1.0],
            [1.0, 1.0],
            [1.0, 2.0],
            [0.0, 2.0],
        ];
        let polygons = vec![GpuPolygon {
            holes: Vec::new(),
            vertices: l_shape.clone(),
            fill_color: [0.9, 0.2, 0.1, 1.0],
            stroke_color: None,
            stroke_width: 0.0,
        }];

        let vertices = triangulate_polygons(&polygons);
        // (6 - 2) triangles = 12 vertices.
        assert_eq!(vertices.len(), 12);

        let expected_area = shoelace_area(&l_shape);
        assert!((expected_area - 3.0).abs() < 1e-9, "fixture area is 3.0");

        let total_area: f64 = vertices
            .chunks_exact(3)
            .map(|tri| triangle_area(tri[0].position, tri[1].position, tri[2].position))
            .sum();
        assert!(
            (total_area - expected_area).abs() < 1e-6,
            "concave fill must cover exactly the contour's area: expected \
             {expected_area}, got {total_area} (a fan would over-fill across \
             the notch)"
        );
    }

    #[test]
    fn emits_a_stroke_outline_after_the_fill() {
        // #3: a stroked contour emits the fill plus one quad (6 vertices) per
        // edge of the closed contour, and the stroke comes last so it
        // composites on top.
        let stroke = [0.1, 0.9, 0.3, 1.0];
        let polygons = vec![GpuPolygon {
            holes: Vec::new(),
            vertices: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
            fill_color: [0.3, 0.4, 0.5, 1.0],
            stroke_color: Some(stroke),
            stroke_width: 0.2,
        }];

        let vertices = triangulate_polygons(&polygons);
        // fill: (4 - 2) * 3 = 6; stroke: 4 edges * 6 = 24.
        assert_eq!(vertices.len(), 30);
        // Fill first, stroke last (drawn on top).
        assert_eq!(vertices[0].color, [0.3, 0.4, 0.5, 1.0]);
        assert_eq!(vertices.last().unwrap().color, stroke);
    }

    #[test]
    fn no_stroke_when_color_is_absent() {
        let polygons = vec![GpuPolygon {
            holes: Vec::new(),
            vertices: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
            fill_color: [0.3, 0.4, 0.5, 1.0],
            stroke_color: None,
            stroke_width: 0.5,
        }];

        // Fill only — 6 vertices, no stroke quads.
        assert_eq!(triangulate_polygons(&polygons).len(), 6);
    }

    /// A coloured zero-width outline is a hairline: its quads are emitted
    /// with a zero half-width and the shader raises them to the camera's
    /// `min_stroke_px`. It used to emit nothing, while the CPU replay drew it.
    #[test]
    fn zero_width_stroke_emits_hairline_quads() {
        let polygons = vec![GpuPolygon {
            holes: Vec::new(),
            vertices: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
            fill_color: [0.3, 0.4, 0.5, 1.0],
            stroke_color: Some([1.0, 1.0, 1.0, 1.0]),
            stroke_width: 0.0,
        }];

        let vertices = triangulate_polygons(&polygons);
        assert_eq!(vertices.len(), 6 + 4 * 6);
        assert!(vertices[6..].iter().all(|v| v.half_width == 0.0));
    }

    /// Stroke corners sit on the edge and carry a unit normal; fill vertices
    /// carry none, so the shader's extrusion can only ever move a stroke.
    #[test]
    fn stroke_corners_carry_the_edge_normal_and_fills_do_not() {
        let polygons = vec![GpuPolygon {
            holes: Vec::new(),
            vertices: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
            fill_color: [0.3, 0.4, 0.5, 1.0],
            stroke_color: Some([1.0, 1.0, 1.0, 1.0]),
            stroke_width: 0.2,
        }];

        let vertices = triangulate_polygons(&polygons);
        let (fill, stroke) = vertices.split_at(6);
        assert!(fill.iter().all(|v| v.extrude == [0.0, 0.0]));

        // First edge runs (0,0) -> (2,0): its corners stay on y = 0 and
        // extrude along ±Y by half of 0.2 mm.
        for v in &stroke[..6] {
            assert_eq!(v.position[1], 0.0);
            assert_eq!(v.extrude[0], 0.0);
            assert_eq!(v.extrude[1].abs(), 1.0);
            assert!((v.half_width - 0.1).abs() < 1e-6);
        }
    }

    /// Each stroke corner carries the miter vector of its end of the edge and
    /// the shorter edge at that corner, so the shader can switch to a gap-free
    /// join (#199).
    #[test]
    fn stroke_corners_carry_the_miter_and_the_edge_length() {
        let polygons = vec![GpuPolygon {
            holes: Vec::new(),
            vertices: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
            fill_color: [0.0; 4],
            stroke_color: Some([1.0; 4]),
            stroke_width: 0.2,
        }];
        let vertices = triangulate_polygons(&polygons);
        // First edge (0,0) -> (2,0). Its left normal is +Y; at a right-angle
        // corner the miter is (+-1, +-1), on the same side as the normal.
        for v in vertices.iter().filter(|v| v.half_width > 0.0).take(6) {
            assert_eq!(v.edge_len, 2.0);
            assert!((v.miter[0].abs() - 1.0).abs() < 1e-6, "{:?}", v.miter);
            assert!((v.miter[1].abs() - 1.0).abs() < 1e-6, "{:?}", v.miter);
            assert_eq!(v.miter[1].signum(), v.extrude[1].signum());
        }
        assert!(
            vertices
                .iter()
                .filter(|v| v.half_width == 0.0)
                .all(|v| v.miter == [0.0, 0.0] && v.edge_len == 0.0),
            "fill vertices carry no stroke data"
        );
    }

    /// A turn too sharp to miter keeps each edge's own normal (no bevel).
    #[test]
    fn a_sharp_turn_falls_back_to_the_edge_normal() {
        let own = [0.0, 1.0];
        // Nearly reversing: the miter would be far longer than the limit.
        let sharp = [0.0, -1.0 + 1e-3];
        assert_eq!(super::miter_vector(own, sharp), own);
        let square = super::miter_vector([0.0, 1.0], [-1.0, 0.0]);
        assert!((square[0].abs() - 1.0).abs() < 1e-6 && (square[1] - 1.0).abs() < 1e-6);
    }

    /// The two quads that share a corner must agree on the length the shader
    /// judges it by, or one mitres and the other does not and they miss each
    /// other: the corner takes the shorter of its two edges, whichever quad
    /// it is read from.
    #[test]
    fn both_quads_at_a_corner_carry_the_shorter_edge() {
        // Edges: 4 (bottom), 0.5 (right), 4 (top), 0.5 (left).
        let polygons = vec![GpuPolygon {
            holes: Vec::new(),
            vertices: vec![[0.0, 0.0], [4.0, 0.0], [4.0, 0.5], [0.0, 0.5]],
            fill_color: [0.0; 4],
            stroke_color: Some([1.0; 4]),
            stroke_width: 0.2,
        }];
        let vertices = triangulate_polygons(&polygons);
        let stroke: Vec<_> = vertices.iter().filter(|v| v.half_width > 0.0).collect();
        // Every corner is a long edge meeting a short one.
        assert!(stroke.iter().all(|v| (v.edge_len - 0.5).abs() < 1e-6));
        // Also at one corner, read from both quads.
        let at_corner: Vec<_> = stroke.iter().filter(|v| v.position == [4.0, 0.0]).collect();
        assert!(at_corner.len() >= 2);
        assert!(at_corner.iter().all(|v| (v.edge_len - 0.5).abs() < 1e-6));
    }

    #[test]
    fn skips_degenerate_polygons() {
        let polygons = vec![
            GpuPolygon {
                holes: Vec::new(),
                vertices: vec![[0.0, 0.0], [1.0, 0.0]],
                fill_color: [0.2, 0.2, 0.2, 1.0],
                stroke_color: None,
                stroke_width: 0.0,
            },
            GpuPolygon {
                holes: Vec::new(),
                vertices: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
                fill_color: [0.9, 0.2, 0.1, 1.0],
                stroke_color: None,
                stroke_width: 0.0,
            },
        ];

        let vertices = triangulate_polygons(&polygons);
        assert_eq!(vertices.len(), 6);
    }
}
