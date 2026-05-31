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

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) ndc_pos: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VertexOut {
    var ndc = vec2<f32>(-1.0, -1.0);
    switch (vi) {
        case 0u: {
            ndc = vec2<f32>(-1.0, -1.0);
        }
        case 1u: {
            ndc = vec2<f32>(3.0, -1.0);
        }
        default: {
            ndc = vec2<f32>(-1.0, 3.0);
        }
    }

    var out: VertexOut;
    out.clip_pos = vec4<f32>(ndc, 0.0, 1.0);
    out.ndc_pos = ndc;
    return out;
}

fn axis_line_alpha(coord_mm: f32, spacing_mm: f32, half_width_mm: f32) -> f32 {
    let phase = fract(coord_mm / spacing_mm);
    let dist_to_line = min(phase, 1.0 - phase) * spacing_mm;
    let soft = max(half_width_mm * 0.75, camera.mm_per_px * 0.25);
    return 1.0 - smoothstep(half_width_mm - soft, half_width_mm + soft, dist_to_line);
}

fn grid_mask(world_mm: vec2<f32>, spacing_mm: f32, half_width_mm: f32) -> f32 {
    let axis_x = axis_line_alpha(world_mm.x, spacing_mm, half_width_mm);
    let axis_y = axis_line_alpha(world_mm.y, spacing_mm, half_width_mm);
    return max(axis_x, axis_y);
}

@fragment
fn fs_main(input: VertexOut) -> @location(0) vec4<f32> {
    let sx = camera.view_proj[0].x;
    let sy = camera.view_proj[1].y;
    let tx = camera.view_proj[3].x;
    let ty = camera.view_proj[3].y;
    let safe_sx = select(sx, 0.000001, abs(sx) < 0.000001);
    let safe_sy = select(sy, 0.000001, abs(sy) < 0.000001);

    let world_mm = vec2<f32>(
        (input.ndc_pos.x - tx) / safe_sx,
        (input.ndc_pos.y - ty) / safe_sy,
    );

    let mm_per_px = max(camera.mm_per_px, 0.000001);
    let px_per_mm = 1.0 / mm_per_px;

    let minor_spacing_mm = 2.54;
    let major_spacing_mm = minor_spacing_mm * 4.0;

    let minor_spacing_px = minor_spacing_mm * px_per_mm;
    let major_spacing_px = major_spacing_mm * px_per_mm;

    let minor_fade = smoothstep(4.0, 12.0, minor_spacing_px);
    let major_fade = smoothstep(2.0, 8.0, major_spacing_px);

    let minor_half_width_mm = mm_per_px * 0.35;
    let major_half_width_mm = mm_per_px * 0.65;

    let minor_alpha = grid_mask(world_mm, minor_spacing_mm, minor_half_width_mm) * minor_fade * 0.35;
    let major_alpha = grid_mask(world_mm, major_spacing_mm, major_half_width_mm) * major_fade * 0.65;
    let alpha = max(minor_alpha, major_alpha);

    let color = vec3<f32>(0.45, 0.52, 0.62);
    return vec4<f32>(color, alpha);
}