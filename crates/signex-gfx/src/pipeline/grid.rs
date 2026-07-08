//! Grid pipeline implementation.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use crate::shader;

const MINOR_GRID_MM: f32 = 2.54;
const MAJOR_GRID_MM: f32 = MINOR_GRID_MM * 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridLodFactors {
    pub minor_alpha: f32,
    pub major_alpha: f32,
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if (edge1 - edge0).abs() <= f32::EPSILON {
        return if x < edge0 { 0.0 } else { 1.0 };
    }

    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn lod_fade_factors(mm_per_px: f32) -> GridLodFactors {
    let safe_mm_per_px = mm_per_px.max(0.000001);
    let px_per_mm = 1.0 / safe_mm_per_px;

    let minor_spacing_px = MINOR_GRID_MM * px_per_mm;
    let major_spacing_px = MAJOR_GRID_MM * px_per_mm;

    GridLodFactors {
        minor_alpha: smoothstep(4.0, 12.0, minor_spacing_px),
        major_alpha: smoothstep(2.0, 8.0, major_spacing_px),
    }
}

/// Fullscreen grid renderer with mm-per-pixel driven LOD fade.
pub struct GridPipeline {
    render_pipeline: wgpu::RenderPipeline,
}

impl GridPipeline {
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        camera_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("signex_gfx_grid_shader"),
            source: wgpu::ShaderSource::Wgsl(shader::GRID_WGSL.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("signex_gfx_grid_pipeline_layout"),
            bind_group_layouts: &[Some(camera_bind_group_layout)],
            immediate_size: 0,
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("signex_gfx_grid_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
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

        Self { render_pipeline }
    }

    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_bind_group: &'a wgpu::BindGroup,
    ) {
        render_pass.set_pipeline(&self.render_pipeline);
        render_pass.set_bind_group(0, camera_bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::lod_fade_factors;

    #[test]
    fn grid_lod_fade_is_density_aware() {
        let low_zoom = lod_fade_factors(2.0);
        let high_zoom = lod_fade_factors(0.02);

        assert!(high_zoom.minor_alpha > low_zoom.minor_alpha);
        assert!(high_zoom.major_alpha > low_zoom.major_alpha);
    }

    #[test]
    fn grid_lod_fade_stays_in_unit_range() {
        let factors = lod_fade_factors(0.5);
        assert!((0.0..=1.0).contains(&factors.minor_alpha));
        assert!((0.0..=1.0).contains(&factors.major_alpha));
    }
}
