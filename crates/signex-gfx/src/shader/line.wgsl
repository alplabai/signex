// CLEAN ROOM DECLARATION
// This shader was written without reference to GPL-licensed software.
// Sources: IPC-2612-1, IEEE 315, IEC 60617, wgpu/WGSL public docs.

struct Camera {
    view_proj: mat4x4<f32>,
    viewport: vec2<f32>,
    mm_per_px: f32,
    // Screen-space legibility floors, in pixels; 0.0 disables. Mirrors
    // `CameraUniform` — the two layouts must agree byte for byte.
    min_stroke_px: f32,
    min_radius_px: f32,
    // Three scalars, not a vec3: a `vec3<f32>` carries 16-byte alignment in
    // WGSL, which would push the struct to 112 bytes and mismatch the
    // 96-byte `CameraUniform`. wgpu rejects the bind group outright.
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
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
    @location(5) @interpolate(flat) style: u32,
};

// Low `style` bit selects the dash pattern — mirrors `LineSegment::STYLE_DASHED`
// (`crates/signex-gfx/src/primitive/line.rs`).
const STYLE_DASHED: u32 = 1u;

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
    // Legibility floor, matching what the CPU replays apply in screen space: a
    // stroke never renders thinner than `min_stroke_px` pixels, so hairlines
    // stay visible zoomed out instead of fading to sub-pixel coverage.
    // `mm_per_px` converts that pixel floor into the world units this instance
    // is expressed in. `0.0` leaves the width untouched.
    let min_width = camera.min_stroke_px * camera.mm_per_px;
    let hw = max(inst.width, min_width) * 0.5;
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
    out.style = inst.style;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let edge_soft = camera.mm_per_px * 0.4;
    let d = sdf_segment(input.world_pos, input.p0, input.p1);
    var alpha = 1.0 - smoothstep(input.half_width - edge_soft, input.half_width + edge_soft, d);

    if ((input.style & STYLE_DASHED) != 0u) {
        // Dash/gap length in world mm, derived from `camera.mm_per_px` so the
        // pattern reads the same physical pixel size at any zoom — matching
        // the CPU `pcb_canvas::draw_dashed_line` dash=8px / gap=5px constants.
        let dir = normalize(input.p1 - input.p0);
        let along_mm = dot(input.world_pos - input.p0, dir);
        let dash_mm = 8.0 * camera.mm_per_px;
        let gap_mm = 5.0 * camera.mm_per_px;
        let period_mm = dash_mm + gap_mm;
        let pos_in_period = along_mm - period_mm * floor(along_mm / period_mm);
        if (pos_in_period > dash_mm) {
            alpha = 0.0;
        }
    }

    return vec4<f32>(input.color.rgb, input.color.a * alpha);
}
