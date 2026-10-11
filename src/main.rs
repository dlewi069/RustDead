use std::path::Path;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use surroundead::menus::pause::RespawnPlayer;
use surroundead::menus::{AppState, CurrentGame, MenuData, MenuPlugin, MenuScreen};
use surroundead::menus::data::CharacterProfile;
use surroundead::level::{KinematicBody, LEVEL_DIR, Level, LevelPlugin, LevelZombie, Terrain};
use surroundead::gameplay::{GameplayPlugin, player_in_control};
use surroundead::models::{AnimSet, Animated, Locomotion, ModelsPlugin, spawn_character};

#[derive(Component)]
struct Player {
    speed: f32,
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
    speed: f32,
}



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
        .add_plugins((MenuPlugin, ModelsPlugin, LevelPlugin, GameplayPlugin))
        .add_systems(OnEnter(AppState::InGame), setup_world)
        .add_systems(
            Update,
            (
                player_movement.run_if(player_in_control),
                camera_controls.run_if(player_in_control),
                zombie_behavior,
                sync_player_to_save,
                respawn_player,
                adopt_level_zombies,
            )
                .run_if(in_state(AppState::InGame))
                .run_if(in_state(MenuScreen::None)),
        )
        .run();
}

fn setup_world(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    game: Res<CurrentGame>,
    menu_data: Res<MenuData>,
    terrain: Option<Res<Terrain>>,
    level: Option<Res<Level>>,
) {
    // LongdownValley comes from the level module; without exported level data fall back to a
    // flat plane so the game still runs.
    let terrain = terrain.map(|t| t.clone()).or_else(|| Terrain::load(LEVEL_DIR));
    if terrain.is_none() {
        commands.spawn((
            DespawnOnExit(AppState::InGame),
            Mesh3d(meshes.add(Plane3d::default().mesh().size(600.0, 600.0))),
            MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::srgb(0.35, 0.45, 0.3), perceptual_roughness: 1.0, ..default() })),
        ));
    }
    let ground = |x: f32, z: f32| terrain.as_ref().map_or(0.0, |t| t.height(x, z));

    // Saved games resume where they left off; a new game picks one of the map's PlayerStarts.
    let saved = game.state.as_ref().map(|s| Vec3::new(s.player_pos.x, s.player_pos.y, s.player_pos.z)).filter(|p| *p != Vec3::ZERO);
    let starts = level.map(|l| l.player_starts()).or_else(|| Level::peek_player_starts()).unwrap_or_default();
    let mut spawn = saved.unwrap_or_else(|| {
        // SD_START=<n> pins the PlayerStart (screenshot tours, debugging a spot).
        let seed = std::env::var("SD_START").ok().and_then(|s| s.parse().ok());
        let seed = seed.unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as usize);
        let i = seed % starts.len().max(1);
        starts.get(i).copied().unwrap_or(Vec3::ZERO)
    });
    spawn.y = ground(spawn.x, spawn.z);

    // The survivor from the character creator, built from the game's modular body parts.
    let profile = game.profile.clone().unwrap_or_else(|| CharacterProfile::new(&menu_data.creator));
    spawn_character(
        &mut commands,
        &asset_server,
        &profile,
        &menu_data.creator,
        (
            DespawnOnExit(AppState::InGame),
            Player { speed: 4.5 },
            KinematicBody::new(0.35, 1.8),
            Transform::from_translation(spawn),
        ),
    );

    // Sun: cascades sized for third-person play over a 4.6 km map.
    commands.spawn((
        DespawnOnExit(AppState::InGame),
        DirectionalLight { shadow_maps_enabled: true, illuminance: 12000.0, ..default() },
        bevy::light::CascadeShadowConfigBuilder { maximum_distance: 220.0, first_cascade_far_bound: 25.0, ..default() }.build(),
        Transform::from_xyz(45.0, 75.0, 40.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(GlobalAmbientLight { brightness: 600.0, ..default() });
    commands.insert_resource(ClearColor(Color::srgb(0.62, 0.72, 0.82)));

    // Orbit camera; distance fog hides the prop streaming radius.
    commands.spawn((
        DespawnOnExit(AppState::InGame),
        CameraController::default(),
        Camera3d::default(),
        DistanceFog {
            color: Color::srgb(0.62, 0.72, 0.82),
            falloff: FogFalloff::Linear { start: 250.0, end: 520.0 },
            ..default()
        },
        Transform::from_translation(spawn + Vec3::new(-8.0, 10.0, 12.0)).looking_at(spawn + Vec3::Y, Vec3::Y),
    ));
    // The HUD is gameplay/hud.rs (BP_Ingame).
}

/// Responsive WASD Movement + Physics-based Jump
fn player_movement(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    camera_query: Query<&CameraController>,
    mut query: Query<(&Player, &mut KinematicBody, &mut Transform, &mut Locomotion)>,
) {
    let camera_yaw = camera_query.iter().next().map(|c| c.yaw).unwrap_or(0.0);

    for (player, mut body, mut transform, mut loco) in &mut query {
        let mut moving = None;
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
            moving = Some(sprint_mult > 1.0);

            // Smoothly rotate character to face travel direction
            let target_rotation = Quat::from_rotation_y(direction.x.atan2(direction.z));
            transform.rotation = transform.rotation.slerp(target_rotation, 12.0 * time.delta_secs());
        }

        // Gravity, landing and walls are the level's KinematicBody (level/collision.rs).
        if keyboard_input.just_pressed(KeyCode::Space) {
            body.jump = Some(9.0);
        }
        let next = match (body.grounded, moving) {
            (false, _) => Locomotion::Air,
            (true, Some(true)) => Locomotion::Sprint,
            (true, Some(false)) => Locomotion::Run,
            (true, None) => Locomotion::Idle,
        };
        if *loco != next {
            *loco = next;
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
    mut zombie_query: Query<(&Zombie, &mut Transform, &mut Locomotion), Without<Player>>,
    time: Res<Time>,
) {
    let Ok(player_transform) = player_query.single() else {
        return;
    };

    for (zombie, mut transform, mut loco) in &mut zombie_query {
        let mut delta = player_transform.translation - transform.translation;
        delta.y = 0.0;
        let dist = delta.length();

        if dist < zombie.detection_range {
            // Turn toward the survivor and shamble after them.
            let target_rot = Quat::from_rotation_y(delta.x.atan2(delta.z));
            transform.rotation = transform.rotation.slerp(target_rot, 3.5 * time.delta_secs());
            if dist > 1.3 {
                transform.translation += delta / dist * zombie.speed * time.delta_secs();
                loco.set_if_neq(Locomotion::Walk);
            } else {
                loco.set_if_neq(Locomotion::Attack);
            }
        } else {
            // Idle ambient sway
            let sway = (time.elapsed_secs() * 0.8 + zombie.base_yaw).sin() * 0.25;
            let idle_rot = Quat::from_rotation_y(zombie.base_yaw + sway);
            transform.rotation = transform.rotation.slerp(idle_rot, 1.5 * time.delta_secs());
            loco.set_if_neq(Locomotion::Idle);
        }
    }
}

/// Zombies spawned by the level's spawners get the same behaviour as any other.
fn adopt_level_zombies(mut commands: Commands, added: Query<(Entity, &Transform), Added<LevelZombie>>) {
    for (e, t) in &added {
        let yaw = t.rotation.to_euler(EulerRot::YXZ).0;
        commands.entity(e).insert((
            Zombie { base_yaw: yaw, detection_range: 18.0, speed: 1.4 },
            KinematicBody::new(0.35, 1.8),
            Animated(AnimSet::Zombie),
            Locomotion::Idle,
        ));
    }
}


/// Keep the session's simulation state in step with the rendered player so Save writes the
/// real position.
fn sync_player_to_save(player: Query<&Transform, With<Player>>, mut game: ResMut<CurrentGame>) {
    let (Ok(t), Some(state)) = (player.single(), game.state.as_mut()) else { return };
    state.player_pos.x = t.translation.x;
    state.player_pos.y = t.translation.y;
    state.player_pos.z = t.translation.z;
    state.player_yaw = t.rotation.to_euler(EulerRot::YXZ).0;
}

/// Pause → Suicide → YES: drop the player at a spawn point away from where they died.
fn respawn_player(
    mut commands: Commands,
    request: Option<Res<RespawnPlayer>>,
    mut player: Query<(&mut Transform, &mut KinematicBody), With<Player>>,
    level: Option<Res<Level>>,
    terrain: Option<Res<Terrain>>,
) {
    if request.is_none() {
        return;
    }
    commands.remove_resource::<RespawnPlayer>();
    let spawns = level.map(|l| l.player_starts()).unwrap_or_default();
    let Ok((mut t, mut p)) = player.single_mut() else { return };
    let mut pick = spawns.iter().copied().max_by(|a, b| a.distance(t.translation).total_cmp(&b.distance(t.translation))).unwrap_or(Vec3::ZERO);
    pick.y = terrain.as_ref().map_or(0.0, |t| t.height(pick.x, pick.z));
    t.translation = pick;
    p.vertical_velocity = 0.0;
    p.grounded = true;
}
