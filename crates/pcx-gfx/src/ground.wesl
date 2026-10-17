import bevy_pbr::render::mesh_functions;
import bevy_pbr::render::view_transformations::position_world_to_clip;

struct GroundUniform {
    layer_base: array<vec4<u32>, 4>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var ground_texture: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var ground_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var<uniform> ground: GroundUniform;

struct VertexIn {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(7) selectors: vec4<u32>,
}

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) selectors: vec4<u32>,
}

@vertex
fn vertex(vertex: VertexIn) -> VertexOut {
    var out: VertexOut;
    let world_from_local = mesh_functions::get_world_from_local(vertex.instance_index);
    let world_position = mesh_functions::mesh_position_local_to_world(
        world_from_local,
        vec4<f32>(vertex.position, 1.0),
    );
    out.position = position_world_to_clip(world_position.xyz);
    out.uv = vertex.uv;
    out.selectors = vertex.selectors;
    return out;
}

fn layer_lane(packed: vec4<u32>, lane: u32) -> u32 {
    switch lane {
        case 0u: { return packed.x; }
        case 1u: { return packed.y; }
        case 2u: { return packed.z; }
        default: { return packed.w; }
    }
}

fn layer_base_at(slot: u32) -> u32 {
    return layer_lane(ground.layer_base[slot / 4u], slot % 4u);
}

fn sample_selector(packed: u32, uv: vec2<f32>) -> vec4<f32> {
    let slot = packed & 31u;
    let empty = slot == 17u;
    let safe = select(slot, 0u, empty);
    let layer = layer_base_at(safe) + (packed >> 5u);
    let color = textureSample(ground_texture, ground_sampler, uv, layer);
    return select(color, vec4<f32>(0.0), empty);
}

fn over(above: vec4<f32>, below: vec4<f32>) -> vec4<f32> {
    return above * above.a + below * (1.0 - above.a);
}

@fragment
fn fragment(in: VertexOut) -> @location(0) vec4<f32> {
    let uv = vec2<f32>(in.uv.x, 1.0 - in.uv.y);
    var color = sample_selector(in.selectors.w, uv);
    color = over(color, sample_selector(in.selectors.z, uv));
    color = over(color, sample_selector(in.selectors.y, uv));
    color = over(color, sample_selector(in.selectors.x, uv));
    return vec4<f32>(color.rgb, 1.0);
}
