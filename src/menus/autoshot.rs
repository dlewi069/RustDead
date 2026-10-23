//! `SD_MENU_SHOTS=<dir> cargo run`: walk every menu page, save a PNG of each, then exit.
//! For eyeballing layout changes against the original without clicking through by hand.

use bevy::prelude::*;
use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};

use super::{AppState, CurrentGame, MenuScreen};

const TOUR: [(AppState, MenuScreen, &str); 79] = [
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
    // J: the journal opens on its INFO page (the watch, the license, the kill stats, days
    // survived and the distance travelled).
    (AppState::InGame, MenuScreen::None, "08za_journal_info"),
    // A level up (one skill point), J, the SKILLS tab: Health Regen clicked, then Lockpick+ (its
    // parent is locked).
    (AppState::InGame, MenuScreen::None, "08za_journal"),
    // A GPS equipped, standing at the POI nearest the spawn: discovered (XP, the banner); M opens
    // the map, zoomed to step -1 and centred on the survivor: the arrow and the POI's marker.
    (AppState::InGame, MenuScreen::None, "08zb_map"),
    // Back at the spawn: in the Main audio area, waiting for its first track; the wait skipped,
    // Main_ExplorationMusic plays.
    (AppState::InGame, MenuScreen::None, "08zc_music"),
    // Combat Roll unlocked, the survivor set burning, D held and C pressed: they turn right and
    // roll ~5 m that way, for 8 stamina, and the roll puts the fire out.
    (AppState::InGame, MenuScreen::None, "08zd_roll"),
    // Dropped into the lake south-east of the spawn (2.5 m deep): afloat, then W looking down
    // dives under (oxygen drains), Space brings them back up; the oxygen bar refilling.
    (AppState::InGame, MenuScreen::None, "08ze_swim"),
    // The 2.1 m board fence west of the spawn, faced square on and Space pressed: MQ_Climb_RM1
    // takes the survivor up and MQ_JumpingDownFromWall_RM1 down the far side, for no stamina and
    // without a jump.
    (AppState::InGame, MenuScreen::None, "08zf_vault"),
    // Back at the spawn, Huntsman unlocked: a deer 15 m ahead facing the survivor sees them and
    // runs; killed, it lies on its side, toasts "Deer Killed, XP Given", loots as "Dead Deer"
    // and leaves the Animal Meat bag.
    (AppState::InGame, MenuScreen::None, "08zg_deer"),
    // A wolf 12 m ahead facing the survivor snarls, runs at them and bites (health kept up only
    // after the first bite).
    (AppState::InGame, MenuScreen::None, "08zh_wolf"),
    // 1.5 m in front of the Supplies Trader nearest the spawn, with $200: F begins its dialogue
    // (the opening line and the two replies).
    (AppState::InGame, MenuScreen::None, "08zi_trader"),
    // "What are you selling?" opens the trade screen (Currency: $500, prices on the stock);
    // the first item is bought and a pocket item sold, each settled in Money.
    (AppState::InGame, MenuScreen::None, "08zj_trade"),
    // Back at the spawn: a Medical Airdrop crate 40 m up, 14 m ahead, sinks under its parachute
    // (camera looking up) while an AirdropAircraft passes 200 m over; "Airdrop incoming..." and
    // its map marker.
    (AppState::InGame, MenuScreen::None, "08zk_airdrop"),
    // Landed (parachute gone), the survivor holds F 2 m from it through the 6 s search: the
    // "Airdrop" loot from AirdropMedical.
    (AppState::InGame, MenuScreen::None, "08zl_airdrop_loot"),
    // The nearest standing tree to the spawn, faced from 1 m with a Fireman's Axe drawn: swings
    // until it falls (6-8 hits) for 8-12 Wooden Logs, shot while it tips over.
    (AppState::InGame, MenuScreen::None, "08zm_chop"),
    // Beside the burnt-out vehicle nearest the spawn: F starts the 10 s salvage; 12-20 Scrap Metal.
    (AppState::InGame, MenuScreen::None, "08zn_salvage"),
    // 1.5 m from the trash pile nearest the spawn with a Crowbar drawn: swings until it breaks
    // (50 Health, 2-3 swings) for 1-5 Scrap Metal.
    (AppState::InGame, MenuScreen::None, "08zo_trash"),
    // On the shore of the lake south-east of the spawn with a Fishing Rod drawn, looking down at
    // the water: a click casts (the lure bobs on the water, movement off); at the first bite a
    // click reels in a fish (Small/Raw/Large Raw Fish, Fishing XP).
    (AppState::InGame, MenuScreen::None, "08zp_fish"),
    // Beside the vehicle nearest the spawn: F gets in (seated, VehicleUI), W for 3 s drives off
    // (speed, gear, fuel burning), S brakes it to a stop, F gets out by the driver's door.
    (AppState::InGame, MenuScreen::None, "08zq_drive"),
    // A Military Intel Laptop scanned: a Classified Container quest starts ("QUEST STARTED", its
    // map marker and crate); 1.3 m from the crate (inside the 10 m goal: "QUEST UPDATED", the
    // marker goes) the survivor holds F through the 4 s search; the lid opens, a named weapon
    // lands on the ground and the quest succeeds for 400 XP.
    (AppState::InGame, MenuScreen::None, "08zr_quest"),
    // J, then the QUESTS tab: the finished Classified Container in the Quest Log folder (the Safe
    // Zone's quest shown or listed too), FORGET asks, NO keeps it.
    (AppState::InGame, MenuScreen::None, "08zs_journal_quests"),
    // By the crafting station nearest the spawn with 10 Scrap Metal: F opens the workbench's
    // crafting screen (the CraftingWorkbench recipes); Nails are crafted (5 for 5 Scrap Metal,
    // "Item Crafted, XP Given: ...").
    (AppState::InGame, MenuScreen::None, "08zt_workbench"),
    // Back at the spawn: an Undead Boss (BP_ZombieBoss_Military_Small) 8 m ahead facing the
    // survivor sees them, closes in and swings (its AttackMontage on the upper body) until the
    // arm trace lands; killed, it toasts "Zombie Boss Killed, XP Given", counts as a boss kill
    // and loots as "Undead Boss" with its unique sets.
    (AppState::InGame, MenuScreen::None, "08zu_boss"),
    // 3.5 m from the open-air barrel fire nearest the spawn: its FX_Fire is burning (the level
    // emitter within its AIO layer); back at the spawn a frag blast (FX_Explosion_New) bursts, plays out and goes,
    // and a second one is in the screenshot.
    (AppState::InGame, MenuScreen::None, "08zv_fx"),
    // The abandoned camp nearest the spawn (BP_PrefabSpawner's Prefab_AbandonedCamp), spawned if
    // its roll didn't: its meshes drawn and solid, its 5 loot crates searchable, its zombie
    // spawner added.
    (AppState::InGame, MenuScreen::None, "08zw_prefab"),
    // The bandit camp nearest the spawn, spawned: its spawning volume's bandits appear armed,
    // see the survivor 25 m off and open fire; killed, they count as human kills ("Bandit
    // Killed, XP Given") and leave lootable corpses.
    (AppState::InGame, MenuScreen::None, "08zx_bandits"),
    // The laptop's Elimination quest (Eliminate Bandits 001) begun: at the bandit camp's spot
    // the quest camp stands and its 8 bandits spawn; killing them completes it (400 XP).
    (AppState::InGame, MenuScreen::None, "08zy_elimination"),
    // Two wandering traders by the spawn: one met, talked to (it stands still) and traded with
    // (the shot), the other killed ("Wandering Trader Killed", its trade gone); the level's
    // random-chance volumes rolled and the military / Shakes & Cones traders placed.
    (AppState::InGame, MenuScreen::None, "08zz_wanderer"),
    // The settlement BP_SettlementManager picked: standing in it, "Now Entering" its safe zone
    // and the SAFE ZONE badge, its traders, well, stations and map marker there.
    (AppState::InGame, MenuScreen::None, "08zzb_settlement"),
    // A bandit turret: it finds the survivor 12 m in front and fires; held F from behind
    // disables it (a Turret Gun drops); a 100-damage round destroys another.
    (AppState::InGame, MenuScreen::None, "08zzc_turret"),
    // The Blood Moon at 23:00: red moonlight, a roused roamer running at 1.5x, the toast; then
    // sunrise settles it.
    (AppState::InGame, MenuScreen::None, "08zzd_blood_moon"),
    // A Horde Beacon placed from build mode by the spawn (the second stays in hand, then RMB
    // stops), started with F; the countdown skipped, wave 1's four horde zombies coming in.
    (AppState::InGame, MenuScreen::None, "08zze_horde"),
    // The horde killed and the beacon skipped to its last wave: "Final wave defeated!", the
    // rewards on the ground, the beacon gone.
    (AppState::InGame, MenuScreen::None, "08zzf_horde_end"),
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
            .init_resource::<WolfBitten>()
            .add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default())
            // Keys it taps go in after the input update, so Update sees them just pressed.
            .add_systems(PreUpdate, tour_sit.after(bevy::input::InputSystems))
            .add_systems(Last, steady_frames)
            .init_resource::<TourCursor>()
            .add_systems(PostUpdate, restore_cursor)
            .add_systems(Update, airdrop_notice.before(tour_airdrop))
            .add_systems(Update, throw_blockers.after(tour_throw))
            .add_systems(Update, (tour_journal.after(tour).before(crate::gameplay::journal::toggle), tour_journal_info.after(tour).before(crate::gameplay::journal::toggle), tour_map.after(tour).before(crate::gameplay::map::toggle), tour_music.after(tour), tour_roll.after(tour).before(crate::gameplay::roll::RollSet), tour_swim.after(tour), tour_vault.after(tour).before(crate::gameplay::vault::VaultSet), tour_animals.after(tour), tour_trader.after(tour).before(crate::gameplay::interact::search_containers), tour_airdrop.after(tour).before(crate::gameplay::interact::search_containers), keep_alive))
            .add_systems(Update, (tour_chop.after(tour).before(crate::gameplay::combat::CombatSet), tour_salvage.after(tour).before(crate::gameplay::interact::search_containers), tour_trash.after(tour).before(crate::gameplay::combat::CombatSet), tour_fish.after(tour).before(crate::gameplay::fishing::use_rod), tour_drive.after(tour).before(crate::gameplay::vehicles::VehicleSet), tour_quest.after(tour).before(crate::gameplay::interact::search_containers), tour_journal_quests.after(tour).before(crate::gameplay::journal::toggle), tour_workbench.after(tour).before(crate::gameplay::interact::search_containers), tour_boss.after(tour).after(tour_animals), tour_fx.after(tour), tour_prefab.after(tour), tour_bandits.after(tour), tour_elimination.after(tour), tour_wanderer.after(tour), tour_calm_humans.after(tour), tour_settlement.after(tour), tour_turret.after(tour).before(crate::gameplay::turrets::disarm), tour_blood_moon.after(tour)))
            .add_systems(Update, tour_horde.after(tour).before(crate::gameplay::building::BuildSet))
            .add_systems(Update, (tour, tour_combat.before(crate::gameplay::combat::CombatSet), tour_lockpick, tour_hound, tour_medical, tour_weather, tour_corpse, tour_face, tour_door.after(tour).before(crate::level::doors::interact_doors), tour_keypad.after(tour).before(crate::level::doors::interact_doors), tour_ladder.after(tour), tour_power.after(tour).before(crate::level::doors::interact_doors), tour_police.after(tour).before(crate::gameplay::interact::search_containers), tour_shove.after(tour).before(crate::gameplay::combat::CombatSet), tour_throw.after(tour), tour_crouch.after(tour), tour_flashlight.after(tour).before(crate::gameplay::flashlight::press_l), log_fps, nan_bones, part_phase));
    }
}

/// The tour runs vsynced, whatever VERTICAL SYNC says, so its timed steps (a shove's window, a
/// cast's notify, held keys) see the same frame pacing on every machine: uncapped at 200+ fps,
/// steps that passed at 60 failed now and then. (On the window only: the settings aren't touched.)
/// A cursor position the tour set for this frame's systems, and the real one to put back.
#[derive(Resource, Default)]
struct TourCursor(Option<Option<Vec2>>);

/// Put back the real cursor before bevy_winit's `changed_windows` (Last) sees the tour's: the
/// Window is marked changed most frames (apply_settings vs steady_frames over vsync), and it would
/// try to warp the OS cursor there, which Wayland refuses with an error.
fn restore_cursor(mut cursor: ResMut<TourCursor>, mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>) {
    if let (Some(real), Ok(mut w)) = (cursor.0.take(), windows.single_mut()) {
        w.bypass_change_detection().set_physical_cursor_position(real.map(|p| p.as_dvec2()));
    }
}

/// When the tour presses G: what `throwables::start` checks before it throws (a throw or shove
/// still running, a busy weapon), so a throw that never happens says why.
fn throw_blockers(keys: Res<ButtonInput<KeyCode>>, throwing: Res<crate::gameplay::throwables::Throwing>, shoving: Res<crate::gameplay::shove::Shoving>, armed: Res<crate::gameplay::Armed>) {
    if keys.just_pressed(KeyCode::KeyG) {
        info!("autoshot throw press throwing {} shoving {} armed busy {}", throwing.0.is_some(), shoving.0.is_some(), armed.busy());
    }
}

/// 08zk_airdrop: AIRDROP NOTIFICATION ENABLED? on for the step (it's off by default), so
/// "Airdrop incoming..." can be checked; the viewer's own value is put back after.
fn airdrop_notice(tour: Res<Tour>, mut store: ResMut<crate::menus::settings::SettingsStore>, mut saved: Local<Option<Option<crate::menus::settings::Value>>>) {
    use crate::menus::settings::{Value, row_key};
    let key = row_key("settings", "Gameplay", "AIRDROP NOTIFICATION ENABLED?");
    let on = TOUR.get(tour.step).is_some_and(|s| s.2 == "08zk_airdrop");
    match (on, saved.is_some()) {
        (true, false) => {
            *saved = Some(store.values.insert(key, Value::Bool(true)));
        }
        (false, true) => {
            match saved.take().flatten() {
                Some(v) => store.values.insert(key, v),
                None => store.values.remove(&key),
            };
        }
        _ => {}
    }
}

fn steady_frames(mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>) {
    for mut w in &mut windows {
        if w.present_mode != bevy::window::PresentMode::AutoVsync {
            w.present_mode = bevy::window::PresentMode::AutoVsync;
        }
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
    (current_screen, bitten): (Res<State<MenuScreen>>, Res<WolfBitten>),
) {
    tour.t += time.delta_secs();
    // The tour stands still while zombies roam; keep its survivor alive (08zh_wolf waits for its
    // first bite).
    let wolf_step = matches!(TOUR.get(tour.step).map(|s| s.2), Some("08zh_wolf" | "08zu_boss"));
    if let Some(state) = game.state.as_mut()
        && (!wolf_step || bitten.0)
    {
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
        } else if !matches!(*name, "08l_lockpick" | "08t_ladder" | "08u_ladder_top" | "08w_crouch" | "08z_sit" | "08ze_swim" | "08zq_drive") {
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
        } else if *name != "08n_corpse" && *name != "08zzc_turret" {
            // (08n_corpse and 08zzc_turret hold F themselves.)
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
        "08za_journal_info" => 1.5,
        "08za_journal" => 2.0,
        "08zb_map" => 7.0,
        "08zc_music" => 2.0,
        "08zd_roll" => 1.6,
        "08ze_swim" => 4.8,
        "08zf_vault" => 6.0,
        "08zg_deer" => 5.0,
        "08zh_wolf" => 6.0,
        "08zi_trader" => 6.0,
        "08zj_trade" => 2.0,
        "08zk_airdrop" => 3.5,
        "08zl_airdrop_loot" => 9.0,
        // Up to ~8 swings at the axe's pace, then 2 s into the fall.
        "08zm_chop" => 16.0,
        "08zn_salvage" => 12.0,
        // The teleport's streaming, then a few swings.
        "08zo_trash" => 9.0,
        // Streaming, the cast, a few skipped waits for a bite, the reel.
        "08zp_fish" => 6.0,
        // Streaming, getting in, 3 s of throttle, braking, getting out.
        "08zq_drive" => 10.0,
        // Streaming at the crate, the search, the lid and the drop.
        "08zr_quest" => 16.0,
        "08zs_journal_quests" => 2.5,
        // Streaming by the station, F, the craft.
        "08zt_workbench" => 6.0,
        // Streaming, the approach, swings until a hit, the kill and the corpse.
        "08zu_boss" => 9.0,
        // Streaming by the fire, the blast's 6 s smoke, the second blast.
        "08zv_fx" => 9.5,
        // The camp's cells streaming in.
        "08zw_prefab" => 7.0,
        // Streaming, the volume's 2 s delay, the bandits spotting and shooting, the kill.
        "08zx_bandits" => 12.0,
        // The goal tick, the volume's spawns (0.75 s apart), the kill, the quest's end.
        "08zy_elimination" => 13.0,
        // Meeting, talking (2 s held still), the kill, the trade screen.
        "08zz_wanderer" => 9.0,
        // Streaming the camp in, the zone's 0.2 s poll, the banner sliding in (1.5 s), the
        // Officer's lines, the settlers' 3 s delay and their spawns.
        "08zzb_settlement" => 10.0,
        // Streaming, being seen and shot (3.5 s), the 5 s hold, the destroying round.
        "08zzc_turret" => 12.0,
        // The roamer's chase (2.5 s) and the sunrise.
        "08zzd_blood_moon" => 5.0,
        // Building, placing, F, the skipped countdown and wave 1's spawns (0.5-1 s apart).
        "08zze_horde" => 7.0,
        // The 1 s after the last wave and the rewards.
        "08zzf_horde_end" => 3.5,
        "08ib_shove" => 4.5,
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
fn keep_alive(tour: Res<Tour>, mut game: ResMut<CurrentGame>, bitten: Res<WolfBitten>) {
    if TOUR.get(tour.step).map(|s| s.0) != Some(AppState::InGame) {
        return;
    }
    // The wolf and boss steps read their first hit off the health.
    if matches!(TOUR.get(tour.step).map(|s| s.2), Some("08zh_wolf" | "08zu_boss")) && !bitten.0 {
        return;
    }
    if let Some(st) = game.state.as_mut()
        && st.survival.health > 0.0
        && st.survival.health < st.survival.max_health
    {
        st.survival.health = st.survival.max_health;
    }
}

/// 08za_journal_info: J at 0.3 s opens the journal on INFO; at 1.0 s logs the page shown, the
/// stats and whether the page's texts are up; closed when the step ends.
fn tour_journal_info(
    tour: Res<Tour>,
    game: Res<CurrentGame>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut overlay: ResMut<crate::gameplay::GameOverlay>,
    journal: Res<crate::gameplay::journal::Journal>,
    texts: Query<&Text>,
    mut stage: Local<u8>,
) {
    if TOUR.get(tour.step).map(|s| s.2) != Some("08za_journal_info") {
        if *stage != 0 {
            keys.release(KeyCode::KeyJ);
            if *overlay == crate::gameplay::GameOverlay::Journal {
                *overlay = crate::gameplay::GameOverlay::None;
            }
        }
        *stage = 0;
        return;
    }
    match *stage {
        0 if tour.t > 0.3 => {
            keys.press(KeyCode::KeyJ);
            *stage = 1;
        }
        1 => {
            keys.release(KeyCode::KeyJ);
            *stage = 2;
        }
        2 if tour.t > 1.0 => {
            let has = |s: &str| texts.iter().any(|t| t.0 == s);
            let st = game.state.as_ref().map(|s| s.stats.clone()).unwrap_or_default();
            let level = game.state.as_ref().map_or(0, |s| s.progression.level);
            info!(
                "autoshot journal info page {:?} title {} license {} level_text {} zombies {} animals {} days {} distance {:.0} distance_text {}",
                journal.page,
                has("SKULL TRACKER"),
                has("SURVIVOR'S LICENSE"),
                has(&format!("LVL: {level}")),
                st.zombie_kills,
                st.animal_kills,
                st.days_survived,
                st.distance,
                has(&st.distance_text())
            );
            *stage = 3;
        }
        _ => {}
    }
}

/// 08za_journal: XP for one level, J at 0.3 s, the SKILLS tab at 0.5 s, Health Regen clicked at
/// 0.8 s and Lockpick+ at 1.0 s; logs the level, the points and what's unlocked.
#[allow(clippy::type_complexity)]
fn tour_journal(
    mut commands: Commands,
    tour: Res<Tour>,
    mut game: ResMut<CurrentGame>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    (buttons, tabs): (Query<(Entity, &crate::gameplay::journal::TechButton)>, Query<(Entity, &crate::gameplay::journal::TabButton)>),
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
            *stage = 7;
        }
        // The SKILLS tab (JournalUI starts on INFO).
        7 if tour.t > 0.5 => {
            if let Some((e, _)) = tabs.iter().find(|(_, t)| t.0 == crate::gameplay::journal::Page::Skills) {
                commands.trigger(bevy::ui_widgets::Activate { entity: e });
            }
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

/// 08zb_map: a GPS in its slot, the survivor moved to the POI nearest the spawn; waits for its
/// DistanceCheck (1-4 s), logs the XP gained and the banner, then M at 4.5 s, three wheel notches
/// out and two in, the view centred on the survivor; logs the arrow, the markers and the zoom step.
/// Then a double click (the cursor put there) 150 m ahead of the camera places a map marker;
/// logs the markers, their widgets and the marker's compass icon.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn tour_map(
    tour: Res<Tour>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut scroll: ResMut<bevy::input::mouse::AccumulatedMouseScroll>,
    mut game: ResMut<CurrentGame>,
    (db, terrain, items): (Res<crate::gameplay::map::MapDb>, Option<Res<crate::level::Terrain>>, Option<Res<crate::gameplay::ItemDb>>),
    (mut map, mut overlay): (ResMut<crate::gameplay::map::MapView>, ResMut<crate::gameplay::GameOverlay>),
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
    (ui_scale, mut mouse, mut cursor): (Res<UiScale>, ResMut<ButtonInput<MouseButton>>, ResMut<TourCursor>),
    mut player: Query<&mut Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    (arrow, markers, banners): (Query<&InheritedVisibility, With<crate::gameplay::map::PlayerArrow>>, Query<&InheritedVisibility, With<crate::gameplay::map::PoiMarker>>, Query<(), With<crate::gameplay::map::Discovered>>),
    (camera, placed, compass, picker): (
        Query<&GlobalTransform, With<crate::gameplay::PlayerCamera>>,
        Query<(), With<crate::gameplay::markers::PlayerMarkerUi>>,
        Query<(&Node, &Visibility), With<crate::gameplay::markers::CompassIcon>>,
        Query<(), With<crate::gameplay::markers::MarkerPickerUi>>,
    ),
    (mut stage, mut target, mut xp0, mut click_at): (Local<u8>, Local<Option<usize>>, Local<u64>, Local<Vec2>),
) {
    use crate::gameplay::equipment::Slot;
    if TOUR.get(tour.step).map(|s| s.2) != Some("08zb_map") {
        if *stage != 0 {
            keys.release(KeyCode::KeyM);
            // Close the map for the steps after.
            if *overlay == crate::gameplay::GameOverlay::Map {
                *overlay = crate::gameplay::GameOverlay::None;
            }
            if let Some(inv) = game.inventory.as_mut() {
                inv.equipped.remove(&Slot::Gps);
            }
        }
        *stage = 0;
        return;
    }
    let (Some(items), Some(start)) = (items, tour.start) else { return };
    match *stage {
        0 => {
            // The journal step leaves its page open.
            *overlay = crate::gameplay::GameOverlay::None;
            if let Some(inv) = game.inventory.as_mut() {
                inv.equipped.insert(Slot::Gps, items.stack("DA_GPS", 1));
            }
            let near = db.pois.iter().enumerate().min_by(|a, b| a.1.pos.distance(start).total_cmp(&b.1.pos.distance(start))).map(|(i, _)| i);
            *target = near;
            if let (Some(poi), Ok(mut t)) = (near.and_then(|i| db.pois.get(i)), player.single_mut()) {
                let ground = terrain.as_ref().map_or(poi.pos.y, |t| t.height(poi.pos.x, poi.pos.z));
                t.translation = Vec3::new(poi.pos.x, ground.max(poi.pos.y) + 0.5, poi.pos.z);
                info!("autoshot map poi {} at {:.0} m, radius {:.1} m", poi.name, poi.pos.distance(start), poi.radius);
            }
            if let Some(st) = game.state.as_mut() {
                if let Some(poi) = near.and_then(|i| db.pois.get(i)) {
                    st.pois_explored.remove(&poi.actor);
                }
                *xp0 = st.progression.total_earned_xp;
            }
            *stage = 1;
        }
        1 if tour.t > 4.4 => {
            if let (Some(st), Some(poi)) = (game.state.as_ref(), target.and_then(|i| db.pois.get(i))) {
                info!("autoshot map discovered {} xp {} banner {} level {}", st.pois_explored.contains(&poi.actor), st.progression.total_earned_xp - *xp0, banners.iter().count(), st.progression.level);
            }
            keys.press(KeyCode::KeyM);
            *stage = 2;
        }
        2 => {
            keys.release(KeyCode::KeyM);
            *stage = 3;
        }
        // Three notches out, two back in: step -1, where OnScroll draws the arrow at 2x.
        3..=7 if tour.t > 4.6 + 0.1 * (*stage - 3) as f32 => {
            scroll.unit = bevy::input::mouse::MouseScrollUnit::Line;
            scroll.delta.y = if *stage < 6 { -1.0 } else { 1.0 };
            *stage += 1;
        }
        8 if tour.t > 5.2 => {
            if let (Ok(t), Ok(w)) = (player.single(), windows.single()) {
                let at = db.data.to_map(t.translation);
                map.view.look_at(at, w.size() / ui_scale.0);
            }
            *stage = 9;
        }
        9 if tour.t > 5.8 => {
            info!(
                "autoshot map step {} scale {:.3} arrow {} markers {}",
                map.view.step,
                map.view.scale,
                arrow.iter().any(|v| v.get()),
                markers.iter().filter(|v| v.get()).count()
            );
            *stage = 10;
        }
        // The double click, 150 m ahead of the camera on the map: the cursor is put there for the
        // frames of the two presses (`TourCursor`).
        10 | 12 if tour.t > 5.9 => {
            if let (Ok(mut w), Some(cam), Ok(me)) = (windows.single_mut(), camera.iter().next(), player.single()) {
                if *stage == 10 {
                    let f = cam.forward();
                    let ahead = me.translation + Vec3::new(f.x, 0.0, f.z).normalize_or_zero() * 150.0;
                    *click_at = map.view.screen_point(db.data.to_map(ahead), w.size() / ui_scale.0);
                    info!("autoshot map double click at {:.0} for {ahead:.0}", *click_at);
                }
                cursor.0 = Some(w.physical_cursor_position());
                w.bypass_change_detection().set_cursor_position(Some(*click_at * ui_scale.0));
            }
            mouse.press(MouseButton::Left);
            *stage += 1;
        }
        11 | 13 => {
            mouse.release(MouseButton::Left);
            *stage += 1;
        }
        14 if tour.t > 6.6 => {
            let n = game.state.as_ref().map_or(0, |s| s.map_markers.len());
            let icon = compass.iter().next().map(|(n, v)| (*v != Visibility::Hidden, match n.left { Val::Px(x) => x, _ => f32::NAN }));
            info!("autoshot map markers placed {n} widgets {} picker {} compass {:?}", placed.iter().count(), picker.iter().count(), icon);
            *stage = 15;
        }
        _ => {}
    }
}

/// 01_main: logs the menu's music voice. 08zc_music: at the spawn, logs the audio area and the
/// wait (0.3 s), skips it, and 1.5 s on logs the music voices.
fn tour_music(
    tour: Res<Tour>,
    mut music: ResMut<crate::audio::music::Music>,
    areas: Res<crate::audio::music::AudioAreas>,
    voices: Query<&crate::audio::music::Voice>,
    mut player: Query<&mut Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    mut stage: Local<(usize, u8)>,
) {
    if stage.0 != tour.step {
        *stage = (tour.step, 0);
    }
    let live = |layer: crate::audio::music::Layer| voices.iter().filter(|v| v.layer() == layer && !v.dying()).map(|v| v.cue.clone()).collect::<Vec<_>>();
    match (TOUR.get(tour.step).map(|s| s.2), stage.1) {
        (Some("01_main"), 0) if tour.t > 0.5 => {
            info!("autoshot music menu {:?}", live(crate::audio::music::Layer::Menu));
            stage.1 = 1;
        }
        (Some("08zc_music"), 0) => {
            if let (Some(p), Ok(mut t)) = (tour.start, player.single_mut()) {
                t.translation = p;
            }
            stage.1 = 1;
        }
        (Some("08zc_music"), 1) if tour.t > 0.3 => {
            let d = &music.director;
            info!("autoshot music area {} waiting {:?} playing {:?}", d.area.and_then(|a| areas.areas.get(a)).map_or("none", |a| a.name.as_str()), d.waiting(), d.playing());
            stage.1 = 2;
        }
        // Any wait that comes up (a fight fading out starts one) is skipped.
        (Some("08zc_music"), 2) => {
            music.director.skip_wait();
            if tour.t > 1.8 {
                let d = &music.director;
                info!("autoshot music voices {:?} playing {:?} phase {} combat {} menu {:?}", live(crate::audio::music::Layer::Music), d.playing(), d.phase_name(), d.combat, live(crate::audio::music::Layer::Menu));
                stage.1 = 3;
            }
        }
        _ => {}
    }
}

/// 08zd_roll: at the spawn facing the camera's way, Skill_CombatRoll unlocked, full stamina and
/// burning; D held from 0.2 s, C at 0.3 s (the stamina it took read the frame after); 1.5 s in
/// logs how far and which way they went and whether the fire is out.
#[allow(clippy::type_complexity)]
fn tour_roll(
    tour: Res<Tour>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut game: ResMut<CurrentGame>,
    mut burning: ResMut<crate::gameplay::burning::PlayerBurning>,
    camera: Query<&GlobalTransform, With<crate::gameplay::PlayerCamera>>,
    mut player: Query<&mut Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    mut stage: Local<(usize, u8, Vec3, f32)>,
) {
    if TOUR.get(tour.step).map(|s| s.2) != Some("08zd_roll") {
        if stage.1 != 0 {
            keys.release(KeyCode::KeyD);
            keys.release(KeyCode::KeyC);
        }
        stage.1 = 0;
        return;
    }
    let Ok(mut t) = player.single_mut() else { return };
    match stage.1 {
        0 => {
            if let Some(p) = tour.start {
                t.translation = p;
            }
            if let Some(st) = game.state.as_mut() {
                if !st.tech.has(crate::progression::tech::COMBAT_ROLL) {
                    st.tech.unlocked.push(crate::progression::tech::COMBAT_ROLL.into());
                }
                st.survival.stamina = st.survival.max_stamina;
                stage.3 = st.survival.stamina;
            }
            burning.0 = Some(crate::gameplay::burning::Burn { delay: 0.0, left: 30.0, next_tick: 0.8, damage: 0.0 });
            stage.1 = 1;
        }
        1 if tour.t > 0.2 => {
            keys.press(KeyCode::KeyD);
            stage.2 = t.translation;
            stage.1 = 2;
        }
        2 if tour.t > 0.3 => {
            keys.press(KeyCode::KeyC);
            stage.1 = 3;
        }
        3 => {
            keys.release(KeyCode::KeyC);
            // The roll started last frame: what it cost (stamina regenerates during the roll).
            stage.3 -= game.state.as_ref().map_or(0.0, |s| s.survival.stamina);
            stage.1 = 4;
        }
        4 if tour.t > 1.5 => {
            keys.release(KeyCode::KeyD);
            let moved = (t.translation - stage.2).with_y(0.0);
            // The camera's right (D): look x up.
            let right = camera.iter().next().map(|c| (c.rotation() * Vec3::NEG_Z).with_y(0.0).normalize_or_zero().cross(Vec3::Y)).unwrap_or(Vec3::X);
            info!(
                "autoshot roll moved {:.2} along_right {:.2} facing_right {:.2} stamina_spent {:.1} burning {}",
                moved.length(),
                moved.normalize_or_zero().dot(right),
                (t.rotation * Vec3::Z).with_y(0.0).normalize_or_zero().dot(right),
                stage.3,
                burning.0.is_some()
            );
            stage.1 = 5;
        }
        _ => {}
    }
}

/// The lake south-east of the spawn: a BP_WaterBox whose top is at 12.51 m over a 10 m bed.
const LAKE: Vec3 = Vec3::new(267.0, 13.5, -32.1);

/// 08ze_swim: dropped 1 m over LAKE; 2.0 s logs the float; 2.0-3.0 s W with the camera pitched
/// 60° down (a dive), 3.0 s logs being under; 3.0-4.5 s Space (IA_FloatUpwards), 4.6 s logs
/// coming back up with the oxygen it used.
#[allow(clippy::type_complexity)]
fn tour_swim(
    tour: Res<Tour>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    swim: Res<crate::gameplay::swim::Swim>,
    mut pitch: ResMut<crate::gameplay::combat::CameraPitchRequest>,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody, &crate::models::Locomotion), With<crate::models::CharacterModel>>,
    mut stage: Local<(usize, u8)>,
) {
    if stage.0 != tour.step {
        if stage.1 != 0 {
            keys.release(KeyCode::KeyW);
            keys.release(KeyCode::Space);
        }
        *stage = (tour.step, 0);
    }
    if TOUR.get(tour.step).map(|s| s.2) != Some("08ze_swim") {
        return;
    }
    let Ok((mut t, mut body, loco)) = player.single_mut() else { return };
    let log = |what: &str, t: &Transform, loco: &crate::models::Locomotion| {
        info!(
            "autoshot swim {what} swimming {} under {} deepness {:.2} feet {:.2} surface {:?} oxygen {:.1} loco {:?}",
            swim.swimming,
            swim.underwater,
            swim.deepness,
            t.translation.y,
            swim.surface.map(|s| (s * 100.0).round() / 100.0),
            swim.oxygen,
            loco
        );
    };
    match stage.1 {
        0 => {
            t.translation = LAKE;
            body.vertical_velocity = 0.0;
            body.grounded = false;
            stage.1 = 1;
        }
        1 if tour.t > 2.0 => {
            log("afloat", &t, loco);
            keys.press(KeyCode::KeyW);
            stage.1 = 2;
        }
        2 => {
            pitch.0 = Some(60f32.to_radians());
            if tour.t > 3.0 {
                keys.release(KeyCode::KeyW);
                log("dived", &t, loco);
                keys.press(KeyCode::Space);
                stage.1 = 3;
            }
        }
        3 if tour.t > 4.5 => {
            keys.release(KeyCode::Space);
            // Level the camera again for the shot (looking down it sat in a willow).
            pitch.0 = Some(15f32.to_radians());
            stage.1 = 4;
        }
        4 if tour.t > 4.6 => {
            log("surfaced", &t, loco);
            stage.1 = 5;
        }
        _ => {}
    }
}

/// A run of SM_Prop_Fence_Wood_Straight_03 (2.1 m board fencing) 35-50 m west of the spawn: one
/// section's placement, and the step to the next (2.5 m).
const FENCE: Vec3 = Vec3::new(1482.93, 10.79, 2291.56);
const FENCE_ALONG: Vec3 = Vec3::new(-1.88, 0.0, 1.65);

/// 08zf_vault: back at the spawn, 2 s on tries points beside FENCE (both sides, every 0.5 m) for
/// one where the vault's traces find a tall, thin wall (`Kind::Climb`, not thick: climb, then
/// jump down the far side); stands there facing it
/// with full stamina and, if it's still a vault once settled, Space 0.5 s later (else tries on);
/// once the vault is over logs what it was, how far over and up it went, the stamina it took and
/// whether a jump came of it.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn tour_vault(
    tour: Res<Tour>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut game: ResMut<CurrentGame>,
    vault: Res<crate::gameplay::vault::Vault>,
    terrain: Option<Res<crate::level::Terrain>>,
    mut rays: crate::level::collision::PropRays,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), With<crate::models::CharacterModel>>,
    mut stage: Local<(usize, u8, usize, f32, Vec3, f32, Option<crate::gameplay::vault::Kind>, f32)>,
) {
    use crate::gameplay::vault::{Kind, plan};
    const WANT: Kind = Kind::Climb { thick: false };
    if stage.0 != tour.step {
        if stage.1 != 0 {
            keys.release(KeyCode::Space);
        }
        *stage = (tour.step, 0, 0, 0.0, Vec3::ZERO, 0.0, None, 0.0);
    }
    if TOUR.get(tour.step).map(|s| s.2) != Some("08zf_vault") {
        return;
    }
    let (Some(start), Some(terrain), Ok((mut t, mut body))) = (tour.start, terrain, player.single_mut()) else { return };
    match stage.1 {
        // Back at the spawn (the swim left them at the lake), and 2 s for its props to stream in.
        0 => {
            t.translation = start;
            body.vertical_velocity = 0.0;
            stage.1 = 5;
        }
        5 if tour.t > 2.0 => {
            // Points 0.45 m off the fence (a capsule radius and a bit, where walking up to it
            // stops them) every 0.5 m along two sections, either side, facing it.
            let along = FENCE_ALONG.normalize();
            let across = Vec3::new(along.z, 0.0, -along.x);
            while stage.2 < 20 {
                let i = stage.2;
                stage.2 += 1;
                let side = if i.is_multiple_of(2) { 1.0 } else { -1.0 };
                let p = FENCE + along * ((i / 2) as f32 * 0.5 - 2.25) + across * side * 0.45;
                let Some(y) = terrain.ground_under(p.with_y(p.y + 2.0)) else { continue };
                let feet = p.with_y(y);
                if let Some(pl) = plan(|o, d, m| rays.cast_hit(o, d, m), feet, -across * side, false)
                    && pl.kind == WANT
                {
                    t.translation = feet;
                    t.rotation = Quat::from_rotation_y(pl.yaw);
                    body.vertical_velocity = 0.0;
                    body.grounded = true;
                    if let Some(st) = game.state.as_mut() {
                        st.survival.stamina = st.survival.max_stamina;
                    }
                    info!("autoshot vault fence at {feet:.2} top {:.2} try {i}", pl.top - y);
                    stage.3 = tour.t;
                    stage.1 = 1;
                    return;
                }
            }
            info!("autoshot vault no fence found");
            stage.1 = 9;
        }
        1 if tour.t > stage.3 + 0.5 => {
            // Settled (pushed off the boards, onto a kerb...): still a vault from here? Else on.
            let facing = t.rotation * Vec3::Z;
            if plan(|o, d, m| rays.cast_hit(o, d, m), t.translation, facing, false).is_none_or(|pl| pl.kind != WANT) {
                info!("autoshot vault settled off the fence at {:.2}; searching on", t.translation);
                stage.1 = 5;
                return;
            }
            stage.4 = t.translation;
            stage.5 = game.state.as_ref().map_or(0.0, |s| s.survival.stamina);
            info!("autoshot vault press swimming {} climbing {} grounded {} stamina {:.0}", body.swimming, body.climbing, body.grounded, stage.5);
            keys.press(KeyCode::Space);
            stage.1 = 2;
        }
        2 => {
            keys.release(KeyCode::Space);
            stage.6 = vault.current().map(|c| c.0);
            stage.1 = 3;
        }
        3 => {
            stage.7 = stage.7.max(body.vertical_velocity);
            if !vault.active() && body.grounded {
                let moved = t.translation - stage.4;
                let fwd = (t.rotation * Vec3::Z).with_y(0.0).normalize_or_zero();
                let spent = stage.5 - game.state.as_ref().map_or(0.0, |s| s.survival.stamina);
                info!("autoshot vault kind {:?} over {:.2} rise {:.2} stamina_spent {:.1} up_speed {:.2} after {:.2}", stage.6, moved.dot(fwd), moved.y, spent, stage.7, tour.t - stage.3 - 0.5);
                stage.1 = 4;
            }
        }
        _ => {}
    }
}

/// 08zm_chop: back at the spawn with a Fireman's Axe drawn, find the nearest standing tree whose
/// trunk a swing from 1 m reaches (`CollisionCache::cast_item`), stand there facing it and click
/// every 0.25 s (the swing's cooldown paces it) until it's felled; log the hits, the logs gained
/// and whether it's gone from the props and tipping.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn tour_chop(
    tour: Res<Tour>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut game: ResMut<CurrentGame>,
    db: Res<crate::gameplay::ItemDb>,
    mut armed: ResMut<crate::gameplay::Armed>,
    harvest: Res<crate::gameplay::harvest::Harvest>,
    (level, streaming, meshes, mut cache): (Option<Res<crate::level::Level>>, Option<Res<crate::level::PropStreaming>>, Res<Assets<Mesh>>, ResMut<crate::level::collision::CollisionCache>),
    terrain: Option<Res<crate::level::Terrain>>,
    mut yaw: ResMut<crate::gameplay::combat::CameraYawRequest>,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), With<crate::models::CharacterModel>>,
    falling: Query<(), With<crate::gameplay::harvest::Falling>>,
    mut stage: Local<(usize, u8, Option<(crate::level::PropKey, Vec3, Quat)>, u32, u32, f32)>,
) {
    use crate::gameplay::equipment::Slot;
    use crate::level::Harvestable;
    if stage.0 != tour.step {
        if stage.1 != 0 {
            mouse.release(MouseButton::Left);
        }
        *stage = (tour.step, 0, None, 0, 0, 0.0);
    }
    if TOUR.get(tour.step).map(|s| s.2) != Some("08zm_chop") {
        return;
    }
    let (Some(start), Some(level), Some(streaming), Some(terrain), Ok((mut t, mut body))) = (tour.start, level, streaming, terrain, player.single_mut()) else { return };
    match stage.1 {
        0 => {
            t.translation = start;
            body.vertical_velocity = 0.0;
            let inv = crate::gameplay::player_inv(&mut game);
            inv.equip(Slot::Melee, db.stack("DA_FiremansAxe", 1));
            stage.4 = inv.count("DA_WoodenLog");
            armed.drawn = Some(Slot::Melee);
            stage.1 = 1;
        }
        1 if tour.t > 2.0 => {
            for (key, at) in level.harvestables_near(start, 60.0, Harvestable::Tree).into_iter().take(40) {
                let back = (start - at).with_y(0.0).normalize_or(Vec3::Z);
                let p = at + back;
                let Some(y) = terrain.ground_under(p.with_y(at.y + 2.0)) else { continue };
                let feet = p.with_y(y);
                let face = -back;
                if cache.cast_item(&level, &streaming, &meshes, feet + Vec3::Y * 1.2, face, 2.0).is_some_and(|(_, k)| k == key) {
                    let rot = Quat::from_rotation_y(face.x.atan2(face.z));
                    stage.2 = Some((key, feet, rot));
                    info!("autoshot chop tree {key:?} at {at:.1}, standing {feet:.1}");
                    stage.5 = tour.t;
                    stage.1 = 2;
                    return;
                }
            }
            info!("autoshot chop no tree found");
            stage.1 = 9;
        }
        2 => {
            let Some((key, feet, rot)) = stage.2 else { return };
            t.translation = feet;
            t.rotation = rot;
            body.vertical_velocity = 0.0;
            let face = rot * Vec3::Z;
            yaw.0 = Some((-face.x).atan2(-face.z));
            if let Some(st) = game.state.as_mut() {
                st.survival.stamina = st.survival.max_stamina;
            }
            if mouse.pressed(MouseButton::Left) {
                mouse.release(MouseButton::Left);
            } else if tour.t > stage.5 + 0.3 && streaming.is_removed(key) {
                let logs = crate::gameplay::player_inv(&mut game).count("DA_WoodenLog") - stage.4;
                info!("autoshot chop felled after {} clicks: hits_left {:?} logs {logs} removed {} falling {}", stage.3, harvest.hits.get(&key), streaming.is_removed(key), falling.iter().count());
                stage.1 = 3;
            } else if tour.t > stage.5 + 0.25 * (stage.3 + 1) as f32 && stage.3 < 60 {
                mouse.press(MouseButton::Left);
                stage.3 += 1;
            }
        }
        3 => mouse.release(MouseButton::Left),
        _ => {}
    }
}

/// 08zo_trash: by the trash pile nearest the spawn with a Crowbar drawn; once its cell has
/// streamed in, stand 1.5 m off it where a knee-high ray meets it first (8 headings: piles lean on
/// walls), click every 0.4 s until it's gone, then log the swings and the Scrap Metal gained.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn tour_trash(
    tour: Res<Tour>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut game: ResMut<CurrentGame>,
    db: Res<crate::gameplay::ItemDb>,
    mut armed: ResMut<crate::gameplay::Armed>,
    (level, streaming, meshes, mut cache): (Option<Res<crate::level::Level>>, Option<Res<crate::level::PropStreaming>>, Res<Assets<Mesh>>, ResMut<crate::level::collision::CollisionCache>),
    terrain: Option<Res<crate::level::Terrain>>,
    mut yaw: ResMut<crate::gameplay::combat::CameraYawRequest>,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), With<crate::models::CharacterModel>>,
    mut stage: Local<(usize, u8, Option<(crate::level::PropKey, Vec3, Quat)>, u32, u32, f32, Vec3)>,
) {
    use crate::gameplay::equipment::Slot;
    use crate::level::Harvestable;
    if stage.0 != tour.step {
        if stage.1 != 0 {
            mouse.release(MouseButton::Left);
        }
        *stage = (tour.step, 0, None, 0, 0, 0.0, Vec3::ZERO);
    }
    if TOUR.get(tour.step).map(|s| s.2) != Some("08zo_trash") {
        return;
    }
    let (Some(start), Some(level), Some(streaming), Some(terrain), Ok((mut t, mut body))) = (tour.start, level, streaming, terrain, player.single_mut()) else { return };
    if let Some((_, feet, rot)) = stage.2 {
        t.translation = feet;
        t.rotation = rot;
        body.vertical_velocity = 0.0;
        let face = rot * Vec3::Z;
        yaw.0 = Some((-face.x).atan2(-face.z));
    }
    match stage.1 {
        0 => {
            let Some(&(key, at)) = level.harvestables_near(start, 3000.0, Harvestable::Trash).first() else {
                info!("autoshot trash no pile");
                stage.1 = 9;
                return;
            };
            let back = (start - at).with_y(0.0).normalize_or(Vec3::Z);
            let p = at + back * 1.5;
            let feet = p.with_y(terrain.ground_under(p.with_y(at.y + 2.0)).unwrap_or(at.y));
            let face = -back;
            stage.2 = Some((key, feet, Quat::from_rotation_y(face.x.atan2(face.z))));
            stage.6 = at;
            let inv = crate::gameplay::player_inv(&mut game);
            inv.equip(Slot::Melee, db.stack("DA_Crowbar", 1));
            stage.4 = inv.count("DA_ScrapMetal");
            armed.drawn = Some(Slot::Melee);
            info!("autoshot trash pile {key:?} at {at:.1}, standing {feet:.1}");
            stage.1 = 1;
        }
        1 if tour.t > 1.0 => {
            let (Some((key, _, _)), at) = (stage.2, stage.6) else { return };
            for k in 0..8 {
                let dir = Quat::from_rotation_y(k as f32 * std::f32::consts::FRAC_PI_4) * Vec3::Z;
                let p = at + dir * 1.5;
                let feet = p.with_y(terrain.ground_under(p.with_y(at.y + 2.0)).unwrap_or(at.y));
                if cache.cast_item(&level, &streaming, &meshes, feet + Vec3::Y * 0.4, -dir, 2.0).is_some_and(|(_, k)| k == key) {
                    info!("autoshot trash standing {feet:.1}");
                    stage.2 = Some((key, feet, Quat::from_rotation_y((-dir.x).atan2(-dir.z))));
                    stage.5 = tour.t;
                    stage.1 = 2;
                    return;
                }
            }
            if tour.t > 6.0 {
                info!("autoshot trash pile never met");
                stage.1 = 9;
            }
        }
        2 => {
            let Some((key, _, _)) = stage.2 else { return };
            if let Some(st) = game.state.as_mut() {
                st.survival.stamina = st.survival.max_stamina;
            }
            if mouse.pressed(MouseButton::Left) {
                mouse.release(MouseButton::Left);
            } else if streaming.is_removed(key) {
                let scrap = crate::gameplay::player_inv(&mut game).count("DA_ScrapMetal") - stage.4;
                info!("autoshot trash broke after {} clicks: scrap {scrap} removed {}", stage.3, streaming.is_removed(key));
                stage.1 = 3;
            } else if tour.t > stage.5 + 0.4 * (stage.3 + 1) as f32 && stage.3 < 12 {
                mouse.press(MouseButton::Left);
                stage.3 += 1;
            }
        }
        3 => mouse.release(MouseButton::Left),
        _ => {}
    }
}

/// 08zp_fish: on the shore nearest LAKE (16 headings out from it, the first dry ground over the
/// water's 12.51 m, 0.75 m further on), facing the water with the camera 15° down and a Fishing
/// Rod drawn. 1.5 s on, a click casts; once its montage is over the waits are skipped until a
/// bite, and a click reels in;
/// logs the cast, the fish and the Fishing XP.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn tour_fish(
    tour: Res<Tour>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut game: ResMut<CurrentGame>,
    db: Res<crate::gameplay::ItemDb>,
    mut armed: ResMut<crate::gameplay::Armed>,
    mut fishing: ResMut<crate::gameplay::fishing::Fishing>,
    terrain: Option<Res<crate::level::Terrain>>,
    mut yaw: ResMut<crate::gameplay::combat::CameraYawRequest>,
    mut pitch: ResMut<crate::gameplay::combat::CameraPitchRequest>,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), With<crate::models::CharacterModel>>,
    mut stage: Local<(usize, u8, Option<(Vec3, Quat)>, f32, f32)>,
) {
    use crate::gameplay::equipment::Slot;
    use crate::progression::passive::Passive;
    const FISH: [&str; 3] = ["DA_SmallRawFish", "DA_RawFish", "DA_LargeRawFish"];
    if stage.0 != tour.step {
        if stage.1 != 0 {
            mouse.release(MouseButton::Left);
        }
        *stage = (tour.step, 0, None, 0.0, 0.0);
    }
    if TOUR.get(tour.step).map(|s| s.2) != Some("08zp_fish") {
        return;
    }
    let (Some(terrain), Ok((mut t, mut body))) = (terrain, player.single_mut()) else { return };
    if let Some((feet, rot)) = stage.2 {
        if stage.1 < 2 {
            t.translation = feet;
            body.vertical_velocity = 0.0;
        }
        t.rotation = rot;
        let face = rot * Vec3::Z;
        yaw.0 = Some((-face.x).atan2(-face.z));
        // The rod tip at the cast is 0.3-0.4 m over the bank and moves about a metre with the
        // montage's phase: at 35° down the line met the bank first now and then; 15° reaches
        // the water past it.
        pitch.0 = Some(15f32.to_radians());
    }
    let fish = |game: &mut CurrentGame| FISH.map(|f| crate::gameplay::player_inv(game).count(f));
    if mouse.pressed(MouseButton::Left) {
        mouse.release(MouseButton::Left);
        return;
    }
    match stage.1 {
        0 => {
            const WATER: f32 = 12.51;
            let shore = (0..16)
                .filter_map(|k| {
                    let dir = Quat::from_rotation_y(k as f32 * std::f32::consts::TAU / 16.0) * Vec3::Z;
                    (1..80).map(|d| LAKE + dir * d as f32).find(|p| terrain.ground_under(p.with_y(40.0)).is_some_and(|g| g > WATER + 0.2)).map(|p| (p.distance(LAKE), p + dir * 0.75, -dir))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0));
            let Some((_, p, face)) = shore else {
                info!("autoshot fish no shore");
                stage.1 = 9;
                return;
            };
            let feet = p.with_y(terrain.ground_under(p.with_y(40.0)).unwrap_or(WATER));
            stage.2 = Some((feet, Quat::from_rotation_y(face.x.atan2(face.z))));
            let inv = crate::gameplay::player_inv(&mut game);
            inv.equip(Slot::FishingRod, db.stack("DA_FishingRod", 1));
            armed.drawn = Some(Slot::FishingRod);
            stage.4 = game.state.as_ref().map_or(0.0, |s| s.passive.get(Passive::Fishing).xp);
            info!("autoshot fish standing {feet:.1} facing {face:.2}");
            stage.1 = 1;
        }
        1 if tour.t > 1.5 => {
            mouse.press(MouseButton::Left);
            stage.3 = tour.t;
            stage.1 = 2;
        }
        2 => {
            if fishing.active {
                info!("autoshot fish cast: buoy at {:.1}", fishing.buoy_at.unwrap_or_default());
                stage.1 = 3;
            } else if tour.t > stage.3 + 1.5 {
                info!("autoshot fish cast failed");
                stage.1 = 9;
            }
        }
        // (The cast montage, 1 s, has to finish first: clicks during it do nothing.)
        3 if tour.t > stage.3 + 1.2 => {
            if fishing.can_catch {
                let before = fish(&mut game);
                mouse.press(MouseButton::Left);
                stage.3 = before.iter().sum::<u32>() as f32;
                stage.1 = 4;
            } else {
                fishing.skip_wait();
            }
        }
        4 if !fishing.active || tour.t > 5.5 => {
            let now = fish(&mut game);
            let xp = game.state.as_ref().map_or(0.0, |s| s.passive.get(Passive::Fishing).xp) - stage.4;
            info!("autoshot fish caught {:?} (was {}) xp {xp:.1} still fishing {}", now, stage.3, fishing.active);
            stage.1 = 5;
        }
        _ => {}
    }
}

/// 08zq_drive: 1 m outside the driver's door of the vehicle nearest the spawn; 1.5 s on F (in),
/// 2-5 s W held, then S until stopped, F (out); logs each stage with speed, gear, RPM, fuel.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn tour_drive(
    mut commands: Commands,
    assets: Res<AssetServer>,
    terrain: Option<Res<crate::level::Terrain>>,
    dead: Query<(), With<crate::gameplay::Dead>>,
    tour: Res<Tour>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    db: Res<crate::gameplay::vehicles::VehicleDb>,
    driving: Res<crate::gameplay::vehicles::Driving>,
    vehicles: Query<(Entity, &Transform, &crate::gameplay::vehicles::Vehicle), Without<crate::models::CharacterModel>>,
    mut yaw: ResMut<crate::gameplay::combat::CameraYawRequest>,
    mut pitch: ResMut<crate::gameplay::combat::CameraPitchRequest>,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), With<crate::models::CharacterModel>>,
    mut stage: Local<(usize, u8, Option<Entity>, f32, Vec3, f32)>,
    mut roamer: Local<Option<Entity>>,
    mut top: Local<f32>,
    (mut scroll, overlay): (ResMut<bevy::input::mouse::AccumulatedMouseScroll>, Res<crate::gameplay::GameOverlay>),
) {
    use crate::gameplay::vehicles::ue;
    if stage.0 != tour.step {
        *roamer = None;
        *top = 0.0;
        if stage.1 != 0 {
            for k in [KeyCode::KeyF, KeyCode::KeyW, KeyCode::KeyS] {
                keys.release(k);
            }
        }
        *stage = (tour.step, 0, None, 0.0, Vec3::ZERO, 0.0);
    }
    if TOUR.get(tour.step).map(|s| s.2) != Some("08zq_drive") {
        return;
    }
    if keys.pressed(KeyCode::KeyF) {
        keys.release(KeyCode::KeyF);
    }
    let Ok((mut pt, mut body)) = player.single_mut() else { return };
    let log = |what: &str, v: &crate::gameplay::vehicles::Vehicle, t: &Transform, driving: bool| {
        info!("autoshot drive {what}: driving {driving} mph {:.1} gear {} rpm {:.0} fuel {:.2} health {:.1} at {:.1}", v.mph(), v.drive.gear, v.drive.rpm, v.fuel, v.health, t.translation);
    };
    match stage.1 {
        0 => {
            let Some(start) = tour.start else { return };
            let Some((e, t, v)) = vehicles.iter().min_by(|a, b| a.1.translation.distance(start).total_cmp(&b.1.translation.distance(start))) else {
                if tour.t > 2.0 {
                    info!("autoshot drive no vehicle");
                    stage.1 = 99;
                }
                return;
            };
            let Some(def) = db.classes.get(&v.class) else { return };
            stage.2 = Some(e);
            let door = t.transform_point(ue(def.exit_driver));
            pt.translation = door - Vec3::Y * 0.7;
            body.vertical_velocity = 0.0;
            // The camera behind the vehicle (its forward is +X).
            let fwd = t.rotation * Vec3::X;
            yaw.0 = Some((-fwd.x).atan2(-fwd.z));
            pitch.0 = Some(25f32.to_radians());
            info!("autoshot drive vehicle {} ({}) at {:.1}, standing {:.1}", def.name, v.class, t.translation, pt.translation);
            stage.1 = 1;
        }
        1 if tour.t > 1.5 => {
            keys.press(KeyCode::KeyF);
            stage.1 = 2;
        }
        2 => {
            let Some((_, t, v)) = stage.2.and_then(|e| vehicles.get(e).ok()) else { return };
            if driving.active() {
                log("in", v, t, true);
                // A roamer 9 m down the road, to be run over.
                let ahead = t.translation + t.rotation * Vec3::X * 9.0;
                let at = ahead.with_y(terrain.as_ref().and_then(|g| g.ground_under(ahead + Vec3::Y * 2.0)).unwrap_or(ahead.y));
                let kind = crate::gameplay::zombies::ZombieKind::from_class("BP_Zombie_Roamer_C").unwrap_or_default();
                *roamer = Some(
                    commands
                        .spawn((
                            crate::models::model(&assets, TOUR_ZOMBIE),
                            Transform::from_translation(at).looking_to(t.translation - at, Vec3::Y),
                            crate::level::LevelZombie { home: at, spawner: None, model: TOUR_ZOMBIE.into(), material: None, kind },
                            DespawnOnExit(AppState::InGame),
                        ))
                        .id(),
                );
                stage.3 = v.fuel;
                stage.4 = t.translation;
                stage.5 = tour.t;
                keys.press(KeyCode::KeyW);
                keys.press(KeyCode::KeyL);
                stage.1 = 3;
            } else if tour.t > 2.5 {
                info!("autoshot drive F didn't get in");
                stage.1 = 99;
            }
        }
        3 => {
            keys.release(KeyCode::KeyL);
            let Some((_, t, v)) = stage.2.and_then(|e| vehicles.get(e).ok()) else { return };
            if let Some((_, vt, _)) = stage.2.and_then(|e| vehicles.get(e).ok()) {
                let fwd = vt.rotation * Vec3::X;
                yaw.0 = Some((-fwd.x).atan2(-fwd.z));
            }
            *top = top.max(v.mph().abs());
            if tour.t > stage.5 + 3.0 {
                log("throttle", v, t, driving.active());
                info!("autoshot drive moved {:.1} m, top {:.1} mph, fuel used {:.2}, roamer dead {}", t.translation.distance(stage.4), *top, stage.3 - v.fuel, roamer.is_some_and(|r| dead.contains(r)));
                keys.release(KeyCode::KeyW);
                keys.press(KeyCode::KeyS);
                stage.1 = 4;
            }
        }
        4 => {
            let Some((_, t, v)) = stage.2.and_then(|e| vehicles.get(e).ok()) else { return };
            if v.mph().abs() < 0.5 || tour.t > stage.5 + 6.5 {
                keys.release(KeyCode::KeyS);
                log("stopped", v, t, driving.active());
                stage.1 = 5;
            }
        }
        5 => {
            keys.press(KeyCode::KeyF);
            stage.1 = 6;
        }
        6 => {
            let Some((_, t, v)) = stage.2.and_then(|e| vehicles.get(e).ok()) else { return };
            info!("autoshot drive out: driving {} player {:.1} from the vehicle {:.1} m lights {}", driving.active(), pt.translation, pt.translation.distance(t.translation), v.lights_on);
            stage.5 = tour.t;
            stage.1 = 7;
        }
        // The wheel to Open Storage, F.
        7 if tour.t > stage.5 + 0.3 => {
            scroll.delta.y = 1.0;
            stage.1 = 8;
        }
        8 if tour.t > stage.5 + 0.5 => {
            keys.press(KeyCode::KeyF);
            stage.1 = 9;
        }
        9 if tour.t > stage.5 + 0.9 => {
            info!("autoshot drive storage: overlay {:?}", *overlay);
            stage.1 = 10;
        }
        _ => {}
    }
}

/// 08zn_salvage: 1.5 m beside the burnt-out vehicle nearest the spawn, tap F; 10.5 s later log
/// the Scrap Metal gained and a second F's toast.
#[allow(clippy::too_many_arguments)]
fn tour_salvage(
    tour: Res<Tour>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut game: ResMut<CurrentGame>,
    level: Option<Res<crate::level::Level>>,
    terrain: Option<Res<crate::level::Terrain>>,
    harvest: Res<crate::gameplay::harvest::Harvest>,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), With<crate::models::CharacterModel>>,
    mut armed: ResMut<crate::gameplay::Armed>,
    mut stage: Local<(usize, u8, u32, f32)>,
) {
    if stage.0 != tour.step {
        if stage.1 != 0 {
            keys.release(KeyCode::KeyF);
        }
        *stage = (tour.step, 0, 0, 0.0);
    }
    if TOUR.get(tour.step).map(|s| s.2) != Some("08zn_salvage") {
        return;
    }
    let (Some(start), Some(level), Some(terrain), Ok((mut t, mut body))) = (tour.start, level, terrain, player.single_mut()) else { return };
    match stage.1 {
        0 => {
            armed.drawn = None;
            let Some((_, at)) = level.actors.iter().filter(|a| a.class == "BP_HarvestableObject_ScrapMetal_C").map(|a| (a.yaw(), a.position())).min_by(|a, b| a.1.distance(start).total_cmp(&b.1.distance(start))) else {
                info!("autoshot salvage no wreck");
                stage.1 = 9;
                return;
            };
            // Beside it, on the side towards the spawn.
            let p = at + (start - at).with_y(0.0).normalize_or(Vec3::Z) * 1.5;
            t.translation = p.with_y(terrain.ground_under(p.with_y(at.y + 3.0)).unwrap_or(at.y));
            body.vertical_velocity = 0.0;
            stage.2 = crate::gameplay::player_inv(&mut game).count("DA_ScrapMetal");
            info!("autoshot salvage wreck at {at:.1}");
            stage.1 = 1;
        }
        1 if tour.t > 1.0 => {
            keys.press(KeyCode::KeyF);
            stage.3 = tour.t;
            stage.1 = 2;
        }
        2 => {
            keys.release(KeyCode::KeyF);
            if tour.t > stage.3 + 5.0 {
                info!("autoshot salvage midway {:?}", harvest.salvaging.map(|s| s.1 as u32));
                stage.1 = 3;
            }
        }
        3 if tour.t > stage.3 + 10.5 => {
            let scrap = crate::gameplay::player_inv(&mut game).count("DA_ScrapMetal") - stage.2;
            info!("autoshot salvage done: scrap {scrap} salvaged {}", harvest.salvaged.len());
            keys.press(KeyCode::KeyF);
            stage.1 = 4;
        }
        4 => {
            keys.release(KeyCode::KeyF);
            stage.1 = 5;
        }
        _ => {}
    }
}

/// 08zi_trader / 08zj_trade: stand before the Supplies Trader nearest the spawn, camera behind and 30° round,
/// with $200; 3.5 s on press F, and before the shot log the overlay it opened. Then click the dialogue's trade
/// reply (its Activate), log the screen, buy the trader's first item and 0.5 s later sell a
/// pocket item (`inventory_ui::tour_trade` logs both). The trade screen closes after the step.
#[allow(clippy::too_many_arguments)]
fn tour_trader(
    mut commands: Commands,
    tour: Res<Tour>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut game: ResMut<CurrentGame>,
    traders: Res<crate::gameplay::traders::Traders>,
    items: Option<Res<crate::gameplay::ItemDb>>,
    mut overlay: ResMut<crate::gameplay::GameOverlay>,
    replies: Query<(Entity, &crate::gameplay::traders::ReplyButton)>,
    mut yaw: ResMut<crate::gameplay::combat::CameraYawRequest>,
    mut pitch: ResMut<crate::gameplay::combat::CameraPitchRequest>,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), With<crate::models::CharacterModel>>,
    mut stage: Local<(usize, u8, f32)>,
    mut spot: Local<Option<Transform>>,
) {
    use crate::gameplay::GameOverlay;
    let step = TOUR.get(tour.step).map(|s| s.2);
    if stage.0 != tour.step {
        if TOUR.get(stage.0).map(|s| s.2) == Some("08zj_trade") && matches!(*overlay, GameOverlay::Loot(k) | GameOverlay::Dialogue(k) if crate::gameplay::traders::is_trader(k)) {
            *overlay = GameOverlay::None;
        }
        *stage = (tour.step, 0, 0.0);
    }
    match (step, stage.1) {
        (Some("08zi_trader"), 0) => {
            let (Some(items), Ok((mut t, mut body))) = (items, player.single_mut()) else { return };
            let from = tour.start.unwrap_or(t.translation);
            let Some(trader) = traders.list.iter().filter(|tr| tr.class == "BP_SuppliesTrader_C").min_by(|a, b| a.transform.translation.distance(from).total_cmp(&b.transform.translation.distance(from))) else { return };
            let at = trader.transform.translation;
            let front = (trader.transform.rotation * Vec3::X).with_y(0.0).normalize_or_zero();
            t.translation = at + front * 1.5 + Vec3::Y * 0.1;
            let d = at - t.translation;
            t.rotation = Quat::from_rotation_y(d.x.atan2(d.z));
            body.vertical_velocity = 0.0;
            // The camera behind the survivor and 30° round, so the survivor doesn't hide the trader.
            let back = Quat::from_rotation_y(30f32.to_radians()) * -d.with_y(0.0).normalize_or_zero();
            yaw.0 = Some(back.x.atan2(back.z));
            pitch.0 = Some(0.12);
            let _ = crate::gameplay::player_inv(&mut game).insert(&items, items.stack("DA_Money", 200));
            *spot = Some(*t);
            info!("autoshot trader {} at {at:.1}", trader.class);
            // A key still down counts no new press: F starts up.
            keys.release(KeyCode::KeyF);
            stage.1 = 1;
        }
        // Held on the spot until F: once the survivor ended up 9 m off it before F (cause not found;
        // the hounds and the wolf roam here), and the dialogue never opened.
        (Some("08zi_trader"), 1) if tour.t <= 3.5 => {
            if let (Some(s), Ok((mut t, mut body))) = (*spot, player.single_mut()) {
                *t = s;
                body.vertical_velocity = 0.0;
            }
        }
        (Some("08zi_trader"), 1) => {
            if let Ok((t, _)) = player.single() {
                info!("autoshot trader F at {:.2}", t.translation);
            }
            keys.press(KeyCode::KeyF);
            stage.1 = 2;
        }
        (Some("08zi_trader"), 2) if tour.t > 5.5 => {
            keys.release(KeyCode::KeyF);
            info!("autoshot trader overlay {:?} replies {}", *overlay, replies.iter().count());
            stage.1 = 3;
        }
        (Some("08zj_trade"), 0) => {
            if let Some((e, _)) = replies.iter().find(|(_, r)| r.trade) {
                commands.trigger(bevy::ui_widgets::Activate { entity: e });
            }
            stage.1 = 1;
        }
        (Some("08zj_trade"), 1) => {
            info!("autoshot trade screen {:?}", *overlay);
            commands.queue(|w: &mut World| crate::gameplay::inventory_ui::tour_trade(w, false));
            stage.1 = 2;
            stage.2 = tour.t;
        }
        (Some("08zj_trade"), 2) if tour.t > stage.2 + 0.5 => {
            commands.queue(|w: &mut World| crate::gameplay::inventory_ui::tour_trade(w, true));
            stage.1 = 3;
        }
        _ => {}
    }
}

/// 08zk_airdrop / 08zl_airdrop_loot: at the spawn, a Medical Airdrop crate 40 m over a point 14 m
/// ahead and an aircraft (no crates) 200 m up heading for the region; at 3.2 s logs the crate's
/// height and fall speed, the aircraft's speed and heading, the toasts and the map marker. Then
/// waits for the landing, stands 2 m from the crate's centre facing it, holds F and logs what opened.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn tour_airdrop(
    mut commands: Commands,
    tour: Res<Tour>,
    assets: Res<AssetServer>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut drops: ResMut<crate::gameplay::airdrops::Airdrops>,
    markers: Res<crate::gameplay::map::WorldMarkers>,
    containers: Res<crate::gameplay::interact::Containers>,
    mut overlay: ResMut<crate::gameplay::GameOverlay>,
    terrain: Option<Res<crate::level::Terrain>>,
    mut notes: MessageReader<crate::gameplay::notify::Notify>,
    mut pitch: ResMut<crate::gameplay::combat::CameraPitchRequest>,
    mut yaw: ResMut<crate::gameplay::combat::CameraYawRequest>,
    crates: Query<(&Transform, &crate::gameplay::airdrops::AirCrate)>,
    planes: Query<&Transform, (With<crate::gameplay::airdrops::Aircraft>, Without<crate::gameplay::airdrops::AirCrate>)>,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), (With<crate::models::CharacterModel>, Without<crate::gameplay::airdrops::AirCrate>, Without<crate::gameplay::airdrops::Aircraft>)>,
    mut stage: Local<(usize, u8, Option<Entity>, Option<Entity>, Vec3, Vec<String>)>,
) {
    use crate::gameplay::GameOverlay;
    let toasts: Vec<String> = notes.read().map(|n| n.text.clone()).collect();
    let step = TOUR.get(tour.step).map(|s| s.2);
    if stage.0 != tour.step {
        if TOUR.get(stage.0).map(|s| s.2) == Some("08zl_airdrop_loot") && matches!(*overlay, GameOverlay::Loot(_)) {
            *overlay = GameOverlay::None;
        }
        let keep = (stage.2, stage.3);
        *stage = (tour.step, 0, keep.0, keep.1, Vec3::ZERO, std::mem::take(&mut stage.5));
    }
    stage.5.extend(toasts);
    match (step, stage.1) {
        (Some("08zk_airdrop"), 0) => {
            let (Some(start), Ok((mut t, mut body))) = (tour.start, player.single_mut()) else { return };
            t.translation = start;
            body.vertical_velocity = 0.0;
            let fwd = (t.rotation * Vec3::Z).with_y(0.0).normalize_or(Vec3::Z);
            let ground = terrain.as_ref().and_then(|tr| tr.ground_under(start + fwd * 14.0 + Vec3::Y * 2.0)).unwrap_or(start.y);
            let at = (start + fwd * 14.0).with_y(ground + 40.0);
            stage.2 = Some(crate::gameplay::airdrops::spawn_crate(&mut commands, &assets, &mut drops, "AirdropContainer_Medical_C", at));
            if let Some(region) = drops.region {
                let from = start + Vec3::Y * 200.0 - fwd * 60.0;
                let dir = (region.centre - from).with_y(0.0).normalize_or(Vec3::X);
                stage.3 = Some(crate::gameplay::airdrops::spawn_aircraft_at(&mut commands, &assets, &region, from, dir, 0));
                stage.4 = from;
            }
            // Behind the survivor, looking up at the crate.
            yaw.0 = Some((-fwd.x).atan2(-fwd.z));
            pitch.0 = Some(-0.75);
            stage.5.clear();
            stage.1 = 1;
        }
        (Some("08zk_airdrop"), 1) => {
            pitch.0 = Some(-0.75);
            if tour.t > 3.2 {
                let c = stage.2.and_then(|e| crates.get(e).ok());
                let ground = c.and_then(|(t, _)| terrain.as_ref().and_then(|tr| tr.ground_under(t.translation))).unwrap_or(0.0);
                let plane = stage.3.and_then(|e| planes.get(e).ok()).map(|t| t.translation);
                let speed = plane.map_or(0.0, |p| (p - stage.4).length() / tour.t);
                let marker = markers.0.last().map(|m| m.title.clone()).unwrap_or_default();
                info!(
                    "autoshot airdrop falling height {:.1} speed {:.2} plane_speed {:.1} plane_level {} toasts {:?} marker {:?}",
                    c.map_or(-1.0, |(t, _)| t.translation.y - ground),
                    c.map_or(0.0, |(_, c)| -c.vel),
                    speed,
                    plane.is_some_and(|p| (p.y - stage.4.y).abs() < 0.01),
                    stage.5,
                    marker
                );
                stage.1 = 2;
            }
        }
        (Some("08zl_airdrop_loot"), 0) => {
            let Some((ct, c)) = stage.2.and_then(|e| crates.get(e).ok()) else { return };
            if c.key.is_none() {
                return;
            }
            let Ok((mut t, mut body)) = player.single_mut() else { return };
            let to_player = (t.translation - ct.translation).with_y(0.0).normalize_or(Vec3::X);
            t.translation = ct.translation + to_player * 2.0;
            t.rotation = Quat::from_rotation_y((-to_player.x).atan2(-to_player.z));
            body.vertical_velocity = 0.0;
            yaw.0 = Some(to_player.x.atan2(to_player.z));
            pitch.0 = Some(0.3);
            info!("autoshot airdrop landed at {:.1} key {:?} search_from {:.1}", ct.translation, c.key, t.translation);
            stage.1 = 1;
        }
        (Some("08zl_airdrop_loot"), 1) => {
            keys.press(KeyCode::KeyF);
            if let GameOverlay::Loot(k) = *overlay {
                let g = containers.searched.get(&k);
                info!("autoshot airdrop loot opened {:?} name {:?} items {}", stage.2.and_then(|e| crates.get(e).ok()).and_then(|(_, c)| c.key).map(|key| key == k), g.map(|g| g.name.clone()), g.map_or(0, |g| g.items.len()));
                stage.1 = 2;
            }
        }
        _ => {}
    }
}

/// 08zr_quest: a random Classified Container quest begun (the laptop's scan is the inventory's
/// part, unit-tested);
/// 0.5 s on logs the quest, the TaskBox and the marker, and stands the survivor 1.3 m from the
/// crate facing it; 6 s on (streaming, the 0.2 s goal tick) logs the state and holds F until the
/// quest is over and the weapon is on the ground.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn tour_quest(
    tour: Res<Tour>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    game: Res<CurrentGame>,
    task_box: Res<crate::gameplay::quests::TaskBox>,
    markers: Res<crate::gameplay::map::WorldMarkers>,
    ground: Res<crate::gameplay::inventory_ui::Ground>,
    terrain: Option<Res<crate::level::Terrain>>,
    mut scans: MessageWriter<crate::gameplay::quests::BeginQuest>,
    mut yaw: ResMut<crate::gameplay::combat::CameraYawRequest>,
    mut pitch: ResMut<crate::gameplay::combat::CameraPitchRequest>,
    crates: Query<(&Transform, &crate::gameplay::quests::QuestCrate)>,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), (With<crate::models::CharacterModel>, Without<crate::gameplay::quests::QuestCrate>)>,
    mut rays: crate::level::collision::PropRays,
    mut stage: Local<(usize, u8, Option<(Vec3, Quat)>, (u32, u64), usize, f32)>,
    (vehicles, containers, mut overlay): (Query<&Transform, (With<crate::gameplay::vehicles::Vehicle>, Without<crate::models::CharacterModel>, Without<crate::gameplay::quests::QuestCrate>)>, Res<crate::gameplay::interact::Containers>, ResMut<crate::gameplay::GameOverlay>),
) {
    if stage.0 != tour.step {
        if stage.1 == 3 {
            keys.release(KeyCode::KeyF);
        }
        *stage = (tour.step, 0, None, (0, 0), 0, 0.0);
    }
    if TOUR.get(tour.step).map(|s| s.2) != Some("08zr_quest") {
        return;
    }
    let Some(state) = game.state.as_ref() else { return };
    let laptop = state.quest_log.quests.iter().find(|q| q.class.contains("LaptopEvent"));
    if let (Some((feet, rot)), Ok((mut t, mut body))) = (stage.2, player.single_mut()) {
        t.translation = feet;
        t.rotation = rot;
        body.vertical_velocity = 0.0;
        let face = rot * Vec3::Z;
        yaw.0 = Some((-face.x).atan2(-face.z));
        pitch.0 = Some(0.35);
    }
    let lines = |b: &crate::gameplay::quests::TaskBox| b.branches.iter().flat_map(|v| v.0.iter().map(|l| l.0.clone())).collect::<Vec<_>>();
    match stage.1 {
        0 => {
            // (08zq_drive leaves the vehicle's storage open.)
            *overlay = crate::gameplay::GameOverlay::None;
            stage.3 = (state.progression.level, state.progression.current_xp);
            stage.4 = ground.stacks().count();
            // A random Classified Container (SCAN LAPTOP's own draw can now give an elimination
            // quest, which 08zy_elimination covers).
            let n = rand::Rng::gen_range(&mut rand::thread_rng(), 1..=10);
            scans.write(crate::gameplay::quests::BeginQuest(format!("Quest_LaptopEvent_ClassifiedContainer{n:03}_C")));
            stage.1 = 1;
        }
        1 if tour.t > 0.5 => {
            let Some(q) = laptop else {
                info!("autoshot quest none started");
                stage.1 = 9;
                return;
            };
            let Some((ct, c)) = crates.iter().next() else { return };
            let marker = markers.0.iter().any(|m| m.title == "Classified Container");
            info!("autoshot quest started {} state {} box {:?} {:?} {:?} marker {marker} crate {}", q.class, q.state, task_box.subtext, task_box.quest, lines(&task_box), c.argument);
            stage.2 = Some(stand_by(ct.translation, Vec3::X, terrain.as_deref()));
            stage.5 = tour.t;
            stage.1 = 2;
        }
        // Streamed in: a side with room to stand (the crates sit in tents and among boxes).
        2 if tour.t > stage.5 + 4.0 => {
            let Some((ct, _)) = crates.iter().next() else { return };
            let centre = ct.translation + Vec3::Y * 0.9;
            // F goes to the nearest thing: a side where the crate is the nearest container (it
            // sits among military boxes) and away from vehicles (a parked one offers its storage).
            let key = crates.iter().next().map(|(_, c)| c.key);
            let nearest = |p: Vec3| containers.list.iter().min_by(|a, b| a.1.distance(p).total_cmp(&b.1.distance(p))).map(|c| c.0);
            let clear = |d: &Vec3| vehicles.iter().map(|v| v.translation.distance(ct.translation + *d * 1.3)).fold(f32::MAX, f32::min);
            let mut sides: Vec<Vec3> = (0..16).map(|k| Quat::from_rotation_y(k as f32 * std::f32::consts::TAU / 16.0) * Vec3::Z).filter(|d| nearest(ct.translation + *d * 1.3) == key).collect();
            sides.sort_by(|a, b| clear(b).total_cmp(&clear(a)));
            let side = sides.into_iter().find(|d| rays.cast(centre + *d * 0.5, *d, 1.4).is_none());
            info!("autoshot quest standing side {side:?}");
            stage.2 = Some(stand_by(ct.translation, side.unwrap_or(Vec3::X), terrain.as_deref()));
            stage.1 = 5;
        }
        5 if tour.t > stage.5 + 6.0 => {
            let marker = markers.0.iter().any(|m| m.title == "Classified Container");
            info!("autoshot quest arrived state {:?} box {:?} {:?} marker {marker}", laptop.map(|q| q.state.clone()), task_box.subtext, lines(&task_box));
            stage.5 = tour.t;
            stage.1 = 3;
        }
        3 => {
            keys.press(KeyCode::KeyF);
            let opened = crates.iter().next().is_some_and(|(_, c)| c.opened.is_some());
            let dropped: Vec<String> = ground.stacks().skip(stage.4).map(|(_, s)| s.item.clone()).collect();
            if laptop.is_some_and(|q| q.outcome.is_some()) && !dropped.is_empty() || tour.t > stage.5 + 9.0 {
                keys.release(KeyCode::KeyF);
                let xp = (state.progression.level, state.progression.current_xp);
                info!(
                    "autoshot quest done outcome {:?} opened {opened} dropped {dropped:?} xp {:?} -> {:?} box {:?} {:?} after {:.1}",
                    laptop.and_then(|q| q.outcome),
                    stage.3,
                    xp,
                    task_box.subtext,
                    lines(&task_box),
                    tour.t - stage.5
                );
                stage.1 = 4;
            }
        }
        _ => {}
    }
}

/// 08zs_journal_quests: J; 0.4 s on the QUESTS tab is clicked, 0.8 s on the Quest Log folder,
/// 1.2 s on the Classified Container in it; 1.6 s on logs the page (title, folder buttons, list
/// entries) and clicks FORGET, then logs the dialog and clicks NO; 2 s on the quest is still in
/// the log. Shot with the Quest Log open and the quest shown.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn tour_journal_quests(
    mut commands: Commands,
    tour: Res<Tour>,
    game: Res<CurrentGame>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut overlay: ResMut<crate::gameplay::GameOverlay>,
    journal: Res<crate::gameplay::journal::Journal>,
    texts: Query<&Text>,
    buttons: (
        Query<(Entity, &crate::gameplay::journal::TabButton)>,
        Query<(Entity, &crate::gameplay::journal::FolderButton)>,
        Query<Entity, With<crate::gameplay::journal::ForgetButton>>,
        Query<(Entity, &crate::gameplay::journal::ConfirmButton)>,
        Query<(Entity, &crate::gameplay::journal::QuestEntry)>,
    ),
    mut stage: Local<u8>,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), With<crate::models::CharacterModel>>,
) {
    use crate::gameplay::journal::Page;
    if TOUR.get(tour.step).map(|s| s.2) != Some("08zs_journal_quests") {
        if *stage != 0 && *overlay == crate::gameplay::GameOverlay::Journal {
            *overlay = crate::gameplay::GameOverlay::None;
        }
        *stage = 0;
        return;
    }
    let click = |commands: &mut Commands, e: Option<Entity>| {
        if let Some(e) = e {
            commands.trigger(bevy::ui_widgets::Activate { entity: e });
        }
    };
    // Back at the spawn first: some laptop quests' crates are up on roofs, and once 08zr_quest
    // let go the survivor fell from there to their death (999 damage), wrecking this step and
    // the workbench's (death drops the inventory).
    if *stage == 0
        && let (Some(start), Ok((mut t, mut body))) = (tour.start, player.single_mut())
    {
        t.translation = start;
        body.vertical_velocity = 0.0;
    }
    match *stage {
        0 if tour.t > 0.1 => {
            keys.press(KeyCode::KeyJ);
            *stage = 1;
        }
        1 => {
            keys.release(KeyCode::KeyJ);
            *stage = 2;
        }
        2 if tour.t > 0.4 => {
            click(&mut commands, buttons.0.iter().find(|(_, t)| t.0 == Page::Quests).map(|(e, _)| e));
            *stage = 3;
        }
        3 if tour.t > 0.8 => {
            click(&mut commands, buttons.1.iter().find(|(_, f)| f.0).map(|(e, _)| e));
            *stage = 4;
        }
        4 if tour.t > 1.2 => {
            click(&mut commands, buttons.4.iter().find(|(_, q)| q.0.contains("ClassifiedContainer")).map(|(e, _)| e));
            *stage = 8;
        }
        8 if tour.t > 1.6 => {
            let all: Vec<String> = texts.iter().map(|t| t.0.clone()).filter(|t| t.starts_with("Active [") || t.starts_with("Quest Log [") || t == "Classified Container" || t == "The Safe Zone" || t == "TO DO:").collect();
            info!("autoshot journal quests page {:?} shown {:?} log_tab {} texts {all:?}", journal.page, journal.quest, journal.log_tab);
            click(&mut commands, buttons.2.iter().next());
            *stage = 5;
        }
        5 if tour.t > 1.8 => {
            let asks = texts.iter().any(|t| t.0 == "Are you sure you want to forget this quest?");
            info!("autoshot journal quests forget asks {asks}");
            click(&mut commands, buttons.3.iter().find(|(_, c)| !c.0).map(|(e, _)| e));
            *stage = 6;
        }
        6 if tour.t > 2.0 => {
            let kept = game.state.as_ref().is_some_and(|s| s.quest_log.quests.iter().any(|q| q.class.contains("ClassifiedContainer")));
            info!("autoshot journal quests no keeps {kept} dialog {}", journal.confirm_forget);
            *stage = 7;
        }
        _ => {}
    }
}

/// 08zt_workbench: 1.3 m from the crafting station nearest the spawn, on a side where it's the
/// nearest container and nothing is in the way, with 10 Scrap Metal; 3 s on (streaming) F is
/// held until the station's screen opens, which is logged, then Nails are crafted (`tour_craft`).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn tour_workbench(
    mut commands: Commands,
    tour: Res<Tour>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut game: ResMut<CurrentGame>,
    db: Res<crate::gameplay::ItemDb>,
    level: Option<Res<crate::level::Level>>,
    terrain: Option<Res<crate::level::Terrain>>,
    containers: Res<crate::gameplay::interact::Containers>,
    mut overlay: ResMut<crate::gameplay::GameOverlay>,
    mut yaw: ResMut<crate::gameplay::combat::CameraYawRequest>,
    mut rays: crate::level::collision::PropRays,
    texts: Query<&Text>,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), With<crate::models::CharacterModel>>,
    mut stage: Local<(usize, u8, Option<(Vec3, Quat)>, Vec3)>,
) {
    use crate::gameplay::GameOverlay;
    if stage.0 != tour.step {
        if stage.1 != 0 && matches!(*overlay, GameOverlay::Loot(_)) {
            *overlay = GameOverlay::None;
        }
        *stage = (tour.step, 0, None, Vec3::ZERO);
    }
    if TOUR.get(tour.step).map(|s| s.2) != Some("08zt_workbench") {
        return;
    }
    let (Some(start), Some(level)) = (tour.start, level) else { return };
    if let (Some((feet, rot)), Ok((mut t, mut body))) = (stage.2, player.single_mut()) {
        t.translation = feet;
        t.rotation = rot;
        body.vertical_velocity = 0.0;
        let face = rot * Vec3::Z;
        yaw.0 = Some((-face.x).atan2(-face.z));
    }
    match stage.1 {
        0 => {
            *overlay = GameOverlay::None;
            let Some(at) = level.actors.iter().filter(|a| a.class == crate::gameplay::inventory_ui::STATION).map(|a| a.position()).min_by(|a, b| a.distance(start).total_cmp(&b.distance(start))) else {
                info!("autoshot workbench none");
                stage.1 = 9;
                return;
            };
            stage.3 = at;
            stage.2 = Some(stand_by(at, Vec3::X, terrain.as_deref()));
            let inv = crate::gameplay::player_inv(&mut game);
            let _ = inv.insert(&db, db.stack("DA_ScrapMetal", 10));
            info!("autoshot workbench station at {at:.1}");
            stage.1 = 1;
        }
        1 if tour.t > 2.5 => {
            let at = stage.3;
            let nearest = |p: Vec3| containers.list.iter().min_by(|a, b| a.1.distance(p).total_cmp(&b.1.distance(p))).map(|c| c.2.clone());
            let side = (0..16)
                .map(|k| Quat::from_rotation_y(k as f32 * std::f32::consts::TAU / 16.0) * Vec3::Z)
                .filter(|d| nearest(at + *d * 1.3).as_deref() == Some(crate::gameplay::inventory_ui::STATION))
                .find(|d| rays.cast(at + Vec3::Y * 0.9 + *d * 0.5, *d, 1.0).is_none());
            info!("autoshot workbench side {side:?}");
            stage.2 = Some(stand_by(at, side.unwrap_or(Vec3::X), terrain.as_deref()));
            stage.1 = 2;
        }
        2 if tour.t > 3.0 => {
            keys.press(KeyCode::KeyF);
            if let GameOverlay::Loot(k) = *overlay {
                let station = containers.list.iter().any(|c| c.0 == k && c.2 == crate::gameplay::inventory_ui::STATION);
                let has = |s: &str| texts.iter().any(|t| t.0 == s);
                info!("autoshot workbench open station {station} title {} craftable {}", has("CRAFTING STATION"), has("CRAFTABLE ITEMS"));
                stage.1 = 3;
            } else if tour.t > 5.0 {
                info!("autoshot workbench never opened");
                stage.1 = 9;
            }
        }
        3 if tour.t > 3.6 => {
            commands.queue(|w: &mut World| crate::gameplay::inventory_ui::tour_craft(w, "DA_Nails"));
            stage.1 = 4;
        }
        _ => {}
    }
}

/// Feet 1.3 m from `at` along `side`, facing it.
fn stand_by(at: Vec3, side: Vec3, terrain: Option<&crate::level::Terrain>) -> (Vec3, Quat) {
    let p = at + side * 1.3;
    let feet = p.with_y(terrain.and_then(|tr| tr.ground_under(p + Vec3::Y * 2.0)).unwrap_or(at.y).max(at.y - 0.5));
    (feet, Quat::from_rotation_y((-side.x).atan2(-side.z)))
}

/// 08zv_fx: 3.5 m from the open-air barrel fire nearest the spawn, facing it. 2.5 s on: logs the fire's
/// emitter (live particles, spawning) and how many firearms' muzzle flashes and brass resolve,
/// then, back at the spawn, sets off FX_Explosion_New 10 m ahead; 2.8 s: logs the blast's particles; 9 s:
/// logs whether it has gone (its smoke lives 2-6 s) and sets off another for the screenshot.
/// (8.5 s was too soon: the burst comes a frame after the message, its smoke can live 6 s.)
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn tour_fx(
    tour: Res<Tour>,
    level: Option<Res<crate::level::Level>>,
    terrain: Option<Res<crate::level::Terrain>>,
    db: Res<crate::gameplay::ItemDb>,
    mut yaw: ResMut<crate::gameplay::combat::CameraYawRequest>,
    mut fx: MessageWriter<crate::gameplay::particles::Fx>,
    camera: Query<&GlobalTransform, With<crate::gameplay::PlayerCamera>>,
    emitters: Query<(&GlobalTransform, &crate::gameplay::particles::Emitter, Option<&crate::gameplay::particles::LevelFx>)>,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), With<crate::models::CharacterModel>>,
    // (step, stage, stand at, fire at)
    mut stage: Local<(usize, u8, Option<(Vec3, Quat)>, Vec3)>,
) {
    use crate::gameplay::particles::{Fx, fx_data};
    if stage.0 != tour.step {
        *stage = (tour.step, 0, None, Vec3::ZERO);
    }
    if TOUR.get(tour.step).map(|s| s.2) != Some("08zv_fx") {
        return;
    }
    let (Some(start), Some(level)) = (tour.start, level) else { return };
    if let (Some((feet, rot)), Ok((mut t, mut body))) = (stage.2, player.single_mut()) {
        t.translation = feet;
        t.rotation = rot;
        body.vertical_velocity = 0.0;
        let face = rot * Vec3::Z;
        yaw.0 = Some((-face.x).atan2(-face.z));
    }
    let blasts = || emitters.iter().filter(|(_, e, _)| e.system == "FX_Explosion_New").collect::<Vec<_>>();
    // The camera's ground-level forward and right.
    let Ok(cam) = camera.single() else { return };
    let fwd = cam.forward().as_vec3().with_y(0.0).normalize_or(Vec3::Z);
    let right = cam.right().as_vec3().with_y(0.0).normalize_or(Vec3::X);
    let ground = |p: Vec3| p.with_y(terrain.as_ref().map_or(p.y, |t| t.height(p.x, p.z)));
    // 10 m ahead of the survivor, on the ground.
    let side = |_: Vec3, (feet, _): (Vec3, Quat)| ground(feet + fwd * 10.0);
    match stage.1 {
        0 => {
            // Out in the open (some burn in the subway, under the terrain).
            let open = |p: &Vec3| terrain.as_ref().is_none_or(|t| t.height(p.x, p.z) < p.y + 1.0);
            let Some(at) = level.actors.iter().filter(|a| a.class == "BP_BarrelFire_C").map(|a| a.position()).filter(open).min_by(|a, b| a.distance(start).total_cmp(&b.distance(start))) else {
                info!("autoshot fx no barrel fire");
                stage.1 = 9;
                return;
            };
            stage.3 = at;
            let away = Vec3::new(1.0, 0.0, 0.3).normalize();
            let p = at + away * 3.5;
            let feet = p.with_y(terrain.as_ref().and_then(|tr| tr.ground_under(p + Vec3::Y * 2.0)).unwrap_or(at.y).max(at.y - 0.5));
            stage.2 = Some((feet, Quat::from_rotation_y((-away.x).atan2(-away.z))));
            info!("autoshot fx barrel fire at {at:.1}");
            stage.1 = 1;
        }
        1 if tour.t > 2.5 => {
            let fire = emitters.iter().filter(|(_, e, l)| l.is_some() && e.system == "FX_Fire").min_by(|a, b| a.0.translation().distance(stage.3).total_cmp(&b.0.translation().distance(stage.3)));
            let level_n = emitters.iter().filter(|(_, _, l)| l.is_some()).count();
            match fire {
                Some((gt, e, _)) => info!("autoshot fx fire live {} active {} at {:.1} m; {level_n} level emitters", e.live(), e.active, gt.translation().distance(stage.3)),
                None => info!("autoshot fx fire none; {level_n} level emitters"),
            }
            let guns: Vec<_> = db.items.values().filter_map(|d| d.firearm.as_ref()).collect();
            let known = |s: &Option<String>| s.as_ref().is_none_or(|n| fx_data().systems.contains_key(n));
            info!("autoshot fx guns {} flashes {} brass {}", guns.len(), guns.iter().filter(|f| f.muzzle_flash.is_some() && known(&f.muzzle_flash)).count(), guns.iter().filter(|f| known(&f.brass)).count());
            // The blasts go off back at the spawn, out in the open.
            let start_feet = start.with_y(terrain.as_ref().map_or(start.y, |t| t.height(start.x, start.z)));
            stage.2 = Some((start_feet, stage.2.map_or(Quat::IDENTITY, |s| s.1)));
            // With a barrel fire's FX_Fire beside the survivor for the frames: its component is
            // 0.96 m up at 0.25 scale.
            if let Some((feet, _)) = stage.2 {
                let at = ground(feet + fwd * 3.0 + right * 1.2) + Vec3::Y * 0.96;
                fx.write(Fx::at("FX_Fire", at).scaled(Vec3::splat(0.25)).stop_after(6.5));
            }
            if let Some(stand) = stage.2 {
                fx.write(Fx::at("FX_Explosion_New", side(stage.3, stand)).scaled(Vec3::splat(1.5)));
            }
            stage.1 = 2;
        }
        2 if tour.t > 2.8 => {
            let b = blasts();
            // How far the particles' middle is from the emitter across the ground (they fly up
            // fast: the fireballs leave at up to 50 m/s with drag 4).
            let off = b.first().map_or(f32::NAN, |(gt, e, _)| {
                let ps: Vec<Vec3> = e.states.iter().flat_map(|s| s.particles.iter().map(|p| p.pos)).collect();
                (ps.iter().sum::<Vec3>() / ps.len().max(1) as f32).xz().distance(gt.translation().xz())
            });
            info!("autoshot fx blast emitters {} live {} off {off:.1} m", b.len(), b.iter().map(|(_, e, _)| e.live()).sum::<usize>());
            stage.1 = 3;
        }
        3 if tour.t > 9.0 => {
            info!("autoshot fx blast gone {}", blasts().is_empty());
            if let Some(stand) = stage.2 {
                fx.write(Fx::at("FX_Explosion_New", side(stage.3, stand)).scaled(Vec3::splat(1.5)));
            }
            stage.1 = 4;
        }
        _ => {}
    }
}

/// 08zw_prefab: the abandoned camp site nearest the spawn, forced to spawn if its roll didn't;
/// the survivor 14 m off facing it. 5 s on: logs whether it spawned, how many of its meshes are
/// still hidden, its containers and zombie spawners, and whether a ray down onto its first crate
/// hits the crate.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn tour_prefab(
    tour: Res<Tour>,
    level: Option<Res<crate::level::Level>>,
    terrain: Option<Res<crate::level::Terrain>>,
    streaming: Option<Res<crate::level::PropStreaming>>,
    mut prefabs: ResMut<crate::level::prefabs::Prefabs>,
    containers: Res<crate::gameplay::interact::Containers>,
    spawners: Option<Res<crate::level::spawners::Spawners>>,
    mut yaw: ResMut<crate::gameplay::combat::CameraYawRequest>,
    mut rays: crate::level::collision::PropRays,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), With<crate::models::CharacterModel>>,
    // (step, stage, site, stand at)
    mut stage: Local<(usize, u8, Option<usize>, Option<(Vec3, Quat)>)>,
) {
    use crate::level::prefabs::{CONTAINER_KEYS, SPAWNER_KEYS};
    if stage.0 != tour.step {
        *stage = (tour.step, 0, None, None);
    }
    if TOUR.get(tour.step).map(|s| s.2) != Some("08zw_prefab") {
        return;
    }
    let (Some(start), Some(level), Some(streaming)) = (tour.start, level, streaming) else { return };
    if let (Some((feet, rot)), Ok((mut t, mut body))) = (stage.3, player.single_mut()) {
        t.translation = feet;
        t.rotation = rot;
        body.vertical_velocity = 0.0;
        let face = rot * Vec3::Z;
        yaw.0 = Some((-face.x).atan2(-face.z));
    }
    match stage.1 {
        0 => {
            let pick = level
                .prefabs
                .iter()
                .enumerate()
                .filter(|(_, s)| !s.dynamic && s.prefab.starts_with("Prefab_AbandonedCamp") && s.portable())
                .min_by(|a, b| a.1.transform.translation.distance(start).total_cmp(&b.1.transform.translation.distance(start)));
            let Some((i, site)) = pick else {
                info!("autoshot prefab no camp site");
                stage.1 = 9;
                return;
            };
            let at = site.transform.translation;
            let away = Vec3::new(1.0, 0.0, 0.4).normalize();
            let p = at + away * 14.0;
            let feet = p.with_y(terrain.as_ref().and_then(|tr| tr.ground_under(p + Vec3::Y * 3.0)).unwrap_or(at.y));
            stage.3 = Some((feet, Quat::from_rotation_y((-away.x).atan2(-away.z))));
            info!("autoshot prefab site {} at {at:.0} rolled {}", site.prefab, prefabs.spawned.contains(&i));
            prefabs.force(i);
            stage.2 = Some(i);
            stage.1 = 1;
        }
        1 if tour.t > 5.0 => {
            let Some(i) = stage.2 else { return };
            let site = &level.prefabs[i];
            let hidden = site.keys.iter().filter(|k| streaming.is_removed(**k)).count();
            let boxes: Vec<Vec3> = containers.list.iter().filter(|c| (CONTAINER_KEYS + i * 64..CONTAINER_KEYS + (i + 1) * 64).contains(&c.0)).map(|c| c.1).collect();
            let zombie_spawners = (0..64).filter(|j| spawners.as_ref().is_some_and(|s| s.has(SPAWNER_KEYS + i * 64 + j))).count();
            let crate_hit = boxes.first().is_some_and(|b| rays.cast(*b + Vec3::Y * 4.0, Vec3::NEG_Y, 4.5).is_some_and(|d| d < 4.0));
            info!(
                "autoshot prefab {} spawned {} meshes {} hidden {hidden} containers {} spawners {zombie_spawners} crate_hit {crate_hit}",
                site.prefab,
                prefabs.spawned.contains(&i),
                site.keys.len(),
                boxes.len()
            );
            stage.1 = 2;
        }
        _ => {}
    }
}

/// 08zx_bandits: the bandit camp site nearest the spawn, forced to spawn; the survivor 25 m off
/// facing it. 8 s on: logs the humans around it (classes, armed) and the shots that reached the
/// survivor; then they're killed, and 10.5 s on: logs the human kills, the toasts and the corpses.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn tour_bandits(
    tour: Res<Tour>,
    level: Option<Res<crate::level::Level>>,
    terrain: Option<Res<crate::level::Terrain>>,
    mut prefabs: ResMut<crate::level::prefabs::Prefabs>,
    game: Res<CurrentGame>,
    containers: Res<crate::gameplay::interact::Containers>,
    mut yaw: ResMut<crate::gameplay::combat::CameraYawRequest>,
    mut notes: MessageReader<crate::gameplay::notify::Notify>,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), With<crate::models::CharacterModel>>,
    mut humans: Query<(&Transform, &crate::level::LevelZombie, Option<&crate::gameplay::humans::HumanArms>, Option<&mut crate::gameplay::combat::Health>, Has<crate::gameplay::combat::Dead>), Without<crate::models::CharacterModel>>,
    // (step, stage, site, stand at, kills before, toasts)
    mut stage: Local<(usize, u8, Option<usize>, Option<(Vec3, Quat)>, u32, Vec<String>)>,
) {
    let toasts: Vec<String> = notes.read().map(|n| n.text.clone()).filter(|t| t.contains("Killed")).collect();
    if stage.0 != tour.step {
        *stage = (tour.step, 0, None, None, 0, Vec::new());
    }
    if TOUR.get(tour.step).map(|s| s.2) != Some("08zx_bandits") {
        return;
    }
    stage.5.extend(toasts);
    let (Some(start), Some(level)) = (tour.start, level) else { return };
    if let (Some((feet, rot)), Ok((mut t, mut body))) = (stage.3, player.single_mut()) {
        t.translation = feet;
        t.rotation = rot;
        body.vertical_velocity = 0.0;
        let face = rot * Vec3::Z;
        yaw.0 = Some((-face.x).atan2(-face.z));
    }
    let near = |at: Vec3| move |t: &Transform| t.translation.distance(at) < 45.0;
    match stage.1 {
        0 => {
            let pick = level
                .prefabs
                .iter()
                .enumerate()
                .filter(|(_, s)| !s.dynamic && s.prefab.contains("BanditCamp") && !s.prefab.contains("Quest") && s.portable())
                .min_by(|a, b| a.1.transform.translation.distance(start).total_cmp(&b.1.transform.translation.distance(start)));
            let Some((i, site)) = pick else {
                info!("autoshot bandits no camp site");
                stage.1 = 9;
                return;
            };
            let at = site.transform.translation;
            let away = Vec3::new(-0.6, 0.0, 1.0).normalize();
            let p = at + away * 25.0;
            let feet = p.with_y(terrain.as_ref().and_then(|tr| tr.ground_under(p + Vec3::Y * 3.0)).unwrap_or(at.y));
            stage.3 = Some((feet, Quat::from_rotation_y((-away.x).atan2(-away.z))));
            stage.4 = game.state.as_ref().map_or(0, |s| s.stats.human_kills);
            info!("autoshot bandits site {} at {at:.0}", site.prefab);
            prefabs.force(i);
            stage.2 = Some(i);
            stage.1 = 1;
        }
        1 if tour.t > 8.0 => {
            let Some(i) = stage.2 else { return };
            let at = level.prefabs[i].transform.translation;
            let seen: Vec<String> = humans.iter().filter(|h| !h.4 && h.1.kind.human().is_some() && near(at)(h.0)).map(|h| format!("{}:{}", h.1.kind.class_name(), h.2.map_or("unarmed", |a| a.ranged.or(a.melee).unwrap_or("none")))).collect();
            info!("autoshot bandits humans {} [{}]", seen.len(), seen.join(" "));
            for (t, z, _, h, dead) in &mut humans {
                if !dead && z.kind.human().is_some() && near(at)(t)
                    && let Some(mut h) = h
                {
                    h.hp = 0.0;
                }
            }
            stage.1 = 2;
        }
        2 if tour.t > 10.5 => {
            let Some(i) = stage.2 else { return };
            let at = level.prefabs[i].transform.translation;
            let kills = game.state.as_ref().map_or(0, |s| s.stats.human_kills) - stage.4;
            let corpses = humans.iter().filter(|h| h.4 && h.1.kind.human().is_some() && near(at)(h.0)).count();
            let lootable = containers.list.iter().filter(|c| c.0 >= crate::gameplay::interact::CORPSE_KEYS && c.1.distance(at) < 45.0).count();
            info!("autoshot bandits kills {kills} corpses {corpses} lootable {lootable} toasts {:?}", stage.5.iter().take(3).collect::<Vec<_>>());
            stage.1 = 3;
        }
        _ => {}
    }
}

/// The level's random-chance volumes put bandits and soldiers all over the map, and one group
/// once shot the survivor down to nothing mid-salvage: outside the human AI's own steps the tour
/// takes their people away (the spawners still count them alive, so they don't come back).
fn tour_calm_humans(
    mut commands: Commands,
    time: Res<Time>,
    tour: Res<Tour>,
    humans: Query<(Entity, &crate::level::LevelZombie), Without<crate::gameplay::combat::Dead>>,
    mut seen: Local<std::collections::HashMap<Entity, f32>>,
    mut gone: Local<usize>,
) {
    use crate::level::spawners::RANDOM_VOLUME_KEYS;
    if matches!(TOUR.get(tour.step).map(|s| s.2), Some("08zx_bandits" | "08zy_elimination" | "08zz_wanderer")) {
        return;
    }
    let now = time.elapsed_secs();
    for (e, z) in &humans {
        if !z.spawner.is_some_and(|i| (RANDOM_VOLUME_KEYS..crate::level::prefabs::SPAWNER_KEYS).contains(&i)) {
            continue;
        }
        // A second after it appeared, once its weapons, hair and pack are on (their commands
        // must not land on a despawned entity).
        if now - *seen.entry(e).or_insert(now) < 1.0 {
            continue;
        }
        seen.remove(&e);
        commands.entity(e).despawn();
        *gone += 1;
        info!("autoshot calm: {} from a random-chance volume taken away ({} so far)", z.kind.class_name(), *gone);
    }
}

/// 08zzb_settlement: the survivor stands 4 m from the middle of the settlement BP_SettlementManager
/// picked; 3.5 s on the safe zone they're in, the banner and badge, and the traders, wells,
/// crafting/cooking stations and the map marker within 40 m are logged; then with 3000 cash the
/// Settlement Officer's dialogue opens and "What intel do you have?" is chosen (5.5 s on: the cash,
/// the quests begun, the lines and toasts are logged); 9.5 s on the settlers around are logged.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn tour_settlement(
    mut commands: Commands,
    tour: Res<Tour>,
    mut game: ResMut<CurrentGame>,
    items: Option<Res<crate::gameplay::ItemDb>>,
    mut overlay: ResMut<crate::gameplay::GameOverlay>,
    follow: Res<crate::gameplay::traders::FollowUp>,
    replies: Query<(Entity, &crate::gameplay::traders::ReplyButton)>,
    civilians: Query<(&Transform, &crate::gameplay::civilians::Civilian), Without<crate::models::CharacterModel>>,
    mut notes: MessageReader<crate::gameplay::notify::Notify>,
    mut toasts: Local<Vec<String>>,
    (level, terrain, prefabs): (Option<Res<crate::level::Level>>, Option<Res<crate::level::Terrain>>, Res<crate::level::prefabs::Prefabs>),
    (zones, wells, traders, containers, markers): (
        Res<crate::gameplay::zones::SafeZones>,
        Res<crate::gameplay::zones::Wells>,
        Res<crate::gameplay::traders::Traders>,
        Res<crate::gameplay::interact::Containers>,
        Res<crate::gameplay::map::WorldMarkers>,
    ),
    banners: Query<&crate::gameplay::zones::ZoneBanner>,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), With<crate::models::CharacterModel>>,
    mut stage: Local<(usize, u8, Option<Transform>)>,
) {
    let fresh: Vec<String> = notes.read().map(|n| n.text.clone()).collect();
    if stage.0 != tour.step {
        if TOUR.get(stage.0).map(|s| s.2) == Some("08zzb_settlement") {
            *overlay = crate::gameplay::GameOverlay::None;
        }
        *stage = (tour.step, 0, None);
        toasts.clear();
    }
    if TOUR.get(tour.step).map(|s| s.2) != Some("08zzb_settlement") {
        return;
    }
    toasts.extend(fresh);
    if let (Some(at), Ok((mut t, mut body))) = (stage.2, player.single_mut()) {
        *t = at;
        body.vertical_velocity = 0.0;
    }
    let Some(level) = level else { return };
    let camps = crate::level::prefabs::settlement_camps(&level);
    let site = game.state.as_ref().and_then(|s| s.settlement).and_then(|k| camps.get(k).copied());
    match stage.1 {
        0 => {
            let Some(i) = site else {
                info!("autoshot settlement none picked (camps {})", camps.len());
                stage.1 = 9;
                return;
            };
            let s = &level.prefabs[i];
            let p = s.transform.translation + Vec3::new(3.0, 0.0, 2.6);
            let feet = p.with_y(terrain.as_ref().and_then(|tr| tr.ground_under(p + Vec3::Y * 4.0)).unwrap_or(p.y));
            stage.2 = Some(Transform::from_translation(feet + Vec3::Y * 0.05));
            info!("autoshot settlement {} of {} camps: {} at {:.0} spawned {}", i, camps.len(), s.prefab, s.transform.translation, prefabs.spawned.contains(&i));
            stage.1 = 1;
        }
        1 if tour.t > 3.5 => {
            let Some(i) = site else { return };
            let at = level.prefabs[i].transform.translation;
            let near = |p: Vec3| p.distance(at) < 40.0;
            let zone = zones.inside.map(|z| zones.list[z].name.clone());
            let banner: Vec<String> = banners.iter().map(|b| format!("{}:{}", if b.entering { "Now Entering" } else { "Now Leaving" }, b.name)).collect();
            let sellers: Vec<&str> = traders.list.iter().filter(|t| near(t.transform.translation)).map(|t| t.class.as_str()).collect();
            let well = wells.list.iter().filter(|w| near(w.pos)).count();
            let stations: Vec<&str> = containers.list.iter().filter(|c| near(c.1) && crate::gameplay::inventory_ui::station(&c.2).is_some()).map(|c| c.2.as_str()).collect();
            let marker = markers.0.iter().find(|m| near(m.pos)).map(|m| m.title.clone());
            info!("autoshot settlement zone {zone:?} safe {} banner {banner:?} traders {sellers:?} wells {well} stations {stations:?} marker {marker:?}", zones.active());
            // The Settlement Officer, with exactly 3000 cash.
            let officer = traders.list.iter().position(|t| t.class == "BP_QuestGiver_SettlementOfficer_C" && near(t.transform.translation));
            if let (Some(k), Some(items)) = (officer, items.as_ref()) {
                let inv = crate::gameplay::player_inv(&mut game);
                let cash = inv.count("DA_Money");
                inv.remove("DA_Money", cash);
                let _ = inv.insert(items, items.stack("DA_Money", 3000));
                *overlay = crate::gameplay::GameOverlay::Dialogue(crate::gameplay::traders::TRADER_KEYS + k);
            }
            info!("autoshot settlement officer {officer:?}");
            stage.1 = 2;
        }
        2 if tour.t > 4.0 => {
            if let Some((e, _)) = replies.iter().find(|(_, r)| r.reply == 0) {
                commands.trigger(bevy::ui_widgets::Activate { entity: e });
            }
            stage.1 = 3;
        }
        3 if tour.t > 5.5 => {
            let cash = crate::gameplay::player_inv(&mut game).count("DA_Money");
            let quests: Vec<String> = game.state.as_ref().map(|s| s.quest_log.active().map(|q| q.class.clone()).collect()).unwrap_or_default();
            info!("autoshot settlement intel cash {cash} quests {quests:?} lines {:?} overlay {:?} toasts {:?}", follow.lines, *overlay, *toasts);
            stage.1 = 4;
        }
        4 if tour.t > 9.5 => {
            let Some(i) = site else { return };
            let at = level.prefabs[i].transform.translation;
            let around: Vec<String> = civilians.iter().filter(|(t, _)| t.translation.distance(at) < 40.0).map(|(_, c)| format!("{}:{}", c.class, c.name.clone().unwrap_or_default())).collect();
            info!("autoshot settlement settlers {} {around:?}", around.len());
            stage.1 = 5;
        }
        _ => {}
    }
}

/// 08zzc_turret: the survivor 12 m in front of the first bandit turret; 3.5 s on its target, ammo
/// and the shots are logged; 4 s on 1.2 m behind it, F held from 4.5 to 10.5 s; then whether it's
/// disabled and the Turret Gun is on the ground; a 100-damage round at another turret, and at 11.5 s
/// its state.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn tour_turret(
    tour: Res<Tour>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    terrain: Option<Res<crate::level::Terrain>>,
    turrets: Res<crate::gameplay::turrets::Turrets>,
    ground: Res<crate::gameplay::inventory_ui::Ground>,
    mut shots: MessageWriter<crate::gameplay::turrets::Shot>,
    mut yaw: ResMut<crate::gameplay::combat::CameraYawRequest>,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), With<crate::models::CharacterModel>>,
    // (step, stage, stand at)
    mut stage: Local<(usize, u8, Option<Transform>)>,
) {
    use crate::gameplay::turrets::{Target, TurretKind};
    if stage.0 != tour.step {
        if TOUR.get(stage.0).map(|s| s.2) == Some("08zzc_turret") {
            keys.release(KeyCode::KeyF);
        }
        *stage = (tour.step, 0, None);
    }
    if TOUR.get(tour.step).map(|s| s.2) != Some("08zzc_turret") {
        return;
    }
    if let (Some(at), Ok((mut t, mut body))) = (stage.2, player.single_mut()) {
        *t = at;
        body.vertical_velocity = 0.0;
    }
    let bandits: Vec<usize> = turrets.list.iter().enumerate().filter(|(_, t)| t.kind == TurretKind::Bandit).map(|(i, _)| i).collect();
    let Some(&first) = bandits.first() else {
        if stage.1 == 0 {
            info!("autoshot turret none");
            stage.1 = 9;
        }
        return;
    };
    let t0 = &turrets.list[first];
    let fwd = (t0.facing * Vec3::X).with_y(0.0).normalize_or(Vec3::X);
    let stand = |p: Vec3, look: Vec3| {
        let feet = p.with_y(terrain.as_ref().and_then(|tr| tr.ground_under(p + Vec3::Y * 3.0)).unwrap_or(p.y));
        let d = (look - feet).with_y(0.0);
        Transform::from_translation(feet + Vec3::Y * 0.05).with_rotation(Quat::from_rotation_y(d.x.atan2(d.z)))
    };
    match stage.1 {
        0 => {
            let at = stand(t0.pivot + fwd * 12.0, t0.pivot);
            let back = -(at.rotation * Vec3::Z);
            yaw.0 = Some(back.x.atan2(back.z));
            stage.2 = Some(at);
            info!("autoshot turret {} of {} at {:.0}", first, turrets.list.len(), t0.pivot);
            stage.1 = 1;
        }
        1 if tour.t > 3.5 => {
            info!("autoshot turret sees player {} ammo {} yaw {:.0} pitch {:.0}", t0.target == Some(Target::Player), t0.ammo, t0.yaw, t0.pitch);
            stage.2 = Some(stand(t0.pivot - fwd * 1.2, t0.pivot));
            stage.1 = 2;
        }
        2 if tour.t > 4.5 => {
            keys.press(KeyCode::KeyF);
            stage.1 = 3;
        }
        3 if tour.t > 10.5 => {
            keys.release(KeyCode::KeyF);
            let gun = ground.stacks().any(|(p, st)| st.item == "DA_TurretGun" && p.distance(t0.pivot) < 4.0);
            info!("autoshot turret disabled {} gun dropped {gun}", t0.disabled);
            if let Some(&other) = bandits.get(1) {
                let t1 = &turrets.list[other];
                let from = t1.pivot + Vec3::new(5.0, 0.3, 0.0);
                shots.write(crate::gameplay::turrets::Shot { origin: from, dir: (t1.pivot - from).normalize(), reach: 10.0, damage: 100.0 });
            }
            stage.1 = 4;
        }
        4 if tour.t > 11.5 => {
            let other = bandits.get(1).map(|&k| (turrets.list[k].dead, turrets.list[k].health));
            info!("autoshot turret shot down {other:?}");
            stage.1 = 5;
        }
        _ => {}
    }
}

/// 08zzd_blood_moon: back at the spawn at 23:00 with a roamer 10 m off; 0.5 s on the Blood Moon
/// starts (as a sunset's roll would); 1.5-2.5 s the roamer's run is timed and the moonlight,
/// the boost and the toasts are logged; 3 s on it's 06:30 (sunrise), and at 4 s the moon's state,
/// the boost and the toasts are logged.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn tour_blood_moon(
    mut commands: Commands,
    tour: Res<Tour>,
    assets: Res<AssetServer>,
    terrain: Option<Res<crate::level::Terrain>>,
    mut game: ResMut<CurrentGame>,
    moon: Res<crate::gameplay::world_events::BloodMoon>,
    sun: Query<&DirectionalLight, With<crate::gameplay::daynight::Sun>>,
    mut notes: MessageReader<crate::gameplay::notify::Notify>,
    mut noise: MessageWriter<crate::gameplay::zombies::Noise>,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), With<crate::models::CharacterModel>>,
    zombies: Query<(&Transform, Has<crate::gameplay::world_events::BloodMoonBoost>), Without<crate::models::CharacterModel>>,
    // (step, stage, roamer, stand at, its place at 1.5 s, toasts)
    mut stage: Local<(usize, u8, Option<Entity>, Option<Transform>, Vec3, Vec<String>)>,
) {
    let fresh: Vec<String> = notes.read().map(|n| n.text.clone()).filter(|t| t.contains("Blood Moon")).collect();
    if stage.0 != tour.step {
        *stage = (tour.step, 0, None, None, Vec3::ZERO, Vec::new());
    }
    if TOUR.get(tour.step).map(|s| s.2) != Some("08zzd_blood_moon") {
        return;
    }
    stage.5.extend(fresh);
    if let (Some(at), Ok((mut t, mut body))) = (stage.3, player.single_mut()) {
        *t = at;
        body.vertical_velocity = 0.0;
    }
    let set_hour = |game: &mut CurrentGame, h: f32| {
        if let Some(st) = game.state.as_mut() {
            st.environment.time_of_day_hours = h;
        }
    };
    match stage.1 {
        0 => {
            let (Some(start), Some(terrain)) = (tour.start, terrain.as_ref()) else { return };
            let ground = |p: Vec3| p.with_y(terrain.ground_under(p + Vec3::Y * 3.0).unwrap_or(p.y));
            let me = ground(start);
            stage.3 = Some(Transform::from_translation(me + Vec3::Y * 0.05));
            set_hour(&mut game, 23.0);
            let at = ground(me + Vec3::new(10.0, 0.0, 0.0));
            let lz = crate::level::LevelZombie { home: at, spawner: None, model: TOUR_ZOMBIE.into(), material: None, kind: Default::default() };
            stage.2 = Some(crate::level::spawners::spawn_zombie(&mut commands, &assets, Transform::from_translation(at), lz));
            stage.1 = 1;
        }
        1 if tour.t > 0.5 => {
            commands.queue(crate::gameplay::world_events::force_blood_moon);
            // A noise at the survivor: the roamer comes running (Investigate, at Alert Speed).
            if let Some(at) = stage.3 {
                noise.write(crate::gameplay::zombies::Noise { at: at.translation, range: 30.0 });
            }
            stage.1 = 2;
        }
        2 if tour.t > 1.5 => {
            stage.4 = stage.2.and_then(|e| zombies.get(e).ok()).map_or(Vec3::ZERO, |z| z.0.translation);
            stage.1 = 3;
        }
        3 if tour.t > 2.5 => {
            let (pos, roused) = stage.2.and_then(|e| zombies.get(e).ok()).map_or((Vec3::ZERO, false), |z| (z.0.translation, z.1));
            let light = sun.iter().next().map(|l| l.color.to_linear());
            info!("autoshot blood moon on {} roused {roused} ran {:.2} m/s light {light:?} toasts {:?}", moon.on, pos.xz().distance(stage.4.xz()), stage.5);
            set_hour(&mut game, 6.5);
            stage.1 = 4;
        }
        4 if tour.t > 4.0 => {
            let roused = stage.2.and_then(|e| zombies.get(e).ok()).is_some_and(|z| z.1);
            info!("autoshot blood moon sunrise on {} roused {roused} toasts {:?}", moon.on, stage.5);
            if let Some(e) = stage.2.take() {
                commands.entity(e).despawn();
            }
            stage.1 = 5;
        }
        _ => {}
    }
}

/// 08zz_wanderer: back at the spawn, wandering trader A 3 m ahead and B 9 m off; 3 s on the
/// traders met are logged and A's dialogue opens; 5 s on whether A stood still is logged, its
/// trade screen opens (the shot) and B is killed; 7.5 s on B's fate and the kill toast. Also
/// logged: the random-chance volumes' groups and the spawned traders' heights above the ground.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn tour_wanderer(
    mut commands: Commands,
    tour: Res<Tour>,
    assets: Res<AssetServer>,
    terrain: Option<Res<crate::level::Terrain>>,
    spawners: Option<Res<crate::level::spawners::Spawners>>,
    traders: Res<crate::gameplay::traders::Traders>,
    containers: Res<crate::gameplay::interact::Containers>,
    mut overlay: ResMut<crate::gameplay::GameOverlay>,
    mut notes: MessageReader<crate::gameplay::notify::Notify>,
    mut yaw: ResMut<crate::gameplay::combat::CameraYawRequest>,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), With<crate::models::CharacterModel>>,
    mut bodies: Query<(&Transform, Option<&mut crate::gameplay::combat::Health>, Has<crate::gameplay::zombies::HeldStill>, Has<crate::gameplay::combat::Dead>), Without<crate::models::CharacterModel>>,
    // (step, stage, A, B, A's place when the talk began, stand at, toasts)
    mut stage: Local<(usize, u8, Option<Entity>, Option<Entity>, Vec3, Option<Transform>, Vec<String>)>,
) {
    use crate::gameplay::GameOverlay;
    use crate::gameplay::traders::TRADER_KEYS;
    let toasts: Vec<String> = notes.read().map(|n| n.text.clone()).filter(|t| t.contains("Trader")).collect();
    if stage.0 != tour.step {
        if TOUR.get(stage.0).map(|s| s.2) == Some("08zz_wanderer") {
            *overlay = GameOverlay::None;
        }
        *stage = (tour.step, 0, None, None, Vec3::ZERO, None, Vec::new());
    }
    if TOUR.get(tour.step).map(|s| s.2) != Some("08zz_wanderer") {
        return;
    }
    stage.6.extend(toasts);
    if let (Some(at), Ok((mut t, mut body))) = (stage.5, player.single_mut()) {
        *t = at;
        body.vertical_velocity = 0.0;
    }
    let key_of = |e: Option<Entity>| traders.list.iter().position(|t| t.entity == e && e.is_some()).map(|i| TRADER_KEYS + i);
    match stage.1 {
        0 => {
            let (Some(start), Some(terrain)) = (tour.start, terrain.as_ref()) else { return };
            let Some(kind) = crate::gameplay::zombies::ZombieKind::from_class("BP_MasterTrader_C") else {
                info!("autoshot wanderer no BP_MasterTrader_C class");
                stage.1 = 9;
                return;
            };
            let ground = |p: Vec3| p.with_y(terrain.ground_under(p + Vec3::Y * 3.0).unwrap_or(p.y));
            let me = ground(start);
            let ahead = Vec3::new(0.6, 0.0, 0.8);
            stage.5 = Some(Transform::from_translation(me + Vec3::Y * 0.05).with_rotation(Quat::from_rotation_y(ahead.x.atan2(ahead.z))));
            yaw.0 = Some((-ahead.x).atan2(-ahead.z));
            let mut rng = rand::thread_rng();
            for (slot, at) in [(2, ground(me + ahead * 3.0)), (3, ground(me + Vec3::new(ahead.z, 0.0, -ahead.x) * 9.0))] {
                let (model, material) = kind.pick_look(&mut rng);
                let back = me - at;
                let lz = crate::level::LevelZombie { home: at, spawner: None, model, material, kind };
                let e = crate::level::spawners::spawn_zombie(&mut commands, &assets, Transform::from_translation(at).with_rotation(Quat::from_rotation_y(back.x.atan2(back.z))), lz);
                if slot == 2 { stage.2 = Some(e) } else { stage.3 = Some(e) }
            }
            if let Some(sp) = spawners.as_ref() {
                let groups: Vec<String> = sp.defs().filter(|(i, _)| (crate::level::spawners::RANDOM_VOLUME_KEYS..crate::level::prefabs::SPAWNER_KEYS).contains(i)).map(|(_, d)| d.kind.class_name().to_string()).collect();
                let wandering = groups.iter().filter(|c| c.as_str() == "BP_MasterTrader_C").count();
                info!("autoshot wanderer volumes groups {} wandering {wandering}", groups.len());
            }
            for t in traders.list.iter().filter(|t| matches!(t.class.as_str(), "BP_MilitaryTrader_C" | "BP_ShakesConesTrader_C")) {
                let p = t.transform.translation;
                info!("autoshot wanderer spawned trader {} at {p:.0} above ground {:.2}", t.class, p.y - terrain.height(p.x, p.z));
            }
            stage.1 = 1;
        }
        1 if tour.t > 3.0 => {
            let met = traders.list.iter().filter(|t| t.spawner.is_none() && t.class == "BP_MasterTrader_C" && !t.dead).count();
            let (Some(a), Some(_)) = (key_of(stage.2), key_of(stage.3)) else {
                info!("autoshot wanderer met {met}: A or B not a trader");
                stage.1 = 9;
                return;
            };
            stage.4 = stage.2.and_then(|e| bodies.get(e).ok()).map_or(Vec3::ZERO, |b| b.0.translation);
            info!("autoshot wanderer met {met} prompt {:?}", traders.get(&crate::gameplay::traders::TraderDb::load(), a).map(|(_, c)| crate::gameplay::traders::prompt(c)));
            *overlay = GameOverlay::Dialogue(a);
            stage.1 = 2;
        }
        2 if tour.t > 5.0 => {
            let a = stage.2.and_then(|e| bodies.get(e).ok()).map(|b| (b.0.translation.distance(stage.4), b.2));
            info!("autoshot wanderer talking held {:?} moved {:?}", a.map(|a| a.1), a.map(|a| a.0));
            if let Some(k) = key_of(stage.2) {
                *overlay = GameOverlay::Loot(k);
            }
            if let Some(mut h) = stage.3.and_then(|e| bodies.get_mut(e).ok()).and_then(|b| b.1) {
                h.hp = 0.0;
            }
            stage.1 = 3;
        }
        3 if tour.t > 7.5 => {
            let b = stage.3.and_then(|e| traders.list.iter().position(|t| t.entity == Some(e)));
            let dead = b.is_some_and(|i| traders.list[i].dead);
            let trade_gone = b.is_some_and(|i| !containers.list.iter().any(|c| c.0 == TRADER_KEYS + i));
            let corpse = stage.3.and_then(|e| bodies.get(e).ok()).is_some_and(|b| b.3);
            let a_held = stage.2.and_then(|e| bodies.get(e).ok()).is_some_and(|b| b.2);
            info!("autoshot wanderer killed dead {dead} trade gone {trade_gone} corpse {corpse} A held {a_held} overlay {:?} toasts {:?}", *overlay, stage.6);
            stage.1 = 4;
        }
        _ => {}
    }
}

/// 08zy_elimination: Eliminate Bandits 001 begun; 0.5 s on the survivor stands on its goal (the
/// "Bandit Camp" marker); 9 s on logs the quest's state, the quest camp and the volume's living
/// bandits, which are then killed; 12 s on logs the outcome, the XP and the task line.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn tour_elimination(
    tour: Res<Tour>,
    game: Res<CurrentGame>,
    world: Res<crate::gameplay::quests::QuestWorld>,
    markers: Res<crate::gameplay::map::WorldMarkers>,
    task_box: Res<crate::gameplay::quests::TaskBox>,
    terrain: Option<Res<crate::level::Terrain>>,
    mut begin: MessageWriter<crate::gameplay::quests::BeginQuest>,
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), With<crate::models::CharacterModel>>,
    mut humans: Query<(&crate::level::LevelZombie, Option<&mut crate::gameplay::combat::Health>, Has<crate::gameplay::combat::Dead>), Without<crate::models::CharacterModel>>,
    // (step, stage, stand at, xp before)
    mut stage: Local<(usize, u8, Option<Vec3>, (u32, u64))>,
) {
    const QUEST: &str = "Quest_LaptopEvent_EliminateBandits001_C";
    if stage.0 != tour.step {
        *stage = (tour.step, 0, None, (0, 0));
    }
    if TOUR.get(tour.step).map(|s| s.2) != Some("08zy_elimination") {
        return;
    }
    let Some(state) = game.state.as_ref() else { return };
    if let (Some(feet), Ok((mut t, mut body))) = (stage.2, player.single_mut()) {
        t.translation = feet;
        body.vertical_velocity = 0.0;
    }
    let quest = state.quest_log.quests.iter().find(|q| q.class == QUEST);
    let ours = |z: &crate::level::LevelZombie| z.spawner.is_some_and(|i| world.human_spawners.contains_key(&i));
    match stage.1 {
        0 => {
            stage.3 = (state.progression.level, state.progression.current_xp);
            begin.write(crate::gameplay::quests::BeginQuest(QUEST.into()));
            stage.1 = 1;
        }
        1 if tour.t > 0.5 => {
            let Some(m) = markers.0.iter().find(|m| m.title == "Bandit Camp") else {
                info!("autoshot elimination no marker (quest {:?})", quest.map(|q| q.state.clone()));
                stage.1 = 9;
                return;
            };
            let at = m.pos.with_y(terrain.as_ref().map_or(m.pos.y, |t| t.height(m.pos.x, m.pos.z)));
            stage.2 = Some(at);
            info!("autoshot elimination camp at {at:.0}");
            stage.1 = 2;
        }
        2 if tour.t > 9.0 => {
            let alive = humans.iter().filter(|h| !h.2 && ours(h.0)).count();
            info!("autoshot elimination state {:?} volumes {} bandits {alive} task {:?}", quest.map(|q| q.state.clone()), world.human_spawners.len(), task_box.branches.iter().flat_map(|b| b.0.iter().map(|l| l.0.clone())).collect::<Vec<_>>());
            for (z, h, dead) in &mut humans {
                if !dead && ours(z)
                    && let Some(mut h) = h
                {
                    h.hp = 0.0;
                }
            }
            stage.1 = 3;
        }
        3 if tour.t > 12.0 => {
            let xp = (state.progression.level, state.progression.current_xp);
            info!("autoshot elimination outcome {:?} xp {:?} -> {xp:?} box {:?}", quest.and_then(|q| q.outcome), stage.3, task_box.subtext);
            stage.1 = 4;
        }
        _ => {}
    }
}

/// 08zh_wolf / 08zu_boss have seen their first hit (keep_alive heals from then on).
#[derive(Resource, Default)]
struct WolfBitten(bool);

/// 08zg_deer / 08zh_wolf: back at the spawn facing the camera's way, an animal spawned ahead
/// facing the survivor. The deer: 1.5 s on logs its brain, gait and how far it went, then it's
/// killed (Huntsman unlocked); 3 s on logs the toast, its corpse's fall and container, and the
/// meat bag. The wolf: logs its brain, gait and the health its bites took, then dies.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn tour_animals(
    mut commands: Commands,
    tour: Res<Tour>,
    assets: Res<AssetServer>,
    mut game: ResMut<CurrentGame>,
    mut bitten: ResMut<WolfBitten>,
    camera: Query<&GlobalTransform, With<crate::gameplay::PlayerCamera>>,
    terrain: Option<Res<crate::level::Terrain>>,
    containers: Res<crate::gameplay::interact::Containers>,
    mut rays: crate::level::collision::PropRays,
    mut notes: MessageReader<crate::gameplay::notify::Notify>,
    mut player: Query<&mut Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    mut animals: Query<(&Transform, &crate::models::Locomotion, Option<&crate::gameplay::animals::Animal>, Option<&mut crate::gameplay::combat::Health>, Has<crate::gameplay::combat::Dead>), (With<crate::level::LevelZombie>, Without<crate::models::CharacterModel>)>,
    mut stage: Local<(usize, u8, Option<Entity>, Vec3, f32, String, f32, bool)>,
) {
    use crate::gameplay::zombies::ZombieKind;
    let toasts: Vec<String> = notes.read().map(|n| n.text.clone()).filter(|t| t.contains("Killed")).collect();
    let step = TOUR.get(tour.step).map(|s| s.2);
    if stage.0 != tour.step {
        *stage = (tour.step, 0, None, Vec3::ZERO, 0.0, String::new(), 0.0, false);
        bitten.0 = false;
    }
    let (class, dist) = match step {
        Some("08zg_deer") => ("BP_AnimalDeer_C", 15.0),
        Some("08zh_wolf") => ("BP_AnimalWolf_C", 12.0),
        _ => return,
    };
    if let Some(t) = toasts.last() {
        stage.5 = t.clone();
    }
    let (Some(kind), Some(start), Ok(mut me)) = (ZombieKind::from_class(class), tour.start, player.single_mut()) else { return };
    let health = |g: &CurrentGame| g.state.as_ref().map_or(0.0, |s| s.survival.health);
    // Times below are from when the animal is placed (`stage.6`).
    let since = tour.t - stage.6;
    // Until the wolf has bitten, zombies and hounds coming within 30 m are put down too: hounds
    // run fast and spawners respawn, and one's 4-point bite was once taken for the wolf's.
    if step == Some("08zh_wolf") && stage.1 != 0 && !bitten.0 {
        for (t, _, animal, h, dead) in &mut animals {
            if animal.is_none() && !dead && t.translation.distance(me.translation) < 30.0
                && let Some(mut h) = h
            {
                h.hp = 0.0;
            }
        }
    }
    match stage.1 {
        // Back at the spawn, then 1 s for its props to stream in: placed at once, a prop that
        // loaded after the clear-view test could hide the survivor (the deer then never saw them).
        0 => {
            me.translation = start;
            stage.1 = 5;
        }
        5 if tour.t > 1.0 => {
            // The wolf's bite is measured off the survivor's health: zombies and hounds roaming
            // the spawn bit too (a 3-point "bite" once), so those near are put down first.
            if step == Some("08zh_wolf") {
                for (t, _, animal, h, dead) in &mut animals {
                    if animal.is_none() && !dead && t.translation.distance(start) < 80.0
                        && let Some(mut h) = h
                    {
                        h.hp = 0.0;
                    }
                }
            }
            let Ok(cam) = camera.single() else { return };
            // Ahead of the camera, or the first of 16 headings round from there (at the distance,
            // then a little nearer) with nothing between the animal and the survivor (the spawn
            // camp's tents and the trees' trunks block sight).
            let look = cam.forward().as_vec3();
            let yaw0 = look.x.atan2(look.z);
            let place = |k: usize| {
                let yaw = yaw0 + (k % 16) as f32 * std::f32::consts::FRAC_PI_8;
                let ahead = Vec3::new(yaw.sin(), 0.0, yaw.cos());
                let mut at = start + ahead * dist * if k < 16 { 1.0 } else { 0.75 };
                at.y = terrain.as_ref().map_or(at.y, |t| t.height(at.x, at.z));
                (ahead, at)
            };
            // The lines `animals::sense` traces: the animal's centre to the survivor's (where they
            // stand now), past the props and the landscape.
            let eye = me.translation + Vec3::Y * 0.82;
            let clear = |rays: &mut crate::level::collision::PropRays, at: Vec3| {
                let from = kind.centre(&Transform::from_translation(at));
                rays.cast(from, (eye - from).normalize_or_zero(), eye.distance(from)).is_none() && crate::gameplay::zombies::clear_line(terrain.as_deref(), from, eye)
            };
            let pick = (0..32).find(|&k| clear(&mut rays, place(k).1));
            let (ahead, at) = place(pick.unwrap_or(0));
            info!("autoshot {class} placed at {at:.1} (try {pick:?}; None = no clear view)");
            let (model, material) = kind.pick_look(&mut rand::thread_rng());
            let facing = Quat::from_rotation_y((-ahead).x.atan2((-ahead).z));
            let lz = crate::level::LevelZombie { home: at, spawner: None, model, material, kind };
            stage.2 = Some(crate::level::spawners::spawn_zombie(&mut commands, &assets, Transform::from_translation(at).with_rotation(facing), lz));
            stage.3 = at;
            if let Some(st) = game.state.as_mut() {
                st.survival.health = st.survival.max_health;
                if !st.tech.has(crate::progression::tech::HUNTSMAN) {
                    st.tech.unlocked.push(crate::progression::tech::HUNTSMAN.into());
                }
            }
            stage.4 = health(&game);
            stage.6 = tour.t;
            stage.1 = 1;
        }
        1 => {
            let Some((_, loco, a, _, _)) = stage.2.and_then(|e| animals.get(e).ok()) else { return };
            // Whether it ran from them at all: the flee branch's "Can See Player" decorator aborts
            // it (FlowAbortMode Both) as soon as the survivor is in the deer's blind spot behind,
            // so a deer bolting straight away is back to roaming within the second.
            stage.7 |= a.is_some_and(|a| matches!(a.brain, crate::gameplay::animals::Brain::Flee { .. }));
            // The wolf: its first bite (a drop while it's attacking; roaming zombies hit too).
            if step == Some("08zh_wolf") && !bitten.0 && health(&game) < stage.4 - 1.0 {
                if *loco == crate::models::Locomotion::Attack {
                    bitten.0 = true;
                    info!("autoshot wolf bit for {:.0} at {:.1}", stage.4 - health(&game), tour.t);
                } else {
                    stage.4 = health(&game);
                }
            }
            // The deer 1.5 s on; the wolf once it's had time to bite (it arrives ~1.4 s in, and
            // each bite takes the AttackMontage's 1.03 s).
            let at = if step == Some("08zg_deer") { 1.5 } else { 4.5 };
            if since > at {
                let Some((t, loco, a, _, _)) = stage.2.and_then(|e| animals.get(e).ok()) else { return };
                let brain = a.map(|a| format!("{:?}", a.brain).split([' ', '{']).next().unwrap_or("").to_string()).unwrap_or_default();
                info!(
                    "autoshot {} brain {brain} loco {:?} moved {:.1} from_player {:.1} seen {} fled {}",
                    if step == Some("08zg_deer") { "deer" } else { "wolf" },
                    loco,
                    t.translation.distance(stage.3),
                    t.translation.xz().distance(me.translation.xz()),
                    a.is_some_and(|a| a.seen),
                    stage.7
                );
                if step == Some("08zh_wolf") {
                    info!("autoshot wolf bitten {}", bitten.0);
                }
                // `animals::sense`'s test, part by part, from where the animal is now.
                if let Some(traits) = kind.animal() {
                    let (centre, eye) = (kind.centre(t), me.translation + Vec3::Y * 0.82);
                    let cone = crate::gameplay::animals::in_sight(centre, t.rotation * Vec3::Z, eye, traits.sight, traits.peripheral);
                    let land = crate::gameplay::zombies::clear_line(terrain.as_deref(), centre, eye);
                    let prop = rays.cast(centre, (eye - centre).normalize_or_zero(), eye.distance(centre));
                    info!("autoshot {class} sight cone {cone} terrain_clear {land} prop_hit {prop:?} centre {centre:.2} eye {eye:.2}");
                }
                stage.1 = 2;
            }
        }
        2 => {
            // Killed outright (kill_zombies: XP, the toast, the corpse).
            if let Some((_, _, _, Some(mut h), _)) = stage.2.and_then(|e| animals.get_mut(e).ok()) {
                h.hp = 0.0;
            }
            stage.1 = 3;
        }
        3 if since > 3.0 && step == Some("08zg_deer") => {
            let Some((t, loco, _, _, dead)) = stage.2.and_then(|e| animals.get(e).ok()) else { return };
            let side = (t.rotation * Vec3::Y).dot(Vec3::Y);
            let corpse = containers.list.iter().any(|c| c.2 == class && c.1.distance(t.translation) < 2.0);
            let meat = containers.list.iter().any(|c| c.2 == "Container_AnimalMeat_C" && c.1.distance(t.translation) < 2.0);
            let def = crate::gameplay::interact::container_def(&crate::gameplay::ItemDb::load(), class).map(|d| (d.name, d.table));
            info!("autoshot deer dead {dead} loco {loco:?} up_dot {side:.2} corpse {corpse} container {def:?} meat {meat} toast {:?}", stage.5);
            stage.1 = 4;
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
    shoving: Res<crate::gameplay::shove::Shoving>,
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
        // Once the stomp has run its course (a late start used to be logged before its hit).
        6 if tour.t > 3.5 && shoving.0.is_none() => {
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
    (shoving, armed): (Res<crate::gameplay::shove::Shoving>, Res<crate::gameplay::Armed>),
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
        // G once nothing would refuse it (`throwables::start`: a shove or stomp still running, a
        // busy weapon).
        1 if tour.t > 0.3 && shoving.0.is_none() && !armed.busy() => {
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
            // With CROUCH TOGGLE (the game's default) letting go of Ctrl doesn't stand them up:
            // tap it again first.
            if body.height < 1.3 {
                stage.0 = 10;
                return;
            }
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
        (Some("08x_fall"), 10) => {
            keys.press(KeyCode::ControlLeft);
            stage.0 = 11;
        }
        (Some("08x_fall"), 11) => {
            keys.release(KeyCode::ControlLeft);
            stage.0 = 0;
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

/// 08zu_boss: back at the spawn, zombies near it put down, a BP_ZombieBoss_Military_Small 8 m
/// ahead (the first heading with a clear view) facing the survivor. Logs the first hit off the
/// survivor's health with how far the boss stood, then at 6 s its brain and the swings; kills it
/// and 2 s later logs the toast, the boss kill count and the corpse's container and unique sets.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn tour_boss(
    mut commands: Commands,
    tour: Res<Tour>,
    assets: Res<AssetServer>,
    mut game: ResMut<CurrentGame>,
    mut bitten: ResMut<WolfBitten>,
    camera: Query<&GlobalTransform, With<crate::gameplay::PlayerCamera>>,
    terrain: Option<Res<crate::level::Terrain>>,
    containers: Res<crate::gameplay::interact::Containers>,
    mut rays: crate::level::collision::PropRays,
    mut notes: MessageReader<crate::gameplay::notify::Notify>,
    mut player: Query<&mut Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    mut zombies: Query<(&Transform, &crate::level::LevelZombie, Option<&crate::gameplay::zombies::Zombie>, Option<&crate::models::UpperAction>, Option<&mut crate::gameplay::combat::Health>, Has<crate::gameplay::combat::Dead>), Without<crate::models::CharacterModel>>,
    // (step, stage, boss, health before, toast, placed at, swings seen, last action serial)
    mut stage: Local<(usize, u8, Option<Entity>, f32, String, f32, u32, u32)>,
) {
    use crate::gameplay::zombies::ZombieKind;
    let toasts: Vec<String> = notes.read().map(|n| n.text.clone()).filter(|t| t.contains("Killed")).collect();
    if stage.0 != tour.step {
        *stage = (tour.step, 0, None, 0.0, String::new(), 0.0, 0, 0);
    }
    if TOUR.get(tour.step).map(|s| s.2) != Some("08zu_boss") {
        return;
    }
    if let Some(t) = toasts.last() {
        stage.4 = t.clone();
    }
    const CLASS: &str = "BP_ZombieBoss_Military_Small_C";
    let (Some(kind), Some(start), Ok(mut me)) = (ZombieKind::from_class(CLASS), tour.start, player.single_mut()) else { return };
    let health = |g: &CurrentGame| g.state.as_ref().map_or(0.0, |s| s.survival.health);
    let since = tour.t - stage.5;
    match stage.1 {
        0 => {
            me.translation = start;
            bitten.0 = false;
            stage.1 = 5;
        }
        5 if tour.t > 1.0 => {
            for (t, _, _, _, h, dead) in &mut zombies {
                if !dead && t.translation.distance(start) < 80.0
                    && let Some(mut h) = h
                {
                    h.hp = 0.0;
                }
            }
            let Ok(cam) = camera.single() else { return };
            let look = cam.forward().as_vec3();
            let yaw0 = look.x.atan2(look.z);
            let place = |k: usize| {
                let yaw = yaw0 + k as f32 * std::f32::consts::FRAC_PI_8;
                let ahead = Vec3::new(yaw.sin(), 0.0, yaw.cos());
                let mut at = start + ahead * 8.0;
                at.y = terrain.as_ref().map_or(at.y, |t| t.height(at.x, at.z));
                (ahead, at)
            };
            let eye = me.translation + Vec3::Y * 1.6;
            let clear = |rays: &mut crate::level::collision::PropRays, at: Vec3| {
                let from = at + Vec3::Y * 1.6;
                rays.cast(from, (eye - from).normalize_or_zero(), eye.distance(from)).is_none() && crate::gameplay::zombies::clear_line(terrain.as_deref(), from, eye)
            };
            let pick = (0..16).find(|&k| clear(&mut rays, place(k).1));
            let (ahead, at) = place(pick.unwrap_or(0));
            info!("autoshot boss placed at {at:.1} (try {pick:?}; None = no clear view)");
            let (model, material) = kind.pick_look(&mut rand::thread_rng());
            let facing = Quat::from_rotation_y((-ahead).x.atan2((-ahead).z));
            let lz = crate::level::LevelZombie { home: at, spawner: None, model, material, kind };
            stage.2 = Some(crate::level::spawners::spawn_zombie(&mut commands, &assets, Transform::from_translation(at).with_rotation(facing), lz));
            if let Some(st) = game.state.as_mut() {
                st.survival.health = st.survival.max_health;
            }
            stage.3 = health(&game);
            stage.5 = tour.t;
            stage.1 = 1;
        }
        1 => {
            let Some((t, _, brain, action, _, _)) = stage.2.and_then(|e| zombies.get(e).ok()) else { return };
            if let Some(a) = action
                && a.serial != stage.7
            {
                stage.7 = a.serial;
                if a.kind == Some(crate::models::ActionKind::Attack) {
                    stage.6 += 1;
                }
            }
            let dist = t.translation.xz().distance(me.translation.xz());
            if !bitten.0 && health(&game) < stage.3 - 1.0 {
                bitten.0 = true;
                info!("autoshot boss hit for {:.0} at {since:.1} s from {dist:.1} m after {} swings", stage.3 - health(&game), stage.6);
            }
            if since > 6.0 {
                let brain = brain.map(|z| format!("{:?}", z.brain).split([' ', '{']).next().unwrap_or("").to_string()).unwrap_or_default();
                info!("autoshot boss brain {brain} swings {} hit {} from_player {dist:.1} scale {:.2}", stage.6, bitten.0, t.scale.y);
                if let Some((_, _, _, _, Some(mut h), _)) = stage.2.and_then(|e| zombies.get_mut(e).ok()) {
                    info!("autoshot boss hp {:.0}", h.max);
                    h.hp = 0.0;
                }
                stage.1 = 2;
            }
        }
        2 if since > 8.0 => {
            let Some((t, _, _, _, _, dead)) = stage.2.and_then(|e| zombies.get(e).ok()) else { return };
            let corpse = containers.list.iter().find(|c| c.2 == CLASS && c.1.distance(t.translation) < 2.0).map(|c| c.0);
            let unique = corpse.and_then(|k| containers.spawned.get(&k)).map(|u| u.1.join(",")).unwrap_or_default();
            let def = crate::gameplay::interact::container_def(&crate::gameplay::ItemDb::load(), CLASS).map(|d| (d.name, d.table));
            let kills = game.state.as_ref().map_or(0, |s| s.stats.boss_zombie_kills);
            info!("autoshot boss dead {dead} boss_kills {kills} corpse {} container {def:?} unique {unique} toast {:?}", corpse.is_some(), stage.4);
            stage.1 = 3;
        }
        _ => {}
    }
}

/// 08zze_horde / 08zzf_horde_end: at the spawn at noon with two Horde Beacons, build mode
/// (LaunchBuild) looking 25° down; 1 s on the ghost is logged and LMB places one (one left, so
/// build mode carries on), RMB stops it; the survivor steps 2 m from the beacon and presses F;
/// the countdown's timers are skipped and at 6.5 s the horde (its toasts, its AI and how far
/// they are from the beacon) is logged. The next step kills them, moves the beacon to its last
/// wave and at 3 s logs the toasts, what's left of the beacon and the rewards around it.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn tour_horde(
    tour: Res<Tour>,
    terrain: Option<Res<crate::level::Terrain>>,
    mut game: ResMut<CurrentGame>,
    items: Res<crate::gameplay::ItemDb>,
    build: Res<crate::gameplay::building::BuildMode>,
    mut placed: ResMut<crate::gameplay::building::Buildables>,
    ground: Res<crate::gameplay::inventory_ui::Ground>,
    mut launch: MessageWriter<crate::gameplay::building::LaunchBuild>,
    mut notes: MessageReader<crate::gameplay::notify::Notify>,
    (mut keys, mut mouse): (ResMut<ButtonInput<KeyCode>>, ResMut<ButtonInput<MouseButton>>),
    (mut yaw, mut pitch): (ResMut<crate::gameplay::combat::CameraYawRequest>, ResMut<crate::gameplay::combat::CameraPitchRequest>),
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), With<crate::models::CharacterModel>>,
    mut zombies: Query<(&Transform, Option<&mut crate::gameplay::combat::Health>), Without<crate::models::CharacterModel>>,
    // (step, stage, stand at, toasts, the beacon's place)
    mut stage: Local<(usize, u8, Option<Transform>, Vec<String>, Vec3)>,
) {
    use crate::gameplay::building::Phase;
    let fresh: Vec<String> = notes.read().map(|n| n.text.clone()).collect();
    let name = TOUR.get(tour.step).map(|s| s.2);
    if stage.0 != tour.step {
        let keep = (stage.3.clone(), stage.4);
        *stage = (tour.step, 0, None, Vec::new(), Vec3::ZERO);
        if name == Some("08zzf_horde_end") {
            (stage.3, stage.4) = (Vec::new(), keep.1);
        }
    }
    if !matches!(name, Some("08zze_horde" | "08zzf_horde_end")) {
        return;
    }
    stage.3.extend(fresh);
    if let (Some(at), Ok((mut t, mut body))) = (stage.2, player.single_mut()) {
        *t = at;
        body.vertical_velocity = 0.0;
    }
    if name == Some("08zzf_horde_end") {
        match stage.1 {
            0 => {
                let Some(h) = placed.list.first_mut().and_then(|l| l.horde.as_mut()) else { return };
                for &e in &h.enemies {
                    if let Ok((_, Some(mut hp))) = zombies.get_mut(e) {
                        hp.hp = 0.0;
                    }
                }
                h.wave = 7;
                h.phase = Phase::Checking;
                h.timer = 0.5;
                stage.1 = 1;
            }
            1 if tour.t > 3.0 => {
                let near = ground.stacks().filter(|(p, _)| p.distance(stage.4) < 4.0).map(|(_, s)| format!("{} x{}", s.item, s.count)).collect::<Vec<_>>();
                info!("autoshot horde end beacons {} rewards {near:?} toasts {:?}", placed.list.len(), stage.3);
                stage.1 = 2;
            }
            _ => {}
        }
        return;
    }
    match stage.1 {
        0 => {
            let (Some(start), Some(terrain)) = (tour.start, terrain.as_ref()) else { return };
            // 30 m from the spawn, clear of 08zz_wanderer's traders (their prompt takes F).
            let off = start + Vec3::new(0.0, 0.0, 30.0);
            let me = off.with_y(terrain.ground_under(off + Vec3::Y * 30.0).unwrap_or(start.y));
            stage.2 = Some(Transform::from_translation(me + Vec3::Y * 0.05));
            if let Some(st) = game.state.as_mut() {
                st.environment.time_of_day_hours = 12.0;
            }
            let _ = crate::gameplay::interact::player_inv(&mut game).insert(&items, items.stack("DA_HordeBeacon", 2));
            launch.write(crate::gameplay::building::LaunchBuild { item: "DA_HordeBeacon".into() });
            yaw.0 = Some(std::f32::consts::FRAC_PI_2);
            pitch.0 = Some(25f32.to_radians());
            stage.1 = 1;
        }
        1 if tour.t > 1.0 => {
            pitch.0 = Some(25f32.to_radians());
            info!("autoshot horde ghost on {} can build {} at {:.1}", build.on(), build.can_build, build.transform.translation);
            mouse.press(MouseButton::Left);
            stage.1 = 2;
        }
        2 if tour.t > 1.2 => {
            mouse.release(MouseButton::Left);
            let left = crate::gameplay::interact::player_inv(&mut game).count("DA_HordeBeacon");
            info!("autoshot horde placed {} left {left} building {}", placed.list.len(), build.on());
            mouse.press(MouseButton::Right);
            stage.1 = 3;
        }
        3 if tour.t > 1.4 => {
            mouse.release(MouseButton::Right);
            info!("autoshot horde stopped building {}", !build.on());
            let Some(b) = placed.list.first().map(|l| l.transform.translation) else { return };
            stage.4 = b;
            let me = stage.2.map_or(b, |t| t.translation);
            let to = (me - b).with_y(0.0).normalize_or(Vec3::X);
            let mut at = b + to * 2.0;
            at.y = terrain.as_ref().and_then(|t| t.ground_under(at + Vec3::Y * 3.0)).unwrap_or(at.y) + 0.05;
            stage.2 = Some(Transform::from_translation(at));
            // Look back over the beacon at the ground beyond it.
            yaw.0 = Some(to.x.atan2(to.z));
            pitch.0 = Some(15f32.to_radians());
            stage.1 = 4;
        }
        // F once the survivor stands there (the teleport applies on the next frame).
        4 if tour.t > 1.6 => {
            keys.press(KeyCode::KeyF);
            stage.1 = 5;
        }
        5 if tour.t > 1.8 => {
            keys.release(KeyCode::KeyF);
            stage.1 = 6;
        }
        6 => {
            // Skip the countdown and the wave's intro.
            if let Some(h) = placed.list.first_mut().and_then(|l| l.horde.as_mut())
                && matches!(h.phase, Phase::Activated | Phase::Countdown(_) | Phase::WaveIntro)
            {
                h.timer = h.timer.min(0.0);
            }
            if tour.t > 6.5 {
                let h = placed.list.first().and_then(|l| l.horde.as_ref());
                let away: Vec<String> = h.map(|h| h.enemies.iter().filter_map(|&e| zombies.get(e).ok()).map(|(t, _)| format!("{:.1}", t.translation.distance(stage.4))).collect()).unwrap_or_default();
                info!("autoshot horde started {} wave {} phase {:?} enemies {} away {away:?} toasts {:?}", h.is_some_and(|h| h.started), h.map_or(0, |h| h.wave + 1), h.map(|h| h.phase), h.map_or(0, |h| h.enemies.len()), stage.3);
                stage.1 = 7;
            }
        }
        _ => {}
    }
}
