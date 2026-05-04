//! Text pipeline foundation.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use crate::primitive::text::TextItem;
use crate::shader;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TextInstance {
    position: [f32; 2],
    size: [f32; 2],
    color: [f32; 4],
    rotation: f32,
    _pad: [f32; 3],
}

fn estimate_text_size(item: &TextItem) -> [f32; 2] {
    let char_count = item.content.chars().count().max(1) as f32;
    let width = char_count * item.size_mm.max(0.01) * 0.6;
    let height = item.size_mm.max(0.01);
    [width, height]
}

fn build_instances(texts: &[TextItem]) -> Vec<TextInstance> {
    let mut instances = Vec::with_capacity(texts.len());

    for text in texts {
        instances.push(TextInstance {
            position: text.position,
            size: estimate_text_size(text),
            color: text.color,
            rotation: text.rotation,
            _pad: [0.0; 3],
        });
    }

    instances
}

/// GPU text path foundation using text item bounding quads.
pub struct TextPipeline {
    render_pipeline: wgpu::RenderPipeline,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    instance_count: u32,
}

impl TextPipeline {
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("signex_gfx_text_shader"),
            source: wgpu::ShaderSource::Wgsl(shader::TEXT_WGSL.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("signex_gfx_text_pipeline_layout"),
            bind_group_layouts: &[camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("signex_gfx_text_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TextInstance>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 8,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 16,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 3,
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
                entry_point: "fs_main",
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

        let instance_capacity = 1usize;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("signex_gfx_text_instances"),
            size: std::mem::size_of::<TextInstance>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            render_pipeline,
            instance_buffer,
            instance_capacity,
            instance_count: 0,
        }
    }

    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, texts: &[TextItem]) {
        let instances = build_instances(texts);
        self.instance_count = instances.len() as u32;

        if instances.is_empty() {
            return;
        }

        if instances.len() > self.instance_capacity {
            self.instance_capacity = instances.len().next_power_of_two();
            self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("signex_gfx_text_instances"),
                size: (self.instance_capacity * std::mem::size_of::<TextInstance>())
                    as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
    }

    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
    ) {
        if self.instance_count == 0 {
            return;
        }

        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        render_pass.draw(0..6, 0..self.instance_count);
    }

    pub fn instance_count(&self) -> u32 {
        self.instance_count
    }
}

#[cfg(test)]
mod tests {
    use super::{build_instances, estimate_text_size};
    use crate::primitive::text::TextItem;

    #[test]
    fn text_extent_scales_with_content_length() {
        let short = TextItem {
            content: "R1".to_string(),
            position: [0.0, 0.0],
            size_mm: 1.0,
            color: [1.0, 1.0, 1.0, 1.0],
            bold: false,
            italic: false,
            rotation: 0.0,
        };

        let long = TextItem {
            content: "REFERENCE".to_string(),
            ..short.clone()
        };

        let short_size = estimate_text_size(&short);
        let long_size = estimate_text_size(&long);
        assert!(long_size[0] > short_size[0]);
        assert_eq!(short_size[1], 1.0);
    }

    #[test]
    fn builds_text_instances_with_style_fields() {
        let texts = vec![TextItem {
            content: "U3".to_string(),
            position: [4.0, 5.0],
            size_mm: 1.2,
            color: [0.1, 0.9, 0.3, 1.0],
            bold: true,
            italic: false,
            rotation: 0.25,
        }];

        let instances = build_instances(&texts);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].position, [4.0, 5.0]);
        assert_eq!(instances[0].color, [0.1, 0.9, 0.3, 1.0]);
        assert_eq!(instances[0].rotation, 0.25);
    }

    #[test]
    fn text_extent_handles_scale_and_empty_content() {
        let base = TextItem {
            content: "AB".to_string(),
            position: [0.0, 0.0],
            size_mm: 1.0,
            color: [1.0, 1.0, 1.0, 1.0],
            bold: false,
            italic: false,
            rotation: 0.0,
        };

        let small = TextItem {
            size_mm: 0.5,
            ..base.clone()
        };
        let large = TextItem {
            size_mm: 2.0,
            ..base.clone()
        };

        let small_size = estimate_text_size(&small);
        let large_size = estimate_text_size(&large);
        assert!(large_size[0] > small_size[0]);
        assert!(large_size[1] > small_size[1]);

        let empty = TextItem {
            content: String::new(),
            size_mm: 0.0,
            ..base
        };
        let empty_size = estimate_text_size(&empty);

        assert!((empty_size[0] - 0.006).abs() < 0.000001);
        assert!((empty_size[1] - 0.01).abs() < 0.000001);
    }

    #[test]
    fn build_instances_preserves_rotation_for_edge_cases() {
        let texts = vec![
            TextItem {
                content: "N1".to_string(),
                position: [0.0, 0.0],
                size_mm: 1.0,
                color: [1.0, 1.0, 1.0, 1.0],
                bold: false,
                italic: false,
                rotation: -std::f32::consts::FRAC_PI_2,
            },
            TextItem {
                content: "N2".to_string(),
                position: [2.0, 2.0],
                size_mm: 1.0,
                color: [0.5, 0.5, 0.5, 1.0],
                bold: false,
                italic: true,
                rotation: std::f32::consts::TAU + 0.25,
            },
        ];

        let instances = build_instances(&texts);
        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].rotation, -std::f32::consts::FRAC_PI_2);
        assert_eq!(instances[1].rotation, std::f32::consts::TAU + 0.25);
    }
}
