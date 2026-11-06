// The PostProcess volumes' ColorOffset (+ ColorOffsetShadows): added (blend One + One), already
// blended by the volumes' weights (`gameplay/post_volumes.rs`).

import bevy_ui_render::ui_vertex_output::UiVertexOutput;

@group(1) @binding(0)
var<uniform> params: vec4<f32>;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(params.rgb, 0.0);
}
