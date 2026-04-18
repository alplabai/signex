# Best Practices

> Source: github.com/gfx-rs/wgpu/wiki/Do%27s-and-Dont%27s,
> github.com/gfx-rs/wgpu/wiki/Encapsulating-Graphics-Work

---

## Official Do's and Don'ts (from wgpu wiki)

### DON'T: create temporary mapped buffers when updating data

```rust
// BAD: manual staging buffer every frame
let staging = device.create_buffer(&wgpu::BufferDescriptor {
    usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
    mapped_at_creation: true,
    ..
});
// ... write data, copy, submit ...
// This creates allocation pressure every frame.

// GOOD: use queue.write_buffer() for small/medium data
queue.write_buffer(&my_buffer, 0, bytemuck::cast_slice(&data));
```

If you upload large amounts of **generated** data (not sitting in a `Vec`),
consider a reusable staging buffer pool.

---

### DO: group resource bindings by change frequency

```
Group 0 — per-frame: camera, time, global light
Group 1 — per-pass:  shadow map, environment
Group 2 — per-material: albedo, roughness, normal map
Group 3 — per-object: (prefer push constants instead)
```

Changing group N invalidates all groups >= N. Only rebind what changed.

---

### DON'T: create many resources per frame

```rust
// BAD: create a new buffer every frame
fn render(&self) {
    let vb = device.create_buffer_init(..);  // expensive!
    // ...
}

// GOOD: create once, reuse with write_buffer
struct Renderer {
    vertex_buffer: wgpu::Buffer,  // pre-allocated
}
fn render(&self) {
    queue.write_buffer(&self.vertex_buffer, 0, &data);  // fast
}
```

This applies to textures, bind groups, and pipelines too.

---

### DON'T: submit many times per frame

```rust
// BAD: one submission per object
for obj in &objects {
    let mut encoder = device.create_command_encoder(..);
    // ... encode obj ...
    queue.submit(std::iter::once(encoder.finish()));  // expensive!
}

// GOOD: batch everything into one submission
let mut encoder = device.create_command_encoder(..);
for obj in &objects {
    // ... encode all objects into the same encoder ...
}
queue.submit(std::iter::once(encoder.finish()));  // one call per frame
```

Target: **1–5 `submit()` calls per frame**. Multiple `CommandBuffer`s in one `submit` is fine.

---

## Middleware / encapsulation pattern (from wgpu wiki)

When building a reusable renderer or library, use the "middleware pattern":

```rust
pub struct TrackRenderer {
    pipeline: wgpu::RenderPipeline,
    instance_buffer: wgpu::Buffer,
    instance_count: u32,
}

impl TrackRenderer {
    /// Create all static resources (pipeline, bind groups, etc.)
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let pipeline = build_pipeline(device, surface_format);
        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label:              Some("Track Instance Buffer"),
            size:               MAX_TRACKS * INSTANCE_SIZE,
            usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { pipeline, instance_buffer, instance_count: 0 }
    }

    /// Update GPU data for this frame. Does NOT borrow &mut self exclusively in render().
    pub fn prepare(&mut self, queue: &wgpu::Queue, tracks: &[TrackInstance]) {
        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(tracks));
        self.instance_count = tracks.len() as u32;
    }

    /// Record draw commands into a caller-provided render pass.
    pub fn render<'rp>(&'rp self, rpass: &mut wgpu::RenderPass<'rp>) {
        rpass.set_pipeline(&self.pipeline);
        rpass.set_vertex_buffer(1, self.instance_buffer.slice(..));
        rpass.draw_indexed(0..6, 0, 0..self.instance_count);
    }
}
```

**Why `prepare` + `render` split?**
- `prepare` takes `&mut self` — serial, updates buffers.
- `render` takes `&self` — can be called from multiple places, or even in parallel
  into multiple command buffers.
- Accept `TextureFormat` not `SurfaceConfiguration` — callers may render to off-screen textures.

**Why accept `&mut RenderPass` not `&mut CommandEncoder`?**
- Renders to whatever target the caller provides.
- Minimises render pass count — tiled GPUs pay a significant cost per pass end.

---

## Label everything

```rust
// Every descriptor has a label field. Always fill it.
device.create_buffer(&wgpu::BufferDescriptor {
    label: Some("PCB Track Instance Buffer"),
    ..
});
device.create_texture(&wgpu::TextureDescriptor {
    label: Some("Board Copper Layer"),
    ..
});
device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
    label: Some("Track Render Pipeline"),
    ..
});
```

Labels appear in RenderDoc, Xcode GPU Frame Debugger, and wgpu validation error messages.

---

## Validation and error handling

```rust
// Enable wgpu validation (default in debug builds):
// Set RUST_LOG=wgpu_core=warn for warnings, RUST_LOG=wgpu_core=trace for verbose.

// Catch validation errors at runtime:
device.push_error_scope(wgpu::ErrorFilter::Validation);

// ... create resources or submit commands ...

let error = device.pop_error_scope().await;
if let Some(e) = error {
    eprintln!("wgpu validation: {}", e);
}

// Device lost callback:
device.on_uncaptured_error(Box::new(|err| {
    eprintln!("uncaptured wgpu error: {:?}", err);
}));
```

---

## Debugging tools

| Tool | Platform | Notes |
|------|----------|-------|
| RenderDoc | Win/Linux | Best GPU debugger; captures frames |
| Xcode GPU Frame Debugger | macOS | Native Metal debugging |
| PIX | Windows | DX12 profiling |
| `WGPU_BACKEND=vulkan` | All | Force a specific backend |
| `RUST_LOG=wgpu_core=warn` | All | Verbose validation messages |
| `wgpu::Trace::On(path)` | All | Record all API calls to replay |

---

## Performance checklist

| Item | Impact |
|------|--------|
| Group bind groups by frequency | High — avoids redundant state changes |
| Batch all draws into 1–5 submissions | High — `submit()` has CPU overhead |
| Pre-allocate buffers; use `write_buffer` | High — avoids allocation per frame |
| Use instanced drawing for repeated geometry | High — 100K+ objects in one call |
| Avoid `device.poll(Wait)` during rendering | High — stalls CPU waiting for GPU |
| Use `LoadOp::Clear` instead of a clear pass | Medium — tiled GPU optimisation |
| Add labels to all resources | Zero cost — helps debugging enormously |
| Use `PresentMode::AutoVsync` | App-dependent — reduces CPU spin |
| Prefer `Depth32Float` over `Depth24Plus` | Medium — consistent across backends |
| Use index buffers | Medium — reduces vertex duplication |

---

## Memory management

```rust
// wgpu resources are ref-counted (Arc internally).
// Dropping a resource schedules it for GPU-side destruction.
// Resources referenced by in-flight GPU work stay alive until the GPU is done.

// Explicitly free large resources early:
drop(old_texture);        // queues destruction
device.poll(wgpu::Maintain::Poll);  // advance destruction (non-blocking)

// For frequent resize (e.g., depth texture): drop old, create new.
self.depth_texture = create_depth_texture(&device, new_width, new_height);
// old depth texture is dropped and freed.
```

---

## Cross-platform gotchas

| Issue | Detail |
|-------|--------|
| **Coordinate Y-flip** | wgpu UV origin is top-left; OpenGL is bottom-left |
| **Depth range** | wgpu: [0,1]; OpenGL: [-1,1] |
| **sRGB surface** | Always prefer `is_srgb()` format; output is linear->sRGB automatic |
| **`Bgra8Unorm` on Windows** | DX12 default surface format is BGRA not RGBA |
| **WGSL array alignment** | `array<f32>` in uniform = 16 bytes per element (std140) |
| **Vulkan on macOS** | Requires MoltenVK; use Metal backend instead |
| **WebGL fallback** | `Limits::downlevel_webgl2_defaults()` for web targets |
| **`POLYGON_MODE_LINE`** | Requires feature flag; unavailable on WebGL |

---

## Texture sRGB usage guidance

```
Albedo / diffuse textures  -> Rgba8UnormSrgb  (GPU auto-converts on sample)
Normal maps                -> Rgba8Unorm      (linear data, NOT sRGB)
Roughness / metallic       -> Rgba8Unorm      (linear data)
HDR render targets         -> Rgba16Float
LUT textures               -> Rgba16Float or Rgba8Unorm
Swapchain / surface        -> Bgra8UnormSrgb (Windows) / Rgba8UnormSrgb (macOS/Linux)
```

If you write to an sRGB surface format, wgpu/the GPU driver automatically
converts your linear colour output to sRGB — no manual gamma correction needed.
If you write to a non-sRGB surface, you must apply gamma correction manually
in the fragment shader: `out_color = pow(linear_color, vec4(1.0/2.2))`.

---

## Shader compilation errors

```rust
// Check shader compilation:
let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
    label:  Some("My Shader"),
    source: wgpu::ShaderSource::Wgsl(source.into()),
});

// Get compilation info asynchronously:
let info = module.get_compilation_info().await;
for msg in &info.messages {
    match msg.message_type {
        wgpu::CompilationMessageType::Error   => eprintln!("Error:   {}", msg.message),
        wgpu::CompilationMessageType::Warning => eprintln!("Warning: {}", msg.message),
        wgpu::CompilationMessageType::Info    => println!("Info:    {}", msg.message),
    }
}
```
