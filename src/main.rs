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
    /// Sprint ran out of stamina (StaminaComponent's f_CheckStamina turned it off); Shift has to be
    /// pressed again, with stamina above 0.5 (f_SprintOnOff's NoStamina check).
    exhausted: bool,
    /// Crouched (IA_Crouch, Left Ctrl: held, or pressed to toggle with CROUCH TOGGLE).
    crouched: bool,
    /// Sitting on the ground (IA_SitDown, -).
    sitting: bool,
}

/// StaminaComponent overrides on BP_PlayerCharacter: WalkSpeed 400, SprintSpeed 750 cm/s.
const WALK_SPEED: f32 = 4.0;
const SPRINT_SPEED: f32 = 7.5;
/// CharMoveComp MaxWalkSpeedCrouched 120 cm/s; `Crouch Sprint` raises it to 420.
const CROUCH_SPEED: f32 = 1.2;
const CROUCH_SPRINT_SPEED: f32 = 4.2;
/// The capsule: radius 30, half height 82 standing and 50 crouched (CrouchedHalfHeight), cm.
const CAPSULE_RADIUS: f32 = 0.30;
const STANDING_HEIGHT: f32 = 1.64;
const CROUCHED_HEIGHT: f32 = 1.0;
/// Jumping needs and costs 5 stamina (`Svr_ReduceStamina(5.0)`).
const JUMP_STAMINA: f32 = 5.0;

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
    /// How far crouching has lowered the pivot (eased).
    drop: f32,
}

impl Default for CameraController {
    fn default() -> Self {
        Self { distance: 3.0, arm: 3.0, side: 0.0, pitch: 12.0f32.to_radians(), yaw: -30.0f32.to_radians(), drop: 0.0 }
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
        .add_plugins(surroundead::audio::plugin)
        .add_systems(OnEnter(AppState::InGame), setup_world)
        .add_systems(Update, grab_cursor)
        .add_systems(
            Update,
            (
                player_movement.run_if(surroundead::gameplay::on_foot).after(surroundead::gameplay::vault::VaultSet),
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
    let starts = level.map(|l| l.player_starts()).or_else(Level::peek_player_starts).unwrap_or_default();
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
            Player { speed: WALK_SPEED, exhausted: false, crouched: false, sitting: false },
            KinematicBody::new(CAPSULE_RADIUS, STANDING_HEIGHT),
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
        surroundead::audio::Listener,
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

/// WASD (camera-relative), sprint, crouch and jump, after BP_PlayerCharacter's input events.
#[allow(clippy::too_many_arguments)]
fn player_movement(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    armed: Res<Armed>,
    mut game: ResMut<surroundead::menus::CurrentGame>,
    store: Res<surroundead::menus::settings::SettingsStore>,
    stun: Res<surroundead::gameplay::survival::LandingStun>,
    camera_query: Query<&CameraController>,
    mut query: Query<(&mut Player, &mut KinematicBody, &mut Transform, &mut Locomotion)>,
    roll: Res<surroundead::gameplay::roll::Roll>,
    swim: Res<surroundead::gameplay::swim::Swim>,
    vault: Res<surroundead::gameplay::vault::Vault>,
    fishing: Res<surroundead::gameplay::fishing::Fishing>,
) {
    let camera_yaw = camera_query.iter().next().map(|c| c.yaw).unwrap_or(0.0);
    let stamina = game.state.as_ref().map_or(100.0, |s| s.survival.stamina);
    // f_SprintOnOff: UpdatePlayerSpeed(FitnessMultiplier x SprintSpeed).
    let fitness = game.state.as_ref().map_or(1.0, |s| s.passive.multiplier(surroundead::progression::passive::Passive::Fitness));
    let shift = [KeyCode::ShiftLeft, KeyCode::ShiftRight];
    let crouch_toggle = store.find("settings/Controls/CROUCH TOGGLE") == Some(surroundead::menus::settings::Value::Bool(true));

    for (mut player, mut body, mut transform, mut loco) in &mut query {
        // On a ladder: W/S climb at MaxFlySpeed, strafing does nothing, Jump lets go.
        if body.climbing {
            // Event_Crouch does nothing on a ladder (ClimbingLadder?).
            player.crouched = false;
            body.height = STANDING_HEIGHT;
            let dir = keyboard_input.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) as i32 as f32 - keyboard_input.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) as i32 as f32;
            // The body moves it, and the speed carries on when it leaves the top (Flying -> Falling).
            body.vertical_velocity = dir * surroundead::level::ladders::CLIMB_SPEED;
            if keyboard_input.just_pressed(KeyCode::Space) {
                body.climbing = false;
            }
            let next = if dir != 0.0 { Locomotion::Climb } else { Locomotion::ClimbIdle };
            if *loco != next {
                *loco = next;
            }
            continue;
        }
        // Rolling (IsPlayerRolling?): the roll's root motion moves them (gameplay/roll.rs).
        if roll.active() {
            continue;
        }
        // Vaulting (gameplay/vault.rs): its montages move them, and the press that started it
        // doesn't jump (the BP's Flying movement mode).
        if vault.active() {
            continue;
        }
        // Swimming (gameplay/swim.rs): W/S along the camera's whole forward (looking down dives),
        // A/D along its right, Space rises under water (IA_FloatUpwards); 3 m/s, Shift = SwimFast
        // (5 m/s x FitnessSwimmingMultiplier, a sprint for stamina). Never above the afloat height.
        if body.swimming {
            use surroundead::gameplay::swim::{FLOAT, SWIM_FAST, SWIM_SPEED};
            player.sitting = false;
            player.crouched = false;
            body.height = STANDING_HEIGHT;
            let (yaw, pitch) = camera_query.iter().next().map_or((0.0, 0.0), |c| (c.yaw, c.pitch));
            let look = -Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos());
            let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
            let axis = |pos: [KeyCode; 2], neg: [KeyCode; 2]| keyboard_input.any_pressed(pos) as i32 as f32 - keyboard_input.any_pressed(neg) as i32 as f32;
            let mut dir = look * axis([KeyCode::KeyW, KeyCode::ArrowUp], [KeyCode::KeyS, KeyCode::ArrowDown]) + right * axis([KeyCode::KeyD, KeyCode::ArrowRight], [KeyCode::KeyA, KeyCode::ArrowLeft]);
            if swim.underwater && keyboard_input.pressed(KeyCode::Space) {
                dir += right.cross(look);
            }
            if keyboard_input.any_just_pressed(shift) && stamina > 0.5 {
                player.exhausted = false;
            } else if stamina <= 0.0 {
                player.exhausted = true;
            }
            let moving = dir.length_squared() > 1e-6;
            let fast = moving && !player.exhausted && stamina > 0.0 && keyboard_input.any_pressed(shift);
            if moving {
                let fitness_swim = game.state.as_ref().map_or(1.0, |s| s.passive.fitness_swimming);
                let speed = if fast { SWIM_FAST * fitness_swim } else { SWIM_SPEED };
                let d = dir.normalize();
                let before = transform.translation.y;
                transform.translation += d * speed * time.delta_secs();
                if let Some(s) = swim.surface {
                    transform.translation.y = transform.translation.y.min((s - FLOAT).max(before));
                }
                let flat = d.with_y(0.0);
                if flat.length_squared() > 1e-6 {
                    let target = Quat::from_rotation_y(flat.x.atan2(flat.z));
                    transform.rotation = transform.rotation.slerp(target, 8.0 * time.delta_secs());
                }
            }
            let next = match (swim.underwater, moving, fast) {
                (true, true, _) => Locomotion::SwimUnder,
                (_, false, _) => Locomotion::SwimIdle,
                (false, true, true) => Locomotion::SwimFast,
                (false, true, false) => Locomotion::Swim,
            };
            if *loco != next {
                *loco = next;
            }
            continue;
        }
        // IA_SitDown (-): only standing still on the ground, not aiming or sprinting
        // (DisableMovement + SittingPlayer_Anim_Montage); - again or Jump stands up (StopAnimMontage,
        // SetMovementMode walking), and Jump doesn't jump then. Crouch does nothing while seated.
        // Its Event_SitDownRegen (5 s in) restarts the health regen timer, which ours never stops.
        let wasd = keyboard_input.any_pressed([KeyCode::KeyW, KeyCode::KeyA, KeyCode::KeyS, KeyCode::KeyD, KeyCode::ArrowUp, KeyCode::ArrowDown, KeyCode::ArrowLeft, KeyCode::ArrowRight]);
        let mut stood_up = false;
        if player.sitting {
            if keyboard_input.any_just_pressed([KeyCode::Minus, KeyCode::Space]) {
                player.sitting = false;
                stood_up = true;
            } else {
                if *loco != Locomotion::Sit {
                    *loco = Locomotion::Sit;
                }
                continue;
            }
        } else if keyboard_input.just_pressed(KeyCode::Minus)
            && body.grounded
            && !wasd
            && !armed.aiming
            && stun.0 <= 0.0
            && !matches!(*loco, Locomotion::Sprint | Locomotion::CrouchSprint)
        {
            // (Sitting from a crouch stands the capsule up: the montage is a standing-height pose.)
            player.sitting = true;
            player.crouched = false;
            body.height = STANDING_HEIGHT;
            *loco = Locomotion::Sit;
            continue;
        }
        // IA_Crouch (Left Ctrl): Character.CrouchToggle flips it on each press, otherwise
        // pressed crouches and released stands. (UnCrouch isn't stopped by a low ceiling here:
        // the collision has no ceilings.)
        player.crouched = if crouch_toggle { player.crouched != keyboard_input.just_pressed(KeyCode::ControlLeft) } else { keyboard_input.pressed(KeyCode::ControlLeft) };
        body.height = if player.crouched { CROUCHED_HEIGHT } else { STANDING_HEIGHT };
        if keyboard_input.any_just_pressed(shift) && stamina > 0.5 {
            player.exhausted = false;
        } else if stamina <= 0.0 && matches!(*loco, Locomotion::Sprint | Locomotion::CrouchSprint) {
            player.exhausted = true;
        }
        // A hard landing (OnLanded) disables movement for a moment; so does a line in the water
        // (CastRod's SetCharacterMovementEnabled(false)).
        let stunned = stun.0 > 0.0 || fishing.active;
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
        if stunned {
            raw_direction = Vec3::ZERO;
        }

        // Camera-relative direction vector
        if raw_direction.length_squared() > 0.0 {
            let forward = Vec3::new(camera_yaw.sin(), 0.0, camera_yaw.cos());
            let right = Vec3::new(camera_yaw.cos(), 0.0, -camera_yaw.sin());

            let direction = (forward * raw_direction.z + right * raw_direction.x).normalize();
            aim_dir = direction;
            // Sprint input while crouched is the crouch sprint (`Crouch Sprint`).
            let sprinting = !armed.aiming && !player.exhausted && stamina > 0.0 && keyboard_input.any_pressed(shift);
            // Walking while aiming (the Ironsights blend spaces sample 115 cm/s; ours is a bit
            // quicker so it doesn't feel stuck). Crouched, MaxWalkSpeedCrouched rules either way.
            let speed = match (player.crouched, sprinting) {
                (true, true) => CROUCH_SPRINT_SPEED,
                (true, false) => CROUCH_SPEED,
                (false, _) if armed.aiming => AIM_WALK,
                (false, true) => SPRINT_SPEED * fitness,
                (false, false) => player.speed,
            };
            transform.translation += direction * speed * time.delta_secs();
            moving = Some(sprinting);

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
        // Event_Jump: not crouched, falling or aiming, and 5 stamina, which it spends.
        if keyboard_input.just_pressed(KeyCode::Space) && !stood_up && !stunned && !player.crouched && !armed.aiming && body.grounded && stamina >= JUMP_STAMINA {
            // CharacterMovement's JumpZVelocity, 600 cm/s; a broken bone drops it to 400
            // (MedicalComponent `Svr_SetJumpVelocity`).
            let v = game.state.as_ref().map_or(surroundead::gameplay::medical::JUMP_VELOCITY, |s| s.effects.jump_velocity());
            body.jump = Some(v / 100.0);
            if let Some(s) = game.state.as_mut() {
                s.survival.stamina = (s.survival.stamina - JUMP_STAMINA).max(0.0);
            }
        }
        let mut next = match (body.grounded, moving, player.crouched) {
            (false, _, _) => Locomotion::Air,
            (true, Some(true), false) => Locomotion::Sprint,
            (true, Some(false), false) => Locomotion::Run,
            (true, None, false) => Locomotion::Idle,
            (true, Some(true), true) => Locomotion::CrouchSprint,
            (true, Some(false), true) => Locomotion::CrouchWalk,
            (true, None, true) => Locomotion::CrouchIdle,
        };
        // Aiming: the *_Ironsights blend spaces, by walking direction relative to the facing
        // (the model faces +Z, its right is -X).
        if armed.aiming && body.grounded {
            use Locomotion::*;
            let [idle, fwd, bwd, left, right] = if player.crouched {
                [CrouchAimIdle, CrouchAimFwd, CrouchAimBwd, CrouchAimLeft, CrouchAimRight]
            } else {
                [AimIdle, AimFwd, AimBwd, AimLeft, AimRight]
            };
            next = if raw_direction.length_squared() == 0.0 {
                idle
            } else {
                let local = transform.rotation.inverse() * aim_dir;
                if local.z.abs() >= local.x.abs() {
                    if local.z > 0.0 { fwd } else { bwd }
                } else if local.x < 0.0 {
                    right
                } else {
                    left
                }
            };
        }
        if *loco != next {
            *loco = next;
        }
    }
}

/// Third-person mouse-look camera on a spring arm (see [`CameraController`]).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn camera_controls(
    mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_scroll: Res<AccumulatedMouseScroll>,
    armed: Res<Armed>,
    terrain: Option<Res<Terrain>>,
    player_query: Query<(&Transform, &KinematicBody), (With<Player>, Without<CameraController>)>,
    mut camera_query: Query<(&mut CameraController, &mut Transform)>,
    time: Res<Time>,
    mut yaw_request: ResMut<surroundead::gameplay::combat::CameraYawRequest>,
    mut pitch_request: ResMut<surroundead::gameplay::combat::CameraPitchRequest>,
    mut recoil: ResMut<surroundead::gameplay::recoil::Recoil>,
    mut rays: surroundead::level::collision::PropRays,
    driving: Res<surroundead::gameplay::vehicles::Driving>,
    build: Res<surroundead::gameplay::building::BuildMode>,
) {
    let Ok((player_transform, body)) = player_query.single() else {
        return;
    };
    let Ok((mut controller, mut camera_transform)) = camera_query.single_mut() else {
        return;
    };

    if let Some(yaw) = yaw_request.0.take() {
        controller.yaw = yaw;
    }
    // Mouse look (IA_CameraRotation) and wheel zoom (IA_CameraScroll).
    let delta = mouse_motion.delta;
    controller.yaw -= delta.x * 0.003;
    controller.pitch = (controller.pitch + delta.y * 0.003).clamp(-50.0f32.to_radians(), 70.0f32.to_radians());
    // Recoil (degrees up and right): looking up lowers the camera, turning right lowers the yaw.
    let kick = std::mem::take(&mut recoil.pending);
    controller.yaw -= kick.y.to_radians();
    controller.pitch = (controller.pitch - kick.x.to_radians()).clamp(-50.0f32.to_radians(), 70.0f32.to_radians());
    if let Some(pitch) = pitch_request.0.take() {
        controller.pitch = pitch;
    }
    // In build mode the wheel turns the build.
    if mouse_scroll.delta.y != 0.0 && !build.on() {
        controller.distance = (controller.distance - mouse_scroll.delta.y * 0.5).clamp(1.5, 6.0);
    }
    let (arm, side) = if armed.aiming {
        (AIM_ARM, AIM_SIDE * armed.shoulder)
    } else if driving.active() {
        (controller.distance.max(surroundead::gameplay::vehicles::VEHICLE_ARM), 0.0)
    } else {
        (controller.distance, 0.0)
    };
    let ease = (12.0 * time.delta_secs()).min(1.0);
    controller.arm += (arm - controller.arm) * ease;
    controller.side += (side - controller.side) * ease;
    // Crouching shrinks the capsule the arm hangs from (CrouchSpringArm's own curve isn't read).
    controller.drop += ((STANDING_HEIGHT - body.height) - controller.drop) * ease;

    let (yaw, pitch) = (controller.yaw, controller.pitch);
    let back = Vec3::new(yaw.sin() * pitch.cos(), pitch.sin(), yaw.cos() * pitch.cos());
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    let pivot = player_transform.translation + Vec3::Y * (PIVOT_HEIGHT - controller.drop) + right * controller.side;
    // The arm's collision test: walls and props between the pivot and the camera pull it in.
    let arm = surroundead::level::collision::spring_arm(controller.arm, rays.cast(pivot, back, controller.arm + surroundead::level::collision::CAMERA_PROBE));
    let mut eye = pivot + back * arm;
    // ...and the ground.
    if let Some(h) = terrain.as_ref().and_then(|t| t.ground_under(eye)) {
        eye.y = eye.y.max(h + 0.25);
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
