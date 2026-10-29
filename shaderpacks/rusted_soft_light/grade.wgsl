// Original Rusted colour grading, authored from the documented native post-stage API.
// Preserve alpha and HDR values; only the world's picture is graded, not the HUD.
override warmth: f32 = 0.04;
override saturation: f32 = 1.05;

fn post(input: PostInput) -> vec4<f32> {
    let source = input.color.rgb;
    let luminance = dot(source, vec3<f32>(0.2126, 0.7152, 0.0722));
    let colour = mix(vec3<f32>(luminance), source, saturation);
    let white_balance = vec3<f32>(1.0 + warmth, 1.0 + warmth * 0.16, 1.0 - warmth * 0.6);
    return vec4<f32>(max(colour * white_balance, vec3<f32>(0.0)), input.color.a);
}
