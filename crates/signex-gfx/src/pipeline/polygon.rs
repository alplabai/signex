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

fn triangulate_polygons(polygons: &[GpuPolygon]) -> Vec<PolygonVertex> {
    let mut vertices = Vec::new();

    for polygon in polygons {
        let points = &polygon.vertices;
        if points.len() < 3 {
            continue;
        }

        if points.len() % 3 == 0 {
            for point in points {
                vertices.push(PolygonVertex {
                    position: *point,
                    color: polygon.fill_color,
                });
            }
            continue;
        }

        // Fallback triangle fan for simple contours.
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

    vertices
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
            bind_group_layouts: &[Some(camera_bind_group_layout)],
            immediate_size: 0,
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
            multiview_mask: None,
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
        self.vertex_count = vertices.len() as u32;

        if vertices.is_empty() {
            return;
        }

        if vertices.len() > self.vertex_capacity {
            self.vertex_capacity = vertices.len().next_power_of_two();
            self.vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("signex_gfx_polygon_vertices"),
                size: (self.vertex_capacity * std::mem::size_of::<PolygonVertex>())
                    as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
    }

    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
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
    fn keeps_pretriangulated_vertex_count() {
        let polygons = vec![GpuPolygon {
            vertices: vec![
                [0.0, 0.0],
                [1.0, 0.0],
                [0.0, 1.0],
                [1.0, 0.0],
                [1.0, 1.0],
                [0.0, 1.0],
            ],
            fill_color: [0.9, 0.2, 0.1, 1.0],
            stroke_color: None,
            stroke_width: 0.0,
        }];

        let vertices = triangulate_polygons(&polygons);
        assert_eq!(vertices.len(), 6);
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
