// Landscape splat: blends Terrain_Material's paint layers by their painted weights, then hands
// the result to Bevy's PBR lighting (shadows, fog, etc. as for any StandardMaterial).

import bevy_pbr::render::{
    pbr_functions::alpha_discard,
    pbr_fragment::pbr_input_from_standard_material,
};

@if(PREPASS_PIPELINE)
import bevy_pbr::{
    prepass::io::{VertexOutput, FragmentOutput},
    deferred::functions::deferred_output,
};
@else
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
};

struct TerrainParams {
    bounds: vec4<f32>,
    tiling: array<vec4<f32>, 3>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> params: TerrainParams;
@group(constants::MATERIAL_BIND_GROUP) @binding(101) var weights: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(102) var weights_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(103) var layers: texture_2d_array<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(104) var layers_sampler: sampler;

fn tiling(i: i32) -> f32 {
    return params.tiling[i / 4][i % 4];
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

    let world = in.world_position.xz;
    let wuv = (world - params.bounds.xy) / params.bounds.zw;

    // Explicit gradients: layer samples sit in weight-dependent branches (non-uniform flow).
    let ddx = dpdx(world);
    let ddy = dpdy(world);
    let w0 = textureSample(weights, weights_sampler, wuv, 0);
    let w1 = textureSample(weights, weights_sampler, wuv, 1);
    let w2 = textureSample(weights, weights_sampler, wuv, 2);
    var color = vec3<f32>(0.0);
    var total = 0.0;
    for (var t = 0; t < 3; t++) {
        var w = w0;
        if t == 1 { w = w1; } else if t == 2 { w = w2; }
        for (var c = 0; c < 4; c++) {
            let i = t * 4 + c;
            let weight = w[c];
            // Channel 11 is NoFoliage (a grass mask), not a paint layer.
            if weight > 0.003 && i < 11 {
                let k = tiling(i);
                let albedo = textureSampleGrad(layers, layers_sampler, world * k, i, ddx * k, ddy * k).rgb;
                color += albedo * weight;
                total += weight;
            }
        }
    }
    // Unpainted ground falls back to the grass layer.
    if total < 0.05 {
        let k = tiling(0);
        color += textureSampleGrad(layers, layers_sampler, world * k, 0, ddx * k, ddy * k).rgb * (0.05 - total) * 20.0;
        total = 1.0;
    }
    pbr_input.material.base_color = vec4<f32>(color / max(total, 1e-4), 1.0);
    pbr_input.material.base_color = alpha_discard(pbr_input.material.flags, pbr_input.material.alpha_cutoff, pbr_input.material.base_color);

@if(PREPASS_PIPELINE)
    let out = deferred_output(in, pbr_input);
@if(!PREPASS_PIPELINE)
    var out: FragmentOutput;
@if(!PREPASS_PIPELINE)
    out.color = apply_pbr_lighting(pbr_input);
@if(!PREPASS_PIPELINE)
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
