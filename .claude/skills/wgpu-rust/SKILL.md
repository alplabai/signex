---
name: wgpu-rust
description: >
  wgpu kütüphanesiyle Rust GPU programlama için kapsamlı referans.
  Instance/Adapter/Device/Queue bootstrap, render pipeline, vertex/index buffer,
  uniform buffer, bind group, texture, instanced drawing, compute pipeline,
  WGSL shader yazımı, buffer upload stratejileri (write_buffer, staging),
  bytemuck Pod/Zeroable, best practices, middleware pattern gibi konuları kapsar.
  "wgpu", "wgpu rust", "gpu rendering rust", "wgsl", "render pipeline",
  "vertex buffer", "instance buffer", "bind group", "uniform buffer",
  "compute shader", "gpu instancing" gibi ifadelerde mutlaka tetiklenmeli.
---

# wgpu — Rust GPU Library Reference

> Source: docs.rs/wgpu 28.0.0, github.com/gfx-rs/wgpu wiki, sotrh.github.io/learn-wgpu
> Version: wgpu 28.x (April 2026). MSRV: 1.87.

---

## Reference map

| Topic | File |
|-------|------|
| Bootstrap — Instance, Adapter, Device, Queue, Surface | `references/bootstrap.md` |
| Buffers — vertex, index, uniform, storage, staging | `references/buffers.md` |
| Render pipeline — descriptor, vertex layout, blend, depth | `references/render-pipeline.md` |
| Bind groups — layout, uniform, texture, sampler | `references/bind-groups.md` |
| Textures — creation, upload, sampler, sRGB | `references/textures.md` |
| Instanced drawing — instance buffer, GPU instancing | `references/instancing.md` |
| Compute pipeline — dispatch, storage buffers, readback | `references/compute.md` |
| WGSL — types, structs, vertex/fragment/compute | `references/wgsl.md` |
| Best practices and Do's/Don'ts | `references/best-practices.md` |

---

## Key concepts at a glance

```
Instance          Entry point; enumerate adapters.
  └─ Adapter      Physical GPU handle; query capabilities.
       └─ Device  Logical GPU; create all resources.
       └─ Queue   Submit CommandBuffers; upload data.

Device creates:
  Buffer          GPU-accessible memory (vertex, index, uniform, storage, staging).
  Texture         2D/3D image on the GPU.
  ShaderModule    Compiled WGSL/SPIR-V shader.
  BindGroupLayout Declares what resources a shader slot expects.
  BindGroup       Binds actual resources to a layout.
  RenderPipeline  Fixed draw pipeline (vertex + fragment shaders + state).
  ComputePipeline Compute-only pipeline.

CommandEncoder   Records CPU-side commands; finalises into CommandBuffer.
RenderPass       Records draw calls within an encoder.
ComputePass      Records dispatch calls within an encoder.
Queue::submit()  Sends CommandBuffers to the GPU.
```

---

## Minimal setup (wgpu 28)

```rust
use wgpu::util::DeviceExt;

async fn init(window: &Window) -> (wgpu::Device, wgpu::Queue, wgpu::Surface<'_>, wgpu::SurfaceConfiguration) {
    let instance = wgpu::Instance::default();

    let surface = instance.create_surface(window).unwrap();

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference:       wgpu::PowerPreference::HighPerformance,
            compatible_surface:     Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .expect("no suitable adapter");

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label:            Some("Primary Device"),
            required_features: wgpu::Features::empty(),
            required_limits:   wgpu::Limits::default(),
            memory_hints:      wgpu::MemoryHints::default(),
            trace:             wgpu::Trace::Off,
        })
        .await
        .expect("failed to create device");

    let size       = window.inner_size();
    let cap        = surface.get_capabilities(&adapter);
    let surf_fmt   = cap.formats.iter()
        .find(|f| f.is_srgb())
        .copied()
        .unwrap_or(cap.formats[0]);

    let config = wgpu::SurfaceConfiguration {
        usage:        wgpu::TextureUsages::RENDER_ATTACHMENT,
        format:       surf_fmt,
        width:        size.width,
        height:       size.height,
        present_mode: wgpu::PresentMode::AutoVsync,
        alpha_mode:   cap.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &config);

    (device, queue, surface, config)
}
```

---

## Cargo.toml

```toml
[dependencies]
wgpu     = "28"
bytemuck = { version = "1", features = ["derive"] }
winit    = "0.30"
pollster = "0.4"  # block_on for async init on native
```

---

## Coordinate system

wgpu uses the **D3D / Metal** coordinate convention:
- NDC: X in [-1, 1] left-to-right, Y in [-1, 1] bottom-to-top, Z in **[0, 1]** front-to-back.
- UV origin is top-left (0,0), unlike OpenGL which is bottom-left.
- Depth range is [0, 1] (not [-1, 1] like OpenGL).

---

## The frame render loop

```rust
fn render(
    surface: &wgpu::Surface,
    device:  &wgpu::Device,
    queue:   &wgpu::Queue,
    pipeline: &wgpu::RenderPipeline,
) {
    let output  = surface.get_current_texture().expect("lost surface");
    let view    = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Frame Encoder"),
    });

    {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Main Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view:           &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load:  wgpu::LoadOp::Clear(wgpu::Color { r: 0.05, g: 0.05, b: 0.1, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes:    None,
        });

        rpass.set_pipeline(pipeline);
        rpass.draw(0..3, 0..1);  // 3 vertices, 1 instance
    } // render pass dropped here; encoder borrows released

    queue.submit(std::iter::once(encoder.finish()));
    output.present();
}
```

---

## Critical rules

1. **`#[repr(C)]` + `Pod` + `Zeroable` on every GPU struct** — required for `bytemuck::cast_slice`.
2. **Labels on everything** — `label: Some("Vertex Buffer")` — invaluable in GPU debuggers (RenderDoc, Xcode).
3. **Group bind groups by change frequency** — group 0 = per-frame, group 1 = per-pass, group 2 = per-material.
4. **Do not create resources per frame** — buffers, textures, and pipelines are expensive. Create once, reuse.
5. **Limit `queue.submit()` calls** — aim for 1–5 per frame; there is significant CPU cost per submission.
6. **Use `queue.write_buffer` for small uploads** — not `map_async` + staging for simple cases.
7. **Drop render pass before calling `encoder.finish()`** — the borrow checker enforces this.
8. **Depth range is [0,1]** — not [-1,1]. Use a reversed-Z projection for better precision.
