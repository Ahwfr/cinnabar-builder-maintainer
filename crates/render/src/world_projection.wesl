
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
@if(MOD_BLOCK_TINT || MOD_BLOCK_OPACITY)
@group(2) @binding(0) var<uniform> block_tint: vec4<f32>;

fn apply_block_tint(colour: vec4<f32>, world_position: vec3<f32>) -> vec4<f32> {
    @if(MOD_BLOCK_TINT && MOD_BLOCK_OUTLINE) {
        let fraction = fract(world_position);
        let edge = min(fraction, vec3(1.0) - fraction);
        // Two close boundaries form an edge. One close boundary is a face interior.
        let distance_to_edge = min(max(edge.x, edge.y), min(max(edge.y, edge.z), max(edge.z, edge.x)));
        let line = 1.0 - smoothstep(0.016, 0.035, distance_to_edge);
        let shaded = mix(colour.rgb * block_tint.rgb, vec3(0.10, 0.67, 0.91), line);
        return vec4(shaded, colour.a);
    }
    @if(MOD_BLOCK_TINT && !MOD_BLOCK_OUTLINE)
    return vec4(colour.rgb * block_tint.rgb, colour.a);
    @if(!MOD_BLOCK_TINT)
    return colour;
}

fn apply_block_opacity(colour: vec4<f32>) -> vec4<f32> {
    @if(MOD_BLOCK_OPACITY)
    return vec4(colour.rgb, colour.a * block_tint.a);
    @if(!MOD_BLOCK_OPACITY)
    return colour;
}
