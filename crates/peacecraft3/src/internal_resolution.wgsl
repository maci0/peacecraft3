@group(1) @binding(0) var source: texture_2d<f32>;
@group(1) @binding(1) var<uniform> factor: u32;

// Render targets store display-encoded RGB; sampling them returns linear channels.
fn to_working(value: vec3<f32>) -> vec3<f32> {
    return select(value * 12.92, 1.055 * pow(max(value, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - vec3<f32>(0.055), value > vec3<f32>(0.0031308));
}

@fragment
fn fragment(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let size = textureDimensions(source);
    let output_size = size / factor;
    let pixel = min(vec2<u32>(uv * vec2<f32>(output_size)), output_size - vec2<u32>(1u));
    var sum = vec4<f32>(0.0);
    for (var y = 0u; y < factor; y += 1u) {
        for (var x = 0u; x < factor; x += 1u) {
            sum += textureLoad(source, vec2<i32>(pixel * factor + vec2<u32>(x,y)), 0);
        }
    }
    let averaged = sum / f32(factor * factor);
    return vec4<f32>(to_working(averaged.rgb), averaged.a);
}
