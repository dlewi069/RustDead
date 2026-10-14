//! `SD_MENU_SHOTS=<dir> cargo run`: walk every menu page, save a PNG of each, then exit.
//! For eyeballing layout changes against the original without clicking through by hand.

use bevy::prelude::*;
use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};

use super::{AppState, CurrentGame, MenuScreen};

const TOUR: [(AppState, MenuScreen, &str); 48] = [
    (AppState::MainMenu, MenuScreen::Main, "01_main"),
    (AppState::MainMenu, MenuScreen::Modes, "02_modes"),
    (AppState::MainMenu, MenuScreen::QuitToDesktop, "03_quit"),
    (AppState::MainMenu, MenuScreen::Settings, "04_settings"),
    (AppState::MainMenu, MenuScreen::Saves, "05_load"),
    (AppState::MainMenu, MenuScreen::NewGame, "06_new_game"),
    (AppState::MainMenu, MenuScreen::Creator, "07_creator"),
    // Animation checks on the preview: run cycle at three moments, then sprint.
    (AppState::MainMenu, MenuScreen::Creator, "07a_run1"),
    (AppState::MainMenu, MenuScreen::Creator, "07b_run2"),
    (AppState::MainMenu, MenuScreen::Creator, "07c_run3"),
    (AppState::MainMenu, MenuScreen::Creator, "07d_sprint"),
    // Close-up of the preview's face: hair, beard, eyebrows and features on their head bones.
    (AppState::MainMenu, MenuScreen::Creator, "07e_face"),
    (AppState::InGame, MenuScreen::None, "08_in_game"),
    // Same screen again, holding W the whole time: checks walking, collision and ground.
    (AppState::InGame, MenuScreen::None, "08b_walked"),
    // Teleport beside the nearest container and hold F through its search.
    (AppState::InGame, MenuScreen::None, "08c_loot"),
    // Same screen, CRAFTING tab with the first recipe chosen; then the split dialog.
    (AppState::InGame, MenuScreen::None, "08e_crafting"),
    (AppState::InGame, MenuScreen::None, "08f_split"),
    // Inventory closed again: worn clothing on the survivor, equipped M4 on action slot 1.
    (AppState::InGame, MenuScreen::None, "08d_dressed"),
    // Rifle drawn and aimed over the shoulder at a zombie, a few shots fired; ammo box up.
    (AppState::InGame, MenuScreen::None, "08g_armed"),
    // Still drawn, not aiming, survivor turned side-on: the rifle in the right hand.
    (AppState::InGame, MenuScreen::None, "08h_drawn"),
    // Machete drawn (4) and swung: the 1H attack montage over the upper body.
    (AppState::InGame, MenuScreen::None, "08i_melee"),
    // A roamer a metre ahead: V shoves it (it reels and stops), then looking down V stomps it.
    (AppState::InGame, MenuScreen::None, "08ib_shove"),
    // Two frag grenades in the Throwable slot, a roamer 8 m ahead: G throws one at it.
    (AppState::InGame, MenuScreen::None, "08ic_throw"),
    // The same with a Molotov: its fire sets the roamer burning.
    (AppState::InGame, MenuScreen::None, "08id_molotov"),
    // A zombie hound side-on in front of the survivor (spawned like the level's hound spawners).
    (AppState::InGame, MenuScreen::None, "08m_hound"),
    // Beside the zombie shot in 08g, F opens its "Dead Zombie" loot (the AI loot table).
    (AppState::InGame, MenuScreen::None, "08n_corpse"),
    // Weather forced to a thunderstorm (rain falling around the camera), then a dust storm.
    (AppState::InGame, MenuScreen::None, "08o_rain"),
    (AppState::InGame, MenuScreen::None, "08p_dust"),
    // The clock moved to sunset, then the middle of the night.
    (AppState::InGame, MenuScreen::None, "08j_sunset"),
    (AppState::InGame, MenuScreen::None, "08k_night"),
    // Beside the nearest safe / locker with lockpicks: the minigame, turning the lock.
    (AppState::InGame, MenuScreen::None, "08l_lockpick"),
    // In front of the nearest door that opens freely, F once: it swings / slides open.
    (AppState::InGame, MenuScreen::None, "08q_door"),
    // The oil rig keypad door's note read with F, then its keypad: three digits typed (the shot),
    // then the last one opens the door.
    (AppState::InGame, MenuScreen::None, "08r_note"),
    (AppState::InGame, MenuScreen::None, "08s_keypad"),
    // At the foot of the shortest drawn ladder, W held: climbing (shot), then off the top.
    (AppState::InGame, MenuScreen::None, "08t_ladder"),
    (AppState::InGame, MenuScreen::None, "08u_ladder_top"),
    // Beside a lab generator, F: the lab lights come on.
    (AppState::InGame, MenuScreen::None, "08v_lab_power"),
    // The police station: hold F on the spawned Police Briefcase (the generator key inside), then
    // fuel the police generator, start it with the key and pull the lever: the garage gate opens.
    (AppState::InGame, MenuScreen::None, "08va_briefcase"),
    (AppState::InGame, MenuScreen::None, "08vb_garage"),
    // Back outside at the spawn, Left Ctrl and W held: crouch-walking (slow, quiet footsteps).
    (AppState::InGame, MenuScreen::None, "08w_crouch"),
    // Dropped from 8 m: a hard landing (fall damage, broken bone, stunned).
    (AppState::InGame, MenuScreen::None, "08x_fall"),
    // Midnight at the spawn, a flashlight with a battery equipped, L: the beam.
    (AppState::InGame, MenuScreen::None, "08y_flashlight"),
    // At the spawn, - : sitting (W held does nothing; the shot), then Space stands up, no jump.
    (AppState::InGame, MenuScreen::None, "08z_sit"),
    // A level up (one skill point), J: the journal's Skills page; Health Regen clicked, then
    // Lockpick+ (its parent is locked).
    (AppState::InGame, MenuScreen::None, "08za_journal"),
    (AppState::InGame, MenuScreen::Pause, "09_pause"),
    (AppState::InGame, MenuScreen::DifficultySettings, "10_difficulty"),
    (AppState::InGame, MenuScreen::Saves, "11_save"),
    (AppState::InGame, MenuScreen::Suicide, "12_suicide"),
];

pub struct AutoshotPlugin {
    pub dir: String,
}

#[derive(Resource)]
struct Tour {
    dir: String,
    step: usize,
    t: f32,
    shot: bool,
    /// Where the survivor spawned, to return there for outdoor shots.
    start: Option<Vec3>,
}

impl Plugin for AutoshotPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Tour { dir: self.dir.clone(), step: 0, t: 0.0, shot: false, start: None })
            .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default())
            // Keys it taps go in after the input update, so Update sees them just pressed.
            .add_systems(PreUpdate, tour_sit.after(bevy::input::InputSystems))
            .add_systems(Update, (tour_journal.after(tour).before(crate::gameplay::journal::toggle), keep_alive))
            .add_systems(Update, (tour, tour_combat.before(crate::gameplay::combat::CombatSet), tour_lockpick, tour_hound, tour_medical, tour_weather, tour_corpse, tour_face, tour_door.after(tour).before(crate::level::doors::interact_doors), tour_keypad.after(tour).before(crate::level::doors::interact_doors), tour_ladder.after(tour), tour_power.after(tour).before(crate::level::doors::interact_doors), tour_police.after(tour).before(crate::gameplay::interact::search_containers), tour_shove.after(tour), tour_throw.after(tour), tour_crouch.after(tour), tour_flashlight.after(tour).before(crate::gameplay::flashlight::press_l), log_fps, nan_bones, part_phase));
    }
}

fn tour(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut tour: ResMut<Tour>,
    mut game: ResMut<CurrentGame>,
    mut next_app: ResMut<NextState<AppState>>,
    mut next_screen: ResMut<NextState<MenuScreen>>,
    mut exit: MessageWriter<AppExit>,
    data: Res<super::MenuData>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut preview: ResMut<super::creator::PreviewLocomotion>,
    bodies: Query<&GlobalTransform, With<crate::models::CharacterModel>>,
    mut player: Query<&mut Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    level: Option<Res<crate::level::Level>>,
    items: Option<Res<crate::gameplay::ItemDb>>,
    mut overlay: ResMut<crate::gameplay::GameOverlay>,
    current_screen: Res<State<MenuScreen>>,
) {
    tour.t += time.delta_secs();
    // The tour stands still while zombies roam; keep its survivor alive.
    if let Some(state) = game.state.as_mut() {
        state.survival.health = state.survival.max_health;
        state.survival.bleeding = false;
    }
    if let Some((_, _, name)) = TOUR.get(tour.step) {
        preview.0 = match *name {
            "07a_run1" | "07b_run2" | "07c_run3" => crate::models::Locomotion::Run,
            "07d_sprint" => crate::models::Locomotion::Sprint,
            _ => crate::models::Locomotion::Idle,
        };
        if *name == "08b_walked" {
            keys.press(KeyCode::KeyW);
            if let Some(p) = bodies.iter().next()
                && (tour.t * 2.0).fract() < time.delta_secs() * 2.0
            {
                info!("autoshot walk {:.2} {:.2} {:.2}", p.translation().x, p.translation().y, p.translation().z);
            }
        } else if !matches!(*name, "08l_lockpick" | "08t_ladder" | "08u_ladder_top" | "08w_crouch" | "08z_sit") {
            keys.release(KeyCode::KeyW);
        }
        if *name == "08b_walked" && tour.start.is_none() {
            tour.start = bodies.iter().next().map(|g| g.translation());
        }
        if tour.t < 0.05 && !tour.shot {
            let page = match *name {
                "08e_crafting" => Some(crate::gameplay::inventory_ui::TourPage::Crafting),
                "08f_split" => Some(crate::gameplay::inventory_ui::TourPage::Split),
                "08d_dressed" => Some(crate::gameplay::inventory_ui::TourPage::Equipment),
                _ => None,
            };
            if let Some(page) = page {
                commands.queue(move |w: &mut World| crate::gameplay::inventory_ui::tour_page(w, page));
            }
        }
        if let (Some(hour), Some(state)) = (
            match *name {
                "08j_sunset" => Some(18.0),
                "08k_night" | "08y_flashlight" => Some(23.5),
                "08va_briefcase" | "08vb_garage" => Some(12.0),
                _ => None,
            },
            game.state.as_mut(),
        ) {
            state.environment.time_of_day_hours = hour;
        }
        if *name == "08d_dressed" {
            if overlay.is_open() {
                *overlay = crate::gameplay::GameOverlay::None;
            }
            if tour.t < 0.1
                && let (Some(p), Ok(mut t)) = (tour.start, player.single_mut())
            {
                t.translation = p;
            }
        }
        if *name == "08c_loot" {
            if tour.t < 0.1
                && let (Some(level), Some(items), Ok(mut t)) = (&level, &items, player.single_mut())
            {
                let here = t.translation;
                let target = level
                    .actors
                    .iter()
                    .filter(|a| items.containers.get(&a.class).is_some_and(|c| !c.lockpick && c.hold <= 4.0))
                    .min_by(|a, b| a.position().distance(here).total_cmp(&b.position().distance(here)));
                if let Some(a) = target {
                    t.translation = a.position() + Vec3::new(1.0, 0.0, 0.0);
                    info!("autoshot loot {} at {}", a.class, a.position());
                }
                // Something in every column: worn backpack + coat (their grids), pocket items, a weapon.
                let inv = crate::gameplay::player_inv(&mut game);
                if inv.equipped.is_empty() {
                    use crate::gameplay::equipment::Slot;
                    inv.equip(Slot::Backpack, items.stack("DA_SchoolBackpack", 1));
                    inv.equip(Slot::Torso, items.stack("DA_WinterCoat", 1));
                    // A rifle with a magazine of 30 attached.
                    let mut m4 = items.stack("DA_M4CQB", 1);
                    m4.attachments.push(crate::gameplay::items::Stack { rounds: 30, ..items.stack("DA_556Mag", 1) });
                    inv.equip(Slot::Primary, m4);
                    inv.equip(Slot::Melee, items.stack("DA_Machete", 1));
                    for (it, n) in [("DA_556Ammo", 60), ("DA_CannedBakedBeans", 3), ("DA_WaterBottle", 1), ("DA_Cloth", 3)] {
                        let _ = inv.insert(items, items.stack(it, n));
                    }
                    // Something turned on its side in the backpack.
                    if let Some(g) = inv.grid_mut(crate::gameplay::equipment::GridRef::Worn(Slot::Backpack, 0)) {
                        let _ = g.place_rotated(items, items.stack("DA_Machete", 1), 1, 4, true);
                    }
                }
            }
            if tour.t > 1.0 {
                keys.press(KeyCode::KeyF);
            }
        } else if *name != "08n_corpse" {
            // (08n_corpse holds F itself.)
            keys.release(KeyCode::KeyF);
        }
    }
    let Some(&(app, screen, name)) = TOUR.get(tour.step) else {
        if tour.t > 1.0 {
            exit.write(AppExit::Success);
        }
        return;
    };
    // Steps that drive the world themselves (no menu screen to switch to).
    let world_step = |n: &str| (n.starts_with("08") && n != "08_in_game") || matches!(n, "07a_run1" | "07b_run2" | "07c_run3" | "07d_sprint" | "07e_face");
    if tour.t < 0.05 && !tour.shot && TOUR.get(tour.step).is_some_and(|s| !world_step(s.2)) {
        if screen == MenuScreen::Creator && game.profile.is_none() {
            // Exercise the tinted parts: hair, beard, eyebrows and a head decal.
            let mut p = super::CharacterProfile::new(&data.creator);
            p.body[2] = 3; // Male Hair 3
            p.body[3] = 11; // Red hair
            p.body[5] = 1; // Beard 1
            p.body[6] = 3; // Blonde beard
            p.body[8] = 5; // Scar 1
            p.body[1] = 1; // Skin Color 2
            game.profile = Some(p);
        }
        if app == AppState::InGame && game.state.is_none() {
            // Zero position = new game, so the world picks one of the map's PlayerStarts.
            let mut state = crate::engine::GameState::new();
            (state.player_pos.x, state.player_pos.y, state.player_pos.z) = (0.0, 0.0, 0.0);
            game.state = Some(state);
            next_app.set(AppState::InGame);
        }
        next_screen.set(screen);
    }
    // Stray input (someone pressing Esc in the window) mustn't derail the in-game steps: they all
    // expect no menu, so put it back.
    if app == AppState::InGame && screen == MenuScreen::None && *current_screen.get() != MenuScreen::None && tour.t > 0.1 {
        warn!("autoshot: {:?} opened during {name}; closed", current_screen.get());
        next_screen.set(MenuScreen::None);
    }
    // The in-game view streams terrain and props in; give it longer before the shot.
    let settle = match name {
        "08_in_game" => 12.0,
        "08b_walked" => 6.0,
        "08c_loot" => 7.0,
        "08d_dressed" => 3.0,
        "08e_crafting" | "08f_split" => 1.5,
        "08g_armed" => 4.0,
        "08h_drawn" => 1.0,
        "08i_melee" => 0.75,
        "08za_journal" => 2.0,
        "08ib_shove" => 3.8,
        // The flash: the fuse runs out 3 s after the 0.8 s release.
        "08ic_throw" => 4.15,
        // The roamer alight: the fire starts 0.2 s after the smash (~1.6 s), sets it burning on
        // its first tick (0.8 s on), and it catches 0.1-1 s later.
        "08id_molotov" => 4.0,
        "08m_hound" => 3.0,
        "08n_corpse" => 2.0,
        "08o_rain" | "08p_dust" => 2.5,
        "08j_sunset" | "08k_night" => 1.0,
        "08l_lockpick" => 1.5,
        "08q_door" => 3.0,
        "08r_note" => 1.0,
        "08s_keypad" => 1.0,
        "08t_ladder" => 2.5,
        "08u_ladder_top" => 3.5,
        "08v_lab_power" => 2.0,
        "08va_briefcase" => 5.0,
        "08vb_garage" => 6.0,
        "08w_crouch" => 2.5,
        "08x_fall" => 2.5,
        "08y_flashlight" => 2.0,
        "08z_sit" => 1.2,
        "07a_run1" => 0.9,
        "07b_run2" => 0.25,
        "07c_run3" => 0.3,
        "07e_face" => 0.5,
        _ => 2.5,
    };
    // SD_BURST=<step>,<step>: also a frame every 0.25 s of those steps (<step>_<ms>.png), to see motion.
    // (Not on the main shot's frame: a second screenshot request that frame loses one.)
    let main_shot_now = tour.t > settle && !tour.shot;
    if !main_shot_now && std::env::var("SD_BURST").is_ok_and(|v| v.split(',').any(|s| s == name)) && (tour.t * 4.0).floor() != ((tour.t - time.delta_secs()) * 4.0).floor() {
        let path = format!("{}/{name}_{:05}.png", tour.dir, (tour.t * 1000.0) as u32);
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
    if tour.t > settle && !tour.shot {
        let path = format!("{}/{name}.png", tour.dir);
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
        tour.shot = true;
    }
    if tour.t > settle + 0.5 {
        tour.step += 1;
        tour.t = 0.0;
        tour.shot = false;
    }
}

const TOUR_ZOMBIE: &str = "game/SurrounDead/Content/PolygonFiles/PolygonCityZombies/Meshes/Characters/Normal/SK_Zombie_Hoodie_Male_01.glb";

/// 08g_armed: spawn a zombie ahead, draw the rifle (1), aim (RMB), fire five shots (LMB).
#[allow(clippy::too_many_arguments)]
fn tour_combat(
    mut commands: Commands,
    tour: Res<Tour>,
    assets: Res<AssetServer>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    camera: Query<&GlobalTransform, With<crate::gameplay::PlayerCamera>>,
    mut player_mut: Query<&mut Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    mut zombies: Query<(&mut Transform, Option<&crate::gameplay::combat::Health>), (With<crate::level::LevelZombie>, Without<crate::models::CharacterModel>)>,
    terrain: Option<Res<crate::level::Terrain>>,
    kids: Query<&Children>,
    meshes: Query<(&GlobalTransform, &ViewVisibility), With<Mesh3d>>,
    mut spawned: Local<Option<Entity>>,
    mut shots: Local<u32>,
    mut pitch: ResMut<crate::gameplay::combat::CameraPitchRequest>,
) {
    if TOUR.get(tour.step).map(|s| s.2) == Some("08i_melee") {
        // Draw on the first frame, swing at 0.3 s.
        if tour.t < 0.05 {
            keys.press(KeyCode::Digit4);
        } else {
            keys.release(KeyCode::Digit4);
        }
        if (0.3..0.35).contains(&tour.t) {
            mouse.press(MouseButton::Left);
        } else {
            mouse.release(MouseButton::Left);
        }
        return;
    }
    if TOUR.get(tour.step).map(|s| s.2) == Some("08h_drawn") {
        mouse.release(MouseButton::Right);
        mouse.release(MouseButton::Left);
        if let (Ok(cam), Ok(mut me)) = (camera.single(), player_mut.single_mut()) {
            // Right side to the camera.
            let f = cam.forward().as_vec3();
            me.rotation = Quat::from_rotation_y(f.x.atan2(f.z) - std::f32::consts::FRAC_PI_2);
        }
        return;
    }
    if TOUR.get(tour.step).map(|s| s.2) != Some("08g_armed") {
        if spawned.is_some() {
            mouse.release(MouseButton::Right);
            mouse.release(MouseButton::Left);
        }
        return;
    }
    // The camera's default pitch, every frame (a mouse moved over the window would aim elsewhere).
    pitch.0 = Some(12.0f32.to_radians());
    let (Ok(cam), Ok(me)) = (camera.single(), player_mut.single()) else { return };
    let me = *me;
    let ahead = (cam.forward().as_vec3() * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero();
    // On the crosshair's line (the camera sits over the shoulder), at the survivor's height.
    let mut at = cam.translation() + ahead * 4.5;
    at.y = terrain.as_ref().map_or(me.translation.y, |t| t.height(at.x, at.z));
    let Some(target) = *spawned else {
        *spawned = Some(commands.spawn((
            crate::models::model(&assets, TOUR_ZOMBIE),
            Transform::from_translation(at).looking_to(-ahead, Vec3::Y),
            crate::level::LevelZombie { home: at, spawner: None, model: TOUR_ZOMBIE.into(), material: None, kind: Default::default() },
            DespawnOnExit(AppState::InGame),
        )).id());
        keys.press(KeyCode::Digit1);
        return;
    };
    // Held in the crosshair so the shots are a fixed test.
    if let Ok((mut t, _)) = zombies.get_mut(target) {
        t.translation = at;
    }
    keys.release(KeyCode::Digit1);
    mouse.press(MouseButton::Right);
    // Semi-auto: one press per shot, half a second apart from 1.5 s in.
    let due = ((tour.t - 1.5) / 0.5).floor().max(-1.0) as i32 + 1;
    if (*shots as i32) < due.min(5) && mouse.pressed(MouseButton::Left) {
        mouse.release(MouseButton::Left);
        *shots += 1;
        if let Ok((_, Some(h))) = zombies.get(target) {
            let f = cam.forward().as_vec3();
            info!("autoshot shot {}: target hp {:.0}/{:.0} camera yaw {:.2}", *shots, h.hp, h.max, f.x.atan2(f.z).to_degrees());
        }
        let meshes: Vec<_> = kids.iter_descendants(target).filter_map(|e| meshes.get(e).ok()).map(|(g, v)| (g.translation(), *v)).take(3).collect();
        info!("autoshot target at {at}, {} descendants, meshes {meshes:?}", kids.iter_descendants(target).count());
    } else if (*shots as i32) < due.min(5) {
        mouse.press(MouseButton::Left);
    }
}

/// Any animated bone with a non-finite transform (blending at zero total weight gives NaN and
/// the character vanishes) fails the tour through `ran_cleanly`.
/// At each step's end, how far apart the survivor's body parts are in their clip (a part that
/// joined late, e.g. new clothes, used to run out of step with the head and hands).
#[allow(clippy::type_complexity)]
fn part_phase(
    tour: Res<Tour>,
    lib: Res<crate::models::anim::AnimLibrary>,
    root: Query<&Children, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    parts: Query<(&crate::models::anim::PartPlayer, &AnimationPlayer)>,
    game: Res<CurrentGame>,
    played: Res<crate::audio::Played>,
    mut last: Local<usize>,
) {
    if tour.step == *last {
        return;
    }
    let prev = *last;
    *last = tour.step;
    let (Some(step), Ok(kids)) = (TOUR.get(prev), root.single()) else { return };
    if step.0 != AppState::InGame {
        return;
    }
    if let Some(spread) = crate::models::anim::phase_spread(&lib, crate::models::anim::AnimSet::Survivor, kids.iter().filter_map(|e| parts.get(e).ok())) {
        info!("autoshot parts {} spread {spread:.3}", step.2);
    }
    // The sounds started so far, by cue.
    info!("autoshot sounds {} {}", step.2, played.0.iter().map(|(k, n)| format!("{k}={n}")).collect::<Vec<_>>().join(" "));
    // The passive skills' XP so far (crouched steps: Sneaking, hits survived: Toughness).
    if let Some(st) = game.state.as_ref() {
        use crate::progression::passive::Passive;
        let all: Vec<String> = Passive::ALL.iter().map(|&p| format!("{} {:.2}", p.name().replace(' ', ""), st.passive.get(p).xp)).collect();
        info!("autoshot passive {} {}", step.2, all.join(" "));
    }
}

fn nan_bones(bones: Query<(&Transform, &Name), With<bevy::animation::AnimatedBy>>, mut reported: Local<bool>) {
    if *reported {
        return;
    }
    if let Some((t, n)) = bones.iter().find(|(t, _)| !(t.translation.is_finite() && t.rotation.is_finite() && t.scale.is_finite())) {
        *reported = true;
        error!("autoshot: bone {n} has a non-finite transform {t:?}");
    }
}

/// 08n_corpse: stand beside the nearest corpse, press F, log what its loot grid holds; the
/// overlay is closed again for the next step.
#[allow(clippy::type_complexity)]
fn tour_corpse(
    tour: Res<Tour>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    corpses: Query<&Transform, (With<crate::gameplay::combat::Dead>, With<crate::level::LevelZombie>, Without<crate::models::CharacterModel>)>,
    mut player: Query<&mut Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    mut overlay: ResMut<crate::gameplay::GameOverlay>,
    containers: Res<crate::gameplay::interact::Containers>,
    (level, power): (Option<Res<crate::level::Level>>, Option<Res<crate::level::power::Power>>),
    mut was_on: Local<bool>,
    mut logged: Local<bool>,
) {
    let on = TOUR.get(tour.step).map(|s| s.2) == Some("08n_corpse");
    if !on {
        if *was_on {
            *was_on = false;
            keys.release(KeyCode::KeyF);
            *overlay = crate::gameplay::GameOverlay::None;
        }
        return;
    }
    *was_on = true;
    let Ok(mut me) = player.single_mut() else { return };
    if tour.t < 0.1
        && let Some(c) = corpses.iter().min_by(|a, b| a.translation.distance(me.translation).total_cmp(&b.translation.distance(me.translation)))
    {
        me.translation = c.translation + Vec3::new(0.8, 0.0, 0.0);
    }
    // Held from 0.4 s until the loot opens (a single short press can fall between frames).
    if tour.t >= 0.4 && tour.t < 1.5 && !matches!(*overlay, crate::gameplay::GameOverlay::Loot(_)) {
        keys.press(KeyCode::KeyF);
    } else {
        keys.release(KeyCode::KeyF);
    }
    if let crate::gameplay::GameOverlay::Loot(k) = *overlay
        && !*logged
        && let Some(g) = containers.searched.get(&k)
    {
        *logged = true;
        info!("autoshot corpse loot '{}': {} stacks (key {k})", g.name, g.items.len());
    }
    // Diagnostics for the (rare) runs where it doesn't open.
    if tour.t > 1.9 && !*logged {
        *logged = true;
        let near: Vec<f32> = corpses.iter().map(|c| c.translation.distance(me.translation)).collect();
        let listed: Vec<Vec3> = containers.list.iter().filter(|c| c.0 >= crate::gameplay::interact::CORPSE_KEYS).map(|c| c.1).collect();
        let door = level.as_ref().and_then(|l| crate::level::doors::reach_distance(&l.doors, power.as_deref(), me.translation));
        info!("autoshot corpse not opened: overlay {:?}, corpse distances {near:?}, at {}, listed {listed:?}, door/switch in reach {door:?}", *overlay, me.translation);
    }
}

/// 08m_hound also shows every medical effect icon (bleed, heavy bleed, broken bone, radiation
/// sickness); they're cleared once the step is over.
fn tour_medical(
    tour: Res<Tour>,
    mut game: ResMut<CurrentGame>,
    icons: Query<&Node, With<crate::gameplay::hud::MedicalIcon>>,
    mut was_on: Local<bool>,
    mut logged: Local<bool>,
) {
    let on = TOUR.get(tour.step).map(|s| s.2) == Some("08m_hound");
    let Some(state) = game.state.as_mut() else { return };
    if !on {
        if *was_on {
            *was_on = false;
            state.effects = Default::default();
        }
        return;
    }
    *was_on = true;
    // Kept on for the whole step: a zombie hit while bleeding would turn the bleed heavy.
    let mut rng = rand::thread_rng();
    if state.effects.bleed.is_none() {
        state.effects.add_bleed(&mut rng);
    }
    state.effects.add_heavy_bleed(&mut rng);
    state.effects.add_broken_bone(&mut rng);
    state.effects.radiation_sickness = true;
    if tour.t > 1.0 && !*logged {
        *logged = true;
        let shown = icons.iter().filter(|n| n.display != Display::None).count();
        info!("autoshot medical icons: {shown} shown");
    }
}

/// The tour's weather: clear skies (so brightness checks don't depend on a random roll), a
/// thunderstorm for 08o_rain and a dust storm for 08p_dust.
fn tour_weather(
    tour: Res<Tour>,
    mut game: ResMut<CurrentGame>,
    drops: Query<&Visibility, With<Mesh3d>>,
    weather: Res<crate::gameplay::weather::Weather>,
    mut logged: Local<Vec<&'static str>>,
) {
    let Some((_, _, name)) = TOUR.get(tour.step) else { return };
    let Some(state) = game.state.as_mut() else { return };
    let want = match *name {
        "08o_rain" => "Rain_Thunderstorm",
        "08p_dust" => "Sand_Dust_Storm",
        _ => "Clear_Skies",
    };
    let w = &mut state.weather;
    if w.current != want || w.blend < 1.0 {
        *w = crate::gameplay::weather::WeatherSave { current: want.into(), previous: want.into(), blend: 1.0, changed_on: Some(state.environment.days) };
    }
    if (*name == "08o_rain" || *name == "08p_dust") && tour.t > 2.0 && !logged.contains(name) {
        logged.push(name);
        let shown = drops.iter().filter(|v| **v != Visibility::Hidden).count();
        info!("autoshot weather {want}: rain {:.1} dust {:.1} fog {:.1}, {shown} visible meshes", weather.0.rain, weather.0.dust, weather.0.fog);
    }
}

/// 07e_face: the creator's preview camera (order -1) 0.9 m in front of the survivor's face.
fn tour_face(tour: Res<Tour>, survivor: Query<&GlobalTransform, With<crate::models::CharacterModel>>, mut cams: Query<(&Camera, &mut Transform), With<Camera3d>>) {
    if TOUR.get(tour.step).map(|s| s.2) != Some("07e_face") {
        return;
    }
    let Some(body) = survivor.iter().next() else { return };
    let (_, rot, pos) = body.to_scale_rotation_translation();
    let eyes = pos + Vec3::Y * 1.66;
    for (cam, mut t) in &mut cams {
        if cam.order == -1 {
            *t = Transform::from_translation(eyes + rot * Vec3::new(0.0, 0.0, 0.9)).looking_at(eyes, Vec3::Y);
        }
    }
}

/// 08m_hound: a hound 6 m ahead of the camera, side-on, held in place (it would chase).
#[allow(clippy::too_many_arguments)]
fn tour_hound(
    mut commands: Commands,
    tour: Res<Tour>,
    assets: Res<AssetServer>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    camera: Query<&GlobalTransform, With<crate::gameplay::PlayerCamera>>,
    mut hounds: Query<(&mut Transform, Option<&crate::gameplay::combat::Health>), With<crate::level::LevelZombie>>,
    terrain: Option<Res<crate::level::Terrain>>,
    mut spawned: Local<Option<Entity>>,
    mut logged: Local<bool>,
) {
    if TOUR.get(tour.step).map(|s| s.2) != Some("08m_hound") {
        return;
    }
    // Holster (\) so the survivor stands unarmed.
    if tour.t < 0.05 {
        keys.press(KeyCode::Backslash);
    } else {
        keys.release(KeyCode::Backslash);
    }
    let Ok(cam) = camera.single() else { return };
    let ahead = (cam.forward().as_vec3() * Vec3::new(1.0, 0.0, 1.0)).normalize_or_zero();
    let mut at = cam.translation() + ahead * 6.0;
    at.y = terrain.as_ref().map_or(at.y, |t| t.height(at.x, at.z));
    let side = Quat::from_rotation_y(ahead.x.atan2(ahead.z) + std::f32::consts::FRAC_PI_2);
    let Some(e) = *spawned else {
        const HOUND: &str = "game/SurrounDead/Content/PolygonFiles/PolygonDog/Meshes/SK_Animal_Dog_Zombie_GermanShepherd.glb";
        *spawned = Some(commands.spawn((
            crate::models::model(&assets, HOUND),
            Transform::from_translation(at).with_rotation(side),
            crate::level::LevelZombie { home: at, spawner: None, model: HOUND.into(), material: None, kind: crate::gameplay::zombies::ZombieKind::hound() },
            DespawnOnExit(AppState::InGame),
        )).id());
        return;
    };
    if let Ok((mut t, hp)) = hounds.get_mut(e) {
        t.translation = at;
        t.rotation = side;
        if let Some(h) = hp
            && !*logged
        {
            *logged = true;
            info!("autoshot hound hp {:.0}/{:.0}", h.hp, h.max);
        }
    }
}

/// 08l_lockpick: next to the nearest picklock container with three lockpicks, the minigame on a
/// Standard lock, the pick swept onto the sweet spot's edge, then W held to turn.
#[allow(clippy::too_many_arguments)]
fn tour_lockpick(
    tour: Res<Tour>,
    level: Option<Res<crate::level::Level>>,
    items: Option<Res<crate::gameplay::ItemDb>>,
    mut containers: ResMut<crate::gameplay::interact::Containers>,
    mut overlay: ResMut<crate::gameplay::GameOverlay>,
    mut game: ResMut<CurrentGame>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut player: Query<&mut Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    mut motion: ResMut<bevy::input::mouse::AccumulatedMouseMotion>,
    time: Res<Time<Real>>,
    mut swept: Local<f32>,
    mut started: Local<bool>,
) {
    if TOUR.get(tour.step).map(|s| s.2) != Some("08l_lockpick") {
        if *started {
            keys.release(KeyCode::KeyW);
            *overlay = crate::gameplay::GameOverlay::None;
            *started = false;
        }
        return;
    }
    let (Some(level), Some(items), Ok(mut t)) = (level, items, player.single_mut()) else { return };
    if !*started {
        *started = true;
        let here = t.translation;
        let Some((i, a)) = level
            .actors
            .iter()
            .enumerate()
            .filter(|(_, a)| items.containers.get(&a.class).is_some_and(|c| c.lockpick))
            .min_by(|a, b| a.1.position().distance(here).total_cmp(&b.1.position().distance(here)))
        else {
            return;
        };
        t.translation = a.position() + Vec3::new(1.0, 0.0, 0.0);
        let _ = crate::gameplay::player_inv(&mut game).insert(&items, items.stack(crate::gameplay::lockpick::LOCKPICK, 3));
        containers.locks.insert(i, crate::gameplay::lockpick::Lock { difficulty: crate::gameplay::lockpick::Difficulty::Standard, range: (100.0, 107.0) });
        *overlay = crate::gameplay::GameOverlay::Lockpick(i);
        *swept = 0.0;
        info!("autoshot lockpick {} at {}", a.class, a.position());
        return;
    }
    // Sweep the pick to 90° (5 px of mouse per degree), then turn: the lock stops at 80°.
    if *swept < 450.0 {
        let px = (900.0 * time.delta_secs()).min(450.0 - *swept);
        motion.delta.x = px;
        *swept += px;
    } else {
        keys.press(KeyCode::KeyW);
    }
}

/// 08q_door: beside the nearest door that opens without a key, facing it, F pressed once; logs
/// how far it opened and its collision triangles.
#[allow(clippy::too_many_arguments)]
fn tour_door(
    tour: Res<Tour>,
    level: Option<Res<crate::level::Level>>,
    doors: Res<crate::level::doors::Doors>,
    cache: Res<crate::level::collision::CollisionCache>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut player: Query<&mut Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    mut target: Local<Option<usize>>,
    mut pressed: Local<bool>,
    mut logged: Local<bool>,
) {
    if TOUR.get(tour.step).map(|s| s.2) != Some("08q_door") {
        *target = None;
        return;
    }
    let (Some(level), Ok(mut me)) = (level, player.single_mut()) else { return };
    let Some(i) = *target else {
        let here = me.translation;
        let Some((i, d)) = level
            .doors
            .iter()
            .enumerate()
            .filter(|(_, d)| d.opens_freely())
            .min_by(|a, b| a.1.pos.distance(here).total_cmp(&b.1.pos.distance(here)))
        else {
            return;
        };
        // On its front side, looking at it.
        me.translation = d.pos + d.front * 1.5;
        let eye_y = me.translation.y;
        me.look_at(Vec3::new(d.pos.x, eye_y, d.pos.z), Vec3::Y);
        info!("autoshot door {} at {}", level.actors[d.actor].class, d.pos);
        *target = Some(i);
        *pressed = false;
        *logged = false;
        return;
    };
    if tour.t > 0.5 && !*pressed {
        *pressed = true;
        keys.press(KeyCode::KeyF);
    }
    if tour.t > 2.9 && !*logged {
        *logged = true;
        info!("autoshot door open {:.2} tris {}", doors.open_fraction(i), cache.dynamic_tris(i));
    }
}

/// 08r_note / 08s_keypad: at the keypad door's note, F (LetterUI shows the code); then in front
/// of the door, F (KeypadUI), all but the last digit before the shot and the last after it; logs
/// the code, how far the door opened and whether it's still locked.
#[allow(clippy::too_many_arguments)]
fn tour_keypad(
    tour: Res<Tour>,
    level: Option<Res<crate::level::Level>>,
    doors: Res<crate::level::doors::Doors>,
    mut entry: ResMut<crate::gameplay::keypad::KeypadEntry>,
    mut codes: MessageWriter<crate::gameplay::keypad::KeypadCode>,
    mut overlay: ResMut<crate::gameplay::GameOverlay>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut player: Query<&mut Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    mut stage: Local<u8>,
) {
    use crate::gameplay::GameOverlay;
    let step = TOUR.get(tour.step).map(|s| s.2);
    if !matches!(step, Some("08r_note" | "08s_keypad")) {
        if *stage != 0 {
            *overlay = GameOverlay::None;
            keys.release(KeyCode::KeyF);
        }
        *stage = 0;
        return;
    }
    let (Some(level), Ok(mut me)) = (level, player.single_mut()) else { return };
    let Some((i, d)) = level.doors.iter().enumerate().find(|(_, d)| matches!(d.kind, crate::level::doors::Kind::Keypad { .. })) else {
        if *stage == 0 {
            info!("autoshot keypad: no keypad door");
            *stage = 9;
        }
        return;
    };
    let code = doors.keypad_code(i).to_string();
    match (step, *stage) {
        (Some("08r_note"), 0) => {
            if let crate::level::doors::Kind::Keypad { note: Some(n) } = d.kind {
                // On the note's side facing its door (a fixed offset put them past the rig's
                // railing, over the sea); from the note's height they drop onto the deck.
                let toward = (d.pos - n).with_y(0.0).normalize_or(Vec3::X);
                me.translation = n + toward * 1.0;
            }
            info!("autoshot keypad code {code}");
            *stage = 1;
        }
        (Some("08r_note"), 1) if tour.t > 0.3 => {
            keys.press(KeyCode::KeyF);
            *stage = 2;
        }
        (Some("08s_keypad"), s) if s < 3 => {
            *overlay = GameOverlay::None;
            keys.release(KeyCode::KeyF);
            me.translation = d.pos + d.front * 1.5;
            let eye_y = me.translation.y;
            me.look_at(Vec3::new(d.pos.x, eye_y, d.pos.z), Vec3::Y);
            *stage = 3;
        }
        (Some("08s_keypad"), 3) if tour.t > 0.3 => {
            keys.press(KeyCode::KeyF);
            *stage = 4;
        }
        (Some("08s_keypad"), 4) if tour.t > 0.6 && matches!(*overlay, GameOverlay::Keypad(_)) => {
            crate::gameplay::keypad::type_digits(&mut entry, &mut codes, &mut overlay, &code[..code.len() - 1]);
            *stage = 5;
        }
        (Some("08s_keypad"), 5) if tour.t > 1.05 => {
            crate::gameplay::keypad::type_digits(&mut entry, &mut codes, &mut overlay, &code[code.len() - 1..]);
            *stage = 6;
        }
        (Some("08s_keypad"), 6) if tour.t > 1.45 => {
            info!("autoshot keypad open {:.2} locked {}", doors.open_fraction(i), doors.locked(i));
            *stage = 7;
        }
        _ => {}
    }
}

/// 08t_ladder / 08u_ladder_top: at the foot of the shortest drawn ladder, W held throughout;
/// logs how far it rose while climbing, then whether it let go at the top.
#[allow(clippy::too_many_arguments)]
fn tour_ladder(
    tour: Res<Tour>,
    mut camera: ResMut<crate::gameplay::combat::CameraYawRequest>,
    ladders: Option<Res<crate::level::ladders::Ladders>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut player: Query<(&mut Transform, &crate::level::KinematicBody, &crate::models::Locomotion), With<crate::models::CharacterModel>>,
    terrain: Option<Res<crate::level::Terrain>>,
    mut start: Local<Option<(usize, f32)>>,
    mut logged: Local<u8>,
) {
    let step = TOUR.get(tour.step).map(|s| s.2);
    if !matches!(step, Some("08t_ladder" | "08u_ladder_top")) {
        if start.is_some() {
            keys.release(KeyCode::KeyW);
        }
        *start = None;
        *logged = 0;
        return;
    }
    let (Some(ladders), Ok((mut me, body, loco))) = (ladders, player.single_mut()) else { return };
    let Some((i, y0)) = *start else {
        // Drawn, standing on the terrain (some go down hatches), the shortest.
        let on_ground = |d: &crate::level::ladders::LadderDef| {
            let (at, _) = d.grip(d.pos.y - d.bottom / 100.0);
            terrain.as_ref().is_some_and(|t| (t.height(at.x, at.z) - at.y).abs() < 0.5)
        };
        let Some((i, d)) = ladders.defs.iter().enumerate().filter(|(_, d)| d.visible && on_ground(d)).min_by(|a, b| a.1.bottom.total_cmp(&b.1.bottom)) else {
            info!("autoshot ladder: none");
            *start = Some((usize::MAX, 0.0));
            return;
        };
        // The foot, at ground level just in front: the climb box grabs next frame.
        let (at, facing) = d.grip(d.pos.y - d.bottom / 100.0);
        let ground = terrain.as_ref().map_or(at.y, |t| t.height(at.x, at.z)).max(at.y);
        me.translation = Vec3::new(at.x, ground, at.z);
        me.rotation = facing;
        // Looking at the ladder, so W carries on over the top (it's camera-relative off the ladder).
        camera.0 = Some((-d.x.x).atan2(-d.x.z)); // W walks along -(sin, cos) of the yaw
        info!("autoshot ladder at {} height {:.2}", d.pos, d.bottom / 100.0);
        *start = Some((i, ground));
        return;
    };
    if i == usize::MAX {
        return;
    }
    if tour.t > 0.3 {
        keys.press(KeyCode::KeyW);
    }
    let top = ladders.defs[i].pos.y;
    if step == Some("08t_ladder") && tour.t > 2.4 && *logged == 0 {
        *logged = 1;
        info!("autoshot ladder climbing {} rise {:.2} loco {:?}", body.climbing, me.translation.y - y0, loco);
    }
    if step == Some("08u_ladder_top") && tour.t > 3.4 && *logged == 1 {
        *logged = 2;
        let d = &ladders.defs[i];
        info!("autoshot ladder top climbing {} above {:.2} ahead {:.2}", body.climbing, me.translation.y - top, (me.translation - d.pos).dot(d.x));
    }
}

/// Zombies and hounds roam into the later steps and can kill the survivor, which drops the
/// clothes and ends in the death pose; the tour keeps health full (damage checks read the log,
/// and one frame's hits can't take 100).
fn keep_alive(tour: Res<Tour>, mut game: ResMut<CurrentGame>) {
    if TOUR.get(tour.step).map(|s| s.0) != Some(AppState::InGame) {
        return;
    }
    if let Some(st) = game.state.as_mut()
        && st.survival.health > 0.0
        && st.survival.health < st.survival.max_health
    {
        st.survival.health = st.survival.max_health;
    }
}

/// 08za_journal: XP for one level, J at 0.3 s, Health Regen clicked at 0.8 s and Lockpick+ at
/// 1.0 s; logs the level, the points and what's unlocked.
fn tour_journal(
    mut commands: Commands,
    tour: Res<Tour>,
    mut game: ResMut<CurrentGame>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    buttons: Query<(Entity, &crate::gameplay::journal::TechButton)>,
    mut stage: Local<u8>,
) {
    if TOUR.get(tour.step).map(|s| s.2) != Some("08za_journal") {
        if *stage != 0 {
            keys.release(KeyCode::KeyJ);
        }
        *stage = 0;
        return;
    }
    let click = |commands: &mut Commands, id: &str| {
        if let Some((e, _)) = buttons.iter().find(|(_, b)| b.0 == id) {
            commands.trigger(bevy::ui_widgets::Activate { entity: e });
        }
    };
    match *stage {
        0 => {
            if let Some(st) = game.state.as_mut() {
                let need = st.progression.xp_for_next_level() - st.progression.current_xp;
                st.progression.add_xp(need);
                info!("autoshot journal level {} points {}", st.progression.level, st.progression.unspent_points);
            }
            *stage = 1;
        }
        1 if tour.t > 0.3 => {
            keys.press(KeyCode::KeyJ);
            *stage = 2;
        }
        2 => {
            keys.release(KeyCode::KeyJ);
            *stage = 3;
        }
        3 if tour.t > 0.8 => {
            info!("autoshot journal buttons {}", buttons.iter().count());
            click(&mut commands, "Skill_HealthRegen");
            *stage = 4;
        }
        4 if tour.t > 1.0 => {
            click(&mut commands, "Skill_Lockpick");
            *stage = 5;
        }
        5 if tour.t > 1.5 => {
            if let Some(st) = game.state.as_ref() {
                info!("autoshot journal points {} unlocked {:?}", st.progression.unspent_points, st.tech.unlocked);
            }
            *stage = 6;
        }
        _ => {}
    }
}

/// 08v_lab_power: next to the first lab generator, F at 0.5 s; logs whether it's on, how many lab
/// lamps are lit and how many point lights shine within 20 m.
fn tour_power(
    tour: Res<Tour>,
    power: Option<Res<crate::level::power::Power>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut player: Query<&mut Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    lamps: Query<(&PointLight, &GlobalTransform)>,
    status: Query<(&crate::level::power::StatusLamp, &GlobalTransform)>,
    mut stage: Local<u8>,
) {
    if TOUR.get(tour.step).map(|s| s.2) != Some("08v_lab_power") {
        if *stage != 0 {
            keys.release(KeyCode::KeyF);
        }
        *stage = 0;
        return;
    }
    let (Some(power), Ok(mut me)) = (power, player.single_mut()) else { return };
    let Some(g) = power.generators.first() else {
        if *stage == 0 {
            info!("autoshot power: no generator");
            *stage = 9;
        }
        return;
    };
    match *stage {
        0 => {
            me.translation = g.pos + Vec3::new(0.8, 0.0, 0.0);
            info!("autoshot power generator at {}", g.pos);
            *stage = 1;
        }
        1 if tour.t > 0.5 => {
            info!("autoshot power status before {}", status_summary(&status, me.translation));
            keys.press(KeyCode::KeyF);
            *stage = 2;
        }
        2 if tour.t > 0.6 => {
            keys.release(KeyCode::KeyF);
            *stage = 3;
        }
        3 if tour.t > 1.9 => {
            let lit = power.switches.iter().flat_map(|s| &s.lights).filter(|l| power.lit(l)).count();
            let near = lamps.iter().filter(|(l, t)| l.intensity > 0.0 && t.translation().distance(me.translation) < 20.0).count();
            info!("autoshot power on {} lit {lit} lamps {near}", g.on);
            info!("autoshot power status after {}", status_summary(&status, me.translation));
            *stage = 4;
        }
        _ => {}
    }
}

/// Status lamps within 60 m by what they show: "switch On 3 switch Warning 2 door Unlocked 1".
fn status_summary(status: &Query<(&crate::level::power::StatusLamp, &GlobalTransform)>, at: Vec3) -> String {
    let mut counts = std::collections::BTreeMap::new();
    for (l, _) in status.iter().filter(|(_, t)| t.translation().distance(at) < 60.0) {
        *counts.entry(format!("{:?} {:?}", l.kind, l.shown)).or_insert(0) += 1;
    }
    counts.iter().map(|(k, n)| format!("[{k}] {n}")).collect::<Vec<_>>().join(" ")
}

/// 08ib_shove: a roamer spawned 1 m ahead facing the survivor; V at 0.4 s (a shove), then the
/// camera pitched 60° down and V at 2.4 s (a stomp). Logs the zombie's state after each.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn tour_shove(
    mut commands: Commands,
    tour: Res<Tour>,
    assets: Res<AssetServer>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut pitch: ResMut<crate::gameplay::combat::CameraPitchRequest>,
    player: Query<&Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    zombies: Query<(&Transform, Option<&crate::gameplay::combat::Health>, Option<&crate::gameplay::zombies::Shoved>, Option<&crate::models::Locomotion>), Without<crate::models::CharacterModel>>,
    (mut spawned, mut stage): (Local<Option<Entity>>, Local<u8>),
) {
    if TOUR.get(tour.step).map(|s| s.2) != Some("08ib_shove") {
        if let Some(e) = spawned.take() {
            commands.entity(e).despawn();
        }
        *stage = 0;
        return;
    }
    let Ok(me) = player.single() else { return };
    let facing = me.rotation * Vec3::Z;
    match *stage {
        0 => {
            let at = me.translation + facing * 1.0;
            let kind = crate::gameplay::zombies::ZombieKind::from_class("BP_Zombie_Roamer_C").unwrap_or_default();
            *spawned = Some(
                commands
                    .spawn((
                        crate::models::model(&assets, TOUR_ZOMBIE),
                        Transform::from_translation(at).looking_to(-facing, Vec3::Y),
                        crate::level::LevelZombie { home: at, spawner: None, model: TOUR_ZOMBIE.into(), material: None, kind },
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id(),
            );
            *stage = 1;
        }
        1 if tour.t > 0.4 => {
            keys.press(KeyCode::KeyV);
            *stage = 2;
        }
        2 => {
            keys.release(KeyCode::KeyV);
            *stage = 3;
        }
        3 if tour.t > 1.4 => {
            if let Some((t, hp, shoved, loco)) = spawned.and_then(|e| zombies.get(e).ok()) {
                info!("autoshot shove shoved {} loco {:?} dist {:.2} hp {:.0}", shoved.is_some_and(|s| s.0 > 0.0), loco, t.translation.distance(me.translation), hp.map_or(-1.0, |h| h.hp));
            }
            pitch.0 = Some(60f32.to_radians());
            *stage = 4;
        }
        // Held looking down (stray mouse motion over the window would turn the stomp into a shove).
        4 => {
            pitch.0 = Some(60f32.to_radians());
            if tour.t > 2.4 {
                keys.press(KeyCode::KeyV);
                *stage = 5;
            }
        }
        5 => {
            pitch.0 = Some(60f32.to_radians());
            keys.release(KeyCode::KeyV);
            *stage = 6;
        }
        6 if tour.t > 3.5 => {
            if let Some((_, hp, _, _)) = spawned.and_then(|e| zombies.get(e).ok()) {
                info!("autoshot stomp hp {:.0}/{:.0}", hp.map_or(-1.0, |h| h.hp), hp.map_or(-1.0, |h| h.max));
            }
            pitch.0 = Some(12f32.to_radians());
            *stage = 7;
        }
        _ => {}
    }
}

/// 08ic_throw: two frags in the Throwable slot and a roamer 8 m ahead; G at 0.3 s; logs the
/// grenades left after the release and the zombie's health after the blast.
/// 08id_molotov: the same with two Molotovs (landing 4-6 m off, at the edge of the fire's 5 m);
/// logs the zombie's health and burning, whether the survivor caught fire and how far away the
/// fire is, then puts the fire and the survivor out.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn tour_throw(
    mut commands: Commands,
    tour: Res<Tour>,
    assets: Res<AssetServer>,
    db: Option<Res<crate::gameplay::ItemDb>>,
    mut game: ResMut<CurrentGame>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut yaw: ResMut<crate::gameplay::combat::CameraYawRequest>,
    mut pitch: ResMut<crate::gameplay::combat::CameraPitchRequest>,
    mut player: Query<&mut Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    zombies: Query<(&Transform, Option<&crate::gameplay::combat::Health>, Has<crate::gameplay::Dead>, Option<&crate::gameplay::burning::Burning>), Without<crate::models::CharacterModel>>,
    terrain: Option<Res<crate::level::Terrain>>,
    (mut player_burning, fires): (ResMut<crate::gameplay::burning::PlayerBurning>, Query<(Entity, &Transform), (With<crate::gameplay::throwables::Fire>, Without<crate::models::CharacterModel>)>),
    (mut spawned, mut stage, mut current): (Local<Option<Entity>>, Local<u8>, Local<Option<&'static str>>),
) {
    let step = TOUR.get(tour.step).map(|s| s.2);
    let item = match step {
        Some("08ic_throw") => "DA_FragGrenade",
        Some("08id_molotov") => "DA_MolotovCocktail",
        _ => "",
    };
    if item.is_empty() || *current != step {
        if let Some(e) = spawned.take() {
            commands.entity(e).despawn();
        }
        *stage = 0;
        *current = if item.is_empty() { None } else { step };
        if item.is_empty() {
            return;
        }
    }
    let (Ok(mut me_mut), Some(db)) = (player.single_mut(), db) else { return };
    let me = *me_mut;
    let facing = me.rotation * Vec3::Z;
    match *stage {
        0 => {
            let mut at = me.translation + facing * 8.0;
            at.y = terrain.as_ref().map_or(at.y, |t| t.height(at.x, at.z));
            let kind = crate::gameplay::zombies::ZombieKind::from_class("BP_Zombie_Roamer_C").unwrap_or_default();
            *spawned = Some(
                commands
                    .spawn((
                        crate::models::model(&assets, TOUR_ZOMBIE),
                        Transform::from_translation(at).looking_to(-facing, Vec3::Y),
                        crate::level::LevelZombie { home: at, spawner: None, model: TOUR_ZOMBIE.into(), material: None, kind },
                        DespawnOnExit(AppState::InGame),
                    ))
                    .id(),
            );
            crate::gameplay::player_inv(&mut game).equipped.insert(crate::gameplay::equipment::Slot::Throwable, db.stack(item, 2));
            // Look the way the survivor faces (the throw follows the camera), 20° down: the aim
            // point is a few metres off, so the throw is slow (12.5 m/s) and, bouncing and
            // sliding, comes to rest by the roamer whatever the ground ahead.
            yaw.0 = Some((-facing.x).atan2(-facing.z));
            pitch.0 = Some(20f32.to_radians());
            *stage = 1;
        }
        1 if tour.t > 0.3 => {
            keys.press(KeyCode::KeyG);
            *stage = 2;
        }
        2 => {
            keys.release(KeyCode::KeyG);
            *stage = 3;
        }
        3 if tour.t > 1.3 => {
            let left = crate::gameplay::player_inv(&mut game).equipped.get(&crate::gameplay::equipment::Slot::Throwable).map_or(0, |s| s.count);
            info!("autoshot throw grenades left {left}");
            // The frag's blast (10 m, 85-115, no fall-off) would kill a 100 hp survivor half the
            // time; with it in the air, step 8 m back out of reach, as a player would.
            if item == "DA_FragGrenade" {
                let mut back = me.translation - facing * 8.0;
                back.y = terrain.as_ref().map_or(back.y, |t| t.height(back.x, back.z));
                me_mut.translation = back;
            }
            *stage = 4;
        }
        4 if tour.t > 4.4 => {
            if let Some((_, hp, dead, burning)) = spawned.and_then(|e| zombies.get(e).ok()) {
                let hp = hp.map_or(-1.0, |h| h.hp);
                if item == "DA_FragGrenade" {
                    info!("autoshot throw zombie hp {hp:.0} dead {dead}");
                } else {
                    // How far the survivor's feet are from the fire's ground point (it reaches 5 m).
                    let dist = fires.iter().map(|(_, f)| (f.translation - Vec3::Y * 0.5).distance(me.translation)).reduce(f32::min).unwrap_or(-1.0);
                    info!("autoshot molotov zombie hp {hp:.0} dead {dead} alight {} survivor burning {} fire dist {dist:.1}", burning.is_some_and(|b| b.0.alight()), player_burning.0.is_some());
                }
            }
            // Put the fire and the survivor out, and heal them (a frag resting a few metres
            // off catches the thrower too), before the next steps.
            fires.iter().for_each(|(e, _)| commands.entity(e).despawn());
            player_burning.0 = None;
            if let Some(st) = game.state.as_mut() {
                info!("autoshot throw survivor hp {:.0} (healed)", st.survival.health);
                st.survival.health = 100.0;
            }
            *stage = 5;
        }
        _ => {}
    }
}

/// 08va_briefcase: beside the Police Briefcase, F held until it opens; logs what's inside.
/// 08vb_garage: the key moved to the survivor with a Gas Can; at the police generator, wheel to
/// "Add Gasoline" + F, wheel back + F ("Turn On"); at the lever, F. Logs each state and, 4.5 s
/// on, whether the gate is open and its first panel bone moved.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn tour_police(
    tour: Res<Tour>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut scroll: ResMut<bevy::input::mouse::AccumulatedMouseScroll>,
    mut game: ResMut<CurrentGame>,
    (spawns, power, garages, level, db): (
        Option<Res<crate::level::item_spawners::ItemSpawns>>,
        Option<Res<crate::level::power::Power>>,
        Res<crate::level::garage::Garages>,
        Option<Res<crate::level::Level>>,
        Option<Res<crate::gameplay::ItemDb>>,
    ),
    mut containers: ResMut<crate::gameplay::interact::Containers>,
    mut overlay: ResMut<crate::gameplay::GameOverlay>,
    mut player: Query<&mut Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    bones: Query<(&Name, &Transform), Without<crate::models::CharacterModel>>,
    status: Query<(&crate::level::power::StatusLamp, &GlobalTransform)>,
    (mut stage, mut at_step, mut camera): (Local<u8>, Local<usize>, ResMut<crate::gameplay::combat::CameraYawRequest>),
) {
    let step = TOUR.get(tour.step).map(|s| s.2);
    if *at_step != tour.step {
        *at_step = tour.step;
        *stage = 0;
    }
    if !matches!(step, Some("08va_briefcase" | "08vb_garage")) {
        if *stage != 0 {
            keys.release(KeyCode::KeyF);
        }
        *stage = 0;
        return;
    }
    let (Some(spawns), Some(power), Some(level), Some(db), Ok(mut me)) = (spawns, power, level, db, player.single_mut()) else { return };
    let Some(case) = spawns.0.iter().find(|c| c.class == "Container_SpecialItemSpawner_C") else {
        if *stage == 0 {
            info!("autoshot police: no briefcase");
            *stage = 99;
        }
        return;
    };
    // A one-frame F (`tour` releases it every frame of these steps).
    let tap = |keys: &mut ButtonInput<KeyCode>| {
        keys.press(KeyCode::KeyF);
        true
    };
    match (step, *stage) {
        (Some("08va_briefcase"), 0) => {
            // On the spot itself: two of its points are on a mezzanine, where a step aside falls off.
            let at = case.transform.translation;
            me.translation = at + Vec3::new(0.0, 0.05, 0.0);
            info!("autoshot police briefcase at {at}");
            *stage = 1;
        }
        (Some("08va_briefcase"), 1) if tour.t > 0.5 => {
            keys.press(KeyCode::KeyF);
            if let Some(g) = containers.searched.get(&case.key) {
                keys.release(KeyCode::KeyF);
                let items: Vec<&str> = g.items.iter().map(|p| p.stack.item.as_str()).collect();
                info!("autoshot police briefcase \"{}\" holds {items:?} key {}", g.name, items.contains(&"DA_PoliceGeneratorKey"));
                *stage = 2;
            }
        }
        (Some("08vb_garage"), 0) => {
            *overlay = crate::gameplay::GameOverlay::None;
            // Take the key, and bring a Gas Can.
            let key = containers.searched.get_mut(&case.key).and_then(|g| g.items.iter().position(|p| p.stack.item == "DA_PoliceGeneratorKey").map(|i| g.items.remove(i).stack));
            let inv = crate::gameplay::player_inv(&mut game);
            let took = key.is_some_and(|k| inv.insert(&db, k).is_none());
            let _ = inv.insert(&db, db.stack("DA_GasCan", 1));
            let Some(g) = power.power_generators.first() else {
                info!("autoshot police: no generator");
                *stage = 99;
                return;
            };
            // Clear of the generator's body (1.3 m from its centre was inside it), within its 2.6 m reach.
            me.translation = g.pos + Vec3::new(0.0, 0.05, 2.2);
            let y = me.translation.y;
            me.look_at(Vec3::new(g.pos.x, y, g.pos.z), Vec3::Y);
            info!("autoshot police key taken {took}");
            *stage = 1;
        }
        // Wheel to Option2, Add Gasoline.
        (Some("08vb_garage"), 1) if tour.t > 0.4 => {
            scroll.delta.y = 1.0;
            *stage = 2;
        }
        (Some("08vb_garage"), 2) if tour.t > 0.5 && tap(&mut keys) => *stage = 3,
        (Some("08vb_garage"), 3) if tour.t > 0.8 => {
            scroll.delta.y = 1.0;
            *stage = 4;
        }
        (Some("08vb_garage"), 4) if tour.t > 0.9 && tap(&mut keys) => *stage = 5,
        (Some("08vb_garage"), 5) if tour.t > 1.2 => {
            let g = &power.power_generators[0];
            let keys_left = crate::gameplay::player_inv(&mut game).count("DA_PoliceGeneratorKey");
            info!("autoshot police generator gas {} on {} keys left {keys_left}", g.gas, g.on);
            let Some(lever) = level.doors.iter().find(|d| d.name == "Lever") else {
                info!("autoshot police: no lever");
                *stage = 99;
                return;
            };
            // In front of the lever, facing the garage gate beyond it.
            me.translation = lever.pos + Vec3::new(0.0, 0.05, 1.0);
            let gate = garages.gates.first().map_or(lever.pos, |g| g.transform.translation);
            let y = me.translation.y;
            me.look_at(Vec3::new(gate.x, y, gate.z), Vec3::Y);
            *stage = 6;
        }
        (Some("08vb_garage"), 6) if tour.t > 1.6 && tap(&mut keys) => *stage = 7,
        // Back off 7 m in front of the gate to watch it fold away.
        (Some("08vb_garage"), 7) if tour.t > 1.8 => {
            info!("autoshot police lever pulled gate moving {}", garages.gates.first().is_some_and(|g| g.moving.is_some()));
            info!("autoshot police lever lamps {}", status_summary(&status, me.translation));
            if let Some(g) = garages.gates.first() {
                let a = &level.actors[g.actor];
                let front = Vec3::new(a.transform[0], 0.0, a.transform[2]).normalize_or_zero();
                let at = g.transform.translation;
                // Out on the forecourt, the camera turned to the gate.
                me.translation = Vec3::new(at.x, a.position().y + 0.05, at.z) + front * 7.0;
                let d = at - me.translation;
                camera.0 = Some((-d.x).atan2(-d.z));
            }
            *stage = 8;
        }
        (Some("08vb_garage"), 8) if tour.t > 5.5 => {
            let g1 = bones.iter().find(|(n, _)| n.as_str() == "g1").map(|(_, t)| t.translation.length());
            let gate = garages.gates.first();
            info!("autoshot police gate open {} blocks {} g1 moved {:.2}", gate.is_some_and(|g| g.open), gate.is_none_or(|g| g.blocks()), g1.unwrap_or(0.0));
            *stage = 9;
        }
        _ => {}
    }
}

/// 08w_crouch: at the spawn, Left Ctrl and W held from 0.3 s; logs the state, capsule height,
/// speed over a second and the footsteps heard. 08x_fall: 8 m above the spawn's ground, let go.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn tour_crouch(
    tour: Res<Tour>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut game: ResMut<CurrentGame>,
    steps: Res<crate::gameplay::footsteps::Footsteps>,
    (stun, db): (Res<crate::gameplay::survival::LandingStun>, Option<Res<crate::gameplay::ItemDb>>),
    terrain: Option<Res<crate::level::Terrain>>,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody, &crate::models::Locomotion), With<crate::models::CharacterModel>>,
    mut stage: Local<(u8, Vec3, u32)>,
) {
    let step = TOUR.get(tour.step).map(|s| s.2);
    if !matches!(step, Some("08w_crouch" | "08x_fall")) {
        if stage.0 != 0 {
            keys.release(KeyCode::ControlLeft);
            keys.release(KeyCode::KeyW);
        }
        stage.0 = 0;
        return;
    }
    let (Some(start), Ok((mut me, mut body, loco))) = (tour.start, player.single_mut()) else { return };
    let ground = |p: Vec3| terrain.as_ref().map_or(p.y, |t| t.height(p.x, p.z));
    match (step, stage.0) {
        (Some("08w_crouch"), 0) => {
            me.translation = Vec3::new(start.x, ground(start), start.z);
            body.vertical_velocity = 0.0;
            stage.0 = 1;
        }
        (Some("08w_crouch"), 1) if tour.t > 0.3 => {
            keys.press(KeyCode::ControlLeft);
            keys.press(KeyCode::KeyW);
            stage.0 = 2;
        }
        (Some("08w_crouch"), 2) if tour.t > 1.0 => {
            *stage = (3, me.translation, steps.count);
        }
        (Some("08w_crouch"), 3) if tour.t > 2.0 => {
            let speed = (me.translation - stage.1).xz().length();
            info!("autoshot crouch loco {loco:?} height {:.2} speed {speed:.2} footsteps {} range {:?}", body.height, steps.count - stage.2, steps.last_range);
            stage.0 = 4;
        }
        (Some("08x_fall"), 0..=4) => {
            keys.release(KeyCode::ControlLeft);
            keys.release(KeyCode::KeyW);
            // Wearing heavy armor rolled at 30 protection: the landing's 30 comes to 30 / 1.3.
            if let (Some(inv), Some(db)) = (game.inventory.as_mut(), db.as_ref()) {
                let mut vest = db.stack("DA_HeavyMilitaryBodyArmor", 1);
                vest.stats.insert("ArmorProtection".into(), 30.0);
                inv.equipped.insert(crate::gameplay::equipment::Slot::BodyArmor, vest);
            }
            me.translation = Vec3::new(start.x, ground(start) + 8.0, start.z);
            body.vertical_velocity = 0.0;
            body.grounded = false;
            stage.0 = 5;
        }
        (Some("08x_fall"), 5) if tour.t > 2.0 => {
            let bone = game.state.as_ref().is_some_and(|s| s.effects.broken_bone.is_some());
            info!("autoshot fall grounded {} broken bone {bone} stunned {}", body.grounded, stun.0 > 0.0);
            // Heal it and take the armor off for the remaining steps.
            if let Some(s) = game.state.as_mut() {
                s.effects.broken_bone = None;
            }
            if let Some(inv) = game.inventory.as_mut() {
                inv.equipped.remove(&crate::gameplay::equipment::Slot::BodyArmor);
            }
            stage.0 = 6;
        }
        _ => {}
    }
}

/// 08y_flashlight: at the spawn, a flashlight with a full battery in the Flashlight slot, L at
/// 0.5 s; logs whether it's on, the beam's lumens and the battery left. Taken away afterwards.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn tour_flashlight(
    tour: Res<Tour>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut game: ResMut<CurrentGame>,
    db: Option<Res<crate::gameplay::ItemDb>>,
    mut light: ResMut<crate::gameplay::flashlight::Flashlight>,
    (beam, hud): (Query<(&SpotLight, &ViewVisibility), With<crate::gameplay::flashlight::FlashlightBeam>>, Query<&InheritedVisibility, With<crate::gameplay::hud::FlashlightDur>>),
    mut player: Query<&mut Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    mut stage: Local<u8>,
) {
    use crate::gameplay::equipment::Slot;
    if TOUR.get(tour.step).map(|s| s.2) != Some("08y_flashlight") {
        if *stage != 0 {
            light.on = false;
            if let Some(inv) = game.inventory.as_mut() {
                inv.equipped.remove(&Slot::Flashlight);
            }
        }
        *stage = 0;
        return;
    }
    let Some(db) = db else { return };
    match *stage {
        0 => {
            if let (Some(p), Ok(mut t)) = (tour.start, player.single_mut()) {
                t.translation = p;
            }
            let mut f = db.stack("DA_Flashlight", 1);
            f.attachments = vec![db.stack("DA_Battery", 1)];
            if let Some(inv) = game.inventory.as_mut() {
                inv.equipped.insert(Slot::Flashlight, f);
            }
            *stage = 1;
        }
        1 if tour.t > 0.5 => {
            keys.press(KeyCode::KeyL);
            *stage = 2;
        }
        2 if tour.t > 0.6 => {
            keys.release(KeyCode::KeyL);
            *stage = 3;
        }
        3 if tour.t > 1.9 => {
            let lit = beam.iter().find(|(_, v)| v.get()).map(|(s, _)| s.intensity);
            let charge = game.inventory.as_ref().and_then(|i| i.equipped.get(&Slot::Flashlight)).and_then(|f| crate::gameplay::flashlight::battery(f, &db)).map(|b| crate::gameplay::flashlight::charge(b, &db));
            let shown = hud.iter().any(|v| v.get());
            info!("autoshot flashlight on {} beam {lit:?} battery {charge:?} hud {shown}", light.on);
            *stage = 4;
        }
        _ => {}
    }
}

/// 08z_sit: at the spawn, - at 0.2 s, W held 0.3-1.0 s; logs the state and how far W moved them,
/// then Space at 1.3 s and logs whether they stood up without jumping.
#[allow(clippy::type_complexity)]
fn tour_sit(
    tour: Res<Tour>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut player: Query<(&mut Transform, &crate::level::KinematicBody, &crate::models::Locomotion), With<crate::models::CharacterModel>>,
    mut stage: Local<(u8, Vec3)>,
) {
    if TOUR.get(tour.step).map(|s| s.2) != Some("08z_sit") {
        if stage.0 != 0 {
            keys.release(KeyCode::KeyW);
        }
        stage.0 = 0;
        return;
    }
    let Ok((mut me, body, loco)) = player.single_mut() else { return };
    // Taps last one frame.
    keys.release(KeyCode::Minus);
    keys.release(KeyCode::Space);
    match stage.0 {
        0 => {
            if let Some(p) = tour.start {
                me.translation = p;
            }
            stage.0 = 1;
        }
        1 if tour.t > 0.2 => {
            keys.press(KeyCode::Minus);
            stage.0 = 2;
        }
        2 if tour.t > 0.3 => {
            stage.1 = me.translation;
            keys.press(KeyCode::KeyW);
            stage.0 = 3;
        }
        3 if tour.t > 1.0 => {
            keys.release(KeyCode::KeyW);
            info!("autoshot sit loco {loco:?} moved {:.2}", (me.translation - stage.1).xz().length());
            stage.0 = 4;
        }
        4 if tour.t > 1.3 => {
            keys.press(KeyCode::Space);
            stage.0 = 5;
        }
        5 if tour.t > 1.6 => {
            info!("autoshot stood loco {loco:?} grounded {}", body.grounded);
            stage.0 = 6;
        }
        _ => {}
    }
}

/// Average FPS while the tour sits in the world, so map/streaming changes show their cost.
fn log_fps(diag: Res<bevy::diagnostic::DiagnosticsStore>, time: Res<Time<Real>>, mut last: Local<f32>, screen: Res<State<MenuScreen>>) {
    if *screen.get() != MenuScreen::None || time.elapsed_secs() - *last < 2.0 {
        return;
    }
    *last = time.elapsed_secs();
    if let Some(fps) = diag.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()) {
        info!("autoshot fps {fps:.1}");
    }
}
