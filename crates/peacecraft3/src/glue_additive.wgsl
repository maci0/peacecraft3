// HighlightAlphaMode "ADD": the texture is added to the frame under it. The
// blend state is set in `AdditiveImage::specialize`.
@group(1) @binding(0) var art: texture_2d<f32>;
@group(1) @binding(1) var art_sampler: sampler;

@group(1) @binding(2) var<uniform> uv_transform: vec4<f32>;

@fragment
fn fragment(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    return textureSample(art, art_sampler, uv * uv_transform.xy + uv_transform.zw);
}
