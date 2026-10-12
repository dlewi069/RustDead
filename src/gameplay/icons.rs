//! Inventory item pictures. The game has no icon textures: JigSInventory renders each item's
//! `StaticMeshToInspect` / `SkeletalMeshToInspect` at its `InspectTransform` rotation and `FOV`
//! (`UseAsInventoryImage?`). This does the same, lazily: an item's icon is baked into an image the
//! first time a grid shows it, up to [`SLOTS`] at once on their own render layers.

use std::collections::{HashMap, VecDeque};

use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::world_serialization::WorldInstanceReady;

use super::items::ItemDb;
use crate::models::model;

const SLOTS: usize = 8;
/// Pixels per inventory cell in the baked image.
const PX: u32 = 96;
/// Icon scenes sit far below the map so nothing else is near them.
const STAGE: Vec3 = Vec3::new(0.0, -5000.0, 0.0);

#[derive(Resource, Default)]
pub struct ItemIcons {
    images: HashMap<String, Handle<Image>>,
    queue: VecDeque<String>,
    active: [Option<Bake>; SLOTS],
}

struct Bake {
    model: Entity,
    camera: Entity,
    frames: u32,
    framed: bool,
    /// The inventory cell is at least as wide as tall.
    wide: bool,
}

impl ItemIcons {
    /// The icon for an item, starting its bake if needed. The image fills in once rendered.
    pub fn get(&mut self, item: &str, db: &ItemDb, images: &mut Assets<Image>) -> Option<Handle<Image>> {
        if let Some(h) = self.images.get(item) {
            return Some(h.clone());
        }
        let def = db.items.get(item)?;
        def.mesh.as_ref()?;
        let size = UVec2::new(def.size[0].max(1) as u32 * PX, def.size[1].max(1) as u32 * PX);
        let mut img = Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8Unorm, Some(TextureFormat::Rgba8UnormSrgb));
        img.asset_usage = RenderAssetUsages::RENDER_WORLD;
        let h = images.add(img);
        self.images.insert(item.to_string(), h.clone());
        self.queue.push_back(item.to_string());
        Some(h)
    }
}

#[derive(Component)]
struct IconModel(usize);

pub fn plugin(app: &mut App) {
    app.init_resource::<ItemIcons>().add_systems(Update, bake_icons).add_observer(layer_icon_model);
}

fn layer(slot: usize) -> RenderLayers {
    RenderLayers::layer(10 + slot)
}

/// Scene children don't inherit render layers; tag the whole spawned model.
fn layer_icon_model(ev: On<WorldInstanceReady>, models: Query<&IconModel>, children: Query<&Children>, mut commands: Commands) {
    let Ok(IconModel(slot)) = models.get(ev.entity) else { return };
    for e in children.iter_descendants(ev.entity) {
        commands.entity(e).insert(layer(*slot));
    }
}

#[allow(clippy::too_many_arguments)]
fn bake_icons(
    mut commands: Commands,
    mut icons: ResMut<ItemIcons>,
    db: Res<ItemDb>,
    assets: Res<AssetServer>,
    children: Query<&Children>,
    bounds: Query<(&Aabb, &GlobalTransform)>,
    mut cams: Query<(&mut Transform, &mut Projection)>,
    mut light: Local<Option<Entity>>,
) {
    if light.is_none() {
        // One light for every icon layer (lights only reach matching layers).
        let layers = (0..SLOTS).fold(RenderLayers::none(), |l, s| l.with(10 + s));
        *light = Some(
            commands
                .spawn((DirectionalLight { illuminance: 14000.0, ..default() }, Transform::from_xyz(1.0, 2.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y), layers))
                .id(),
        );
    }
    let icons = &mut *icons;
    for slot in 0..SLOTS {
        // Start a queued bake in a free slot.
        if icons.active[slot].is_none() {
            let Some(item) = icons.queue.pop_front() else { continue };
            let def = &db.items[&item];
            let Some(image) = icons.images.get(&item).cloned() else { continue };
            let origin = STAGE + Vec3::X * slot as f32 * 100.0;
            // InspectTransform rotations don't survive the axis conversion for every mesh (skeletal
            // weapons come out on end), so the camera is aimed from the bounds instead, below.
            let model = commands
                .spawn((model(&assets, format!("game/{}.glb", def.mesh.as_deref().unwrap_or_default())), Transform::from_translation(origin), IconModel(slot), layer(slot)))
                .id();
            let camera = commands
                .spawn((
                    Camera3d::default(),
                    Camera { order: -20 - slot as isize, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
                    RenderTarget::Image(image.into()),
                    Projection::Perspective(PerspectiveProjection { fov: def.fov.clamp(5.0, 60.0).to_radians(), ..default() }),
                    Transform::from_translation(origin + Vec3::Z * 5.0).looking_at(origin, Vec3::Y),
                    layer(slot),
                ))
                .id();
            icons.active[slot] = Some(Bake { model, camera, frames: 0, framed: false, wide: def.size[0] >= def.size[1] });
            continue;
        }
        let bake = icons.active[slot].as_mut().unwrap();
        bake.frames += 1;

        // Frame the camera on the model's bounds once its meshes exist.
        if !bake.framed {
            let (mut lo, mut hi) = (Vec3::MAX, Vec3::MIN);
            let stage = STAGE + Vec3::X * slot as f32 * 100.0;
            let mut settled = true;
            for e in children.iter_descendants(bake.model) {
                if let Ok((aabb, gt)) = bounds.get(e) {
                    // Transforms propagate a frame after spawn; until then children sit at the
                    // world origin and would blow the bounds up.
                    if gt.translation().distance(stage) > 50.0 {
                        settled = false;
                    }
                    let (c, h) = (Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
                    for corner in [Vec3::new(-1., -1., -1.), Vec3::new(1., 1., 1.), Vec3::new(-1., 1., 1.), Vec3::new(1., -1., -1.), Vec3::new(1., -1., 1.), Vec3::new(-1., 1., -1.), Vec3::new(1., 1., -1.), Vec3::new(-1., -1., 1.)] {
                        let p = gt.transform_point(c + h * corner);
                        lo = lo.min(p);
                        hi = hi.max(p);
                    }
                }
            }
            if lo.x <= hi.x && settled {
                let centre = (lo + hi) * 0.5;
                if let Ok((mut t, proj)) = cams.get_mut(bake.camera) {
                    let fov = match &*proj {
                        Projection::Perspective(p) => p.fov,
                        _ => 0.5,
                    };
                    // The bounding sphere overestimates most props; 0.8 fills the cell like the game does.
                    // Look down the item's thinnest axis, with its longest side along the cell's
                    // long side (how JigSInventory's pictures read: rifles lie flat, bottles stand).
                    let ext = hi - lo;
                    let axes = [Vec3::X, Vec3::Y, Vec3::Z];
                    let mut order = [0usize, 1, 2];
                    order.sort_by(|a, b| ext[*a].total_cmp(&ext[*b]));
                    let (view, mid, long) = (axes[order[0]], axes[order[1]], axes[order[2]]);
                    let up = if bake.wide { mid } else { long };
                    let face = (ext[order[1]].max(ext[order[2]])) * 0.5;
                    let dist = face / (fov * 0.5).tan() * 1.25 + ext[order[0]] * 0.5;
                    *t = Transform::from_translation(centre + view * dist).looking_at(centre, up);
                }
                bake.framed = true;
                bake.frames = 0;
            } else if bake.frames > 300 {
                bake.framed = true; // mesh never loaded; give up with an empty icon
            }
            continue;
        }
        // A few frames for materials/textures to land, then keep the image and clean up.
        if bake.frames > 30 {
            commands.entity(bake.model).despawn();
            commands.entity(bake.camera).despawn();
            icons.active[slot] = None;
        }
    }
}
