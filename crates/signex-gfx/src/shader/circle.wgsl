// CLEAN ROOM DECLARATION
// This shader was written without reference to GPL-licensed software.
// Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

struct Camera {
    view_proj: mat4x4<f32>,
    viewport: vec2<f32>,
    mm_per_px: f32,
    _pad: f32,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

struct CircleInstance {
    @location(0) center: vec2<f32>,
    @location(1) radius: f32,
    @location(2) stroke_width: f32,
    @location(3) color: vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec2<f32>,
    @location(1) center: vec2<f32>,
    @location(2) radius: f32,
    @location(3) half_stroke: f32,
    @location(4) color: vec4<f32>,
};

fn sdf_circle(p: vec2<f32>, center: vec2<f32>, radius: f32) -> f32 {
    return length(p - center) - radius;
}

fn sdf_ring(p: vec2<f32>, center: vec2<f32>, radius: f32, half_stroke: f32) -> f32 {
    return abs(length(p - center) - radius) - half_stroke;
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: CircleInstance) -> VertexOut {
    let half_extent = inst.radius + max(inst.stroke_width * 0.5, 0.0);

    let c0 = inst.center + vec2<f32>(-half_extent, -half_extent);
    let c1 = inst.center + vec2<f32>(-half_extent, half_extent);
    let c2 = inst.center + vec2<f32>(half_extent, -half_extent);
    let c3 = inst.center + vec2<f32>(half_extent, half_extent);

    var world = c2;
    switch (vi) {
        case 0u: {
            world = c0;
        }
        case 1u: {
            world = c1;
        }
        case 2u: {
            world = c2;
        }
        case 3u: {
            world = c1;
        }
        case 4u: {
            world = c3;
        }
        default: {
            world = c2;
        }
    }

    var out: VertexOut;
    out.clip_pos = camera.view_proj * vec4<f32>(world, 0.0, 1.0);
    out.world_pos = world;
    out.center = inst.center;
    out.radius = inst.radius;
    out.half_stroke = inst.stroke_width * 0.5;
    out.color = inst.color;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let edge_soft = camera.mm_per_px * 0.4;

    let ring_dist = sdf_ring(input.world_pos, input.center, input.radius, input.half_stroke);
    let fill_dist = sdf_circle(input.world_pos, input.center, input.radius);
    let use_fill = input.half_stroke < 0.001;
    let d = select(ring_dist, fill_dist, use_fill);

    let alpha = 1.0 - smoothstep(-edge_soft, edge_soft, d);
    return vec4<f32>(input.color.rgb, input.color.a * alpha);
}
