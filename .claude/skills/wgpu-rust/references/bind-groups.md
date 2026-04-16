# Bind Groups

---

## Concept

A **BindGroupLayout** declares what a shader expects at each binding slot.
A **BindGroup** binds actual GPU resources to those slots.
The two must be compatible — create the layout once, create BindGroups many times.

---

## Bind group layout

```rust
let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
    label:   Some("Camera BGL"),
    entries: &[
        // binding 0: uniform buffer, visible to vertex and fragment shaders
        wgpu::BindGroupLayoutEntry {
            binding:    0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty:                 wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size:   None,
            },
            count: None,
        },
    ],
});

let texture_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
    label:   Some("Texture BGL"),
    entries: &[
        // binding 0: texture
        wgpu::BindGroupLayoutEntry {
            binding:    0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                multisampled:   false,
                view_dimension: wgpu::TextureViewDimension::D2,
                sample_type:    wgpu::TextureSampleType::Float { filterable: true },
            },
            count: None,
        },
        // binding 1: sampler
        wgpu::BindGroupLayoutEntry {
            binding:    1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty:    wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
    ],
});
```

---

## Bind group

```rust
let camera_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
    label:  Some("Camera BG"),
    layout: &camera_bgl,
    entries: &[
        wgpu::BindGroupEntry {
            binding:  0,
            resource: camera_buffer.as_entire_binding(),
        },
    ],
});

let material_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
    label:  Some("Material BG"),
    layout: &texture_bgl,
    entries: &[
        wgpu::BindGroupEntry {
            binding:  0,
            resource: wgpu::BindingResource::TextureView(&diffuse_view),
        },
        wgpu::BindGroupEntry {
            binding:  1,
            resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
        },
    ],
});
```

---

## Binding in the render pass

```rust
rpass.set_bind_group(0, &camera_bg,   &[]);  // group 0
rpass.set_bind_group(1, &material_bg, &[]);  // group 1
```

The `&[]` is for dynamic offsets (see below).

---

## Frequency grouping — critical performance tip

**Group bind groups by how often they change.** Lower group numbers change less frequently:

| Group | Changes per... | Example |
|-------|---------------|---------|
| 0 | Frame | Camera, viewport, time, global lighting |
| 1 | Render pass | Shadow map, environment map |
| 2 | Material / draw call | Albedo texture, PBR params |
| 3 | Per-object | Rarely used; prefer push constants instead |

Changing group N invalidates all groups >= N. Setting group 0 once per frame and group 2 once per draw call is efficient; setting group 0 per draw call is not.

---

## Dynamic offsets

Allows reusing one bind group to address different regions of a buffer:

```rust
// Buffer must be UNIFORM | COPY_DST, with extra alignment:
let alignment = device.limits().min_uniform_buffer_offset_alignment as u64;
// Each object's uniform data must start at an aligned offset.

// Bind group with has_dynamic_offset: true:
wgpu::BindingType::Buffer {
    ty:                 wgpu::BufferBindingType::Uniform,
    has_dynamic_offset: true,
    min_binding_size:   NonZeroU64::new(std::mem::size_of::<ObjectUniforms>() as u64),
},

// At draw time, pass the offset:
rpass.set_bind_group(2, &per_object_bg, &[object_index * aligned_stride as u32]);
rpass.draw_indexed(0..index_count, 0, 0..1);
```

---

## Storage buffer binding

```rust
// Read-only storage (shader reads, CPU writes via COPY_DST):
wgpu::BindingType::Buffer {
    ty:                 wgpu::BufferBindingType::Storage { read_only: true },
    has_dynamic_offset: false,
    min_binding_size:   None,
},

// Read-write storage (compute shader reads and writes):
wgpu::BindingType::Buffer {
    ty:                 wgpu::BufferBindingType::Storage { read_only: false },
    has_dynamic_offset: false,
    min_binding_size:   None,
},
```

---

## Binding type summary

| `BindingType` | WGSL declaration |
|---------------|-----------------|
| `Buffer::Uniform` | `var<uniform> u: Uniforms;` |
| `Buffer::Storage { read_only: true }` | `var<storage, read> buf: array<f32>;` |
| `Buffer::Storage { read_only: false }` | `var<storage, read_write> buf: array<f32>;` |
| `Texture` (sampled) | `var t: texture_2d<f32>;` |
| `Sampler(Filtering)` | `var s: sampler;` |
| `Sampler(NonFiltering)` | `var s: sampler;` |
| `Sampler(Comparison)` | `var s: sampler_comparison;` |
| `StorageTexture` | `var t: texture_storage_2d<rgba8unorm, write>;` |
