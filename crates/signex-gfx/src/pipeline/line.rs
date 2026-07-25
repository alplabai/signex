//! Line pipeline implementation.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use crate::primitive::line::LineSegment;
use crate::shader;

/// GPU line pipeline for instanced SDF segment rendering.
pub struct LinePipeline {
    render_pipeline: wgpu::RenderPipeline,
    instance_buffer: wgpu::Buffer,
    instance_capacity: usize,
    instance_count: u32,
    /// Second, independent instance buffer for overlay geometry (draw-order
    /// bucket, not a base/overlay *style* difference — same shader, same
    /// vertex layout). Kept separate from `instance_buffer` rather than a
    /// single concatenated buffer with a base/overlay split index, because
    /// callers `prepare()` (upload) and `draw()` (render) as two disjoint
    /// passes with the base buckets in between — see
    /// `crate::scene_shader::ScenePrimitive::draw`.
    overlay_instance_buffer: wgpu::Buffer,
    overlay_instance_capacity: usize,
    overlay_instance_count: u32,
}

impl LinePipeline {
    /// Create a line pipeline bound to a target surface format.
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("signex_gfx_line_shader"),
            source: wgpu::ShaderSource::Wgsl(shader::LINE_WGSL.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("signex_gfx_line_pipeline_layout"),
            bind_group_layouts: &[camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("signex_gfx_line_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<LineSegment>() as wgpu::BufferAddress,
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
                            format: wgpu::VertexFormat::Float32,
                        },
                        wgpu::VertexAttribute {
                            offset: 20,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 36,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Uint32,
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

        let initial_capacity = 1usize;
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("signex_gfx_line_instances"),
            size: std::mem::size_of::<LineSegment>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let overlay_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("signex_gfx_line_overlay_instances"),
            size: std::mem::size_of::<LineSegment>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            render_pipeline,
            instance_buffer,
            instance_capacity: initial_capacity,
            instance_count: 0,
            overlay_instance_buffer,
            overlay_instance_capacity: initial_capacity,
            overlay_instance_count: 0,
        }
    }

    /// Upload line instances into the instance buffer.
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, lines: &[LineSegment]) {
        Self::upload_into(
            device,
            queue,
            lines,
            &mut self.instance_buffer,
            &mut self.instance_capacity,
            &mut self.instance_count,
            "signex_gfx_line_instances",
        );
    }

    /// Upload overlay line instances into the dedicated overlay buffer, drawn
    /// by [`Self::draw_overlay`] in a separate later pass — see the struct
    /// doc for why this is a second buffer rather than a shared one.
    pub fn upload_overlay(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        lines: &[LineSegment],
    ) {
        Self::upload_into(
            device,
            queue,
            lines,
            &mut self.overlay_instance_buffer,
            &mut self.overlay_instance_capacity,
            &mut self.overlay_instance_count,
            "signex_gfx_line_overlay_instances",
        );
    }

    fn upload_into(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        lines: &[LineSegment],
        buffer: &mut wgpu::Buffer,
        capacity: &mut usize,
        count: &mut u32,
        label: &'static str,
    ) {
        if lines.is_empty() {
            *count = 0;
            return;
        }

        let writable = super::growth::ensure_capacity(
            device,
            buffer,
            capacity,
            lines.len(),
            std::mem::size_of::<LineSegment>(),
            label,
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            device.limits().max_buffer_size,
        );
        *count = writable as u32;

        queue.write_buffer(buffer, 0, bytemuck::cast_slice(&lines[..writable]));
    }

    /// Draw all uploaded line instances.
    pub fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        camera_bind_group: &wgpu::BindGroup,
    ) {
        Self::draw_from(
            render_pass,
            camera_bind_group,
            &self.render_pipeline,
            &self.instance_buffer,
            self.instance_count,
        );
    }

    /// Draw all uploaded overlay line instances. Callers composite this in a
    /// pass strictly after every base bucket, so overlay content always
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
            &self.overlay_instance_buffer,
            self.overlay_instance_count,
        );
    }

    fn draw_from(
        render_pass: &mut wgpu::RenderPass<'_>,
        camera_bind_group: &wgpu::BindGroup,
        render_pipeline: &wgpu::RenderPipeline,
        instance_buffer: &wgpu::Buffer,
        instance_count: u32,
    ) {
        if instance_count == 0 {
            return;
        }

        render_pass.set_pipeline(render_pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, instance_buffer.slice(..));
        render_pass.draw(0..6, 0..instance_count);
    }

    pub fn instance_count(&self) -> u32 {
        self.instance_count
    }
}
