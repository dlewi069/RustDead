//! `SD_MENU_SHOTS=<dir> cargo run`: walk every menu page, save a PNG of each, then exit.
//! For eyeballing layout changes against the original without clicking through by hand.

use bevy::prelude::*;
use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};

use super::{AppState, CurrentGame, MenuScreen};

const TOUR: [(AppState, MenuScreen, &str); 32] = [
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
            .add_systems(Update, (tour, tour_combat.before(crate::gameplay::combat::CombatSet), tour_lockpick, tour_hound, tour_medical, tour_weather, tour_corpse, tour_face, log_fps, nan_bones));
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
        } else if *name != "08l_lockpick" {
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
                "08k_night" => Some(23.5),
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
    if tour.t < 0.05 && !tour.shot && TOUR.get(tour.step).is_some_and(|s| !s.2.starts_with("08b") && !s.2.starts_with("08c") && !s.2.starts_with("08d") && !s.2.starts_with("08e") && !s.2.starts_with("08f") && !s.2.starts_with("08g") && !s.2.starts_with("08h") && !s.2.starts_with("08i") && !s.2.starts_with("08j") && !s.2.starts_with("08k") && !s.2.starts_with("08l") && !s.2.starts_with("08m") && !s.2.starts_with("08n") && !s.2.starts_with("08o") && !s.2.starts_with("08p") && !matches!(s.2, "07a_run1" | "07b_run2" | "07c_run3" | "07d_sprint" | "07e_face")) {
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
        "08m_hound" => 3.0,
        "08n_corpse" => 2.0,
        "08o_rain" | "08p_dust" => 2.5,
        "08j_sunset" | "08k_night" => 1.0,
        "08l_lockpick" => 1.5,
        "07a_run1" => 0.9,
        "07b_run2" => 0.25,
        "07c_run3" => 0.3,
        "07e_face" => 0.5,
        _ => 2.5,
    };
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
            info!("autoshot shot {}: target hp {:.0}/{:.0}", *shots, h.hp, h.max);
        }
        let meshes: Vec<_> = kids.iter_descendants(target).filter_map(|e| meshes.get(e).ok()).map(|(g, v)| (g.translation(), *v)).take(3).collect();
        info!("autoshot target at {at}, {} descendants, meshes {meshes:?}", kids.iter_descendants(target).count());
    } else if (*shots as i32) < due.min(5) {
        mouse.press(MouseButton::Left);
    }
}

/// Any animated bone with a non-finite transform (blending at zero total weight gives NaN and
/// the character vanishes) fails the tour through `ran_cleanly`.
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
