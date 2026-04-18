# Render Pipeline

---

## Shader module

```rust
// Inline WGSL:
let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
    label:  Some("Main Shader"),
    source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
});

// Or via macro (compile-time file inclusion):
let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
```

---

## Full render pipeline descriptor

```rust
let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
    label:  Some("Main Render Pipeline"),
    layout: Some(&pipeline_layout),   // or None for auto layout

    vertex: wgpu::VertexState {
        module:      &shader,
        entry_point: Some("vs_main"),
        buffers:     &[Vertex::desc(), Instance::desc()],
        compilation_options: wgpu::PipelineCompilationOptions::default(),
    },

    fragment: Some(wgpu::FragmentState {
        module:      &shader,
        entry_point: Some("fs_main"),
        targets: &[Some(wgpu::ColorTargetState {
            format:     surface_format,
            blend:      Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })],
        compilation_options: wgpu::PipelineCompilationOptions::default(),
    }),

    primitive: wgpu::PrimitiveState {
        topology:           wgpu::PrimitiveTopology::TriangleList,
        strip_index_format: None,
        front_face:         wgpu::FrontFace::Ccw,  // counter-clockwise = front
        cull_mode:          Some(wgpu::Face::Back),
        polygon_mode:       wgpu::PolygonMode::Fill,
        unclipped_depth:    false,
        conservative:       false,
    },

    depth_stencil: Some(wgpu::DepthStencilState {
        format:              wgpu::TextureFormat::Depth32Float,
        depth_write_enabled: true,
        depth_compare:       wgpu::CompareFunction::Less,
        stencil:             wgpu::StencilState::default(),
        bias:                wgpu::DepthBiasState::default(),
    }),

    multisample: wgpu::MultisampleState {
        count:                     1,      // 1 = no MSAA, 4 = 4x MSAA
        mask:                      !0,
        alpha_to_coverage_enabled: false,
    },

    multiview: None,
    cache:     None,
});
```

---

## Pipeline layout

```rust
let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
    label:                Some("Pipeline Layout"),
    bind_group_layouts:   &[&camera_bgl, &material_bgl],  // group 0, group 1
    push_constant_ranges: &[],
});
```

Using `layout: None` in the pipeline descriptor creates an auto-layout, but
explicitly declaring the layout allows you to share it across pipelines with
compatible bind group layouts.

---

## Blend modes

```rust
// Common presets:
wgpu::BlendState::REPLACE         // no blending (opaque)
wgpu::BlendState::ALPHA_BLENDING  // standard transparency: src.a * src + (1-src.a) * dst
wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING

// Custom blend:
wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::SrcAlpha,
        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
        operation:  wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent::OVER,
}
```

---

## Depth buffer

Create a dedicated depth texture:

```rust
fn create_depth_texture(
    device: &wgpu::Device,
    width: u32, height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label:           Some("Depth Texture"),
        size:            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count:    1,
        dimension:       wgpu::TextureDimension::D2,
        format:          wgpu::TextureFormat::Depth32Float,
        usage:           wgpu::TextureUsages::RENDER_ATTACHMENT
                       | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats:    &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

// Use in render pass:
depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
    view: &depth_view,
    depth_ops: Some(wgpu::Operations {
        load:  wgpu::LoadOp::Clear(1.0),  // 1.0 = far
        store: wgpu::StoreOp::Store,
    }),
    stencil_ops: None,
}),
```

Recreate the depth texture whenever the window is resized.

---

## Primitive topology

| Topology | Description |
|----------|-------------|
| `TriangleList` | Every 3 vertices form one triangle (default) |
| `TriangleStrip` | Adjacent triangles share an edge |
| `LineList` | Every 2 vertices form a line segment |
| `LineStrip` | Connected line segments |
| `PointList` | Each vertex is a point |

---

## Push constants (optional feature)

Fast small data updates — no bind group needed, but limited to 128–256 bytes:

```rust
// Request feature:
required_features: wgpu::Features::PUSH_CONSTANTS,
required_limits:   wgpu::Limits { max_push_constant_size: 128, ..Default::default() },

// Pipeline layout:
push_constant_ranges: &[wgpu::PushConstantRange {
    stages: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
    range:  0..64,
}],

// Encode:
rpass.set_push_constants(
    wgpu::ShaderStages::VERTEX,
    0,
    bytemuck::cast_slice(&[model_matrix]),
);
```

WGSL:
```wgsl
var<push_constant> pc: PushConstants;
```

---

## Wireframe rendering

```rust
// Requires Features::POLYGON_MODE_LINE:
polygon_mode: wgpu::PolygonMode::Line,
```

---

## Pipeline caching

```rust
// wgpu 22+ supports pipeline caches to speed up startup:
let cache = unsafe {
    device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
        label: Some("Pipeline Cache"),
        data:  None,  // or load from disk on subsequent launches
        fallback: true,
    })
};

let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
    cache: Some(&cache),
    ..
});

// Save cache data to disk:
let data = cache.get_data();
std::fs::write("pipeline_cache.bin", &data)?;
```
