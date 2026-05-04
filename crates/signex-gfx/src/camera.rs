//! Camera uniform definitions for 2D and 3D views.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use wgpu::util::DeviceExt;

/// Shared camera uniform uploaded once per frame.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub viewport: [f32; 2],
    pub mm_per_px: f32,
    pub _pad: f32,
}

impl CameraUniform {
    /// Build an orthographic camera for 2D views.
    pub fn ortho(viewport_px: [f32; 2], offset_mm: [f32; 2], scale_px_per_mm: f32) -> Self {
        let width_mm = viewport_px[0] / scale_px_per_mm;
        let height_mm = viewport_px[1] / scale_px_per_mm;

        let left = offset_mm[0];
        let right = left + width_mm;
        let top = offset_mm[1];
        let bottom = top + height_mm;

        let proj = glam::Mat4::orthographic_rh_gl(left, right, bottom, top, -1.0, 1.0);

        Self {
            view_proj: proj.to_cols_array_2d(),
            viewport: viewport_px,
            mm_per_px: 1.0 / scale_px_per_mm,
            _pad: 0.0,
        }
    }

    /// Build a perspective camera for future 3D views.
    pub fn perspective(
        viewport_px: [f32; 2],
        eye: glam::Vec3,
        target: glam::Vec3,
        fov_rad: f32,
    ) -> Self {
        let aspect = if viewport_px[1] > 0.0 {
            viewport_px[0] / viewport_px[1]
        } else {
            1.0
        };

        let view = glam::Mat4::look_at_rh(eye, target, glam::Vec3::Y);
        let proj = glam::Mat4::perspective_rh_gl(fov_rad, aspect, 0.01, 10_000.0);
        let view_proj = proj * view;

        Self {
            view_proj: view_proj.to_cols_array_2d(),
            viewport: viewport_px,
            mm_per_px: 0.0,
            _pad: 0.0,
        }
    }
}

/// GPU resources for the shared camera uniform.
pub struct CameraGpu {
    buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

impl CameraGpu {
    pub fn new(device: &wgpu::Device, initial: CameraUniform) -> Self {
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("signex_gfx_camera_uniform"),
            contents: bytemuck::bytes_of(&initial),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("signex_gfx_camera_layout"),
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("signex_gfx_camera_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        Self {
            buffer,
            bind_group_layout,
            bind_group,
        }
    }

    pub fn update(&self, queue: &wgpu::Queue, camera: CameraUniform) {
        queue.write_buffer(&self.buffer, 0, bytemuck::bytes_of(&camera));
    }

    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}
