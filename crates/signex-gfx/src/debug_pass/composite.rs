//! Composite (text+geometry / grid+overlay+text) smoke passes.

use super::*;

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

pub(super) async fn run_grid_overlay_text_composite_smoke_pass_with(
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
        .upload(
            &device,
            &queue,
            text_items,
            scale_px_per_mm,
            [128, 128],
            [0.0, 0.0],
        )
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
