// Backdrop reconstruction convention: independently repeat each authored axis.
@group(1) @binding(0) var art: texture_2d<f32>;
@group(1) @binding(1) var art_sampler: sampler;
@group(1) @binding(2) var<uniform> uv_transform: vec4<f32>;
@group(1) @binding(3) var<uniform> uv_rows: vec4<f32>;

@fragment
fn fragment(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let oriented = vec2<f32>(dot(uv_rows.xy, uv), dot(uv_rows.zw, uv));
    return textureSample(art, art_sampler, oriented * uv_transform.xy + uv_transform.zw);
}
