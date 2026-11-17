//! The lit triangle-mesh pipeline of 3D views (#2390): one indexed mesh
//! ([`Scene3d`]) drawn with a depth buffer of its own, then the highlighted
//! parts washed over in the selection colour.
//!
//! The pass is its own render pass on the caller's encoder (iced's
//! `Primitive::render`), loading the colour already drawn under it and
//! clearing its own depth. The depth texture follows the target size the
//! caller reports in [`MeshPipeline::prepare_depth`]; a frame whose target
//! differs (or is multisampled) is skipped rather than failing validation.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: wgpu/WGSL public docs.

use std::ops::Range;

use crate::camera3d::MeshUniform;
use crate::scene3d::{MeshVertex, Scene3d};
use crate::shader;

/// Depth format of the pass.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// A rectangle in physical pixels: x, y, width, height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PixelRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

struct Depth {
    view: wgpu::TextureView,
    size: [u32; 2],
}

/// GPU state of the mesh pipeline, created once.
pub struct MeshPipeline {
    lit: wgpu::RenderPipeline,
    highlight: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: u64,
    index_buffer: wgpu::Buffer,
    index_capacity: u64,
    index_count: u32,
    /// The generation of the mesh in the buffers; equal means no upload.
    resident: Option<u64>,
    depth: Option<Depth>,
}

impl MeshPipeline {
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("signex_gfx_mesh_shader"),
            source: wgpu::ShaderSource::Wgsl(shader::MESH_WGSL.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("signex_gfx_mesh_uniform_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("signex_gfx_mesh_uniform"),
            size: std::mem::size_of::<MeshUniform>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("signex_gfx_mesh_bind_group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("signex_gfx_mesh_pipeline_layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let lit = build_pipeline(
            device,
            &pipeline_layout,
            &module,
            target_format,
            Variant::Lit,
        );
        let highlight = build_pipeline(
            device,
            &pipeline_layout,
            &module,
            target_format,
            Variant::Highlight,
        );
        let vertex_buffer = buffer(
            device,
            "signex_gfx_mesh_vertices",
            4,
            wgpu::BufferUsages::VERTEX,
        );
        let index_buffer = buffer(
            device,
            "signex_gfx_mesh_indices",
            4,
            wgpu::BufferUsages::INDEX,
        );
        Self {
            lit,
            highlight,
            uniform,
            bind_group,
            vertex_buffer,
            vertex_capacity: 4,
            index_buffer,
            index_capacity: 4,
            index_count: 0,
            resident: None,
            depth: None,
        }
    }

    /// Upload `scene` unless generation `generation` is already resident.
    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &Scene3d,
        generation: u64,
    ) {
        if self.resident == Some(generation) {
            return;
        }
        let vertices: &[u8] = bytemuck::cast_slice(&scene.vertices);
        let indices: &[u8] = bytemuck::cast_slice(&scene.indices);
        if vertices.len() as u64 > self.vertex_capacity {
            self.vertex_capacity = (vertices.len() as u64).next_power_of_two();
            self.vertex_buffer = buffer(
                device,
                "signex_gfx_mesh_vertices",
                self.vertex_capacity,
                wgpu::BufferUsages::VERTEX,
            );
        }
        if indices.len() as u64 > self.index_capacity {
            self.index_capacity = (indices.len() as u64).next_power_of_two();
            self.index_buffer = buffer(
                device,
                "signex_gfx_mesh_indices",
                self.index_capacity,
                wgpu::BufferUsages::INDEX,
            );
        }
        // Both strides are multiples of 4, as `write_buffer` requires.
        if !vertices.is_empty() {
            queue.write_buffer(&self.vertex_buffer, 0, vertices);
        }
        if !indices.is_empty() {
            queue.write_buffer(&self.index_buffer, 0, indices);
        }
        self.index_count = scene.indices.len() as u32;
        self.resident = Some(generation);
    }

    /// Set the camera, light and highlight colour for this frame.
    pub fn set_uniform(&self, queue: &wgpu::Queue, uniform: &MeshUniform) {
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(uniform));
    }

    /// Size the depth buffer to the render target, `size` physical pixels.
    pub fn prepare_depth(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        let size = [size[0].max(1), size[1].max(1)];
        if self.depth.as_ref().is_some_and(|d| d.size == size) {
            return;
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("signex_gfx_mesh_depth"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.depth = Some(Depth { view, size });
    }

    /// Whether a frame drawn to `target` can use the depth buffer: same
    /// size, single-sampled.
    pub fn fits_target(&self, target: &wgpu::TextureView) -> bool {
        let texture = target.texture();
        self.depth
            .as_ref()
            .is_some_and(|d| d.size == [texture.width(), texture.height()])
            && texture.sample_count() == 1
    }

    /// Draw the mesh into `area` of `target`, clipped to `clip`, then wash
    /// the index ranges in `highlights` in the highlight colour.
    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        area: PixelRect,
        clip: [u32; 4],
        highlights: &[Range<u32>],
    ) {
        let Some(depth) = self.depth.as_ref() else {
            return;
        };
        if self.index_count == 0 || !self.fits_target(target) || clip[2] == 0 || clip[3] == 0 {
            return;
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("signex_gfx_mesh_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(area.x, area.y, area.width, area.height, 0.0, 1.0);
        pass.set_scissor_rect(clip[0], clip[1], clip[2], clip[3]);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.set_pipeline(&self.lit);
        pass.draw_indexed(0..self.index_count, 0, 0..1);
        pass.set_pipeline(&self.highlight);
        for range in highlights {
            let end = range.end.min(self.index_count);
            if range.start < end {
                pass.draw_indexed(range.start..end, 0, 0..1);
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Variant {
    /// Shaded, writing depth.
    Lit,
    /// The highlight colour over what is already there, depth read only.
    Highlight,
}

fn build_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    target_format: wgpu::TextureFormat,
    variant: Variant,
) -> wgpu::RenderPipeline {
    let (label, entry, depth_write, compare) = match variant {
        Variant::Lit => (
            "signex_gfx_mesh_lit_pipeline",
            "fs_main",
            true,
            wgpu::CompareFunction::Less,
        ),
        Variant::Highlight => (
            "signex_gfx_mesh_highlight_pipeline",
            "fs_highlight",
            false,
            wgpu::CompareFunction::LessEqual,
        ),
    };
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<MeshVertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: wgpu::VertexFormat::Float32x3,
                    },
                    wgpu::VertexAttribute {
                        offset: 12,
                        shader_location: 1,
                        format: wgpu::VertexFormat::Float32x4,
                    },
                ],
            })],
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
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: Some(depth_write),
            depth_compare: Some(compare),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn buffer(
    device: &wgpu::Device,
    label: &'static str,
    size: u64,
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
