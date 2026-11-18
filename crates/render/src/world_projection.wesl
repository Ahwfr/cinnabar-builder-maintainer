
// Projects shared world corners through one camera-relative path for identical clip coordinates.
// Integer block subtraction preserves precision far from the origin.

/// Returns local relative to camera, subtracting whole section blocks as integers first.
/// Shared corners remain identical across sections and precise far from the origin.
fn section_camera_offset(section_origin: vec3<i32>, local: vec3<f32>, camera: vec3<f32>) -> vec3<f32> {
    let camera_block = vec3<i32>(floor(camera));
    return vec3<f32>(section_origin - camera_block) + local - (camera - vec3<f32>(camera_block));
}

/// Clip position of a point `offset` from `camera`.
/// Declare position outputs invariant to preserve matching arithmetic across world passes.
/// Absolute positions pass `world - camera` so overlays align with camera-relative terrain.
/// Pass a matrix read from its binding. Startup crashes in an Adreno Vulkan shader compiler
/// coincided with forwarding a matrix parameter to another function, so avoid that pattern.
fn camera_offset_clip(clip_from_world: mat4x4<f32>, camera: vec3<f32>, offset: vec3<f32>) -> vec4<f32> {
    return clip_from_world * vec4(offset, 0.0) + clip_from_world * vec4(camera, 1.0);
}
/// Tint and opacity in `tint`; the tinted colour blends toward `wash.rgb` by `wash.a`.
struct BlockShading {
    tint: vec4<f32>,
    wash: vec4<f32>,
}

@if(MOD_BLOCK_TINT || MOD_BLOCK_OPACITY)
@group(2) @binding(0) var<uniform> block_shading: BlockShading;

@if(MOD_BLOCK_TINT)
fn washed_block_colour(colour: vec3<f32>) -> vec3<f32> {
    return mix(colour * block_shading.tint.rgb, block_shading.wash.rgb, block_shading.wash.a);
}

// Greedy terrain faces can span several full blocks. Keep a border for each cell.
@if(MOD_BLOCK_OUTLINE)
fn block_grid_edge(world_position: vec3<f32>) -> f32 {
    let fraction = fract(world_position);
    let edge = min(fraction, vec3(1.0) - fraction);
    return min(max(edge.x, edge.y), min(max(edge.y, edge.z), max(edge.z, edge.x)));
}

@if(MOD_BLOCK_OUTLINE)
fn line_distance(point: vec3<f32>, a: vec3<f32>, b: vec3<f32>) -> f32 {
    let segment = b - a;
    let length_squared = dot(segment, segment);
    if (length_squared < 1e-10) { return 1.0; }
    return length(cross(point - a, segment)) * inverseSqrt(length_squared);
}

// Interpolate distances to the four geometric edges separately. Taking the
// minimum in the fragment covers the whole perimeter without a triangle diagonal.
@if(MOD_BLOCK_OUTLINE)
fn quad_border(point: vec3<f32>, corners: array<vec3<f32>, 4>) -> vec4<f32> {
    return vec4(
        line_distance(point, corners[0], corners[1]),
        line_distance(point, corners[1], corners[2]),
        line_distance(point, corners[2], corners[3]),
        line_distance(point, corners[3], corners[0]),
    );
}

fn apply_shape_tint(colour: vec4<f32>, edges: vec4<f32>) -> vec4<f32> {
    @if(MOD_BLOCK_TINT && MOD_BLOCK_OUTLINE) {
        let distance_to_edge = min(min(edges.x, edges.y), min(edges.z, edges.w));
        let line = 1.0 - smoothstep(0.005, 0.011, distance_to_edge);
        return vec4(mix(washed_block_colour(colour.rgb), vec3(0.10, 0.67, 0.91), line), colour.a);
    }
    @if(MOD_BLOCK_TINT && !MOD_BLOCK_OUTLINE)
    return vec4(washed_block_colour(colour.rgb), colour.a);
    @if(!MOD_BLOCK_TINT)
    return colour;
}

fn apply_block_tint(colour: vec4<f32>, world_position: vec3<f32>) -> vec4<f32> {
    @if(MOD_BLOCK_OUTLINE)
    return apply_shape_tint(colour, vec4(block_grid_edge(world_position)));
    @if(!MOD_BLOCK_OUTLINE)
    return apply_shape_tint(colour, vec4(1.0));
}

fn apply_block_opacity(colour: vec4<f32>) -> vec4<f32> {
    @if(MOD_BLOCK_OPACITY)
    return vec4(colour.rgb, colour.a * block_shading.tint.a);
    @if(!MOD_BLOCK_OPACITY)
    return colour;
}
