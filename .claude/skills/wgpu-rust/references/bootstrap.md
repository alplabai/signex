# Bootstrap — Instance, Adapter, Device, Queue, Surface

---

## Instance

```rust
// Default: all backends enabled for the current platform.
let instance = wgpu::Instance::default();

// Explicit backend selection:
let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
    backends: wgpu::Backends::VULKAN | wgpu::Backends::METAL | wgpu::Backends::DX12,
    ..Default::default()
});

// List available adapters:
for adapter in instance.enumerate_adapters(wgpu::Backends::all()) {
    println!("{:?}", adapter.get_info());
}
```

---

## Adapter

```rust
// Async request (works on native + web):
let adapter = instance
    .request_adapter(&wgpu::RequestAdapterOptions {
        power_preference:       wgpu::PowerPreference::HighPerformance,
        compatible_surface:     Some(&surface), // None for headless
        force_fallback_adapter: false,
    })
    .await
    .expect("no suitable GPU adapter found");

// Query adapter info and limits:
let info = adapter.get_info();
println!("GPU: {} ({:?})", info.name, info.backend);

let limits = adapter.limits();
println!("max_bind_groups: {}", limits.max_bind_groups);
```

**`PowerPreference`**:
- `None` — let the driver decide
- `LowPower` — prefer integrated GPU (battery saving)
- `HighPerformance` — prefer discrete GPU

---

## Device and Queue

```rust
let (device, queue) = adapter
    .request_device(&wgpu::DeviceDescriptor {
        label:             Some("Main Device"),
        required_features: wgpu::Features::empty(),
        required_limits:   wgpu::Limits::default(),
        memory_hints:      wgpu::MemoryHints::default(),
        trace:             wgpu::Trace::Off,  // use Trace::On for API capture
    })
    .await
    .unwrap();
```

### Requesting optional features

```rust
// Check first, then request:
let available = adapter.features();
let needed = wgpu::Features::PUSH_CONSTANTS
    | wgpu::Features::POLYGON_MODE_LINE;

let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor {
    required_features: available & needed,  // only request what is available
    required_limits:   wgpu::Limits {
        max_push_constant_size: 128,
        ..wgpu::Limits::default()
    },
    ..Default::default()
}).await.unwrap();
```

### Error handling

```rust
// Push an error scope to catch validation errors:
device.push_error_scope(wgpu::ErrorFilter::Validation);
// ... create resources or encode commands ...
if let Some(err) = device.pop_error_scope().await {
    eprintln!("wgpu validation error: {}", err);
}
```

---

## Surface and SurfaceConfiguration

```rust
// Create surface from a winit Window (raw-window-handle 0.6):
let surface = instance.create_surface(window)?;

// Query what the surface supports:
let caps    = surface.get_capabilities(&adapter);
let formats = &caps.formats;

// Choose sRGB format if available (recommended for correct colour):
let surface_format = formats.iter()
    .find(|f| f.is_srgb())
    .copied()
    .unwrap_or(formats[0]);

let config = wgpu::SurfaceConfiguration {
    usage:        wgpu::TextureUsages::RENDER_ATTACHMENT,
    format:       surface_format,
    width:        window_width,
    height:       window_height,
    present_mode: wgpu::PresentMode::AutoVsync,
    alpha_mode:   caps.alpha_modes[0],
    view_formats: vec![],
    desired_maximum_frame_latency: 2,
};
surface.configure(&device, &config);
```

### Present modes

| Mode | Description |
|------|-------------|
| `AutoVsync` | Prefer vsync; falls back to no-vsync if unsupported |
| `AutoNoVsync` | No vsync; lowest latency |
| `Fifo` | Strict vsync; always supported |
| `Immediate` | No vsync; may tear |
| `Mailbox` | Triple-buffer; low latency + no tear |

### Resize handling

```rust
fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
    if new_size.width == 0 || new_size.height == 0 { return; }
    self.config.width  = new_size.width;
    self.config.height = new_size.height;
    self.surface.configure(&self.device, &self.config);
    // Also recreate depth texture here if you have one.
}
```

### Frame acquisition

```rust
let output = match surface.get_current_texture() {
    Ok(t)  => t,
    Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
        // Reconfigure and skip this frame:
        surface.configure(&device, &config);
        return;
    }
    Err(wgpu::SurfaceError::OutOfMemory) => panic!("out of GPU memory"),
    Err(e) => { eprintln!("surface error: {e:?}"); return; }
};
let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
// ... encode and submit ...
output.present();
```

---

## Headless (no window) setup

```rust
async fn headless_device() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::default();
    let adapter  = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None,
            ..Default::default()
        })
        .await.unwrap();
    adapter.request_device(&Default::default()).await.unwrap()
}
```

Used for compute-only workloads, screenshot generation, and offline rendering.

---

## Blocking init (native)

```rust
fn main() {
    // pollster::block_on wraps async init for native targets:
    let (device, queue) = pollster::block_on(headless_device());
}
```
