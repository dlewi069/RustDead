// M_Highlight_Inst's line (`gameplay/outline.rs`): params.rgb where a pixel outside the mask
// (alpha 0: nothing outlined there) has a masked one within params.w pixels.

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@group(1) @binding(0)
var<uniform> params: vec4<f32>;
@group(1) @binding(1)
var mask: texture_2d<f32>;
@group(1) @binding(2)
var mask_sampler: sampler;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(mask));
    let here = clamp(vec2<i32>(in.uv * vec2<f32>(size)), vec2<i32>(0), size - 1);
    if textureLoad(mask, here, 0).a > 0.5 {
        return vec4<f32>(0.0);
    }
    let w = i32(params.w);
    var near = 0.0;
    for (var y = -w; y <= w; y++) {
        for (var x = -w; x <= w; x++) {
            let p = clamp(here + vec2<i32>(x, y), vec2<i32>(0), size - 1);
            near = max(near, textureLoad(mask, p, 0).a);
        }
    }
    return vec4<f32>(params.rgb, step(0.5, near));
}
