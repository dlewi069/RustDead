// The PostProcess volumes' film grain and vignette (`gameplay/post_volumes.rs`): the scene
// multiplied by 1 ± params.x (a fresh noise per pixel each frame) and by UE's vignette mask at
// intensity params.y (ComputeVignetteMask: cosine-fourth of the VignetteSpace position). The
// blend is Dst x rgb + Dst x alpha, so the multiplier (up to 2) is split between the two.

import bevy_render::globals::Globals;
import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@group(0) @binding(1)
var<uniform> globals: Globals;
@group(1) @binding(0)
var<uniform> params: vec4<f32>;

fn hash(p: vec3<u32>) -> f32 {
    var h = p.x * 1664525u + p.y * 1013904223u + p.z * 2654435761u;
    h = (h ^ (h >> 16u)) * 2246822519u;
    h = (h ^ (h >> 13u)) * 3266489917u;
    h = h ^ (h >> 16u);
    return f32(h) / 4294967295.0;
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let n = hash(vec3<u32>(vec2<u32>(in.position.xy), globals.frame_count));
    let grain = 1.0 + params.x * (n * 2.0 - 1.0);
    // VignetteSpace: screen position -1..1, y scaled by the aspect, then sqrt(2) / sqrt(1 + a²).
    let aspect = in.size.y / max(in.size.x, 1.0);
    let scale = sqrt(2.0) / sqrt(1.0 + aspect * aspect);
    let p = (in.uv * 2.0 - 1.0) * vec2<f32>(1.0, aspect) * scale * params.y;
    let cos4 = 1.0 / ((dot(p, p) + 1.0) * (dot(p, p) + 1.0));
    let m = grain * cos4;
    return vec4<f32>(vec3<f32>(m * 0.5), m * 0.5);
}
