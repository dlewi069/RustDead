use std::f32::consts::PI;
use std::path::Path;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use bevy::render::mesh::VertexAttributeValues;
use surroundead::obj_loader::load_obj_mesh;
use surroundead::world::map::WorldMap;

#[derive(Component)]
struct Player {
    speed: f32,
    vertical_velocity: f32,
    is_grounded: bool,
}

#[derive(Component)]
struct CameraController {
    distance: f32,
    pitch: f32,
    yaw: f32,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            distance: 14.0,
            pitch: 38.0f32.to_radians(),
            yaw: -30.0f32.to_radians(),
        }
    }
}

#[derive(Component)]
struct Zombie {
    base_yaw: f32,
    detection_range: f32,
}

#[derive(Component)]
struct CompassHeadingText;

#[derive(Component)]
struct VitalsText;

fn main() {
    println!("==================================================");
    println!("        SurrounDead (Rust Bevy 3D Port)           ");
    println!("==================================================");
    println!("Launching Longdown Valley world with authentic client assets...");

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "SurrounDead - v0.7.1g (Rust Edition)".into(),
                        resolution: (1280u32, 720u32).into(),
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    file_path: if Path::new("assets").exists() {
                        "assets".to_string()
                    } else if Path::new("../../assets").exists() {
                        "../../assets".to_string()
                    } else {
                        "assets".to_string()
                    },
                    ..default()
                }),
        )
        .add_systems(Startup, setup_world)
        .add_systems(
            Update,
            (
                player_movement,
                camera_controls,
                zombie_behavior,
                update_hud,
            ),
        )
        .run();
}

fn setup_world(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let world_map = WorldMap::generate_default_valley();

    // Helper to load mesh or fallback
    let mut load_mesh = |path: &str| -> Handle<Mesh> {
        let p = Path::new(path);
        if let Ok(m) = load_obj_mesh(p) {
            meshes.add(m)
        } else {
            meshes.add(Cuboid::new(1.0, 1.0, 1.0))
        }
    };

    // Preload Authentic Meshes
    let player_mesh = load_mesh("assets/models/SM_Char_Player_Male.obj");
    let zombie_walker_mesh = load_mesh("assets/models/SM_Zombie_Walker.obj");
    let zombie_runner_mesh = load_mesh("assets/models/SM_Zombie_Runner.obj");
    let sedan_mesh = load_mesh("assets/models/SM_Veh_Sedan_01.obj");
    let pickup_mesh = load_mesh("assets/models/SM_Veh_Pickup_01.obj");
    let crate_mesh = load_mesh("assets/models/SM_Prop_Crate_Military.obj");
    let safe_mesh = load_mesh("assets/models/SM_Prop_Safe_01.obj");
    let pine_mesh = load_mesh("assets/models/SM_Env_PineTree_01.obj");
    let pine_large_mesh = load_mesh("assets/models/SM_Env_PineTree_02.obj");
    let boulder_mesh = load_mesh("assets/models/SM_Env_Boulder_01.obj");
    let barrier_mesh = load_mesh("assets/models/SM_Prop_Barrier_01.obj");
    let dumpster_mesh = load_mesh("assets/models/SM_Prop_Dumpster_01.obj");
    let gas_pump_mesh = load_mesh("assets/models/SM_Prop_GasPump_01.obj");
    let road_piece_mesh = load_mesh("assets/models/SM_Env_Road_01.obj");

    // Preload Authentic Textures & Materials directly from SurrounDead PAK
    let apocalypse_tex: Handle<Image> = asset_server.load("textures/apocalypse.png");
    let character_tex: Handle<Image> = asset_server.load("textures/character.png");
    let road_tex: Handle<Image> = asset_server.load("textures/authentic_road.png");
    let ground_tex: Handle<Image> = asset_server.load("textures/ground.png");
    let dirt_tex: Handle<Image> = asset_server.load("textures/dirt.png");
    let vehicle_tex: Handle<Image> = asset_server.load("textures/vehicle.png");
    let rock_tex: Handle<Image> = asset_server.load("textures/rock.png");
    let foliage_tex: Handle<Image> = asset_server.load("textures/foliage.png");

    let terrain_material = materials.add(StandardMaterial {
        base_color_texture: Some(ground_tex),
        perceptual_roughness: 0.95,
        ..default()
    });

    let road_material = materials.add(StandardMaterial {
        base_color_texture: Some(road_tex),
        perceptual_roughness: 0.85,
        ..default()
    });

    let dirt_material = materials.add(StandardMaterial {
        base_color_texture: Some(dirt_tex),
        perceptual_roughness: 0.95,
        ..default()
    });

    let apocalypse_material = materials.add(StandardMaterial {
        base_color_texture: Some(apocalypse_tex),
        perceptual_roughness: 0.70,
        ..default()
    });

    let character_material = materials.add(StandardMaterial {
        base_color_texture: Some(character_tex),
        perceptual_roughness: 0.75,
        ..default()
    });

    let vehicle_material = materials.add(StandardMaterial {
        base_color_texture: Some(vehicle_tex),
        perceptual_roughness: 0.45,
        metallic: 0.25,
        ..default()
    });

    let rock_material = materials.add(StandardMaterial {
        base_color_texture: Some(rock_tex),
        perceptual_roughness: 0.90,
        ..default()
    });

    let foliage_material = materials.add(StandardMaterial {
        base_color_texture: Some(foliage_tex),
        perceptual_roughness: 0.80,
        ..default()
    });

    // 1. Base Valley Terrain (with tiled UVs so texture is crisp, not stretched)
    let mut ground_mesh = Plane3d::default().mesh().size(600.0, 600.0).build();
    if let Some(VertexAttributeValues::Float32x2(uvs)) = ground_mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0) {
        for uv in uvs.iter_mut() {
            uv[0] *= 75.0; // Tile 75x75 across 600m
            uv[1] *= 75.0;
        }
    }
    commands.spawn((
        Mesh3d(meshes.add(ground_mesh)),
        MeshMaterial3d(terrain_material),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // 2. Asphalt Road Network (Longdown Valley Route 9 Highway)
    // Main North-South Highway (Z: -280 to +280, width: 9.0m)
    let mut main_road_mesh = Plane3d::default().mesh().size(9.0, 560.0).build();
    if let Some(VertexAttributeValues::Float32x2(uvs)) = main_road_mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0) {
        for uv in uvs.iter_mut() {
            uv[1] *= 70.0; // Tile road markings along length
        }
    }
    commands.spawn((
        Mesh3d(meshes.add(main_road_mesh)),
        MeshMaterial3d(road_material.clone()),
        Transform::from_xyz(0.0, 0.01, 0.0),
    ));

    // Authentic modular damaged road pieces from SurrounDead PAK placed along the route
    for z in (-120..=120).step_by(12) {
        let z_f = z as f32;
        commands.spawn((
            Mesh3d(road_piece_mesh.clone()),
            MeshMaterial3d(road_material.clone()),
            Transform::from_xyz(0.0, 0.015, z_f),
        ));
    }

    // Road Shoulders (Left & Right dirt verges)
    let mut shoulder_mesh = Plane3d::default().mesh().size(2.5, 560.0).build();
    if let Some(VertexAttributeValues::Float32x2(uvs)) = shoulder_mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0) {
        for uv in uvs.iter_mut() {
            uv[1] *= 50.0;
        }
    }
    commands.spawn((
        Mesh3d(meshes.add(shoulder_mesh.clone())),
        MeshMaterial3d(dirt_material.clone()),
        Transform::from_xyz(-5.75, 0.015, 0.0),
    ));
    commands.spawn((
        Mesh3d(meshes.add(shoulder_mesh)),
        MeshMaterial3d(dirt_material),
        Transform::from_xyz(5.75, 0.015, 0.0),
    ));

    // East-West Crossroad to Checkpoint & Outpost (X: -80 to +80, width: 7.5m at Z = 0)
    let mut cross_road_mesh = Plane3d::default().mesh().size(160.0, 7.5).build();
    if let Some(VertexAttributeValues::Float32x2(uvs)) = cross_road_mesh.attribute_mut(Mesh::ATTRIBUTE_UV_0) {
        for uv in uvs.iter_mut() {
            uv[0] *= 20.0;
        }
    }
    commands.spawn((
        Mesh3d(meshes.add(cross_road_mesh)),
        MeshMaterial3d(road_material),
        Transform::from_xyz(0.0, 0.025, 0.0),
    ));

    // 3. Concrete Jersey Barriers along road curves and checkpoint
    for z in [-40.0, -36.0, -32.0, 48.0, 52.0, 56.0] {
        commands.spawn((
            Mesh3d(barrier_mesh.clone()),
            MeshMaterial3d(apocalypse_material.clone()),
            Transform::from_xyz(-4.8, 0.0, z).with_rotation(Quat::from_rotation_y(0.0)),
        ));
    }
    // Checkpoint barrier chicane across the road at Z = 50.0
    commands.spawn((
        Mesh3d(barrier_mesh.clone()),
        MeshMaterial3d(apocalypse_material.clone()),
        Transform::from_xyz(-1.8, 0.0, 50.0).with_rotation(Quat::from_rotation_y(PI / 2.0)),
    ));
    commands.spawn((
        Mesh3d(barrier_mesh.clone()),
        MeshMaterial3d(apocalypse_material.clone()),
        Transform::from_xyz(2.4, 0.0, 54.0).with_rotation(Quat::from_rotation_y(PI / 2.0)),
    ));

    // 4. Pine Forests (Clustered authentic trees lining valley edges and groves)
    let tree_coords = [
        // West Forest Ridge
        (-18.0, -60.0, 1.1, false), (-26.0, -45.0, 1.3, true), (-35.0, -70.0, 1.4, false),
        (-22.0, -20.0, 1.0, true), (-32.0, -10.0, 1.2, false), (-45.0, -30.0, 1.5, true),
        (-19.0,  15.0, 0.9, false), (-28.0,  30.0, 1.3, true), (-40.0,  20.0, 1.4, false),
        (-24.0,  65.0, 1.1, true), (-38.0,  80.0, 1.2, false), (-50.0,  60.0, 1.5, true),
        // East Forest Ridge
        ( 18.0, -55.0, 1.2, true), ( 28.0, -40.0, 1.0, false), ( 42.0, -65.0, 1.4, true),
        ( 22.0, -15.0, 1.1, false), ( 34.0,  -5.0, 1.3, true), ( 48.0, -25.0, 1.5, false),
        ( 17.0,  25.0, 1.0, true), ( 29.0,  40.0, 1.2, false), ( 44.0,  35.0, 1.4, true),
        ( 21.0,  70.0, 1.3, false), ( 36.0,  85.0, 1.1, true), ( 52.0,  75.0, 1.6, false),
        // Deep Valley Groves
        (-60.0, -100.0, 1.3, true), (60.0, -110.0, 1.4, false), (-70.0, 100.0, 1.2, true), (70.0, 110.0, 1.5, false),
    ];
    for (i, &(x, z, scale, is_large)) in tree_coords.iter().enumerate() {
        let rot = (i as f32 * 0.77).sin() * PI;
        let mesh = if is_large { pine_large_mesh.clone() } else { pine_mesh.clone() };
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(foliage_material.clone()),
            Transform::from_xyz(x, 0.0, z)
                .with_scale(Vec3::splat(scale))
                .with_rotation(Quat::from_rotation_y(rot)),
        ));
    }

    // 5. Mountain Boulders & Rock Outcrops
    let boulder_coords = [
        (-14.0, -35.0, 1.3), (15.0, -48.0, 1.1),
        (-22.0,   5.0, 1.6), (18.0,  12.0, 1.2),
        (-16.0,  85.0, 1.4), (20.0,  92.0, 1.5),
    ];
    for &(x, z, scale) in &boulder_coords {
        commands.spawn((
            Mesh3d(boulder_mesh.clone()),
            MeshMaterial3d(rock_material.clone()),
            Transform::from_xyz(x, 0.0, z).with_scale(Vec3::splat(scale)),
        ));
    }

    // 6. Interactive Player Entity (Authentic SurrounDead Survivor Model)
    commands.spawn((
        Player {
            speed: 7.5,
            vertical_velocity: 0.0,
            is_grounded: true,
        },
        Mesh3d(player_mesh),
        MeshMaterial3d(character_material.clone()),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // 7. Authentic POIs from WorldMap
    // Checkpoint Alpha (Z = 50.0): Supply Crates, Weapons Safe, Military Pickup
    commands.spawn((
        Mesh3d(pickup_mesh.clone()),
        MeshMaterial3d(vehicle_material.clone()),
        Transform::from_xyz(4.5, 0.0, 48.0).with_rotation(Quat::from_rotation_y(-0.25)),
    ));
    commands.spawn((
        Mesh3d(crate_mesh.clone()),
        MeshMaterial3d(apocalypse_material.clone()),
        Transform::from_xyz(-2.8, 0.0, 47.0),
    ));
    commands.spawn((
        Mesh3d(crate_mesh.clone()),
        MeshMaterial3d(apocalypse_material.clone()),
        Transform::from_xyz(-2.8, 0.76, 47.0), // Stacked crate!
    ));
    commands.spawn((
        Mesh3d(safe_mesh.clone()),
        MeshMaterial3d(apocalypse_material.clone()),
        Transform::from_xyz(-4.2, 0.0, 49.0).with_rotation(Quat::from_rotation_y(PI / 4.0)),
    ));

    // Abandoned Roadside Station (X = 25.0, Z = -20.0): Civilian Sedan, Gas Pumps, Dumpster & Crates
    commands.spawn((
        Mesh3d(sedan_mesh.clone()),
        MeshMaterial3d(vehicle_material.clone()),
        Transform::from_xyz(22.0, 0.0, -18.0).with_rotation(Quat::from_rotation_y(0.85)),
    ));
    commands.spawn((
        Mesh3d(gas_pump_mesh.clone()),
        MeshMaterial3d(apocalypse_material.clone()),
        Transform::from_xyz(18.0, 0.0, -14.0).with_rotation(Quat::from_rotation_y(PI / 2.0)),
    ));
    commands.spawn((
        Mesh3d(gas_pump_mesh.clone()),
        MeshMaterial3d(apocalypse_material.clone()),
        Transform::from_xyz(18.0, 0.0, -10.0).with_rotation(Quat::from_rotation_y(PI / 2.0)),
    ));
    commands.spawn((
        Mesh3d(dumpster_mesh.clone()),
        MeshMaterial3d(apocalypse_material.clone()),
        Transform::from_xyz(26.0, 0.0, -14.0).with_rotation(Quat::from_rotation_y(-0.4)),
    ));
    commands.spawn((
        Mesh3d(crate_mesh.clone()),
        MeshMaterial3d(apocalypse_material.clone()),
        Transform::from_xyz(26.0, 0.0, -22.0),
    ));

    // Survivor Outpost (X = -35.0, Z = -35.0)
    for (ox, oz) in [(-33.0, -32.0), (-36.0, -36.0), (-31.0, -38.0)] {
        commands.spawn((
            Mesh3d(crate_mesh.clone()),
            MeshMaterial3d(apocalypse_material.clone()),
            Transform::from_xyz(ox, 0.0, oz),
        ));
    }

    // Additional World POIs
    for poi in &world_map.pois {
        if poi.position.x * poi.position.x + poi.position.z * poi.position.z > 25.0 {
            commands.spawn((
                Mesh3d(crate_mesh.clone()),
                MeshMaterial3d(apocalypse_material.clone()),
                Transform::from_xyz(poi.position.x, 0.0, poi.position.z),
            ));
        }
    }

    // 8. Infected Zombies (Walkers & Runners with authentic behavior)
    let zombie_spawns = [
        (-3.5f32,  25.0f32, false), // Walker near highway
        ( 4.2f32, -30.0f32, false), // Walker by road
        (18.0f32, -22.0f32, false), // Walker by abandoned car
        (-2.0f32,  62.0f32, true),  // Runner past checkpoint
        (-30.0f32, -40.0f32, false), // Walker near survivor camp
        (15.0f32,  55.0f32, true),  // Runner lurking in forest edge
    ];
    for (x, z, is_runner) in zombie_spawns {
        let mesh = if is_runner {
            zombie_runner_mesh.clone()
        } else {
            zombie_walker_mesh.clone()
        };
        commands.spawn((
            Zombie {
                base_yaw: (x * 13.0 + z * 7.0).sin() * PI,
                detection_range: if is_runner { 22.0 } else { 16.0 },
            },
            Mesh3d(mesh),
            MeshMaterial3d(character_material.clone()),
            Transform::from_xyz(x, 0.0, z),
        ));
    }

    // 9. Directional Sunlight & Shadows
    commands.spawn((
        DirectionalLight {
            shadow_maps_enabled: true,
            illuminance: 16000.0,
            ..default()
        },
        Transform::from_xyz(45.0, 75.0, 40.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // 10. Dynamic Camera with Orbit Controller
    commands.spawn((
        CameraController::default(),
        Camera3d::default(),
        Transform::from_xyz(-8.0, 10.0, 12.0).looking_at(Vec3::new(0.0, 1.0, 0.0), Vec3::Y),
    ));

    // 11. Atmospheric Ambient Audio
    let theme_sound: Handle<AudioSource> = asset_server.load("audio/theme_menu.wav");
    commands.spawn((
        AudioPlayer::new(theme_sound),
        PlaybackSettings::LOOP,
    ));

    // 12. Tactical HUD
    let logo_image: Handle<Image> = asset_server.load("textures/ui/logo.png");
    let font_bold: Handle<Font> = asset_server.load("fonts/Montserrat-Bold.ttf");

    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            justify_content: JustifyContent::SpaceBetween,
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(16.0)),
            ..default()
        })
        .with_children(|parent| {
            // Top Bar: Logo + Sector Heading
            parent
                .spawn(Node {
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|top| {
                    top.spawn((
                        ImageNode::new(logo_image),
                        Node {
                            width: Val::Px(200.0),
                            height: Val::Px(45.0),
                            ..default()
                        },
                    ));

                    top.spawn((
                        CompassHeadingText,
                        Text::new("180° [S]  |  LONGDOWN VALLEY - ROUTE 9  |  TIME: 08:00 [ClearSky]"),
                        TextFont {
                            font: bevy::text::FontSource::Handle(font_bold.clone()),
                            font_size: bevy::text::FontSize::Px(17.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.96, 0.88, 0.52)),
                    ));
                });

            // Bottom Bar: Survival Vitals & Controls
            parent
                .spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|bottom| {
                    bottom.spawn((
                        VitalsText,
                        Text::new("HEALTH: 100%  |  STAMINA: 100%  |  HUNGER: 100%  |  THIRST: 100%"),
                        TextFont {
                            font: bevy::text::FontSource::Handle(font_bold.clone()),
                            font_size: bevy::text::FontSize::Px(16.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.25, 0.95, 0.35)),
                    ));
                    bottom.spawn((
                        Text::new("[WASD] Move  |  [Space] Jump  |  [Shift] Sprint  |  [Q / E or Mouse Drag] Orbit Camera  |  [Wheel] Zoom"),
                        TextFont {
                            font: bevy::text::FontSource::Handle(font_bold.clone()),
                            font_size: bevy::text::FontSize::Px(13.0),
                            ..default()
                        },
                        TextColor(Color::srgb(0.90, 0.90, 0.90)),
                    ));
                });
        });
}

/// Responsive WASD Movement + Physics-based Jump
fn player_movement(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    camera_query: Query<&CameraController>,
    mut query: Query<(&mut Player, &mut Transform)>,
) {
    let camera_yaw = camera_query.iter().next().map(|c| c.yaw).unwrap_or(0.0);

    for (mut player, mut transform) in &mut query {
        let mut raw_direction = Vec3::ZERO;

        if keyboard_input.pressed(KeyCode::KeyW) || keyboard_input.pressed(KeyCode::ArrowUp) {
            raw_direction.z -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyS) || keyboard_input.pressed(KeyCode::ArrowDown) {
            raw_direction.z += 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyA) || keyboard_input.pressed(KeyCode::ArrowLeft) {
            raw_direction.x -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::KeyD) || keyboard_input.pressed(KeyCode::ArrowRight) {
            raw_direction.x += 1.0;
        }

        // Camera-relative direction vector
        if raw_direction.length_squared() > 0.0 {
            let forward = Vec3::new(camera_yaw.sin(), 0.0, camera_yaw.cos());
            let right = Vec3::new(camera_yaw.cos(), 0.0, -camera_yaw.sin());

            let direction = (forward * raw_direction.z + right * raw_direction.x).normalize();
            let sprint_mult = if keyboard_input.pressed(KeyCode::ShiftLeft)
                || keyboard_input.pressed(KeyCode::ShiftRight)
            {
                1.75
            } else {
                1.0
            };

            transform.translation += direction * player.speed * sprint_mult * time.delta_secs();

            // Smoothly rotate character to face travel direction
            let target_rotation = Quat::from_rotation_y(direction.x.atan2(direction.z));
            transform.rotation = transform.rotation.slerp(target_rotation, 12.0 * time.delta_secs());
        }

        // Jump & Gravity Physics
        let gravity = -26.0;
        let ground_y = 0.0; // Feet rest at ground level

        if keyboard_input.just_pressed(KeyCode::Space) && player.is_grounded {
            player.vertical_velocity = 9.0; // Immediate responsive impulse
            player.is_grounded = false;
        }

        if !player.is_grounded {
            player.vertical_velocity += gravity * time.delta_secs();
            transform.translation.y += player.vertical_velocity * time.delta_secs();

            if transform.translation.y <= ground_y {
                transform.translation.y = ground_y;
                player.vertical_velocity = 0.0;
                player.is_grounded = true;
            }
        }
    }
}

/// Smooth Orbit Camera with both Mouse Drag & Keyboard Q/E support
fn camera_controls(
    mouse_button: Res<ButtonInput<MouseButton>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    player_query: Query<&Transform, (With<Player>, Without<CameraController>)>,
    mut camera_query: Query<(&mut CameraController, &mut Transform)>,
    time: Res<Time>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let Ok((mut controller, mut camera_transform)) = camera_query.single_mut() else {
        return;
    };

    // 1. Zoom with Mouse Scroll Wheel
    if mouse_scroll.delta.y != 0.0 {
        controller.distance = (controller.distance - mouse_scroll.delta.y * 2.0).clamp(5.0, 36.0);
    }

    // 2. Mouse Drag Orbit (Left Button OR Right Button OR Middle Button)
    if mouse_button.pressed(MouseButton::Left)
        || mouse_button.pressed(MouseButton::Right)
        || mouse_button.pressed(MouseButton::Middle)
    {
        let delta = mouse_motion.delta;
        controller.yaw -= delta.x * 0.005;
        controller.pitch = (controller.pitch - delta.y * 0.005).clamp(8.0f32.to_radians(), 75.0f32.to_radians());
    }

    // 3. Keyboard Camera Controls (Q = rotate left, E = rotate right, R = tilt up, F = tilt down)
    let rot_speed = 2.4 * time.delta_secs();
    if keyboard_input.pressed(KeyCode::KeyQ) {
        controller.yaw += rot_speed;
    }
    if keyboard_input.pressed(KeyCode::KeyE) {
        controller.yaw -= rot_speed;
    }
    if keyboard_input.pressed(KeyCode::KeyR) {
        controller.pitch = (controller.pitch + rot_speed).min(75.0f32.to_radians());
    }
    if keyboard_input.pressed(KeyCode::KeyF) {
        controller.pitch = (controller.pitch - rot_speed).max(8.0f32.to_radians());
    }

    // 4. Calculate Camera Offset
    let h_dist = controller.distance * controller.pitch.cos();
    let v_dist = controller.distance * controller.pitch.sin();

    let offset = Vec3::new(
        h_dist * controller.yaw.sin(),
        v_dist,
        h_dist * controller.yaw.cos(),
    );

    let target_camera_pos = player_transform.translation + offset;
    camera_transform.translation = camera_transform.translation.lerp(target_camera_pos, 10.0 * time.delta_secs());
    camera_transform.look_at(player_transform.translation + Vec3::new(0.0, 1.1, 0.0), Vec3::Y);
}

/// Zombie Awareness & Dynamic Facing Behavior
fn zombie_behavior(
    player_query: Query<&Transform, With<Player>>,
    mut zombie_query: Query<(&Zombie, &mut Transform), Without<Player>>,
    time: Res<Time>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };

    for (zombie, mut transform) in &mut zombie_query {
        let delta = player_transform.translation - transform.translation;
        let dist = delta.length();

        if dist < zombie.detection_range {
            // Turn to face approaching survivor
            let target_rot = Quat::from_rotation_y(delta.x.atan2(delta.z));
            transform.rotation = transform.rotation.slerp(target_rot, 3.5 * time.delta_secs());
        } else {
            // Idle ambient sway
            let sway = (time.elapsed_secs() * 0.8 + zombie.base_yaw).sin() * 0.25;
            let idle_rot = Quat::from_rotation_y(zombie.base_yaw + sway);
            transform.rotation = transform.rotation.slerp(idle_rot, 1.5 * time.delta_secs());
        }
    }
}

/// Dynamic Compass & HUD Updates
fn update_hud(
    player_query: Query<&Transform, With<Player>>,
    mut heading_query: Query<&mut Text, With<CompassHeadingText>>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };
    let Ok(mut text) = heading_query.single_mut() else {
        return;
    };

    let heading = (player_transform.rotation.to_euler(EulerRot::YXZ).0.to_degrees() as i32).rem_euclid(360);
    let cardinal = match heading {
        23..=67 => "NE",
        68..=112 => "E",
        113..=157 => "SE",
        158..=202 => "S",
        203..=247 => "SW",
        248..=292 => "W",
        293..=337 => "NW",
        _ => "N",
    };

    **text = format!(
        "{}° [{}]  |  LONGDOWN VALLEY - ROUTE 9  |  TIME: 08:00 [ClearSky]",
        heading, cardinal
    );
}
