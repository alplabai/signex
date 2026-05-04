//! Minimal debug render pass helpers.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use crate::camera::{CameraGpu, CameraUniform};
use crate::pipeline::arc::ArcPipeline;
use crate::pipeline::circle::CirclePipeline;
use crate::pipeline::line::LinePipeline;
use crate::pipeline::polygon::PolygonPipeline;
use crate::pipeline::text::TextPipeline;
use crate::primitive::arc::Arc;
use crate::primitive::circle::Circle;
use crate::primitive::line::LineSegment;
use crate::primitive::polygon::GpuPolygon;
use crate::primitive::text::TextItem;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmokePassReport {
    pub scale_px_per_mm: f32,
    pub line_instances: u32,
    pub circle_instances: u32,
}

pub async fn run_line_circle_smoke_pass(scale_px_per_mm: f32) -> Result<SmokePassReport, String> {
    let instance = wgpu::Instance::default();

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .ok_or_else(|| "failed to acquire adapter".to_string())?;

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("signex_gfx_smoke_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            memory_hints: wgpu::MemoryHints::Performance,
        }, None)
        .await
        .map_err(|err| format!("failed to acquire device: {err}"))?;

    let target_format = wgpu::TextureFormat::Bgra8Unorm;
    let camera = CameraUniform::ortho([128.0, 128.0], [0.0, 0.0], scale_px_per_mm);
    let camera_gpu = CameraGpu::new(&device, camera);

    let mut line_pipeline = LinePipeline::new(&device, target_format, camera_gpu.bind_group_layout());
    let mut circle_pipeline =
        CirclePipeline::new(&device, target_format, camera_gpu.bind_group_layout());

    let lines = [
        LineSegment {
            p0: [1.0, 1.0],
            p1: [8.0, 1.0],
            width: 0.15,
            color: [1.0, 1.0, 1.0, 1.0],
            style: 0,
            _pad: 0,
        },
        LineSegment {
            p0: [1.0, 2.0],
            p1: [8.0, 2.0],
            width: 0.45,
            color: [1.0, 1.0, 1.0, 1.0],
            style: 0,
            _pad: 0,
        },
    ];

    let circles = [Circle {
        center: [4.0, 4.0],
        radius: 0.4,
        stroke_width: 0.0,
        color: [1.0, 1.0, 1.0, 1.0],
    }];

    line_pipeline.upload(&device, &queue, &lines);
    circle_pipeline.upload(&device, &queue, &circles);

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("signex_gfx_smoke_target"),
        size: wgpu::Extent3d {
            width: 128,
            height: 128,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: target_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });

    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("signex_gfx_smoke_encoder"),
    });

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("signex_gfx_smoke_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        line_pipeline.draw(&mut pass, camera_gpu.bind_group());
        circle_pipeline.draw(&mut pass, camera_gpu.bind_group());
    }

    queue.submit(Some(encoder.finish()));
    let _ = device.poll(wgpu::Maintain::Wait);

    Ok(SmokePassReport {
        scale_px_per_mm,
        line_instances: line_pipeline.instance_count(),
        circle_instances: circle_pipeline.instance_count(),
    })
}

async fn run_arc_smoke_pass_with(scale_px_per_mm: f32, arcs: &[Arc]) -> Result<u32, String> {
    let instance = wgpu::Instance::default();

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .ok_or_else(|| "failed to acquire adapter".to_string())?;

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("signex_gfx_arc_smoke_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        )
        .await
        .map_err(|err| format!("failed to acquire device: {err}"))?;

    let target_format = wgpu::TextureFormat::Bgra8Unorm;
    let camera = CameraUniform::ortho([128.0, 128.0], [0.0, 0.0], scale_px_per_mm);
    let camera_gpu = CameraGpu::new(&device, camera);

    let mut arc_pipeline = ArcPipeline::new(&device, target_format, camera_gpu.bind_group_layout());

    arc_pipeline.upload(&device, &queue, arcs);

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("signex_gfx_arc_smoke_target"),
        size: wgpu::Extent3d {
            width: 128,
            height: 128,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: target_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("signex_gfx_arc_smoke_encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("signex_gfx_arc_smoke_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        arc_pipeline.draw(&mut pass, camera_gpu.bind_group());
    }

    queue.submit(Some(encoder.finish()));
    let _ = device.poll(wgpu::Maintain::Wait);

    Ok(arc_pipeline.instance_count())
}

pub async fn run_arc_smoke_pass() -> Result<u32, String> {
    let arcs = [Arc {
        center: [4.0, 4.0],
        radius: 2.0,
        start_angle: 0.0,
        end_angle: 1.5707964,
        width: 0.2,
        color: [1.0, 1.0, 1.0, 1.0],
        _pad: [0.0; 3],
    }];

    run_arc_smoke_pass_with(32.0, &arcs).await
}

async fn run_polygon_smoke_pass_with(
    scale_px_per_mm: f32,
    polygons: &[GpuPolygon],
) -> Result<u32, String> {
    let instance = wgpu::Instance::default();

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .ok_or_else(|| "failed to acquire adapter".to_string())?;

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("signex_gfx_polygon_smoke_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        )
        .await
        .map_err(|err| format!("failed to acquire device: {err}"))?;

    let target_format = wgpu::TextureFormat::Bgra8Unorm;
    let camera = CameraUniform::ortho([128.0, 128.0], [0.0, 0.0], scale_px_per_mm);
    let camera_gpu = CameraGpu::new(&device, camera);
    let mut polygon_pipeline =
        PolygonPipeline::new(&device, target_format, camera_gpu.bind_group_layout());

    polygon_pipeline.upload(&device, &queue, polygons);

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("signex_gfx_polygon_smoke_target"),
        size: wgpu::Extent3d {
            width: 128,
            height: 128,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: target_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("signex_gfx_polygon_smoke_encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("signex_gfx_polygon_smoke_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        polygon_pipeline.draw(&mut pass, camera_gpu.bind_group());
    }

    queue.submit(Some(encoder.finish()));
    let _ = device.poll(wgpu::Maintain::Wait);

    Ok(polygon_pipeline.vertex_count())
}

pub async fn run_polygon_smoke_pass() -> Result<u32, String> {
    let polygons = [GpuPolygon {
        vertices: vec![[2.0, 2.0], [8.0, 2.0], [8.0, 8.0], [2.0, 8.0]],
        fill_color: [0.2, 0.7, 0.9, 1.0],
        stroke_color: None,
        stroke_width: 0.0,
    }];

    run_polygon_smoke_pass_with(32.0, &polygons).await
}

async fn run_text_smoke_pass_with(scale_px_per_mm: f32, texts: &[TextItem]) -> Result<u32, String> {
    let instance = wgpu::Instance::default();

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .ok_or_else(|| "failed to acquire adapter".to_string())?;

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("signex_gfx_text_smoke_device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::Performance,
            },
            None,
        )
        .await
        .map_err(|err| format!("failed to acquire device: {err}"))?;

    let target_format = wgpu::TextureFormat::Bgra8Unorm;
    let camera = CameraUniform::ortho([128.0, 128.0], [0.0, 0.0], scale_px_per_mm);
    let camera_gpu = CameraGpu::new(&device, camera);
    let mut text_pipeline = TextPipeline::new(&device, target_format, camera_gpu.bind_group_layout());

    text_pipeline.upload(&device, &queue, texts);

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("signex_gfx_text_smoke_target"),
        size: wgpu::Extent3d {
            width: 128,
            height: 128,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: target_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("signex_gfx_text_smoke_encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("signex_gfx_text_smoke_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        text_pipeline.draw(&mut pass, camera_gpu.bind_group());
    }

    queue.submit(Some(encoder.finish()));
    let _ = device.poll(wgpu::Maintain::Wait);

    Ok(text_pipeline.instance_count())
}

pub async fn run_text_smoke_pass() -> Result<u32, String> {
    let texts = [
        TextItem {
            content: "R1".to_string(),
            position: [4.0, 6.0],
            size_mm: 1.0,
            color: [0.9, 0.9, 0.9, 1.0],
            bold: false,
            italic: false,
            rotation: 0.0,
        },
        TextItem {
            content: "VIN".to_string(),
            position: [8.0, 3.0],
            size_mm: 1.2,
            color: [0.2, 0.8, 0.9, 1.0],
            bold: true,
            italic: false,
            rotation: 0.2,
        },
    ];

    run_text_smoke_pass_with(32.0, &texts).await
}

#[cfg(test)]
mod tests {
    use super::{
        run_arc_smoke_pass, run_arc_smoke_pass_with, run_line_circle_smoke_pass,
        run_polygon_smoke_pass, run_polygon_smoke_pass_with, run_text_smoke_pass,
        run_text_smoke_pass_with,
    };
    use crate::primitive::arc::Arc;
    use crate::primitive::polygon::GpuPolygon;
    use crate::primitive::text::TextItem;

    #[test]
    fn line_circle_smoke_pass_runs_for_multiple_scales() {
        let low_zoom = pollster::block_on(run_line_circle_smoke_pass(8.0)).expect("low zoom pass");
        let high_zoom =
            pollster::block_on(run_line_circle_smoke_pass(64.0)).expect("high zoom pass");

        assert_eq!(low_zoom.line_instances, 2);
        assert_eq!(low_zoom.circle_instances, 1);
        assert_eq!(high_zoom.line_instances, 2);
        assert_eq!(high_zoom.circle_instances, 1);
    }

    #[test]
    fn arc_smoke_pass_runs() {
        let count = pollster::block_on(run_arc_smoke_pass()).expect("arc pass");
        assert_eq!(count, 1);
    }

    #[test]
    fn arc_smoke_pass_handles_wraparound_sweep() {
        let wrap_start = 2.0 * std::f32::consts::PI - std::f32::consts::FRAC_PI_6;
        let wrap_end = std::f32::consts::FRAC_PI_6;

        let arcs = [Arc {
            center: [4.0, 4.0],
            radius: 2.0,
            start_angle: wrap_start,
            end_angle: wrap_end,
            width: 0.2,
            color: [1.0, 1.0, 1.0, 1.0],
            _pad: [0.0; 3],
        }];

        let count =
            pollster::block_on(run_arc_smoke_pass_with(8.0, &arcs)).expect("wraparound arc pass");
        assert_eq!(count, 1);
    }

    #[test]
    fn arc_smoke_pass_handles_tiny_radius() {
        let arcs = [Arc {
            center: [4.0, 4.0],
            radius: 0.01,
            start_angle: 0.0,
            end_angle: 1.5707964,
            width: 0.005,
            color: [1.0, 1.0, 1.0, 1.0],
            _pad: [0.0; 3],
        }];

        let count =
            pollster::block_on(run_arc_smoke_pass_with(64.0, &arcs)).expect("tiny radius arc pass");
        assert_eq!(count, 1);
    }

    #[test]
    fn polygon_smoke_pass_runs() {
        let vertex_count = pollster::block_on(run_polygon_smoke_pass()).expect("polygon pass");
        assert_eq!(vertex_count, 6);
    }

    #[test]
    fn polygon_smoke_pass_handles_low_and_high_zoom() {
        let polygons = [GpuPolygon {
            vertices: vec![[2.0, 2.0], [8.0, 2.0], [8.0, 8.0], [2.0, 8.0]],
            fill_color: [0.2, 0.7, 0.9, 1.0],
            stroke_color: None,
            stroke_width: 0.0,
        }];

        let low_zoom_count =
            pollster::block_on(run_polygon_smoke_pass_with(8.0, &polygons)).expect("polygon low");
        let high_zoom_count = pollster::block_on(run_polygon_smoke_pass_with(64.0, &polygons))
            .expect("polygon high");

        assert_eq!(low_zoom_count, 6);
        assert_eq!(high_zoom_count, 6);
    }

    #[test]
    fn polygon_smoke_pass_ignores_degenerate_geometry() {
        let polygons = [
            GpuPolygon {
                vertices: vec![[0.0, 0.0], [1.0, 0.0]],
                fill_color: [1.0, 0.0, 0.0, 1.0],
                stroke_color: None,
                stroke_width: 0.0,
            },
            GpuPolygon {
                vertices: vec![[2.0, 2.0], [8.0, 2.0], [8.0, 8.0], [2.0, 8.0]],
                fill_color: [0.2, 0.7, 0.9, 1.0],
                stroke_color: None,
                stroke_width: 0.0,
            },
        ];

        let count = pollster::block_on(run_polygon_smoke_pass_with(32.0, &polygons))
            .expect("polygon degenerate filter");
        assert_eq!(count, 6);
    }

    #[test]
    fn text_smoke_pass_runs() {
        let text_count = pollster::block_on(run_text_smoke_pass()).expect("text pass");
        assert_eq!(text_count, 2);
    }

    #[test]
    fn text_smoke_pass_handles_scale_rotation_and_empty_content() {
        let texts = [
            TextItem {
                content: String::new(),
                position: [4.0, 6.0],
                size_mm: 0.0,
                color: [0.9, 0.9, 0.9, 1.0],
                bold: false,
                italic: false,
                rotation: 0.0,
            },
            TextItem {
                content: "VOUT".to_string(),
                position: [8.0, 3.0],
                size_mm: 2.0,
                color: [0.2, 0.8, 0.9, 1.0],
                bold: true,
                italic: false,
                rotation: -std::f32::consts::FRAC_PI_2,
            },
            TextItem {
                content: "A".to_string(),
                position: [1.0, 1.0],
                size_mm: 0.5,
                color: [0.8, 0.4, 0.2, 1.0],
                bold: false,
                italic: true,
                rotation: std::f32::consts::TAU + 0.25,
            },
        ];

        let low_zoom_count =
            pollster::block_on(run_text_smoke_pass_with(8.0, &texts)).expect("text pass low");
        let high_zoom_count = pollster::block_on(run_text_smoke_pass_with(64.0, &texts))
            .expect("text pass high");

        assert_eq!(low_zoom_count, 3);
        assert_eq!(high_zoom_count, 3);
    }
}