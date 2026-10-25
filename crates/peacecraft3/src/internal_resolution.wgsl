@group(1) @binding(0) var source: texture_2d<f32>;
@group(1) @binding(1) var<uniform> factor: u32;

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
    return sum / f32(factor * factor);
}
