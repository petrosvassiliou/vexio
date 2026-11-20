// Vexio Hardware-Accelerated Canvas Shader (WGSL)
// Transforms canonical Visio page-space coordinates (inches, +X right, +Y up)
// into clip space via CameraUniform, rendering procedural anti-aliased Visio
// page grids on page-sheet quads (flags.x < 0.5) and alpha-blended vector meshes.

struct CameraUniform {
    view_proj: mat4x4<f32>,
    viewport_px: vec2<f32>,
    zoom_ppi: f32,
    grid_spacing_in: f32,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) pos: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) flags: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) page_pos_in: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) flags: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_pos = camera.view_proj * vec4<f32>(in.pos.x, in.pos.y, 0.0, 1.0);
    out.page_pos_in = in.pos;
    out.color = in.color;
    out.flags = in.flags;
    return out;
}

fn compute_grid_tint(base_rgb: vec3<f32>, page_pos_in: vec2<f32>) -> vec3<f32> {
    let spacing = max(camera.grid_spacing_in, 0.0001);
    let minor_px = spacing * camera.zoom_ppi;
    let major_spacing = spacing * 4.0;
    let major_px = major_spacing * camera.zoom_ppi;

    var rgb = base_rgb;

    // Minor subdivisions fade smoothly when zoomed out below 6 px/cell to prevent Moire aliasing.
    if (camera.grid_spacing_in > 0.0 && minor_px >= 6.0) {
        let minor_fade = clamp((minor_px - 6.0) / 8.0, 0.0, 1.0);
        let minor_uv = page_pos_in / spacing;
        let minor_cell_dist = abs(fract(minor_uv - vec2<f32>(0.5, 0.5)) - vec2<f32>(0.5, 0.5));
        let dist_minor_px = min(minor_cell_dist.x, minor_cell_dist.y) * minor_px;
        let minor_alpha = (1.0 - smoothstep(0.0, 1.05, dist_minor_px)) * 0.35 * minor_fade;
        let minor_color = vec3<f32>(0.84, 0.88, 0.94);
        rgb = mix(rgb, minor_color, minor_alpha);
    }

    // Major inch/4x grid lines remain visible further out down to 8 px/major-cell.
    if (camera.grid_spacing_in > 0.0 && major_px >= 8.0) {
        let major_fade = clamp((major_px - 8.0) / 10.0, 0.0, 1.0);
        let major_uv = page_pos_in / major_spacing;
        let major_cell_dist = abs(fract(major_uv - vec2<f32>(0.5, 0.5)) - vec2<f32>(0.5, 0.5));
        let dist_major_px = min(major_cell_dist.x, major_cell_dist.y) * major_px;
        let major_alpha = (1.0 - smoothstep(0.0, 1.15, dist_major_px)) * 0.55 * major_fade;
        let major_color = vec3<f32>(0.74, 0.80, 0.89);
        rgb = mix(rgb, major_color, major_alpha);
    }

    return rgb;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // flags.x == 0.0 designates the Visio page sheet surface receiving procedural anti-aliased grid.
    if (abs(in.flags.x) < 0.5) {
        let gridded = compute_grid_tint(in.color.rgb, in.page_pos_in);
        return vec4<f32>(gridded, in.color.a);
    }
    return in.color;
}
