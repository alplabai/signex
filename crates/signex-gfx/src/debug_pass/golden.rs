//! Offscreen RGBA8 renders of representative smoke scenes, read back to the
//! CPU so a test can compare pixels (#113). Every scene draws into a fixed
//! [`GOLDEN_SIZE_PX`] square target; text shapes with only the faces the
//! caller supplies, never system fonts.

use super::*;

/// Edge length of the square golden target. `128 * 4` bytes is a multiple of
/// `wgpu::COPY_BYTES_PER_ROW_ALIGNMENT`, so readback rows need no padding.
pub const GOLDEN_SIZE_PX: u32 = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GoldenScene {
    LineCircle,
    Arc,
    Polygon,
    Text,
    Grid,
}

/// One readback: tightly packed RGBA8 rows plus the adapter that drew them.
#[derive(Clone, Debug)]
pub struct GoldenFrame {
    pub rgba: Vec<u8>,
    pub adapter_name: String,
    pub backend: String,
    /// Mesa's CPU rasterizer (lavapipe/llvmpipe) — the one adapter the exact
    /// references are generated on.
    pub is_lavapipe: bool,
}

pub async fn render_golden_scene(
    scene: GoldenScene,
    fonts: &[&'static [u8]],
) -> Result<GoldenFrame, String> {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .map_err(|err| format!("failed to acquire adapter: {err}"))?;
    let info = adapter.get_info();
    let lowered = info.name.to_lowercase();
    let is_lavapipe = info.device_type == wgpu::DeviceType::Cpu
        && (lowered.contains("llvmpipe") || lowered.contains("lavapipe"));

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("signex_gfx_golden_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::Performance,
            trace: wgpu::Trace::Off,
        })
        .await
        .map_err(|err| format!("failed to acquire device: {err}"))?;

    let format = wgpu::TextureFormat::Bgra8Unorm;
    let size = GOLDEN_SIZE_PX;
    let scale = if scene == GoldenScene::Grid {
        32.0
    } else {
        12.0
    };
    let camera = CameraUniform::ortho([size as f32; 2], [0.0, 0.0], scale);
    let camera_gpu = CameraGpu::new(&device, camera);
    let layout = camera_gpu.bind_group_layout();

    let mut line = LinePipeline::new(&device, format, layout);
    let mut circle = CirclePipeline::new(&device, format, layout);
    let mut arc = ArcPipeline::new(&device, format, layout);
    let mut polygon = PolygonPipeline::new(&device, format, layout);
    let grid = GridPipeline::new(&device, format, layout);
    let mut text = GlyphonTextPipeline::new(&device, &queue, format);

    match scene {
        GoldenScene::LineCircle => {
            line.upload(&device, &queue, &scene_lines());
            circle.upload(&device, &queue, &scene_circles());
        }
        GoldenScene::Arc => arc.upload(&device, &queue, &scene_arcs()),
        GoldenScene::Polygon => polygon.upload(&device, &queue, &scene_polygons()),
        GoldenScene::Text => {
            // Hermetic: `FontSystem::new` would also load system fonts, and a
            // machine with its own Iosevka would shape with that (#654).
            let mut db = cryoglyph::fontdb::Database::new();
            for font in fonts {
                db.load_font_source(cryoglyph::fontdb::Source::Binary(std::sync::Arc::new(
                    *font,
                )));
            }
            let mut font_system =
                cryoglyph::FontSystem::new_with_locale_and_db("en-US".to_string(), db);
            text.upload(
                &device,
                &queue,
                &mut font_system,
                &scene_texts(),
                scale,
                DEBUG_TEXT_POLICY,
                DEBUG_TEXT_FAMILY,
                [size, size],
                [0.0, 0.0],
            )
            .map_err(|err| format!("failed to prepare text: {err}"))?;
        }
        GoldenScene::Grid => {}
    }

    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("signex_gfx_golden_target"),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("signex_gfx_golden_encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("signex_gfx_golden_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
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
        let bind_group = camera_gpu.bind_group();
        match scene {
            GoldenScene::LineCircle => {
                line.draw(&mut pass, bind_group);
                circle.draw(&mut pass, bind_group);
            }
            GoldenScene::Arc => arc.draw(&mut pass, bind_group),
            GoldenScene::Polygon => polygon.draw(&mut pass, bind_group),
            GoldenScene::Grid => grid.draw(&mut pass, bind_group),
            GoldenScene::Text => text
                .draw(&mut pass)
                .map_err(|err| format!("failed to draw text: {err}"))?,
        }
    }

    let bytes_per_row = size * 4;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("signex_gfx_golden_readback"),
        size: wgpu::BufferAddress::from(bytes_per_row * size),
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
                rows_per_image: Some(size),
            },
        },
        wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
    );
    queue.submit(Some(encoder.finish()));

    let (tx, rx) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |res| {
            // The receiver outlives the poll below, so the send cannot fail.
            let _ = tx.send(res);
        });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|err| format!("device poll failed: {err}"))?;
    rx.recv()
        .map_err(|err| format!("readback map callback dropped: {err}"))?
        .map_err(|err| format!("readback map failed: {err}"))?;

    let mut rgba = readback.slice(..).get_mapped_range().to_vec();
    readback.unmap();
    for px in rgba.chunks_exact_mut(4) {
        px.swap(0, 2); // Bgra8 -> Rgba8
    }
    Ok(GoldenFrame {
        rgba,
        adapter_name: info.name,
        backend: format!("{:?}", info.backend),
        is_lavapipe,
    })
}

fn scene_lines() -> Vec<LineSegment> {
    [(1.5, 0.15), (3.0, 0.45)]
        .into_iter()
        .map(|(y, width)| LineSegment {
            p0: [1.0, y],
            p1: [9.5, y],
            width,
            color: [1.0, 1.0, 1.0, 1.0],
            style: 0,
            _pad: 0,
        })
        .collect()
}

fn scene_circles() -> Vec<Circle> {
    vec![
        Circle {
            center: [3.0, 7.0],
            radius: 1.5,
            stroke_width: 0.0,
            color: [1.0, 1.0, 1.0, 1.0],
        },
        Circle {
            center: [7.5, 7.0],
            radius: 1.5,
            stroke_width: 0.25,
            color: [1.0, 1.0, 1.0, 1.0],
        },
    ]
}

fn scene_arcs() -> Vec<Arc> {
    [(3.5, 0.0, 1.570_796_4, 0.4), (2.0, 2.0, 5.0, 0.3)]
        .into_iter()
        .map(|(radius, start_angle, end_angle, width)| Arc {
            center: [5.0, 5.0],
            radius,
            start_angle,
            end_angle,
            width,
            color: [1.0, 1.0, 1.0, 1.0],
            _pad: [0.0; 3],
        })
        .collect()
}

fn scene_polygons() -> Vec<GpuPolygon> {
    vec![
        GpuPolygon {
            holes: Vec::new(),
            vertices: vec![[1.0, 1.0], [5.0, 1.0], [5.0, 5.0], [1.0, 5.0]],
            fill_color: [0.2, 0.7, 0.9, 1.0],
            stroke_color: None,
            stroke_width: 0.0,
        },
        GpuPolygon {
            holes: Vec::new(),
            vertices: vec![[6.0, 6.0], [10.0, 6.0], [8.0, 10.0]],
            fill_color: [0.9, 0.2, 0.2, 1.0],
            stroke_color: Some([1.0, 1.0, 1.0, 1.0]),
            stroke_width: 0.2,
        },
    ]
}

fn scene_texts() -> Vec<TextItem> {
    [
        ("R1", [0.8, 0.8], false, [0.9, 0.9, 0.9, 1.0]),
        ("VIN", [0.8, 4.0], true, [0.2, 0.8, 0.9, 1.0]),
    ]
    .into_iter()
    .map(|(content, position, bold, color)| TextItem {
        content: content.to_string(),
        position,
        size_mm: 2.0,
        color,
        bold,
        italic: false,
        rotation: 0.0,
        h_align: TextHAlign::Left,
        v_align: TextVAlign::Top,
    })
    .collect()
}
