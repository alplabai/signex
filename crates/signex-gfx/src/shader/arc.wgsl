// CLEAN ROOM DECLARATION
// This shader was written without reference to GPL-licensed software.
// Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

const TAU: f32 = 6.28318530718;

struct Camera {
    view_proj: mat4x4<f32>,
    viewport: vec2<f32>,
    mm_per_px: f32,
    _pad: f32,
}

@group(0) @binding(0)
var<uniform> camera: Camera;

struct ArcInstance {
    @location(0) center: vec2<f32>,
    @location(1) radius: f32,
    @location(2) start_angle: f32,
    @location(3) end_angle: f32,
    @location(4) width: f32,
    @location(5) color: vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_pos: vec2<f32>,
    @location(1) center: vec2<f32>,
    @location(2) radius: f32,
    @location(3) start_angle: f32,
    @location(4) end_angle: f32,
    @location(5) half_width: f32,
    @location(6) color: vec4<f32>,
};

fn normalize_angle(angle: f32) -> f32 {
    return angle - floor(angle / TAU) * TAU;
}

fn sdf_arc(
    p: vec2<f32>,
    center: vec2<f32>,
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    half_width: f32,
) -> f32 {
    let local = p - center;
    let radial = abs(length(local) - radius) - half_width;

    var a = normalize_angle(atan2(local.y, local.x) - start_angle);
    var sweep = normalize_angle(end_angle - start_angle);
    let in_sweep = a <= sweep;

    if (!in_sweep) {
        let p0 = center + vec2<f32>(cos(start_angle), sin(start_angle)) * radius;
        let p1 = center + vec2<f32>(cos(end_angle), sin(end_angle)) * radius;
        return min(length(p - p0), length(p - p1)) - half_width;
    }

    return radial;
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: ArcInstance) -> VertexOut {
    let half_extent = inst.radius + inst.width * 0.5;
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
    out.start_angle = inst.start_angle;
    out.end_angle = inst.end_angle;
    out.half_width = inst.width * 0.5;
    out.color = inst.color;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let edge_soft = camera.mm_per_px * 0.4;
    let d = sdf_arc(
        input.world_pos,
        input.center,
        input.radius,
        input.start_angle,
        input.end_angle,
        input.half_width,
    );

    let alpha = 1.0 - smoothstep(-edge_soft, edge_soft, d);
    return vec4<f32>(input.color.rgb, input.color.a * alpha);
}
