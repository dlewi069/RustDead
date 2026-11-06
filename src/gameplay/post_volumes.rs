//! The level's bounded PostProcess volumes the camera stands in, blended over the level's own
//! (PostProcessVolume_0: unbound, VignetteIntensity 0; it also sets ColorContrastShadows 0.85,
//! auto exposure and AO, not done here):
//!
//! - **Infestations** (BP_Infestation's PostProcess under its Sphere: PostProcessRadius, 5000 cm
//!   by default and up to 10000 on some primaries; half for a random nest; secondaries destroy
//!   both, as does a destroyed nest): ColorOffset (0.01, 0, 0), FilmGrainIntensity 0.25;
//!   BlendRadius 100 cm (the default).
//! - **Radiation zones** (BP_RadiationArea_Sphere's PostProcess under the Sphere, ZoneRadius):
//!   ColorSaturation 0.75, ColorOffset and ColorOffsetShadows (0, 0.002, 0), SceneFringeIntensity
//!   2.5 from ChromaticAberrationStartOffset 0.5, VignetteIntensity 0.25, FilmGrainIntensity 0.5;
//!   BlendRadius 250 cm.
//! - Each volume has weight 1 inside, falling to 0 over its BlendRadius outside; each in turn
//!   lerps the settings it overrides from what's blended so far by its weight.
//!
//! Ours: Bevy has no colour offset, film grain or UE's vignette, so two full-screen UI shader
//! nodes under the HUD do them after tonemapping (UE works before it, so ours lifts the shadows
//! more): one adds the offset (the shadows' offset added everywhere: 0.002 shows only in the
//! dark anyway), the other multiplies by a per-pixel noise grain (1 ± intensity, new each frame;
//! UE samples its grain texture) and UE's cosine-fourth vignette mask. Saturation is the camera's
//! ColorGrading; the fringe is Bevy's ChromaticAberration (red to blue smeared toward the centre
//! by intensity / 100 of the way; UE scales red and green toward the centre, and only past its
//! start offset). Zones blend before nests (equal Priority; UE's order is the world's).

use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor};
use bevy::shader::ShaderRef;
use bevy::ui_render::UiMaterialPlugin;
use bevy::ui_render::ui_material::{MaterialNode, UiMaterial, UiMaterialKey};

use super::infestations::{Infestations, Nest};
use super::radiation::Radiation;
use crate::menus::AppState;

/// What a volume overrides (None: left as blended so far).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Settings {
    pub saturation: Option<f32>,
    pub offset: Option<Vec3>,
    pub offset_shadows: Option<Vec3>,
    /// SceneFringeIntensity (percent).
    pub fringe: Option<f32>,
    pub vignette: Option<f32>,
    pub grain: Option<f32>,
}

/// BP_Infestation's PostProcess.
pub const INFESTATION: Settings = Settings { offset: Some(Vec3::new(0.01, 0.0, 0.0)), grain: Some(0.25), saturation: None, offset_shadows: None, fringe: None, vignette: None };
/// BP_RadiationArea_Sphere's PostProcess.
pub const RADIATION: Settings = Settings {
    saturation: Some(0.75),
    offset: Some(Vec3::new(0.0, 0.002, 0.0)),
    offset_shadows: Some(Vec3::new(0.0, 0.002, 0.0)),
    fringe: Some(2.5),
    vignette: Some(0.25),
    grain: Some(0.5),
};

/// The blended settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub saturation: f32,
    pub offset: Vec3,
    pub offset_shadows: Vec3,
    pub fringe: f32,
    pub vignette: f32,
    pub grain: f32,
}

impl Look {
    /// The level's PostProcessVolume_0 (vignette off) over UE's defaults.
    pub const LEVEL: Look = Look { saturation: 1.0, offset: Vec3::ZERO, offset_shadows: Vec3::ZERO, fringe: 0.0, vignette: 0.0, grain: 0.0 };

    /// One volume's turn: what it overrides lerped toward by `w`.
    pub fn blend(mut self, s: &Settings, w: f32) -> Self {
        if w <= 0.0 {
            return self;
        }
        let f = |v: &mut f32, to: Option<f32>| {
            if let Some(t) = to {
                *v += (t - *v) * w;
            }
        };
        let v3 = |v: &mut Vec3, to: Option<Vec3>| {
            if let Some(t) = to {
                *v = v.lerp(t, w);
            }
        };
        f(&mut self.saturation, s.saturation);
        v3(&mut self.offset, s.offset);
        v3(&mut self.offset_shadows, s.offset_shadows);
        f(&mut self.fringe, s.fringe);
        f(&mut self.vignette, s.vignette);
        f(&mut self.grain, s.grain);
        self
    }

    pub fn is_level(&self) -> bool {
        *self == Self::LEVEL
    }
}

/// A sphere volume's weight at `eye`: 1 inside, down to 0 at `blend` outside.
pub fn sphere_weight(centre: Vec3, radius: f32, blend: f32, eye: Vec3) -> f32 {
    let d = eye.distance(centre) - radius;
    if d <= 0.0 { 1.0 } else { (1.0 - d / blend).max(0.0) }
}

/// PostProcessComponent's default BlendRadius (100 cm): the nests'.
const NEST_BLEND: f32 = 1.0;

/// One nest's PostProcess weight at `eye`.
pub fn weight(n: &Nest, eye: Vec3) -> f32 {
    n.haze.filter(|_| !n.destroyed && !n.gone).map_or(0.0, |r| sphere_weight(n.transform.translation, r, NEST_BLEND, eye))
}

/// Every volume at `eye`, zones first.
pub fn look_at(rad: Option<&Radiation>, inf: Option<&Infestations>, eye: Vec3) -> Look {
    let mut look = Look::LEVEL;
    for z in rad.into_iter().flat_map(|r| &r.zones) {
        look = look.blend(&RADIATION, z.weight(eye));
    }
    for n in inf.into_iter().flat_map(|i| &i.nests) {
        look = look.blend(&INFESTATION, weight(n, eye));
    }
    look
}

/// Adds `params.rgb`: blend One + One.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
pub struct VolumeTint {
    #[uniform(0)]
    pub params: Vec4,
}

/// Multiplies by the grain (`params.x`) and the vignette mask (`params.y`): blend
/// Dst x (rgb + alpha).
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
pub struct VolumeGrain {
    #[uniform(0)]
    pub params: Vec4,
}

fn blend(descriptor: &mut RenderPipelineDescriptor, src: BlendFactor, dst: BlendFactor) {
    let Some(fragment) = descriptor.fragment.as_mut() else { return };
    for target in fragment.targets.iter_mut().flatten() {
        target.blend = Some(BlendState { color: BlendComponent { src_factor: src, dst_factor: dst, operation: BlendOperation::Add }, alpha: BlendComponent::OVER });
    }
}

impl UiMaterial for VolumeTint {
    fn fragment_shader() -> ShaderRef {
        "shaders/volume_tint.wesl".into()
    }

    fn specialize(descriptor: &mut RenderPipelineDescriptor, _key: UiMaterialKey<Self>) {
        blend(descriptor, BlendFactor::One, BlendFactor::One);
    }
}

impl UiMaterial for VolumeGrain {
    fn fragment_shader() -> ShaderRef {
        "shaders/volume_grain.wesl".into()
    }

    // The target clamps what the shader writes to 0-1, so a multiplier up to 2 is split
    // between colour and alpha: Dst x rgb + Dst x alpha.
    fn specialize(descriptor: &mut RenderPipelineDescriptor, _key: UiMaterialKey<Self>) {
        blend(descriptor, BlendFactor::Dst, BlendFactor::SrcAlpha);
    }
}

#[derive(Component)]
struct VolumeNode;

fn spawn_nodes(mut commands: Commands, mut tints: ResMut<Assets<VolumeTint>>, mut grains: ResMut<Assets<VolumeGrain>>) {
    let full = Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() };
    commands.spawn((
        full.clone(),
        MaterialNode(tints.add(VolumeTint::default())),
        // Under the HUD (the game's post process doesn't touch UMG).
        GlobalZIndex(i32::MIN),
        bevy::picking::Pickable::IGNORE,
        Visibility::Hidden,
        VolumeNode,
        DespawnOnExit(AppState::InGame),
        children![(full, MaterialNode(grains.add(VolumeGrain::default())), bevy::picking::Pickable::IGNORE)],
    ));
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn apply(
    mut commands: Commands,
    (rad, inf): (Option<Res<Radiation>>, Option<Res<Infestations>>),
    mut camera: Query<(Entity, &GlobalTransform, Option<&mut bevy::render::view::ColorGrading>, Option<&mut bevy::post_process::effect_stack::ChromaticAberration>), With<super::PlayerCamera>>,
    mut node: Query<(&mut Visibility, &MaterialNode<VolumeTint>, &Children), With<VolumeNode>>,
    grain_nodes: Query<&MaterialNode<VolumeGrain>>,
    mut tints: ResMut<Assets<VolumeTint>>,
    mut grains: ResMut<Assets<VolumeGrain>>,
) {
    let Ok((cam, eye, grading, fringe)) = camera.single_mut() else { return };
    let look = look_at(rad.as_deref(), inf.as_deref(), eye.translation());
    match grading {
        Some(mut g) => {
            if (g.global.post_saturation - look.saturation).abs() > 1e-4 {
                g.global.post_saturation = look.saturation;
            }
        }
        None => {
            let mut g = bevy::render::view::ColorGrading::default();
            g.global.post_saturation = look.saturation;
            commands.entity(cam).insert(g);
        }
    }
    let ca = look.fringe / 100.0;
    match fringe {
        Some(mut f) => {
            if (f.intensity - ca).abs() > 1e-6 {
                f.intensity = ca;
            }
        }
        None if ca > 0.0 => {
            commands.entity(cam).insert(bevy::post_process::effect_stack::ChromaticAberration { intensity: ca, ..default() });
        }
        None => {}
    }
    let Ok((mut vis, tint, kids)) = node.single_mut() else { return };
    let on = look.offset != Vec3::ZERO || look.offset_shadows != Vec3::ZERO || look.grain > 0.0 || look.vignette > 0.0;
    vis.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
    if !on {
        return;
    }
    let want = (look.offset + look.offset_shadows).extend(0.0);
    if tints.get(&tint.0).is_some_and(|t| t.params != want)
        && let Some(mut t) = tints.get_mut(&tint.0)
    {
        t.params = want;
    }
    let want = Vec4::new(look.grain, look.vignette, 0.0, 0.0);
    for g in kids.iter().filter_map(|k| grain_nodes.get(k).ok()) {
        if grains.get(&g.0).is_some_and(|m| m.params != want)
            && let Some(mut m) = grains.get_mut(&g.0)
        {
            m.params = want;
        }
    }
}

pub fn plugin(app: &mut App) {
    app.add_plugins((UiMaterialPlugin::<VolumeTint>::default(), UiMaterialPlugin::<VolumeGrain>::default()))
        .add_systems(OnEnter(AppState::InGame), spawn_nodes)
        .add_systems(Update, apply.run_if(in_state(AppState::InGame)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volumes_lerp_what_they_override_in_turn() {
        // Half inside a zone: everything it sets halfway.
        let z = Look::LEVEL.blend(&RADIATION, 0.5);
        assert!((z.saturation - 0.875).abs() < 1e-5 && (z.vignette - 0.125).abs() < 1e-5 && (z.grain - 0.25).abs() < 1e-5);
        // A nest on top at full weight: its grain wins, its red offset replaces the green, the
        // zone's saturation and vignette stay.
        let both = z.blend(&INFESTATION, 1.0);
        assert_eq!((both.grain, both.offset), (0.25, Vec3::new(0.01, 0.0, 0.0)));
        assert_eq!((both.saturation, both.vignette), (z.saturation, z.vignette));
        // Two half-weight nests: 0.75 of the way (1 - (1 - w)²).
        let two = Look::LEVEL.blend(&INFESTATION, 0.5).blend(&INFESTATION, 0.5);
        assert!((two.grain - 0.25 * 0.75).abs() < 1e-6);
        assert!(Look::LEVEL.blend(&RADIATION, 0.0).is_level());
        // 1 m blend for nests, 2.5 m for zones.
        assert_eq!(sphere_weight(Vec3::ZERO, 50.0, 1.0, Vec3::new(50.5, 0.0, 0.0)), 0.5);
        assert_eq!(sphere_weight(Vec3::ZERO, 50.0, 2.5, Vec3::new(60.0, 0.0, 0.0)), 0.0);
    }
}
