# Buffers

---

## The bytemuck requirement

Every struct sent to the GPU must be `#[repr(C)]` and implement `bytemuck::Pod` + `Zeroable`.

```rust
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Vertex {
    position: [f32; 3],
    color:    [f32; 3],
}

// Padding rules (matches WGSL std140/std430):
// - vec2<f32> -> 8-byte aligned
// - vec3<f32> -> 16-byte aligned (pad to 16)
// - vec4<f32> -> 16-byte aligned
// Always add explicit _padding fields rather than relying on #[repr(align)].

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],  // mat4 -- 64 bytes, 16-byte aligned
    time:       f32,            // 4 bytes
    _pad:      [f32; 3],        // explicit padding to reach 16-byte boundary
}
```

---

## Creating buffers

### `create_buffer_init` (data known at creation)

```rust
use wgpu::util::DeviceExt;

// Vertex buffer:
let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    label:    Some("Vertex Buffer"),
    contents: bytemuck::cast_slice(&vertices),
    usage:    wgpu::BufferUsages::VERTEX,
});

// Index buffer (u16 or u32):
let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    label:    Some("Index Buffer"),
    contents: bytemuck::cast_slice(&indices),  // &[u16] or &[u32]
    usage:    wgpu::BufferUsages::INDEX,
});

// Uniform buffer (frequently updated):
let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    label:    Some("Uniform Buffer"),
    contents: bytemuck::cast_slice(&[uniforms]),
    usage:    wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
});
```

### `create_buffer` (empty or updated later)

```rust
let dynamic_buffer = device.create_buffer(&wgpu::BufferDescriptor {
    label:              Some("Dynamic Vertex Buffer"),
    size:               (MAX_VERTICES * std::mem::size_of::<Vertex>()) as u64,
    usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    mapped_at_creation: false,
});
```

---

## BufferUsages flags

| Flag | Use |
|------|-----|
| `VERTEX` | Vertex buffer slot |
| `INDEX` | Index buffer slot |
| `UNIFORM` | Read-only uniform binding |
| `STORAGE` | Read/write storage binding |
| `COPY_SRC` | Source of a copy operation |
| `COPY_DST` | Destination of a copy / `write_buffer` target |
| `MAP_READ` | CPU readback (map_async for read) |
| `MAP_WRITE` | CPU write via mapping |
| `INDIRECT` | Arguments for indirect draw/dispatch |

Common combinations:
- Upload-once geometry: `VERTEX` only (no `COPY_DST` needed after `create_buffer_init`)
- Per-frame uniforms: `UNIFORM | COPY_DST`
- GPU-to-CPU readback: `COPY_DST | MAP_READ`
- Staging buffer: `MAP_WRITE | COPY_SRC`

---

## Uploading data to GPU

### `queue.write_buffer` — preferred for small/medium uploads

```rust
// Update uniform every frame:
let uniforms = Uniforms { /* ... */ };
queue.write_buffer(&uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

// Update a slice of an instance buffer:
queue.write_buffer(
    &instance_buffer,
    offset_bytes,
    bytemuck::cast_slice(&instances[start..end]),
);
```

wgpu internally uses a staging belt — no manual staging buffer needed.

### Staging buffer — for large batched uploads

```rust
// Create a staging buffer (CPU-writable):
let staging = device.create_buffer(&wgpu::BufferDescriptor {
    label:              Some("Staging Buffer"),
    size:               data.len() as u64,
    usage:              wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
    mapped_at_creation: true,
});

// Write directly to mapped range:
staging.slice(..).get_mapped_range_mut().copy_from_slice(data);
staging.unmap();

// Copy to the GPU-only destination:
let mut encoder = device.create_command_encoder(&Default::default());
encoder.copy_buffer_to_buffer(&staging, 0, &gpu_buffer, 0, data.len() as u64);
queue.submit(std::iter::once(encoder.finish()));
```

For high-throughput uploads, reuse a pool of staging buffers instead of creating new ones each frame.

---

## Reading data back from GPU

```rust
// Buffer must have MAP_READ | COPY_DST (or COPY_DST + a separate readback buffer).
let readback = device.create_buffer(&wgpu::BufferDescriptor {
    label:              Some("Readback Buffer"),
    size:               gpu_buffer_size,
    usage:              wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
    mapped_at_creation: false,
});

// Copy GPU buffer to readback:
let mut encoder = device.create_command_encoder(&Default::default());
encoder.copy_buffer_to_buffer(&gpu_buffer, 0, &readback, 0, gpu_buffer_size);
queue.submit(std::iter::once(encoder.finish()));

// Map asynchronously and read:
let slice = readback.slice(..);
let (tx, rx) = std::sync::mpsc::channel();
slice.map_async(wgpu::MapMode::Read, move |r| { tx.send(r).unwrap(); });
device.poll(wgpu::Maintain::Wait);  // block until GPU work completes
rx.recv().unwrap().unwrap();

let data: &[u8] = &slice.get_mapped_range();
let typed: &[f32] = bytemuck::cast_slice(data);
// ... use typed ...
drop(data);
readback.unmap();
```

---

## Vertex buffer layout

```rust
impl Vertex {
    // Store as a const to return a 'static reference:
    const ATTRIBS: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode:    wgpu::VertexStepMode::Vertex,
            attributes:   &Self::ATTRIBS,
        }
    }
}

// Instance buffer uses VertexStepMode::Instance:
impl Instance {
    const ATTRIBS: [wgpu::VertexAttribute; 4] =
        wgpu::vertex_attr_array![2 => Float32x4, 3 => Float32x4, 4 => Float32x4, 5 => Float32x4];

    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode:    wgpu::VertexStepMode::Instance, // changes once per instance
            attributes:   &Self::ATTRIBS,
        }
    }
}
```

### `vertex_attr_array!` format mapping

| wgpu format | WGSL type | Rust type |
|-------------|-----------|-----------|
| `Float32`   | `f32`     | `f32` |
| `Float32x2` | `vec2<f32>` | `[f32; 2]` |
| `Float32x3` | `vec3<f32>` | `[f32; 3]` |
| `Float32x4` | `vec4<f32>` | `[f32; 4]` |
| `Uint32`    | `u32`     | `u32` |
| `Sint32`    | `i32`     | `i32` |
| `Unorm8x4`  | `vec4<f32>` | `[u8; 4]` (normalised to 0..1) |

---

## Draw calls with buffers

```rust
// In render pass:
rpass.set_vertex_buffer(0, vertex_buffer.slice(..));
rpass.set_vertex_buffer(1, instance_buffer.slice(..));      // slot 1 for instances
rpass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);

rpass.draw_indexed(0..index_count, 0, 0..instance_count);
// draw_indexed(indices, base_vertex, instances)
```

---

## Large buffer coalescing (best practice)

Instead of one small buffer per object, allocate one large buffer and use sub-ranges:

```rust
const MAX_VERTICES: usize = 1_000_000;

let big_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
    label:              Some("Global Vertex Buffer"),
    size:               (MAX_VERTICES * std::mem::size_of::<Vertex>()) as u64,
    usage:              wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    mapped_at_creation: false,
});

// Upload mesh A at offset 0, mesh B at offset size_of_A, etc.
// Draw with a sub-slice:
let offset_a = 0u64;
let size_a   = mesh_a.vertex_count as u64 * std::mem::size_of::<Vertex>() as u64;
rpass.set_vertex_buffer(0, big_vertex_buffer.slice(offset_a..offset_a + size_a));
```
