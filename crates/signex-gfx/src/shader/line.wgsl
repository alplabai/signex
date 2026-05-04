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

struct LineInstance {
    @location(0) p0: vec2<f32>,
    @location(1) p1: vec2<f32>,
    @location(2) width: f32,
    @location(3) color: vec4<f32>,
    @location(4) style: u32,
};

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec2<f32>,
    @location(1) p0: vec2<f32>,
    @location(2) p1: vec2<f32>,
    @location(3) half_width: f32,
    @location(4) color: vec4<f32>,
};

fn sdf_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let denom = max(dot(ba, ba), 0.000001);
    let t = clamp(dot(pa, ba) / denom, 0.0, 1.0);
    return length(pa - ba * t);
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: LineInstance) -> VertexOut {
    let dir_raw = inst.p1 - inst.p0;
    let dir_len = max(length(dir_raw), 0.000001);
    let dir = dir_raw / dir_len;
    let normal = vec2<f32>(-dir.y, dir.x);
    let hw = inst.width * 0.5;
    let ext = dir * hw;

    let c0 = inst.p0 - ext + normal * hw;
    let c1 = inst.p0 - ext - normal * hw;
    let c2 = inst.p1 + ext + normal * hw;
    let c3 = inst.p1 + ext - normal * hw;

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
    out.p0 = inst.p0;
    out.p1 = inst.p1;
    out.half_width = hw;
    out.color = inst.color;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let edge_soft = camera.mm_per_px * 0.4;
    let d = sdf_segment(input.world_pos, input.p0, input.p1);
    let alpha = 1.0 - smoothstep(input.half_width - edge_soft, input.half_width + edge_soft, d);
    return vec4<f32>(input.color.rgb, input.color.a * alpha);
}
