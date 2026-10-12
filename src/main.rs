use std::path::Path;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;
use surroundead::menus::pause::RespawnPlayer;
use surroundead::menus::{AppState, CurrentGame, MenuData, MenuPlugin, MenuScreen};
use surroundead::menus::data::CharacterProfile;
use surroundead::level::{KinematicBody, LEVEL_DIR, Level, LevelPlugin, Terrain};
use surroundead::gameplay::{Armed, GameplayPlugin, PlayerCamera, player_in_control};
use surroundead::models::{Locomotion, ModelsPlugin, spawn_character};

#[derive(Component)]
struct Player {
    speed: f32,
}

/// BP_PlayerCharacter's spring arm: `ThirdPersonTargetArmLength` 300 behind the survivor,
/// mouse look (`bUsePawnControlRotation`), shorter and over a shoulder while aiming.
#[derive(Component)]
struct CameraController {
    /// Arm length the wheel sets (metres).
    distance: f32,
    /// Current arm length, easing toward `distance` or the aiming length.
    arm: f32,
    /// Current sideways offset (shoulder), easing like the arm.
    side: f32,
    /// Camera above (+) or below (-) the pivot, radians.
    pitch: f32,
    yaw: f32,
}

impl Default for CameraController {
    fn default() -> Self {
        Self { distance: 3.0, arm: 3.0, side: 0.0, pitch: 12.0f32.to_radians(), yaw: -30.0f32.to_radians() }
    }
}

/// Aiming arm length and shoulder offset. The ADS spring-arm timelines aren't read; guesses.
const AIM_ARM: f32 = 1.3;
const AIM_SIDE: f32 = 0.55;
/// Walking speed while aiming, m/s.
const AIM_WALK: f32 = 1.6;
/// Spring arm pivot above the feet (capsule top, roughly the shoulders).
const PIVOT_HEIGHT: f32 = 1.55;




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
        .add_systems(Update, grab_cursor)
        .add_systems(
            Update,
            (
                player_movement.run_if(player_in_control),
                camera_controls.run_if(player_in_control),
                sync_player_to_save,
                respawn_player,
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
        surroundead::gameplay::daynight::Sun,
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
        PlayerCamera,
        // Eye adaptation (gameplay/daynight.rs) drives this.
        bevy::camera::Exposure::default(),
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
    armed: Res<Armed>,
    game: Res<surroundead::menus::CurrentGame>,
    camera_query: Query<&CameraController>,
    mut query: Query<(&Player, &mut KinematicBody, &mut Transform, &mut Locomotion)>,
) {
    let camera_yaw = camera_query.iter().next().map(|c| c.yaw).unwrap_or(0.0);

    for (player, mut body, mut transform, mut loco) in &mut query {
        let mut moving = None;
        let mut raw_direction = Vec3::ZERO;
        let mut aim_dir = Vec3::ZERO;

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
            aim_dir = direction;
            let sprint_mult = if !armed.aiming && (keyboard_input.pressed(KeyCode::ShiftLeft) || keyboard_input.pressed(KeyCode::ShiftRight)) {
                1.75
            } else {
                1.0
            };
            // Walking while aiming (the Ironsights blend spaces sample 115 cm/s; ours is a bit
            // quicker so it doesn't feel stuck).
            let speed = if armed.aiming { AIM_WALK } else { player.speed * sprint_mult };
            transform.translation += direction * speed * time.delta_secs();
            moving = Some(sprint_mult > 1.0);

            // Smoothly rotate character to face travel direction (aiming faces the camera below).
            if !armed.aiming {
                let target_rotation = Quat::from_rotation_y(direction.x.atan2(direction.z));
                transform.rotation = transform.rotation.slerp(target_rotation, 12.0 * time.delta_secs());
            }
        }

        // Aiming: face where the camera looks instead of the way they walk.
        if armed.aiming {
            let target_rotation = Quat::from_rotation_y(camera_yaw + std::f32::consts::PI);
            transform.rotation = transform.rotation.slerp(target_rotation, 18.0 * time.delta_secs());
        }

        // Gravity, landing and walls are the level's KinematicBody (level/collision.rs).
        if keyboard_input.just_pressed(KeyCode::Space) {
            // 9 m/s stands for the CharacterMovement's 600 cm/s JumpZVelocity; a broken bone
            // drops it to 400 (MedicalComponent `Svr_SetJumpVelocity`).
            let v = game.state.as_ref().map_or(surroundead::gameplay::medical::JUMP_VELOCITY, |s| s.effects.jump_velocity());
            body.jump = Some(9.0 * v / surroundead::gameplay::medical::JUMP_VELOCITY);
        }
        let mut next = match (body.grounded, moving) {
            (false, _) => Locomotion::Air,
            (true, Some(true)) => Locomotion::Sprint,
            (true, Some(false)) => Locomotion::Run,
            (true, None) => Locomotion::Idle,
        };
        // Aiming: the *_Ironsights blend spaces, by walking direction relative to the facing
        // (the model faces +Z, its right is -X).
        if armed.aiming && body.grounded {
            next = if raw_direction.length_squared() == 0.0 {
                Locomotion::AimIdle
            } else {
                let local = transform.rotation.inverse() * aim_dir;
                if local.z.abs() >= local.x.abs() {
                    if local.z > 0.0 { Locomotion::AimFwd } else { Locomotion::AimBwd }
                } else if local.x < 0.0 {
                    Locomotion::AimRight
                } else {
                    Locomotion::AimLeft
                }
            };
        }
        if *loco != next {
            *loco = next;
        }
    }
}

/// Third-person mouse-look camera on a spring arm (see [`CameraController`]).
#[allow(clippy::too_many_arguments)]
fn camera_controls(
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    armed: Res<Armed>,
    terrain: Option<Res<Terrain>>,
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

    // Mouse look (IA_CameraRotation) and wheel zoom (IA_CameraScroll).
    let delta = mouse_motion.delta;
    controller.yaw -= delta.x * 0.003;
    controller.pitch = (controller.pitch + delta.y * 0.003).clamp(-50.0f32.to_radians(), 70.0f32.to_radians());
    if mouse_scroll.delta.y != 0.0 {
        controller.distance = (controller.distance - mouse_scroll.delta.y * 0.5).clamp(1.5, 6.0);
    }
    let (arm, side) = if armed.aiming { (AIM_ARM, AIM_SIDE * armed.shoulder) } else { (controller.distance, 0.0) };
    let ease = (12.0 * time.delta_secs()).min(1.0);
    controller.arm += (arm - controller.arm) * ease;
    controller.side += (side - controller.side) * ease;

    let (yaw, pitch) = (controller.yaw, controller.pitch);
    let back = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos());
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    let pivot = player_transform.translation + Vec3::Y * PIVOT_HEIGHT + right * controller.side;
    let mut eye = pivot + back * controller.arm;
    // The arm's collision test, against the ground only.
    if let Some(t) = terrain.as_ref() {
        eye.y = eye.y.max(t.height(eye.x, eye.z) + 0.25);
    }
    camera_transform.translation = eye;
    camera_transform.look_to(-back, Vec3::Y);
}

/// The cursor is captured for mouse look while playing, free in menus and the inventory.
fn grab_cursor(
    control: Query<(), With<Player>>,
    app: Res<State<AppState>>,
    screen: Res<State<MenuScreen>>,
    overlay: Res<surroundead::gameplay::GameOverlay>,
    vitals: Res<surroundead::gameplay::PlayerVitals>,
    mut cursor: Query<&mut bevy::window::CursorOptions, With<bevy::window::PrimaryWindow>>,
) {
    let Ok(mut c) = cursor.single_mut() else { return };
    let play = *app.get() == AppState::InGame && *screen.get() == MenuScreen::None && !control.is_empty() && !overlay.is_open() && !vitals.is_dead();
    let mode = if play { bevy::window::CursorGrabMode::Locked } else { bevy::window::CursorGrabMode::None };
    if c.grab_mode != mode {
        c.grab_mode = mode;
        c.visible = !play;
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
