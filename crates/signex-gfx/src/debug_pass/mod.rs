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

/// Render one dashed [`LineSegment`] and read back the framebuffer's red
/// channel at a handful of x pixels along the line's centre row. Proves
/// `line.wgsl` actually honours `LineSegment::STYLE_DASHED` on the GPU
/// (rather than just in the Rust-side `is_dashed()` predicate `order.rs`
/// pins): a solid white line over a black clear reads bright everywhere,
/// while a real dash pattern reads bright only where the shader's dash math
/// says it should. Returns the sampled red-channel bytes (0-255) at the given
/// world-mm x offsets along the line.
///
/// Geometry is chosen so the shader's dash math (`dash_mm = 8 * mm_per_px`,
/// `gap_mm = 5 * mm_per_px`) lands on exact pixel boundaries at
/// `scale_px_per_mm = 4.0` (`mm_per_px = 0.25`): an 8px dash then a 5px gap,
/// repeating every 13px — independent of zoom, since `dash_mm *
/// scale_px_per_mm` always collapses back to the literal `8`.
///
/// `cfg(test)` (unlike the other smoke-pass helpers above, which are `pub`
/// for `tests/regression_golden.rs` to call as a separate crate): this one
/// has no caller outside `debug_pass::tests`.
#[cfg(test)]
async fn run_line_dash_readback_smoke_pass(sample_x_px: &[u32]) -> Result<Vec<u8>, String> {
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
            label: Some("signex_gfx_dash_readback_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        })
        .await
        .map_err(|err| format!("failed to acquire device: {err}"))?;

    let target_format = wgpu::TextureFormat::Bgra8Unorm;
    const WIDTH: u32 = 128;
    const HEIGHT: u32 = 8;
    let scale_px_per_mm = 4.0f32;
    let camera = CameraUniform::ortho([WIDTH as f32, HEIGHT as f32], [0.0, 0.0], scale_px_per_mm);
    let camera_gpu = CameraGpu::new(&device, camera);

    let mut line_pipeline =
        LinePipeline::new(&device, target_format, camera_gpu.bind_group_layout());

    // Centre row (world y = 1mm = HEIGHT/2 px), spanning almost the full
    // viewport width so several dash/gap periods land inside it.
    let lines = [LineSegment {
        p0: [0.0, 1.0],
        p1: [30.0, 1.0],
        width: 1.0,
        color: [1.0, 1.0, 1.0, 1.0],
        style: LineSegment::STYLE_DASHED,
        _pad: 0,
    }];
    line_pipeline.upload(&device, &queue, &lines);

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("signex_gfx_dash_readback_target"),
        size: wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
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
        label: Some("signex_gfx_dash_readback_encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("signex_gfx_dash_readback_render_pass"),
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
    }

    // Bgra8Unorm is 4 bytes/pixel; WIDTH * 4 = 512 is already a multiple of
    // wgpu::COPY_BYTES_PER_ROW_ALIGNMENT (256), so no row padding is needed.
    let bytes_per_row = WIDTH * 4;
    let buffer_size = (bytes_per_row * HEIGHT) as wgpu::BufferAddress;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("signex_gfx_dash_readback_buffer"),
        size: buffer_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(HEIGHT),
            },
        },
        wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
    );

    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    slice.map_async(wgpu::MapMode::Read, |result| {
        result.expect("readback buffer map failed");
    });
    let _ = device.poll(wgpu::PollType::wait_indefinitely());

    let row = HEIGHT / 2;
    let samples = {
        let data = slice.get_mapped_range();
        sample_x_px
            .iter()
            .map(|&x| {
                let offset = (row * bytes_per_row + x * 4) as usize;
                // Bgra8Unorm byte order is [B, G, R, A]; red is byte index 2.
                data[offset + 2]
            })
            .collect::<Vec<u8>>()
    };
    readback.unmap();

    Ok(samples)
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
        .upload(
            &device,
            &queue,
            texts,
            scale_px_per_mm,
            [128, 128],
            [0.0, 0.0],
        )
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

mod composite;
#[cfg(test)]
mod tests;

pub use composite::{
    run_grid_overlay_text_composite_smoke_pass, run_text_geometry_composite_smoke_pass,
};
