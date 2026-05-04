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

struct TextInstance {
    @location(0) position: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) rotation: f32,
};

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: TextInstance) -> VertexOut {
    var local = vec2<f32>(0.5, 0.5);
    switch (vi) {
        case 0u: {
            local = vec2<f32>(-0.5, -0.5);
        }
        case 1u: {
            local = vec2<f32>(-0.5, 0.5);
        }
        case 2u: {
            local = vec2<f32>(0.5, -0.5);
        }
        case 3u: {
            local = vec2<f32>(-0.5, 0.5);
        }
        case 4u: {
            local = vec2<f32>(0.5, 0.5);
        }
        default: {
            local = vec2<f32>(0.5, -0.5);
        }
    }

    let scaled = local * inst.size;
    let c = cos(inst.rotation);
    let s = sin(inst.rotation);
    let rotated = vec2<f32>(
        scaled.x * c - scaled.y * s,
        scaled.x * s + scaled.y * c,
    );

    let world = inst.position + rotated;

    var out: VertexOut;
    out.clip_pos = camera.view_proj * vec4<f32>(world, 0.0, 1.0);
    out.color = inst.color;
    return out;
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    return input.color;
}
