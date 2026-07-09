//! Minimal debug render pass helpers.
//!
//! CLEAN ROOM DECLARATION
//! This module was written without reference to GPL-licensed software.
//! Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

use crate::camera::{CameraGpu, CameraUniform};
use crate::pipeline::arc::ArcPipeline;
use crate::pipeline::circle::CirclePipeline;
use crate::pipeline::grid::{GridLodFactors, GridPipeline, lod_fade_factors};
use crate::pipeline::line::LinePipeline;
use crate::pipeline::polygon::PolygonPipeline;
use crate::pipeline::text::GlyphonTextPipeline;
use crate::primitive::arc::Arc;
use crate::primitive::circle::Circle;
use crate::primitive::line::LineSegment;
use crate::primitive::polygon::GpuPolygon;
use crate::primitive::text::{TextHAlign, TextItem, TextVAlign};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SmokePassReport {
    pub scale_px_per_mm: f32,
    pub line_instances: u32,
    pub circle_instances: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompositeStage {
    Grid,
    Geometry,
    Overlay,
    Text,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompositeSmokeReport {
    pub scale_px_per_mm: f32,
    pub polygon_vertices: u32,
    pub text_instances: u32,
    pub stage_order: Vec<CompositeStage>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OverlayCompositeSmokeReport {
    pub scale_px_per_mm: f32,
    pub geometry_vertices: u32,
    pub overlay_instances: u32,
    pub text_instances: u32,
    pub stage_order: Vec<CompositeStage>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridSmokeReport {
    pub scale_px_per_mm: f32,
    pub minor_lod_alpha: f32,
    pub major_lod_alpha: f32,
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
        .map_err(|err| format!("failed to acquire adapter: {err}"))?;

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("signex_gfx_smoke_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        })
        .await
        .map_err(|err| format!("failed to acquire device: {err}"))?;

    let target_format = wgpu::TextureFormat::Bgra8Unorm;
    let camera = CameraUniform::ortho([128.0, 128.0], [0.0, 0.0], scale_px_per_mm);
    let camera_gpu = CameraGpu::new(&device, camera);

    let mut line_pipeline =
        LinePipeline::new(&device, target_format, camera_gpu.bind_group_layout());
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
                depth_slice: None,
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
    let _ = device.poll(wgpu::PollType::wait_indefinitely());

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
        .map_err(|err| format!("failed to acquire adapter: {err}"))?;

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("signex_gfx_arc_smoke_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        })
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
                depth_slice: None,
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
    let _ = device.poll(wgpu::PollType::wait_indefinitely());

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
        .map_err(|err| format!("failed to acquire adapter: {err}"))?;

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("signex_gfx_polygon_smoke_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        })
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
                depth_slice: None,
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
    let _ = device.poll(wgpu::PollType::wait_indefinitely());

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

async fn run_grid_smoke_pass_with(scale_px_per_mm: f32) -> Result<GridSmokeReport, String> {
    let instance = wgpu::Instance::default();

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .map_err(|err| format!("failed to acquire adapter: {err}"))?;

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("signex_gfx_grid_smoke_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        })
        .await
        .map_err(|err| format!("failed to acquire device: {err}"))?;

    let target_format = wgpu::TextureFormat::Bgra8Unorm;
    let camera = CameraUniform::ortho([128.0, 128.0], [0.0, 0.0], scale_px_per_mm);
    let camera_gpu = CameraGpu::new(&device, camera);
    let grid_pipeline = GridPipeline::new(&device, target_format, camera_gpu.bind_group_layout());

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("signex_gfx_grid_smoke_target"),
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
        label: Some("signex_gfx_grid_smoke_encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("signex_gfx_grid_smoke_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                depth_slice: None,
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

        grid_pipeline.draw(&mut pass, camera_gpu.bind_group());
    }

    queue.submit(Some(encoder.finish()));
    let _ = device.poll(wgpu::PollType::wait_indefinitely());

    let GridLodFactors {
        minor_alpha,
        major_alpha,
    } = lod_fade_factors(camera.mm_per_px);

    Ok(GridSmokeReport {
        scale_px_per_mm,
        minor_lod_alpha: minor_alpha,
        major_lod_alpha: major_alpha,
    })
}

pub async fn run_grid_smoke_pass() -> Result<GridSmokeReport, String> {
    run_grid_smoke_pass_with(32.0).await
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
        .map_err(|err| format!("failed to acquire adapter: {err}"))?;

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("signex_gfx_text_smoke_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        })
        .await
        .map_err(|err| format!("failed to acquire device: {err}"))?;

    let target_format = wgpu::TextureFormat::Bgra8Unorm;
    let mut text_pipeline = GlyphonTextPipeline::new(&device, &queue, target_format);

    text_pipeline
        .upload(&device, &queue, texts, scale_px_per_mm, [128, 128])
        .map_err(|err| format!("failed to prepare text: {err}"))?;

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
                depth_slice: None,
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

        text_pipeline
            .draw(&mut pass)
            .map_err(|err| format!("failed to draw text: {err}"))?;
    }

    queue.submit(Some(encoder.finish()));
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    text_pipeline.trim_atlas();

    Ok(text_pipeline.text_count())
}

pub async fn run_text_smoke_pass() -> Result<u32, String> {
    let texts = [
        TextItem {
            content: "R1".to_string(),
            position: [0.8, 0.8],
            size_mm: 1.0,
            color: [0.9, 0.9, 0.9, 1.0],
            bold: false,
            italic: false,
            rotation: 0.0,
            h_align: TextHAlign::Left,
            v_align: TextVAlign::Top,
        },
        TextItem {
            content: "VIN".to_string(),
            position: [1.6, 0.9],
            size_mm: 1.2,
            color: [0.2, 0.8, 0.9, 1.0],
            bold: true,
            italic: false,
            rotation: 0.2,
            h_align: TextHAlign::Left,
            v_align: TextVAlign::Top,
        },
    ];

    run_text_smoke_pass_with(32.0, &texts).await
}

async fn run_text_geometry_composite_smoke_pass_with(
    scale_px_per_mm: f32,
    polygons: &[GpuPolygon],
    texts: &[TextItem],
) -> Result<CompositeSmokeReport, String> {
    let instance = wgpu::Instance::default();

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .map_err(|err| format!("failed to acquire adapter: {err}"))?;

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("signex_gfx_text_geometry_composite_smoke_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        })
        .await
        .map_err(|err| format!("failed to acquire device: {err}"))?;

    let target_format = wgpu::TextureFormat::Bgra8Unorm;
    let camera = CameraUniform::ortho([128.0, 128.0], [0.0, 0.0], scale_px_per_mm);
    let camera_gpu = CameraGpu::new(&device, camera);
    let mut polygon_pipeline =
        PolygonPipeline::new(&device, target_format, camera_gpu.bind_group_layout());
    let mut text_pipeline = GlyphonTextPipeline::new(&device, &queue, target_format);

    polygon_pipeline.upload(&device, &queue, polygons);
    text_pipeline
        .upload(&device, &queue, texts, scale_px_per_mm, [128, 128])
        .map_err(|err| format!("failed to prepare text: {err}"))?;

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("signex_gfx_text_geometry_composite_smoke_target"),
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
        label: Some("signex_gfx_text_geometry_composite_smoke_encoder"),
    });
    let mut stage_order = Vec::with_capacity(2);
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("signex_gfx_text_geometry_composite_smoke_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                depth_slice: None,
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

        if polygon_pipeline.vertex_count() > 0 {
            polygon_pipeline.draw(&mut pass, camera_gpu.bind_group());
            stage_order.push(CompositeStage::Geometry);
        }

        if text_pipeline.text_count() > 0 {
            text_pipeline
                .draw(&mut pass)
                .map_err(|err| format!("failed to draw text: {err}"))?;
            stage_order.push(CompositeStage::Text);
        }
    }

    queue.submit(Some(encoder.finish()));
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    text_pipeline.trim_atlas();

    Ok(CompositeSmokeReport {
        scale_px_per_mm,
        polygon_vertices: polygon_pipeline.vertex_count(),
        text_instances: text_pipeline.text_count(),
        stage_order,
    })
}

pub async fn run_text_geometry_composite_smoke_pass() -> Result<CompositeSmokeReport, String> {
    let polygons = [GpuPolygon {
        vertices: vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]],
        fill_color: [0.15, 0.25, 0.85, 1.0],
        stroke_color: None,
        stroke_width: 0.0,
    }];

    let texts = [TextItem {
        content: "OVER".to_string(),
        position: [1.0, 1.0],
        size_mm: 1.0,
        color: [1.0, 1.0, 1.0, 1.0],
        bold: true,
        italic: false,
        rotation: 0.0,
        h_align: TextHAlign::Left,
        v_align: TextVAlign::Top,
    }];

    run_text_geometry_composite_smoke_pass_with(32.0, &polygons, &texts).await
}

async fn run_grid_overlay_text_composite_smoke_pass_with(
    scale_px_per_mm: f32,
    grid_enabled: bool,
    overlay_enabled: bool,
    text_enabled: bool,
    polygons: &[GpuPolygon],
    overlay_lines: &[LineSegment],
    texts: &[TextItem],
) -> Result<OverlayCompositeSmokeReport, String> {
    let instance = wgpu::Instance::default();

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .map_err(|err| format!("failed to acquire adapter: {err}"))?;

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("signex_gfx_grid_overlay_text_composite_smoke_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        })
        .await
        .map_err(|err| format!("failed to acquire device: {err}"))?;

    let target_format = wgpu::TextureFormat::Bgra8Unorm;
    let camera = CameraUniform::ortho([128.0, 128.0], [0.0, 0.0], scale_px_per_mm);
    let camera_gpu = CameraGpu::new(&device, camera);

    let grid_pipeline = GridPipeline::new(&device, target_format, camera_gpu.bind_group_layout());
    let mut polygon_pipeline =
        PolygonPipeline::new(&device, target_format, camera_gpu.bind_group_layout());
    let mut overlay_line_pipeline =
        LinePipeline::new(&device, target_format, camera_gpu.bind_group_layout());
    let mut text_pipeline = GlyphonTextPipeline::new(&device, &queue, target_format);

    polygon_pipeline.upload(&device, &queue, polygons);
    if overlay_enabled {
        overlay_line_pipeline.upload(&device, &queue, overlay_lines);
    } else {
        overlay_line_pipeline.upload(&device, &queue, &[]);
    }

    let empty_texts: [TextItem; 0] = [];
    let text_items = if text_enabled { texts } else { &empty_texts };
    text_pipeline
        .upload(&device, &queue, text_items, scale_px_per_mm, [128, 128])
        .map_err(|err| format!("failed to prepare text: {err}"))?;

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("signex_gfx_grid_overlay_text_composite_smoke_target"),
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
        label: Some("signex_gfx_grid_overlay_text_composite_smoke_encoder"),
    });

    let mut stage_order = Vec::with_capacity(4);
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("signex_gfx_grid_overlay_text_composite_smoke_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                depth_slice: None,
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

        if grid_enabled {
            grid_pipeline.draw(&mut pass, camera_gpu.bind_group());
            stage_order.push(CompositeStage::Grid);
        }

        if polygon_pipeline.vertex_count() > 0 {
            polygon_pipeline.draw(&mut pass, camera_gpu.bind_group());
            stage_order.push(CompositeStage::Geometry);
        }

        if overlay_enabled && overlay_line_pipeline.instance_count() > 0 {
            overlay_line_pipeline.draw(&mut pass, camera_gpu.bind_group());
            stage_order.push(CompositeStage::Overlay);
        }

        if text_enabled && text_pipeline.text_count() > 0 {
            text_pipeline
                .draw(&mut pass)
                .map_err(|err| format!("failed to draw text: {err}"))?;
            stage_order.push(CompositeStage::Text);
        }
    }

    queue.submit(Some(encoder.finish()));
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    text_pipeline.trim_atlas();

    Ok(OverlayCompositeSmokeReport {
        scale_px_per_mm,
        geometry_vertices: polygon_pipeline.vertex_count(),
        overlay_instances: overlay_line_pipeline.instance_count(),
        text_instances: text_pipeline.text_count(),
        stage_order,
    })
}

pub async fn run_grid_overlay_text_composite_smoke_pass()
-> Result<OverlayCompositeSmokeReport, String> {
    let polygons = [GpuPolygon {
        vertices: vec![[0.5, 0.5], [4.5, 0.5], [4.5, 4.5], [0.5, 4.5]],
        fill_color: [0.18, 0.22, 0.78, 1.0],
        stroke_color: None,
        stroke_width: 0.0,
    }];

    let overlays = [
        LineSegment {
            p0: [0.75, 0.75],
            p1: [4.25, 4.25],
            width: 0.12,
            color: [1.0, 0.83, 0.27, 1.0],
            style: 0,
            _pad: 0,
        },
        LineSegment {
            p0: [4.25, 0.75],
            p1: [0.75, 4.25],
            width: 0.12,
            color: [1.0, 0.83, 0.27, 1.0],
            style: 0,
            _pad: 0,
        },
    ];

    let texts = [TextItem {
        content: "TOP".to_string(),
        position: [1.2, 1.2],
        size_mm: 0.9,
        color: [1.0, 1.0, 1.0, 1.0],
        bold: true,
        italic: false,
        rotation: 0.0,
        h_align: TextHAlign::Left,
        v_align: TextVAlign::Top,
    }];

    run_grid_overlay_text_composite_smoke_pass_with(
        32.0, true, true, true, &polygons, &overlays, &texts,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::{
        CompositeStage, run_arc_smoke_pass, run_arc_smoke_pass_with,
        run_grid_overlay_text_composite_smoke_pass,
        run_grid_overlay_text_composite_smoke_pass_with, run_grid_smoke_pass,
        run_grid_smoke_pass_with, run_line_circle_smoke_pass, run_polygon_smoke_pass,
        run_polygon_smoke_pass_with, run_text_geometry_composite_smoke_pass, run_text_smoke_pass,
        run_text_smoke_pass_with,
    };
    use crate::primitive::arc::Arc;
    use crate::primitive::line::LineSegment;
    use crate::primitive::polygon::GpuPolygon;
    use crate::primitive::text::{TextHAlign, TextItem, TextVAlign};

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
        let high_zoom_count =
            pollster::block_on(run_polygon_smoke_pass_with(64.0, &polygons)).expect("polygon high");

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
    fn grid_smoke_pass_runs() {
        let report = pollster::block_on(run_grid_smoke_pass()).expect("grid pass");
        assert!((0.0..=1.0).contains(&report.minor_lod_alpha));
        assert!((0.0..=1.0).contains(&report.major_lod_alpha));
    }

    #[test]
    fn grid_smoke_pass_lod_changes_with_zoom() {
        let low_zoom = pollster::block_on(run_grid_smoke_pass_with(0.5)).expect("grid low");
        let high_zoom = pollster::block_on(run_grid_smoke_pass_with(64.0)).expect("grid high");

        assert!(high_zoom.minor_lod_alpha > low_zoom.minor_lod_alpha);
        assert!(high_zoom.major_lod_alpha > low_zoom.major_lod_alpha);
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
                position: [0.4, 0.6],
                size_mm: 0.0,
                color: [0.9, 0.9, 0.9, 1.0],
                bold: false,
                italic: false,
                rotation: 0.0,
                h_align: TextHAlign::Left,
                v_align: TextVAlign::Top,
            },
            TextItem {
                content: "VOUT".to_string(),
                position: [1.2, 0.8],
                size_mm: 2.0,
                color: [0.2, 0.8, 0.9, 1.0],
                bold: true,
                italic: false,
                rotation: -std::f32::consts::FRAC_PI_2,
                h_align: TextHAlign::Left,
                v_align: TextVAlign::Top,
            },
            TextItem {
                content: "A".to_string(),
                position: [1.0, 1.0],
                size_mm: 0.5,
                color: [0.8, 0.4, 0.2, 1.0],
                bold: false,
                italic: true,
                rotation: std::f32::consts::TAU + 0.25,
                h_align: TextHAlign::Left,
                v_align: TextVAlign::Top,
            },
        ];

        let low_zoom_count =
            pollster::block_on(run_text_smoke_pass_with(8.0, &texts)).expect("text pass low");
        let high_zoom_count =
            pollster::block_on(run_text_smoke_pass_with(64.0, &texts)).expect("text pass high");

        assert_eq!(low_zoom_count, 3);
        assert_eq!(high_zoom_count, 3);
    }

    #[test]
    fn text_smoke_pass_clips_fully_outside_viewport() {
        let texts = [
            TextItem {
                content: "INSIDE".to_string(),
                position: [1.0, 1.0],
                size_mm: 1.0,
                color: [1.0, 1.0, 1.0, 1.0],
                bold: false,
                italic: false,
                rotation: 0.0,
                h_align: TextHAlign::Left,
                v_align: TextVAlign::Top,
            },
            TextItem {
                content: "OUTSIDE".to_string(),
                position: [999.0, 999.0],
                size_mm: 1.0,
                color: [1.0, 0.2, 0.2, 1.0],
                bold: false,
                italic: false,
                rotation: 0.0,
                h_align: TextHAlign::Left,
                v_align: TextVAlign::Top,
            },
        ];

        let count =
            pollster::block_on(run_text_smoke_pass_with(32.0, &texts)).expect("text clipping pass");
        assert_eq!(count, 1);
    }

    #[test]
    fn text_smoke_pass_handles_dense_overlap_cluster() {
        let texts = [
            TextItem {
                content: "NET_A".to_string(),
                position: [3.0, 3.0],
                size_mm: 1.0,
                color: [1.0, 1.0, 1.0, 1.0],
                bold: false,
                italic: false,
                rotation: 0.0,
                h_align: TextHAlign::Left,
                v_align: TextVAlign::Top,
            },
            TextItem {
                content: "NET_B".to_string(),
                position: [3.1, 3.0],
                size_mm: 1.0,
                color: [0.8, 1.0, 0.8, 1.0],
                bold: false,
                italic: false,
                rotation: 0.0,
                h_align: TextHAlign::Left,
                v_align: TextVAlign::Top,
            },
            TextItem {
                content: "NET_C".to_string(),
                position: [3.2, 3.0],
                size_mm: 1.0,
                color: [0.8, 0.8, 1.0, 1.0],
                bold: false,
                italic: false,
                rotation: 0.0,
                h_align: TextHAlign::Left,
                v_align: TextVAlign::Top,
            },
        ];

        let count =
            pollster::block_on(run_text_smoke_pass_with(32.0, &texts)).expect("text overlap pass");
        assert_eq!(count, 3);
    }

    #[test]
    fn text_compositing_order_places_text_above_geometry() {
        let report = pollster::block_on(run_text_geometry_composite_smoke_pass())
            .expect("text geometry composite pass");

        assert_eq!(report.polygon_vertices, 6);
        assert_eq!(report.text_instances, 1);
        assert_eq!(
            report.stage_order,
            vec![CompositeStage::Geometry, CompositeStage::Text]
        );
    }

    #[test]
    fn overlay_compositing_order_places_overlay_between_geometry_and_text() {
        let report = pollster::block_on(run_grid_overlay_text_composite_smoke_pass())
            .expect("grid overlay text composite pass");

        assert_eq!(report.geometry_vertices, 6);
        assert_eq!(report.overlay_instances, 2);
        assert_eq!(report.text_instances, 1);
        assert_eq!(
            report.stage_order,
            vec![
                CompositeStage::Grid,
                CompositeStage::Geometry,
                CompositeStage::Overlay,
                CompositeStage::Text,
            ]
        );
    }

    #[test]
    fn grid_overlay_toggles_do_not_change_geometry_draw_work() {
        let polygons = [GpuPolygon {
            vertices: vec![[0.5, 0.5], [4.5, 0.5], [4.5, 4.5], [0.5, 4.5]],
            fill_color: [0.18, 0.22, 0.78, 1.0],
            stroke_color: None,
            stroke_width: 0.0,
        }];
        let overlays = [LineSegment {
            p0: [0.75, 0.75],
            p1: [4.25, 4.25],
            width: 0.12,
            color: [1.0, 0.83, 0.27, 1.0],
            style: 0,
            _pad: 0,
        }];
        let texts = [TextItem {
            content: "TOP".to_string(),
            position: [1.2, 1.2],
            size_mm: 0.9,
            color: [1.0, 1.0, 1.0, 1.0],
            bold: true,
            italic: false,
            rotation: 0.0,
            h_align: TextHAlign::Left,
            v_align: TextVAlign::Top,
        }];

        let baseline = pollster::block_on(run_grid_overlay_text_composite_smoke_pass_with(
            32.0, true, true, true, &polygons, &overlays, &texts,
        ))
        .expect("baseline composite pass");
        let overlay_off = pollster::block_on(run_grid_overlay_text_composite_smoke_pass_with(
            32.0, true, false, true, &polygons, &overlays, &texts,
        ))
        .expect("overlay toggle off pass");
        let grid_off = pollster::block_on(run_grid_overlay_text_composite_smoke_pass_with(
            32.0, false, true, true, &polygons, &overlays, &texts,
        ))
        .expect("grid toggle off pass");

        assert_eq!(baseline.geometry_vertices, overlay_off.geometry_vertices);
        assert_eq!(baseline.geometry_vertices, grid_off.geometry_vertices);
        assert_eq!(overlay_off.overlay_instances, 0);
        assert_eq!(
            overlay_off.stage_order,
            vec![
                CompositeStage::Grid,
                CompositeStage::Geometry,
                CompositeStage::Text,
            ]
        );
        assert_eq!(
            grid_off.stage_order,
            vec![
                CompositeStage::Geometry,
                CompositeStage::Overlay,
                CompositeStage::Text,
            ]
        );
    }
}
