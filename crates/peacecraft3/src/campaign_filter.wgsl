@group(1) @binding(0) var art: texture_2d<f32>;
@group(1) @binding(1) var art_sampler: sampler;
@group(1) @binding(2) var<uniform> uv_transform: vec4<f32>;
@group(1) @binding(3) var<uniform> tint: vec4<f32>;

// IEC sRGB transfer into the existing 3D stack. Alpha has no transfer curve.
fn to_linear(value: vec3<f32>) -> vec3<f32> {
    return select(value / 12.92, pow((value + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4)), value > vec3<f32>(0.04045));
}

@fragment
fn fragment(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let source = textureSample(art, art_sampler, uv * uv_transform.xy + uv_transform.zw) * tint;
    return vec4<f32>(to_linear(source.rgb), source.a);
}
