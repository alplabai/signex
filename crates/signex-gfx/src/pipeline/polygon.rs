//! Polygon pipeline implementation.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use crate::primitive::polygon::GpuPolygon;
use crate::shader;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct PolygonVertex {
    position: [f32; 2],
    color: [f32; 4],
}

/// Build the triangle-list vertices for a batch of polygons: a triangle-fan
/// fill for every contour, followed by a stroke outline (expanded edge quads)
/// for polygons that carry one. Fill and stroke share this pipeline's
/// `PolygonVertex` format, so they draw in a single pass — the stroke vertices
/// come last and composite on top of the fill under alpha blending, matching
/// the CPU `draw_polygons` order (fill, then stroke).
///
/// Every polygon is treated as a **closed contour**, never as a
/// pre-triangulated triangle soup: `GpuPolygon::vertices` is a contour by the
/// renderer's contract (the CPU path always fills it as one), so a vertex count
/// divisible by three must not be reinterpreted as separate triangles — doing
/// so misdrew any 3/6/9-vertex pour. The fan is exact for convex contours;
/// concave contours are an approximation shared with the CPU fallback and want
/// a proper tessellator (earcut) as a follow-up.
fn triangulate_polygons(polygons: &[GpuPolygon]) -> Vec<PolygonVertex> {
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

/// Triangle-fan fill from the first vertex. Assumes `polygon.vertices.len() >= 3`.
fn append_fill(vertices: &mut Vec<PolygonVertex>, polygon: &GpuPolygon) {
    let points = &polygon.vertices;
    let origin = points[0];
    for idx in 1..(points.len() - 1) {
        vertices.push(PolygonVertex {
            position: origin,
            color: polygon.fill_color,
        });
        vertices.push(PolygonVertex {
            position: points[idx],
            color: polygon.fill_color,
        });
        vertices.push(PolygonVertex {
            position: points[idx + 1],
            color: polygon.fill_color,
        });
    }
}

/// Stroke outline: one width-expanded quad per edge of the closed contour
/// (including the closing edge, `last -> first`), so fill-only-invisible areas
/// such as rule/keepout zones still read as an outline. World-space width in
/// mm — the shader's ortho projection scales it with zoom, mirroring the CPU's
/// `stroke_width * camera.scale`. No miter joins (thin strokes only) and no
/// screen-space minimum width; both are shared with the CPU path within
/// tolerance for the sub-0.1 mm widths the renderer emits. No-op when the
/// polygon has no stroke colour or a non-positive width.
fn append_stroke(vertices: &mut Vec<PolygonVertex>, polygon: &GpuPolygon) {
    // `is_stroked()` is the shared definition of "this contour carries an
    // outline": a stroke colour AND a positive width. Keeping the guard here in
    // terms of that predicate is what the CPU↔GPU parity test locks against.
    if !polygon.is_stroked() {
        return;
    }
    let stroke_color = polygon
        .stroke_color
        .expect("is_stroked() guarantees a stroke colour");

    let points = &polygon.vertices;
    let half = polygon.stroke_width * 0.5;
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        append_edge_quad(vertices, a, b, half, stroke_color);
    }
}

/// Emit two triangles for the rectangle centred on edge `a -> b`, `half` units
/// to either side along the edge normal.
fn append_edge_quad(
    vertices: &mut Vec<PolygonVertex>,
    a: [f32; 2],
    b: [f32; 2],
    half: f32,
    color: [f32; 4],
) {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let len = (dx * dx + dy * dy).sqrt();
    if len <= f32::EPSILON {
        return;
    }

    // Unit edge normal scaled to half the stroke width.
    let nx = -dy / len * half;
    let ny = dx / len * half;
    let a0 = [a[0] + nx, a[1] + ny];
    let a1 = [a[0] - nx, a[1] - ny];
    let b0 = [b[0] + nx, b[1] + ny];
    let b1 = [b[0] - nx, b[1] - ny];

    for position in [a0, a1, b0, b0, a1, b1] {
        vertices.push(PolygonVertex { position, color });
    }
}

/// GPU polygon pipeline for filled triangle-list geometry.
pub struct PolygonPipeline {
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: usize,
    vertex_count: u32,
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

        Self {
            render_pipeline,
            vertex_buffer,
            vertex_capacity,
            vertex_count: 0,
        }
    }

    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, polygons: &[GpuPolygon]) {
        let vertices = triangulate_polygons(polygons);

        if vertices.is_empty() {
            self.vertex_count = 0;
            return;
        }

        let writable = super::growth::ensure_capacity(
            device,
            &mut self.vertex_buffer,
            &mut self.vertex_capacity,
            vertices.len(),
            std::mem::size_of::<PolygonVertex>(),
            "signex_gfx_polygon_vertices",
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            device.limits().max_buffer_size,
        );
        self.vertex_count = writable as u32;

        queue.write_buffer(
            &self.vertex_buffer,
            0,
            bytemuck::cast_slice(&vertices[..writable]),
        );
    }

    pub fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        camera_bind_group: &wgpu::BindGroup,
    ) {
        if self.vertex_count == 0 {
            return;
        }

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..self.vertex_count, 0..1);
    }

    pub fn vertex_count(&self) -> u32 {
        self.vertex_count
    }
}

#[cfg(test)]
mod tests {
    use super::triangulate_polygons;
    use crate::primitive::polygon::GpuPolygon;

    #[test]
    fn triangulates_simple_contour_with_fan() {
        let polygons = vec![GpuPolygon {
            vertices: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
            fill_color: [0.3, 0.4, 0.5, 1.0],
            stroke_color: None,
            stroke_width: 0.0,
        }];

        let vertices = triangulate_polygons(&polygons);
        assert_eq!(vertices.len(), 6);
    }

    #[test]
    fn fans_every_contour_and_never_treats_it_as_triangle_soup() {
        // #2 regression: a six-vertex contour must fan into (6 - 2) = 4
        // triangles (12 vertices), NOT be passed through as two triangles
        // because its vertex count is divisible by three.
        let polygons = vec![GpuPolygon {
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

    #[test]
    fn emits_a_stroke_outline_after_the_fill() {
        // #3: a stroked contour emits the fan fill plus one quad (6 vertices)
        // per edge of the closed contour, and the stroke comes last so it
        // composites on top.
        let stroke = [0.1, 0.9, 0.3, 1.0];
        let polygons = vec![GpuPolygon {
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
            vertices: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
            fill_color: [0.3, 0.4, 0.5, 1.0],
            stroke_color: None,
            stroke_width: 0.5,
        }];

        // Fill only — 6 vertices, no stroke quads.
        assert_eq!(triangulate_polygons(&polygons).len(), 6);
    }

    #[test]
    fn no_stroke_when_width_is_non_positive() {
        let polygons = vec![GpuPolygon {
            vertices: vec![[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
            fill_color: [0.3, 0.4, 0.5, 1.0],
            stroke_color: Some([1.0, 1.0, 1.0, 1.0]),
            stroke_width: 0.0,
        }];

        // Zero width contributes no stroke geometry — fill only.
        assert_eq!(triangulate_polygons(&polygons).len(), 6);
    }

    #[test]
    fn skips_degenerate_polygons() {
        let polygons = vec![
            GpuPolygon {
                vertices: vec![[0.0, 0.0], [1.0, 0.0]],
                fill_color: [0.2, 0.2, 0.2, 1.0],
                stroke_color: None,
                stroke_width: 0.0,
            },
            GpuPolygon {
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
