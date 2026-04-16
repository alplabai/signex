# wgpu Integration — GPU Shaders inside iced

> Source: iced/examples/integration, iced_wgpu, iced/widget/shader
> Feature flag: `iced = { features = ["wgpu"] }`

---

## When to use wgpu vs Canvas widget

| Scenario | Use |
|----------|-----|
| Schematics, <10K elements, CPU tessellation | `Canvas` widget |
| PCB boards, 100K+ tracks/pads/vias, GPU instancing | `wgpu` custom shader |
| Custom geometry with mesh primitives | `wgpu` Geometry / Mesh2D |

The key difference: the `Canvas` widget uses CPU path tessellation (fine for schematics).
PCB rendering requires GPU instanced draws for performance.

---

## Custom Shader widget

`iced::widget::shader` provides a `Shader` widget that runs a custom wgpu pipeline
inside the normal iced layout system.

```rust
use iced::widget::shader::{self, Shader};
use iced::{Element, Fill, Rectangle, Size};

/// State stored in the widget tree (not in app State).
#[derive(Default)]
pub struct ShaderState {
    pipeline: Option<GpuPipeline>,
}

/// The shader program — implements the draw logic.
pub struct BoardProgram<'a> {
    board: &'a BoardData,
    viewport: &'a Viewport,
    active_layers: &'a LayerSet,
}

impl<'a> shader::Program for BoardProgram<'a> {
    type State   = ShaderState;
    type Primitive = BoardPrimitive;

    fn draw(
        &self,
        state: &Self::State,
        cursor: iced::mouse::Cursor,
        bounds: Rectangle,
    ) -> Self::Primitive {
        // Build a CPU-side description of what to draw;
        // the primitive is then passed to prepare/render on the GPU thread.
        BoardPrimitive {
            board:    self.board.clone_lightweight(),
            viewport: *self.viewport,
            layers:   self.active_layers.clone(),
        }
    }
}

/// View function usage:
fn view_board(state: &BoardViewState) -> Element<'_, Message> {
    Shader::new(BoardProgram {
        board:         &state.board,
        viewport:      &state.viewport,
        active_layers: &state.visible_layers,
    })
    .width(Fill)
    .height(Fill)
    .into()
}
```

---

## Primitive — bridge between iced and wgpu

The `Primitive` trait is the data passed from the iced thread to the wgpu rendering thread.

```rust
use iced_wgpu::primitive::Primitive;
use iced::{Color, Rectangle, Size};

#[derive(Debug, Clone)]
pub struct BoardPrimitive {
    pub viewport:      Viewport,
    pub segments:      Vec<TrackSegment>,
    pub vias:          Vec<Via>,
    pub pads:          Vec<Pad>,
    pub active_layers: LayerSet,
}

impl Primitive for BoardPrimitive {
    fn prepare(
        &self,
        format:  wgpu::TextureFormat,
        device:  &wgpu::Device,
        queue:   &wgpu::Queue,
        bounds:  Rectangle,
        target_size: Size<u32>,
        scale_factor: f32,
        storage: &mut shader::Storage,
    ) {
        // Create or update GPU pipeline and upload vertex buffers
        if !storage.has::<BoardPipeline>() {
            storage.store(BoardPipeline::new(device, format));
        }
        let pipeline = storage.get_mut::<BoardPipeline>().unwrap();
        pipeline.update(device, queue, &self.segments, &self.vias, &self.pads, &self.viewport);
    }

    fn render(
        &self,
        encoder:  &mut wgpu::CommandEncoder,
        storage:  &shader::Storage,
        target:   &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
    ) {
        let pipeline = storage.get::<BoardPipeline>().unwrap();
        pipeline.render(encoder, target, clip_bounds, &self.active_layers);
    }
}
```

---

## GPU pipeline for instanced track rendering

PCB tracks are ideal for GPU instancing — same geometry, different transforms.

```rust
use wgpu::util::DeviceExt;

/// Per-instance data for one track segment.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TrackInstance {
    pub start:   [f32; 2],  // canvas pixels
    pub end:     [f32; 2],
    pub width:   f32,
    pub color:   [f32; 4],  // RGBA
    pub _pad:    [f32; 1],
}

pub struct BoardPipeline {
    pipeline:        wgpu::RenderPipeline,
    instance_buffer: wgpu::Buffer,
    instance_count:  u32,
    uniform_buffer:  wgpu::Buffer,
    bind_group:      wgpu::BindGroup,
}

impl BoardPipeline {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label:  Some("board_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("board.wgsl").into()),
        });

        // Vertex buffer layout for instanced data
        let instance_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TrackInstance>() as u64,
            step_mode:    wgpu::VertexStepMode::Instance,
            attributes:   &wgpu::vertex_attr_array![
                0 => Float32x2,  // start
                1 => Float32x2,  // end
                2 => Float32,    // width
                3 => Float32x4,  // color
            ],
        };

        // ... create bind group layout, pipeline layout, render pipeline ...
        todo!()
    }

    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue:  &wgpu::Queue,
        segments: &[TrackSegment],
        _vias:    &[Via],
        _pads:    &[Pad],
        viewport: &Viewport,
    ) {
        // Convert nm coordinates to canvas pixels
        let instances: Vec<TrackInstance> = segments.iter().map(|s| {
            let start = viewport.to_canvas(s.start.0, s.start.1);
            let end   = viewport.to_canvas(s.end.0,   s.end.1);
            TrackInstance {
                start:  [start.x, start.y],
                end:    [end.x,   end.y],
                width:  (s.width_nm as f64 * viewport.scale) as f32,
                color:  layer_color(s.layer).into(),
                _pad:   [0.0],
            }
        }).collect();

        // Write directly to buffer if same size; otherwise recreate
        self.instance_count = instances.len() as u32;
        queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
    }

    pub fn render(
        &self,
        encoder:    &mut wgpu::CommandEncoder,
        target:     &wgpu::TextureView,
        clip_bounds: &Rectangle<u32>,
        layers:     &LayerSet,
    ) {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label:                    Some("board_render_pass"),
            color_attachments:        &[Some(wgpu::RenderPassColorAttachment {
                view:           target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load:  wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });

        rpass.set_scissor_rect(
            clip_bounds.x, clip_bounds.y,
            clip_bounds.width, clip_bounds.height,
        );
        rpass.set_pipeline(&self.pipeline);
        rpass.set_bind_group(0, &self.bind_group, &[]);
        rpass.set_vertex_buffer(0, self.instance_buffer.slice(..));
        // Draw a quad (4 vertices) per track instance
        rpass.draw(0..4, 0..self.instance_count);
    }
}
```

---

## WGSL shader for round-capped tracks

```wgsl
// board.wgsl

struct Uniforms {
    viewport_size: vec2<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    // Per-vertex (quad: 4 vertices, indices 0–3)
    @builtin(vertex_index) vid: u32,
    // Per-instance
    @location(0) start:  vec2<f32>,
    @location(1) end:    vec2<f32>,
    @location(2) width:  f32,
    @location(3) color:  vec4<f32>,
};

struct VertexOutput {
    @builtin(position) pos:     vec4<f32>,
    @location(0)       uv:      vec2<f32>,
    @location(1)       color:   vec4<f32>,
    @location(2)       half_len: f32,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    let dir   = normalize(in.end - in.start);
    let perp  = vec2<f32>(-dir.y, dir.x);
    let half  = in.width * 0.5;
    let len   = length(in.end - in.start);

    // Quad corner offsets
    let corners = array<vec2<f32>, 4>(
        vec2(-half, -half),
        vec2( len + half, -half),
        vec2(-half,  half),
        vec2( len + half,  half),
    );
    let c = corners[in.vid];

    let world = in.start + dir * c.x + perp * c.y;
    let ndc   = world / uniforms.viewport_size * vec2(2.0, -2.0) + vec2(-1.0, 1.0);

    var out: VertexOutput;
    out.pos      = vec4(ndc, 0.0, 1.0);
    out.uv       = c;
    out.color    = in.color;
    out.half_len = len * 0.5;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Round caps: SDF distance from segment core
    let r     = in.uv.x;
    let half  = in.half_len;
    let dx    = clamp(r, 0.0, half) - r + max(0.0, r - half);
    let dy    = in.uv.y;
    let d     = sqrt(dx * dx + dy * dy);
    let alpha = 1.0 - smoothstep(in.color.a - 0.5, in.color.a + 0.5, d);
    return vec4(in.color.rgb, alpha);
}
```

---

## Mixing Canvas and wgpu in the same view

For EDA applications, Canvas handles schematics and wgpu handles PCB:

```rust
fn view_document(state: &AppState) -> Element<'_, Message> {
    match &state.active_document {
        Document::Schematic(sch) => {
            // Canvas widget — CPU tessellation, fine for <10K elements
            Canvas::new(SchematicProgram { sch, viewport: &state.viewport })
                .width(Fill).height(Fill).into()
        }
        Document::Board(board) => {
            // wgpu Shader — GPU instancing, needed for 100K+ elements
            Shader::new(BoardProgram { board, viewport: &state.viewport, .. })
                .width(Fill).height(Fill).into()
        }
    }
}
```
