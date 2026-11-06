# SurrounDead → Rust port

A Rust/Bevy reimplementation of **SurrounDead** (UE 5.3 open-world zombie survival, game build
`Preparation - v0.7.1g`). Remote: `github.com/dlewi069/RustDead`.

The goal is parity with the original: layouts, text, data and art come from the game's own
assets wherever possible, not from invention. When something has to be guessed, say so in a
comment (see `menus/settings.rs::default_value`).

## Needs the real game (the user's rule, 2026-11-04, restated 2026-11-06)

You have to own SurrounDead to play this. Its assets are pulled from the player's own Steam
install, and nothing from the game is shipped here.

- **Only code goes to GitHub.** No game files or anything generated from them: `assets/game/`,
  `assets/data/`, audio, textures, fonts, models, saves, screenshots. `.gitignore` lists them, and
  `scripts/check_no_game_files.sh` enforces it from `.git/hooks` (pre-commit runs it with
  `--staged`, pre-push with each pushed rev). Hooks don't come with a clone, so add them again in a
  new one, and never bypass them with `--no-verify`.
- **Getting the assets:** `scripts/verify_ownership.py` looks for the install in the Steam
  libraries (app 1645820). `extractor/export_assets.sh --all` exports what the port loads into
  `assets/game/` and generates `assets/data/` (set `SD_GAME_DIR` if the game is somewhere else).
  `scripts/first_time_extractor.py` only covers fonts, backgrounds and models.
- **Not proven yet:** `verify_ownership.py` only checks that the `.pak` exists, and the setup
  hasn't been run from a fresh clone, so it may not produce everything the port needs.
- `assets/data/` has been git-ignored since 2026-11-04, so where this file or a script says
  "committed JSON", read "generated locally".
- Keep tokens out of the remote URL: `.git/config` is plain text and `git remote -v` prints it.
  Use `gh auth login` or a credential helper.

## Stack

- Bevy **0.20** (`Cargo.toml`), Rust edition 2024. 0.20 API notes that bite:
  - Headless widgets: `bevy::ui_widgets::{Button, Activate}`; `Hovered` and `ui::Pressed` drive
    styling. The old `Interaction` is deprecated.
  - glTF scenes spawn via `WorldAssetRoot(assets.load(GltfAssetLabel::Scene(0).from_asset(..)))`,
    and readiness is `On<WorldInstanceReady>` (`bevy::world_serialization`).
  - Events are `Message`s (`MessageWriter<AppExit>`); state-scoped despawn is `DespawnOnExit(S)`.
  - `NextState::set` with the *current* state still re-runs `OnExit`/`OnEnter`. Loading a save
    in-game relies on that to rebuild the world.
  - A bundle containing the same component twice panics at runtime (e.g. `label(..)` already has
    `TextColor`; use `label_colored`).
  - The initial `OnEnter` runs before `Startup`; resources it needs must be `FromWorld`/inserted in `build`.
- `cargo run` starts at the main menu. `cargo test` covers save-date formatting and that every
  character-creator option resolves to an exported mesh.
- `SD_MENU_SHOTS=<dir> cargo run` tours every menu page plus the world (spawn view, a walk with W
  held, a container search, the CRAFTING tab, the split dialog, the dressed survivor, a rifle drawn
  and fired at a zombie, side-on with the rifle in hand, a roamer shoved (V) then stomped looking down, a frag grenade thrown (G) at a roamer (the survivor steps 8 m back while it's in the air: the blast is 85-115 within 10 m), a Molotov setting a roamer burning, a zombie hound, a creator face close-up, a corpse opened as loot, a thunderstorm and a dust storm, a door opened with F, the keypad door's note and its code typed in, a ladder climbed and stepped off at the top, a lab generator switched on, the police briefcase searched (the generator key) then the police generator fuelled and started and the lever pulled (the garage gate folds up), a crouch walk, an 8 m fall in heavy armor, the flashlight at midnight, sitting down, the journal's tech tree, the journal's INFO page, a POI discovered and the map opened with a GPS (a marker placed 150 m ahead with a double click, its compass icon checked), the music at the spawn, a Combat Roll that puts out a fire, a swim in a lake: afloat, a dive, back up, a 2.1 m board fence climbed over with Space, a deer that flees and is killed (Huntsman's meat bag), a wolf that bites, the Supplies Trader's dialogue, then its trade screen with an item bought and one sold, an airdrop crate falling under its parachute, then landed and searched, a tree near the spawn chopped down with a Fireman's Axe, a burnt-out vehicle salvaged, a trash pile broken with a Crowbar, a fish caught from the lake's shore, the vehicle nearest the spawn driven over a roamer and parked, a Military Intel Laptop scanned and its Classified Container quest done (crate searched, weapon dropped, 400 XP), the journal's QUESTS page with FORGET asked and declined, a crafting station opened and Nails crafted at it, an Undead Boss that closes in, swings until its arm trace lands, and dies into an "Undead Boss" corpse with its unique sets, a barrel fire burning and a frag blast that plays out, an abandoned camp spawned with its crates and zombie spawner, a bandit camp whose bandits shoot and die, the elimination quest, two wandering traders: one talked to and traded with while it stands still, one killed). In-game steps
  close any menu stray input opens (logged as "opened during"), keep the survivor's health full
  (`keep_alive`: roaming zombies used to kill them), writes PNGs, logs FPS/positions/shot
  hits and exits (`src/menus/autoshot.rs`). The tour forces vsync on the window (`steady_frames`; uncapped 200+ fps made timed steps flaky). `SD_START=<n>` pins the PlayerStart; `SD_TOUR_UNTIL=<step>` exits after that step (e.g. `06a_preset` for the menus only, ~30 s); `SD_BURST=08g_armed,...` also saves a frame every 0.25 s of those steps. Use it to check
  changes visually.
- `SD_VIEW=x,y,z,yaw[,pitch[,hour]] SD_VIEW_SHOT=<png> cargo run` (`menus/viewshot.rs`): a new
  game holding the survivor at that spot, a screenshot after `SD_VIEW_WAIT` (10) s, exit.
  `SD_VIEW_ITEMS=DA_a,DA_b` drops items at the feet and opens the inventory (icon checks);
  `+DA_a` wears it instead. `SD_VIEW_PAGE=bag_window` (`TourPage::named`) shows a tour inventory page.
- Dev builds use `opt-level = 1` (deps at 3); the map is too heavy for unoptimised Bevy.

## Testing workflow (required)

- **While working**: after each change run `scripts/test_related.sh` (tests for the files changed
  vs git HEAD: `src/a/b.rs` -> `cargo test a::b::`, data JSON -> the modules that read it, python
  -> compile + data checks). It lists changed Rust files that have no `#[test]`: add tests.
- **New tests go in the full suite** automatically when they are `#[test]`s in `src/`, functions
  in `CHECKS` of `scripts/check_data.py` (data consistency) or `scripts/check_tour.py` (tour
  assertions), or new steps in `src/menus/autoshot.rs`'s TOUR. Add one of those for every feature.
- **The full debug cycle runs ONCE, at the end of each prompt (the user's rule, restated
  2026-10-24).** That means `scripts/check.sh` once, after all of the prompt's features are in,
  testing them all together. Do NOT, mid-prompt:
  - run `scripts/check.sh` after a feature or a fix;
  - run the whole screenshot tour (`SD_MENU_SHOTS=... cargo run`): it is the same ~3 min and the
    same flakiness. Write the new tour step and its `check_tour.py` assertions while building the
    feature and let the end-of-prompt check run them.
  Allowed mid-prompt: `cargo build`, `scripts/test_related.sh`, single `cargo test`s, and
  `SD_VIEW` single-spot screenshots (`menus/viewshot.rs`, ~20 s) for a visual question.
- **If the end-of-prompt check fails**: on the new work, fix it and run `scripts/check.sh` again
  (still the end of the prompt). On a step listed as flaky below, don't rerun to chase a green
  run: report it, and add to its entry here if it's a new variant.
- A targeted tour must match check.sh: `SD_START=3 SD_MENU_SHOTS=<dir> cargo run -q`, then
  `python3 scripts/check_tour.py <log> <dir> <exit code>`. Running `target/debug/surroundead`
  directly loses `CARGO_MANIFEST_DIR` (Bevy's asset root) and nothing loads; without `SD_START`
  the start is random and steps land elsewhere. Flaky steps found and fixed 2026-10-22: 08zg_deer
  (a deer fleeing straight away has the survivor in its blind spot, and the game's Can See Player
  decorator, FlowAbortMode Both, aborts the flight: the step now checks it fled at all), 08ib/08ic
  (a late stomp outlived its step and swallowed the grenade's G: the stomp check waits for the
  move to end, G waits for no shove and an idle weapon), 08zp_fish (the rod tip sits 0.3 m over
  the bank: cast at 15° down, not 35°). Still seen now and then: 08u_ladder_top (2026-10-22, once: back at the
  foot, 8.7 m behind the ladder), 08zg_deer (fled only 2.8 m; twice in a row on 2026-10-24: brain Flee, loco Idle, 2.3 m; 2026-10-28: Flee / Run, 1.8 m; 2026-10-30: Flee / Run, 1.1 m, then Roam / Walk, 2.2 m; 2026-11-02 in the Stop hook's run: Roam / Walk, 2.3 m), 08zh_wolf (a 4-point bite: a hound's
  hit counted again, again 2026-10-30 in the Stop hook's run; 2026-10-30, twice: a 2-point bite, another damage tick caught while the wolf was in its attack pose; fixed: the check now reads `survival::zombie_hit`'s "survivor hit by Wolf for N" log), 08m_hound (effect rolls),
  08ze_swim, 08zf_vault (logs the body's state at the press; 2026-10-28, once: grounded, 100 stamina, the traces found no wall at all, kind None, so the fence's cell may not have streamed in yet; again 2026-10-30, twice, and 2026-11-01 and 2026-11-02 in the Stop hook's runs, the same log), 08zq_drive (2026-10-31, once: a random SUV off-road at [2271, 16, 2430] reached 5.9 mph and stopped dead after 0.9 m, something in its way; the roamer check fails with it; 2026-11-02 in the Stop hook's run: a Utility Truck at [1516, 14, 2582] hit a prop at 23 mph after 14.5 m, before the roamer) + 08ib_shove (2026-10-23,
  once in the same run: moved 0.7 m; roamer not shoved), 08zz_wanderer ("met 2:
  A or B not a trader": 2026-10-23, and twice on 2026-10-30; cause found: A and B were spawned while
  the survivor was still at the previous step, and the AI optimiser stored them and brought them back as new entities;
  the step now finds them again by their spawn points), 08zq_drive (twice: F opened the run-over roamer's corpse beside the car,
  Loot(CORPSE_KEYS + n), instead of its storage; fixed 2026-10-24: the step moves corpses within 4 m of the car out of reach), 08zj_trade (once: "Not enough money!" with 200), 08zzc_turret (2026-11-02 in the Stop hook's run: an Undead Feral Hound bit the survivor through the whole step, so the turret's trace met the hound first: it saw the player, ammo 5, but no "turret shot the survivor"), 08zzd_blood_moon (three times by 2026-10-24: the
  roamer not roused, 0 m/s), 08ib_shove (several times: no shove at all; `shove.rs` now logs "shove: V
  ignored" with the reason when a press is refused; 2026-10-30 again with neither log, so the step
  now releases V before each press and logs "autoshot shove press V held B"; the hook's rerun showed V
  not held but the roamer walked off to 1.9 m, out of the shove's reach: the step's roamer is now
  HeldStill, and `shove.rs` logs each move's start), 08zzzb_barber
  (2026-10-30, once: a wandering trader stood at the chair and F opened its Dialogue; the step now
  moves humans within 5 m of the seat 15 m away before F), 08zzs_shower (2026-10-30, once: the same
  with a wandering trader beside the shower; its open Dialogue then blocked every later step, 12
  errors through 08zzu_infestation. `clear_people` now clears 6 m round the shower, and `tour`
  closes a Dialogue left open at any in-game step's start except 08zj_trade, logging "closed a
  leftover"), 08zh_wolf, a missing last screenshot
  (12_suicide, once), and on 2026-10-24 (night, once, all passing the run before): 08zk_airdrop (aircraft
  49 m/s, crate 8.5 m/s, no toast) + 08zl_airdrop_loot, 08zp_fish (no water along the cast), 08zv_fx
  (blast emitter still there at 6 s), 08zzn_traps (the wire's AI still held). 08zzu_infestation (2026-10-30, once, passing the run before: the 400 round wasn't in yet when stage 3 logged, secondary 1000, waves false; the step writes the Shot and logs one frame later, so it hangs on system order). The tour runs on the viewer's saves/settings.json; steps that depend on a
  setting set it themselves (`airdrop_notice`) or work either way (crouch toggle).

- Never run `scripts/check.sh` in the background and end the turn: the Stop hook starts its own
  run, two games share `target/check/` and the screen, and the logs interleave (2026-10-28: 7
  bogus failures). Run it in the foreground. Since 2026-10-30 (113 steps) the full run can pass the
  10 min tool limit and get moved to the background (then the Stop hook's run collides with it):
  run `scripts/check.sh --quick` in the foreground, then the tour by hand as check.sh does it
  (`SD_RADIO_DIR=$PWD/target/check/radio SD_START=3 SD_MENU_SHOTS=$PWD/target/check/shots timeout 585
  cargo run -q > target/check/tour.log`, then `check_tour.py` on it). Since 2026-10-30 evening
  (117 steps) the tour itself reaches the last step (12_suicide) right at 585 s, so the hand run
  times out (exit 124) before quitting; check_tour.py still passed all 90 checks on that log. If
  the tour grows further, run it with `run_in_background` and wait for it in the same turn
  (the hook only collides with a run still going when the turn ends); check.sh allows 900 s.
  Since 2026-10-30 night (118 steps, 08zzzh_head_gear) the tour no longer fits a foreground call,
  and a background one collided with the hook again (a monitor doesn't keep the turn open: the
  turn ended, the hook's tour started, both wrote target/check/tour.log; 08zzzg_nuke logged twice,
  2 bogus errors). So at the end of a prompt run `scripts/check.sh --quick` in the foreground and
  leave the tour to the Stop hook's own check.sh run (900 s); don't start a tour of your own.

- A tour run whose window is hidden (another workspace) gets ~1 fps on Wayland (no frame
  callbacks), so timed steps fail. `env -u WAYLAND_DISPLAY` (XWayland) keeps 60 fps, but Hyprland
  may resize the window mid-run, which changes the UI scale in the shots. 2026-10-29: the window
  came out 945x1025 (portrait, tiled) and 08zzt_attachments failed: its check expected the ACOG's
  25 degrees across to be ~14 tall, true only at 16:9; it now logs the camera's aspect and
  checks the height that aspect gives (27 at that shape).

- **Progress note**: keep `/home/lev/Qwen/obsidian/SurrounDead in Rust.md` (the user's Obsidian
  vault) current as features land: What works / Not done yet / dated Log.

## Porting playbook (how to finish the port efficiently)

- **What's left**: the roadmap in the Obsidian note (`## Not done yet (roadmap)`: every
  unported Blueprint / widget / level actor, grouped, sized S/M/L, with a "Next up" order). Take
  the next item from there unless the user says otherwise, and move it to "What works" + the
  Log when done. The audit behind it: level actor classes from `actors.json` vs what `src/`,
  `extractor/` and the data files mention; Blueprints listed with `SdExport list`.
- **Read the game first** (never guess): `scripts/dump_bp.sh` dumps all gameplay Blueprints with
  bytecode into `target/bpdump/` (~1 min; re-run after `cargo clean`). Then:
  - who does X: `grep -rl "SpawnFloatingDamageText" target/bpdump`;
  - logic: `python3 extractor/kismet.py target/bpdump/<path>.json [function]` (offsets are jump
    targets; a Delay's `LatentActionInfo(N, ...)` resumes at N; `pop_flow if not` = Sequence/gate);
  - defaults and components: the `Default__<Class>_C` export and `<Name>_GEN_VARIABLE` templates
    (`SCS_Node` parents; children inherit through `SuperStruct`, overrides in
    InheritableComponentHandler);
  - widgets: `python3 extractor/widget_tree.py target/bpdump/<widget>.json`;
  - level placements: `assets/game/levels/LongdownValley/actors.json` (Bevy-space transforms, per
    instance `props`); a component's position in UE cm = Bevy (x, z, y) x 100;
  - any other package: `scripts/dump_bp.sh <out> SurrounDead/Content/<path>`.
- **Where data goes**: per-class data a Blueprint holds goes into the matching generator
  (`gen_buildables.py`, `gen_items.py`, `gen_ai.py`...) and its committed JSON, not hard-coded,
  unless it's a literal inside bytecode (then cite the function in a comment). Keep enum byte
  values straight (Enum `Names` maps names to bytes; they're not in display order).
- **Module shape**: a new `src/gameplay/<feature>.rs` with a doc header that says what the
  Blueprint does (decompiled, with its numbers), then "Ours:" for every deviation and "Not
  ported:"; pure functions for the rules (unit-tested); systems after the existing sets; a tour
  step + `check_tour.py` assertion; CLAUDE.md layout entry; Obsidian note.
- **Reusable pieces**: `building::add_live` (place a buildable directly), `building::save_placed`,
  `Live` fields for per-buildable state, `storage::STORAGE_KEYS` / `building::BENCH_KEYS` (Loot key
  ranges), `inventory_ui::PlacedStation` (any crafting table), `GameOverlay` variants for
  modal UIs (Esc closes; exempt in `close_overlay`), `turrets::Shot` / `harvest::Swing` /
  `barrels::ObjectBlast` (BPI Damage_Object sources), `turrets::NoCredit` (kills without XP),
  `zombies::Trapped` / `HeldStill` (DisableMovement), `throwables::{hurt_zombies, reaches,
  random_scale, Flash}`, `damage_numbers::DamageNumber`, `audio::AmbientLoop` (looping cue on an
  entity), `particles::Emitter::new` / `Fx::at`, `level::Level::prop_near` +
  `PropStreaming::remove`/`hide` + `CollisionCache::forget` (take a static prop out),
  `traps::ue_rotator` (UE rotator -> glb quat), `models::anim::holds_dead_pose`.
- **Tour helpers**: `open_ground(start, terrain, rays, slope)` / `uphill_spot` (late in the tour
  the level spots are used up: direct placements pass a big slope), `stand_by`, the
  `inventory_ui::tour_craft` / `tour_station_tab` world helpers, `SD_VIEW` / `SD_VIEW_ITEMS`.
- **Bevy / tour gotchas met so far**: a bundle with a component twice panics (spawn then
  `insert` to override, e.g. `title_in(...)` + a new `Node`); synthetic `KeyboardInput` also
  presses `ButtonInput<KeyCode>` (send the Released events too); the tour refills health every
  frame unless the step is exempt (`tour()`); the wandering traders' talk sets and clears
  `HeldStill`; float drift over thousands of steps (store UE doubles as f64); a dead survivor's
  vitals must not tick; UE spawns pawns at the capsule centre (feet = point - 0.82 m);
  `combat::terrain_hit` starts sampling 0.5 m out, so a short per-frame step never meets the
  ground (`projectiles::ground_hit` samples the whole step); `open_ground_for` can't see props
  whose cells aren't streamed yet: stand there a second, then check the step's sight lines
  (08zzv_turrets refuses up to 3 spots with `open_ground_except`).

## Layout

- `src/main.rs`: Bevy app, world setup (`setup_world` on `OnEnter(AppState::InGame)`: player at a
  PlayerStart or the saved position, sun, fog, camera, HUD), player input, zombie chase, and the
  BP_PlayerCharacter spring-arm camera (3 m `ThirdPersonTargetArmLength`, mouse look with the cursor
  captured while playing, shorter and over a shoulder while aiming; aim arm/offset are guesses). Gameplay systems run in `AppState::InGame` + `MenuScreen::None` (+ `player_in_control`).
- `src/level/`: LongdownValley imported from `LongdownValley.umap`.
  - `terrain.rs`: 4591² heightmap → 128 m chunks streamed with distance LOD + skirts; splat
    material (`assets/shaders/terrain.wesl`, an `ExtendedMaterial` over StandardMaterial) blends 11
    paint layers. `Terrain::height(x, z)` is the surface; holes come from the visibility layer
    (`weight_DataLayer.png`, 29 holes: lab entrance, bunkers, cellars): hole quads aren't meshed,
    and `ground_under` / `blocks` ignore the surface over a hole or overhead when underground.
    Use those (not `height`) for floors, walls and sight lines.
  - `mod.rs`: ~110k placements + 7.4k spline meshes bucketed into 64 m cells, streamed within
    420 m (fog hides the edge); tree materials split trunk/leaves by vertex alpha.
  - `spawners.rs`: `BP_AISpawner_Master`/`_Zombies` decompiled: per-instance `SpawnAmount`
    (randomised N/1.5..N*1.5, x ZOMBIE SPAWN AMOUNT MULTIPLIER), 9 s initial delay, wake within
    `SpawnRadius` (140 m default, often 100 m), one zombie per 0.5 s at random box points projected
    onto the landscape (rooftops fail unless `SkipMeshes?`) or the instance's `SpawnPoints`, none
    within 25 m of the player; each kill respawns after AI RESPAWN TIMER. Each instance spawns its
    `SpawnedActorClass` (Roamer 560, Crawler 120, Radiated 76, Infected 61, hounds, the 7
    `BP_AISpawner_ZombieBosses` (SpawnAmount 1); none = never spawns), scaled by its `SpawnType`'s row. `optimize_subjects` is the AIOptimizer side: past
    `BP_MasterZombie`'s last optimisation layer (225 m) zombies are despawned into storage (with
    their HP) and come back within 210 m (the return radius is a guess). Unit-tested.
  - `spline.rs`: UE `CalcSliceTransform` port (roads, fences, cables). Unit-tested.
  - `grass.rs`: landscape grass = Terrain_Material's grass output: each paint layer's
    LandscapeGrassType varieties (`assets/data/grass.json`, `extractor/gen_grass.py`) scattered in
    16 m cells by the layer's weight minus NoFoliage (channel 12 of terrain_weights.png), at the
    asset's density per 10x10 m, grid/jitter, scale, slope alignment and cull distance.
  - `lights.rs`: the Blueprints' point lights (`lights.json`: `SdExport levellights` +
    `gen_lights.py`, resolved against component templates): resort lights 18:00-06:00, fires
    flicker, nearest 24 spawned; lab lights follow `power`, infestation lights are drawn by
    `gameplay/infestations.rs`, lamps wait for their systems. `gen_lights.py` merges a child
    Blueprint's light templates over its parents' (the infestation subclasses only move theirs).
  - Water: BP_WaterBox/WaterSpline surfaces (SplineWaterTEST, PlaneSplineMesh) get a translucent
    water material (Mat_Clear_Water's graph isn't readable; colours are a guess) and don't collide.
    BP_WindTurbine blades spin at Wind x 2 deg per 0.01 s.
  - `doors.rs`: the door BPs decompiled (89 doors: locked/picklock+crowbar, keycard, hangar, lab
    sliding/secure/entrance, warehouse, shipping container, subway travel, the oil rig keypad door).
    Their meshes are taken out of the static cells (`find`) and animated by the BP Timelines'
    lengths; moving parts collide through `CollisionCache`'s dynamic sets. BP_StandardNote (10
    readable notes, `Level::notes` from actors.json's `Description` FText, Level.cs `Notes`):
    "Note: [F] Read" opens `GameOverlay::Letter`, LetterUI with the text (`keypad.rs`'s note page,
    shared with the keypad note). Tour step 08sa_letter. F rules live in the pure
    `press` (unit-tested). A door or keypad note in reach that's nearer than a container gets F
    (`reach_distance`; the game uses a look-at trace instead). `BP_Lever` is a door kind too
    (`Kind::Lever`, `Motion::Pitch`: its handle swings 75° -> -75°; locked = no power, "No Power To
    Lever"); pulling it toggles its garage doors. BP_ZombieDoor is only a sound.
  - `garage.rs`: BP_PoliceStationGarageDoor: the FoldingGate skeletal mesh, its montage sampled
    onto the bones by hand (`models::anim::PoseTrack`, forwards to open, backwards to close, 2.76 s,
    ignored while moving); collision = the bind-pose (closed) triangles unless open (ours).
  - `prefabs.rs`: BP_PrefabSpawner(_Dynamic) + PrefabMaster prefabs (assets/data/prefabs.json from
    `extractor/gen_prefabs.py`: each Content/Prefabs Blueprint's components resolved through the
    class chain (templates merged base-first by name, SCS parents), child actors expanded
    recursively: meshes (the child BPs' too), child actors with their ChildActorTemplate props in
    actors.json's format, particles, `humans` = the human AI its spawning volumes hold). The
    spawners' props (Prefab, Chance, PrefabType, Marker?...) are in actors.json (Level.cs
    `Events`). `Level::load` places every site's meshes in the cells (`PrefabSite::keys`);
    `enter_level` puts them all in `PropStreaming::removed`; `run_prefabs` rolls the static sites
    once the survivor exists (Random Check: rand(0,100) <= Chance x the type's difficulty
    multiplier; not within 25 m) and `spawn_site` un-hides the keys (`PropStreaming::restore`
    drops their loaded cells, `CollisionCache::forget`), removes harvestable trees/rocks in
    FoliageRemoverRadius, registers containers (`Containers::add_at`, keys `CONTAINER_KEYS` +
    site x 64 + child) and zombie spawners (`Spawners::add`, `SPAWNER_KEYS` likewise, through
    `SpawnerDef::from_actor`), particle emitters, and the map marker (RANDOM EVENTS HAVE A
    MARKER?, removed within 20 m). Dynamic events: every 60-4800 s past day 3, the next shuffled
    dynamic site out of view (IsOutsideOfPlayerView?) after 3 s, with explosion_deep_low_1.
    Their BP_AISpawningVolume_Prefab elements become human spawners (`gameplay/humans.rs`);
    prefabs whose humans aren't all ported (the settlements' survivor spawners) aren't spawned.
    `Prefabs::force` spawns one (tour steps 08zw_prefab, 08zx_bandits). Settlements: the 4
    BP_SettlementCampSpawners are `PrefabSite`s with `settlement` (never rolled); `run_prefabs`
    spawns the one BP_SettlementManager picks (`settlement_camps` reads its Camps refs;
    `GameState::settlement` keeps the pick). `settlement_actor` handles its traders
    (`Traders::place`), BP_EnterSafeZone (`zones::SafeZones::add`), BP_WaterWell
    (`zones::Wells::add`) and BP_Marker (SDLogoTransparent, size 3); prefab random-chance volumes
    roll through `SpawnerDef::random_chance`. Settlers, the quest giver, turret and gate aren't
    ported (tour step 08zzb_settlement). The observatory: BP_AlternatePOI's Prefab
    (Alternate_ZombiesObservatory) and AlternatePrefab (Alternate_CoyotesObservatory) are two
    `PrefabSite`s with `alternate` (`PrefabSite::alternates`; gen_prefabs.py also reads
    POI/Alternates/Prefabs, Level.cs `Events` exports BP_AlternatePOI* props); the save's
    `GameState::alternate_pois` picks one; the listener sets it 4 s in past 4 days survived.
  - `item_spawners.rs`: BP_SpecialItemContainerSpawner (the "Police Briefcase" with the
    PoliceStationKeys unique loot = the Police Generator Key, at one of 4 points, random yaw) and
    BP_Keycard_Spawner (3 keycard briefcases, one of 2 points each), rolled per session; the
    briefcases join `interact::Containers` keyed by the spawner's actor index (`spawned`: name +
    DT_UniqueLoot sets added to the roll) and draw their class's StaticMesh (`ContainerDef::mesh`).
    BP_HiddenCache_Spawner (7): a Container_HiddenCache at Spawn0 or Spawn1, hundreds of metres
    out, with the point's full rotation (`point_tilted`; Level.cs `Placed` exports the points).
    Tour step 08vc_cache.
  - `power.rs`: lab power (BP_LaboratoryGenerator / LightSwitch / Light decompiled): 2
    generators -> 46 switches + 9 sliding doors (`Doors::set_power`: Unlocked / Warning), switches
    On/Off/Warning -> 195 lab lights (`lights.rs` Behaviour::Lab). Links are per-instance actor
    arrays that `SdExport levelactors` now exports for lab/generator classes; `lights.json` has
    each light's `actor`. F prompts compete by distance with doors and containers.
    `BP_PowerGenerator` (police): Add Gasoline (a Gas Can, once), Turn On/Off (wheel picks the
    option): needs gas, the first start takes the Police Generator Key; it powers the lever.
    Status lamps: the BPs' OnRep_* swap material slot 1 (LabLightswitch: M_LaboratoryLightsStatus_
    On/Off/Warning; a sliding door's LabSmallDoorframe: M_LaboratoryDoorStatus_Locked/Unlocked/
    Warning). Those meshes stream as static props, so `resolve_props` tags the slot-1 part
    (`LampKind::of` on the glTF material name) with `StatusLamp` and `power::status_lamps` matches
    it to the switch / sliding door within 1 m and sets the material. `gen_materials.py` lists
    `Blueprints/Laboratory/Materials/Status/` so the swapped-in materials are in materials.json.
    BP_Lever's base (LeverBase) works the same: slot 1 red / slot 2 grey without power, grey /
    green with it (`LampKind::LeverRed/LeverGreen`, by mesh path; `Meshes/Materials/` is listed too).
    Stairwells: `Power::stairwells` (BP_LaboratoryStairwellLightSwitch, 2 -> 9 lights): F walks the
    `Lights` flipping each (0.6-1.2 s apart; `tick_stairwells`, run by `run_stairwells` without
    marking Power changed unless a light flips); `lights.rs` Behaviour::Stairwell (1000 unitless),
    each light's hum an AmbientLoop (`StairwellHum`). Tour step 08zzzd_stairwell.
  - `sirens.rs`: BP_Siren (7), state in `Power::sirens` (F prompt "Siren: [F] Interact" via
    `Target::Siren`): random 600-7200 s timer -> plays 138 s, Siren_Cue fading 5 s
    (`audio::LoopGain`, `audio::LoopAttenuation` GunshotsRifle), `zombies::Noise` 60 m every 2.5 s;
    F is a FlipFlop. `Siren::tick` applies timers set in a tick from its end (unit-tested).
  - `spotlights.rs`: BP_RoadLightGenerator (7, always on, engine hum via `power::ambient_sound`),
    BP_FloodLights (4) and BP_Lights (2): Bevy `SpotLight`s, on from 17:59 to 05:59 after
    BeginPlay's Delays; only the first sunset fades in (Timeline_TurnOn never rewinds), turn-off
    is instant (a reversed timeline at 0). Exponent-falloff intensities -> `exponent_candela`.
    LightHum_Cue (CampFire). The 5 BP_LaboratoryCable splines were already in splines.json.
    Tour step 08zzze_siren (22:00) logs the night lights.
  - `easter_eggs.rs`: BP_EasterEgg_GlowingEyes (6 ghosts: the hanging body + 2 eye cubes, taken
    out of the static cells with `PropStreaming::hide` and drawn here; `Level::props_near`) and
    BP_EasterEgg_Campfire: hidden until UDS Midnight (`environment.days` rising), then 60 s; the
    ghosts face the survivor (RInterpTo 15 every 0.2 s) and hover (HoverTL), the campfire hides
    within 15 m (its light via `lights.rs`, its flames via `LevelFx::out`; its crackle always
    plays, as in the game). BP_EasterEgg_Nuke: any hit (shot box, swing, `ObjectBlast`) -> once,
    250-500 to every AI and the survivor within 125 m, FX scaled 25-30. Tour steps 08zzzf_eggs
    (23:59.7) and 08zzzg_nuke (from 130 m, a roamer at 20 m).
  - `lab_corpses.rs`: BP_LaboratoryCorpse (28): Lab_CientistCorpse / Lab_HuardCorpse (`Scientist?`)
    posed once by `PoseTrack` at the instance's `AnimFloat` in Lab_CientistCorpse_Anim (actors.json
    props via Level.cs `Powered`'s BP_Laboratory prefix), shown within 40 m (AIOSubject). No
    collision (ours). `export_assets.sh --level` exports the meshes and clip.
  - `ladders.rs`: BP_Ladder decompiled (48 ladders; `MeshBottom` per instance, exported by
    `SdExport levelactors`): climb box / top box overlap *begins and ends* (pure `Ladders::update`,
    unit-tested), snap to PlayerPostition facing the ladder, W/S at MaxFlySpeed 1 m/s, Jump lets
    go, leaving the top keeps the climb speed into the fall (UE Flying -> Falling, AirControl 5).
    `KinematicBody::climbing` turns gravity off. Ladder clips: Player_AnimBP's ladder up/idle.
  - Camera (main.rs `camera_controls`): the spring arm's collision test (`collision::spring_arm`,
    UE's default bDoCollisionTest with a 12 cm probe): a `PropRays` cast from the pivot pulls the
    camera in front of walls and props, then the ground clamp.
  - Trees collide through their simple collision, as in UE (`assets/data/simple_collision.json`,
    `extractor/gen_collision.py` over the tree meshes' BodySetup: SphylElems as 8-sided prisms,
    ConvexElems as their hulls; `PropMesh::simple` replaces the drawn triangles in
    `CollisionCache::item`). The trunk/leaves split by vertex alpha finds nothing in the current
    exports (alpha is 255 on every vertex), so before this trees had no collision at all and the
    trunk colour isn't drawn (still open).
  - `collision.rs`: kinematic capsules (`KinematicBody`) vs nearby prop triangles + terrain;
    walls push out, floors/stairs within 0.45 m step carry you. `PropRays::cast` (a SystemParam)
    raycasts the streamed props: bullets stop at them and they block zombie sight. No physics
    engine exists for Bevy 0.20 yet (Avian is on 0.19).
- Crafting stations (`inventory_ui.rs`, `STATIONS`: BP_CraftingStation over CraftingWorkbench,
  BP_CookingStation over Campfire with W_CampfireUI's "Prepare Selected Item x{n}" / "Item
  Prepared, XP Given"; both buttons always show the count, as GetText does): the level's 5
  BP_CraftingStation actors are
  a container class registered at startup ("Crafting Station", no hold: "Press [F] to use");
  their overlay draws W_CraftingWorkbenchUI (the CRAFTING tab's `build_crafting` over the
  "CraftingWorkbench" table: SetItemsToCraft(0)) in the loot column, and the CRAFTING tab is
  closed meanwhile (one `CraftView`, which knows its `table`). Ingredients come from and products
  go to the survivor's inventory (the ground when full: "Crafted item dropped"). Each craft gives
  `craft_xp` = level x 5 (x the count on the CRAFTING tab) x XP MULTIPLIER with "Item Crafted, XP
  Given: N" (both UIs' response event; the port gave no crafting XP before).
- Containers (`interact.rs`): the prompt is BP_LootContainerWidget (`gameplay/search_widget.rs`:
  the 64 px F square on the crosshair, SquareProgressBar's outline filling clockwise from the top,
  the name, "[Looted]" / "[Storage]", GetLootTableName's loot type, "[LShift] Quick Search" while held, the
  master's options (`search_widget::options`, the wheel selects); crosshair hidden meanwhile) for loot
  containers; corpses, stations and the other interactables keep the plain `Prompt`. The hold is DurationToHold ÷ ScavengingMultiplier × SEARCH TIME
  MULTIPLIER; Quick Search (IA_QuickSearch = Left Shift, held with F; BP_LootContainerWidget) fills
  it 2.85 × ScavengingMultiplier as fast, and every 0.075 s (NoiseEvent) rolls the class's
  ChanceToProduceNoise (10 or 8 %, items.json containers `noise`) ÷ ThiefMultiplier for a 40 m noise,
  once per press. Weapons' SwingSpeed / ReloadBaseSpeed are items.json `swing_speed` /
  `reload_speed`; a magazine's ReloadSpeed stat changes the reload rate by -stat/100
  (GetNewMagInfo). `UpperAction::start_at` plays an action at such a rate.
  Tool-required containers (BP_MASTER_ToolRequiredContainer: the gun lockers, emergency axes;
  items.json `containers.*.tool` from gen_items.py, the instance's `ToolNeeded` from actors.json,
  Level.cs `Notes`): `tool_needed` / `tool_looted`: no hold without the tool ("{name} not found"),
  DurationToHold / ScavengingMultiplier only (no SEARCH TIME MULTIPLIER), then the tool -10 x
  OTHER ITEMS DURABILITY MULTIPLIER ("No durability - {name}"), Thief 40-60 XP, Level x 100
  "Container Looted, XP Given". Tour step 08vd_locker. Every toast is logged ("toast: ...",
  `notify::show`) for tour checks.
- `src/gameplay/`: `items.rs` (ItemDb from `assets/data/items.json`, JigSInventory-style `Grid`,
  loot rolls by spawn chance/count; `ItemDb::spawn` rolls per-instance `RandomStats` and default
  attachments, e.g. a rifle's magazine and rounds; wearables carry their own `contents` grids;
  items can lie rotated), `equipment.rs`
  (`PlayerInventory`: 4x2 pockets + 21 `Slot`s from BP_Inventory/JSIContainer, worn containers'
  grids, action-bar mapping, clothing meshes by sex), `inventory_ui.rs` (the Tab screen: VICINITY|LOOT,
  EQUIPMENT|CRAFTING, INVENTORY columns; drag/drop with picking observers, R turns the dragged
  item, Shift+drop opens the JigSplitWidget split dialog, item-over-item (rounds onto a magazine,
  magazine onto its rifle), Ctrl+click quick move, right-click Equip/Unequip/Use/Unload/Drop, hover
  tooltip like OnHoverTooltipWidget (with rolled stats), ground drops as `Ground` items shown in
  VICINITY within 3 m, the CRAFTING tab = W_PlayerCraftingUI over the "Player" recipes; logic
  lives in `Ctx` and is unit-tested; dead zombies are loot containers keyed from `CORPSE_KEYS`,
  filled from their AI loot table, opened with F at once), `combat.rs` (1-4 draw, `\` holster, LMB fire at the rolled
  RPM / Semi-Auto on X / melee swing, RMB aim with Q/E shoulder, hip fire = HipFire? (`Armed::hip_fire`,
  3 s after the last unaimed shot, Weapon_Hand_Move_02_Cue on the first): Player_AnimBP's IsADS is
  IsADS? OR HipFire?, so `Armed::aim_pose` drives the Ironsights sets and facing the camera (main.rs)
  without the ADS camera; projectile weapons say "You need to ADS to fire", R reload from carried ammo,
  hitscan down the camera ray with terrain occlusion, BP_FirearmPickup's formulas: crits,
  `EffectiveDamage` fall-off, "No ammo"/"No mag in firearm", DurabilityCheck jams and per-shot
  ReduceDurability (both behind their difficulty toggles), noise out to the gun's DefaultSoundRadius;
  headshots × FirearmTypeMultiplier (one-shot only with HEADSHOT MODE, which also quarters body
  hits: DamageComponent/DamageMultiplier), melee ×2 into a zombie's back, zombie HP/death/XP (level × 5-10 × 1.05), weapons drawn in hand or holstered in the skeleton sockets, BP_Ingame AmmoBox),
  `damage_numbers.rs` (BFL_AI.SpawnFloatingDamageText + FloatingDamageNumbersUI: with DAMAGE
  NUMBERS ENABLED?, the rounded damage easing (InterpTo 3) to a random 30-60 px
  side / 0-90 px up offset from where it was dealt (ours, the user's request 2026-10-30: the game
  starts it at the screen centre; each `DamageNumber` carries `at`, `damage_numbers::body` = a
  pawn's upper chest, projected through the camera every frame, hidden behind it), Fade 0.75 -> 0 over 3 s, gone at 2.5 s; Montserrat-ExtraBold 14;
  CriticalHit's gold on crits, grey otherwise (firearms, melee), white for frag/pipe bomb blasts;
  none for shoves, stomps, the Molotov or burning; combat's crit now applies after the fall-off, as
  the BP does), `recoil.rs` (BP_FirearmPickup StartRecoil/WeaponRecoilValues: each shot kicks
  pitch RecoilVertical ± VerticalRandomDeviation, yaw ±(RecoilHorizontal ± HorizontalRandomDeviation),
  ×2 from the hip, crouched ×0.75 aimed / ×1 hip; RecoilTL eases it in (EaseOut 2, rate RecoilRate×5),
  then after a 0.1 s Delay ResetRecoilTL eases back the last kick / ResetRecoilDivider (EaseInOut 2,
  rate RecoilRate), so bursts climb; legacy input scales: 1 unit = 2.5°, negative pitch looks up;
  `Recoil.pending` is drained by main.rs camera_controls; attachments' RecoilControl stats aren't
  ported), `shove.rs` (V = IA_Shove / Svr_WeaponShove: 4 stamina, the stance's push montage at 1.15
  (SmallMelee/Pistol/Rifle push), its AnimNotify_Shove window tests a 25 cm sphere at the hands
  each frame, the first zombie hit gets `zombies::Shoved` (2.33 s: Zombie_HitReact_Head, no
  movement, no attacks) unless its class has no ShovedAnimation (crawlers, hounds); camera pitched
  45° down = the stomp: Attack_06_Seq, 50 cm sphere at the foot, 20-40 damage, 5 m noise; hand and
  foot points are fixed offsets, ours), `throwables.rs` (G = IA_Throwable: the Throwable slot's
  item with a `ThrowableClass` (items.json `throwable`: class + MinDamage/MaxDamage/ExplosionDelay
  from gen_items.py), Throw_Montage, after its 0.8 s Delay used up and spawned at Hand_R with
  SetPhysicsLinearVelocity along the camera at GrenadeThrowSpeed (2x the aim distance, 12.5-37.5 m/s);
  frag 10 m / pipe bomb 8 m blasts hurt zombies and the survivor (through armor), noise 50 m;
  noise maker 160 m; Molotov fire 0.2 s after its first hit, 10 s: every 0.8 s zombies within 5 m take 10-25 (the survivor has no BPI_Damage) and everyone in it is set burning (`burning.rs`: zombies 4 per 0.8 s for 12-18 s after a 0.1-1 s delay, speeds / 1.5; the survivor 3-5 per 0.8 s for 10-12 s through armor). The
  body is BP_MasterThrowable's simulated Capsule (r 15 cm, LinearDamping 0.5 / 0.75) with
  DefaultPhysicalMaterial (restitution 0.3, friction 0.7): `bounce` off `PropRays::cast_hit`
  normals and `Terrain::normal`, at rest below 0.3 m/s (ours; no tumbling); lights stand in for the FX), `zombies.rs` (BT_Zombie: roam 10 m / investigate noises / chase; sight 25-40 m and 70°, hearing
  150 m via the `Noise` message gunshots send, hurt -> chase, 275°/s turning; `ZombieKind` indexes
  `assets/data/zombies.json` (`extractor/gen_ai.py`, `export_assets.sh --ai`): every
  BP_MasterZombie child's inherited speeds, HP, damage, attack timing, medical/radiation chances,
  meshes + material skins (random each, like `SetMeshAndMaterial`), loot table, capsule and
  blendspace clips (nearest sample per speed). Shapes: upright, prone (crawler), hound), `daynight.rs`
  (the level's Ultra_Dynamic_Sky clock: 09:00 start, dawn 6 / dusk 19, DAY/NIGHT LENGTH minutes,
  sun arc matching the cached sun, moonlight, eye adaptation via the camera `Exposure`, TimeUI
  clock), `keypad.rs` (KeypadUI code pad and the
  BP_KeypadNote's LetterUI page: the code rolled per session, wrong code = 80 m noise), `lockpick.rs` (LockpickUI minigame decompiled from the BPs; turning the lock rotates the
  whole LockWidget, pick and screwdriver included, and with no lockpicks left the pick is hidden), `notify.rs` (toasts), `interact.rs` (hold F
  to search containers for the class's `DurationToHold`, overlay state, `dress_player` spawning
  worn clothing glbs onto the survivor via `models::dress`, and swapping the body parts under them:
  `equipment::body_meshes` = BodyPartVisibility over each item's `ClothingSettings.BodyPartSettings`
  (`gen_items.py` overlays the struct's defaults; null hides a part, e.g. a coat hides torso/arms);
  only `equipment::CLOTHING_SLOTS` (Torso, Legs, Feet, Gloves, BodyArmor: the slots that call
  MC_AttachClothing) are drawn as clothing: 13 respirators and 5 accessories carry a copied
  ClothingSettings (OfficePants, SimpleShirtGreen) the game never draws, and until 2026-10-30
  ours did), `icons.rs` (inventory pictures baked at
  runtime from each item's inspect mesh/FOV via render-to-texture, framed by the bounds: the view
  down the thinnest axis; in a square cell the model's own up stays up (`icon_up`: backpacks
  were on their side)),
  `weather.rs` (Ultra_Dynamic_Weather: `assets/data/weather.json` from `gen_weather.py`; random
  preset at start, daily re-roll at 08:00 from the season's Weather Type Probabilities, 0.25 h
  transitions, the southern Weather_Override_Volume forces Foggy; fog/dust pull the fog in, clouds
  dim the sun, rain/snow fall around the camera except under a roof, thunder flashes; the mapping
  from UDS's 0-10 values to our fog/light is ours), `hud.rs` (BP_Ingame: stats rings/bars, action bar, compass, crosshair, the MedicalUI effect icons
  in CurrentEffectsVB), `medical.rs` (MedicalComponent + RadiationComponent: timed bleeds that turn
  heavy on a second bleed, broken bone = lower jump, radiation sickness 15 dmg / 5 s at full
  radiation; consumables heal by `Medical`, cure by `RemoveMedicalEffects`, and `Radiation` lowers
  ours (the game's meter counts up from 0 = sick); state saved as `GameState::effects`), `survival.rs` (SurvivalStats tick with difficulty
  multipliers, zombie melee, death/respawn, HUD vitals; `step_needs` = HungerThirstComponent
  (hunger 0.015 / 0.5 s, thirst 0.02 / 0.501 s, 5 damage per 5 s at 0), StaminaComponent (sprint
  drains 3/s, recovery 4/s after 1 s, sprint stops at 0 until Shift is pressed again; walk 400 /
  sprint 750 cm/s) and RegenHealth (0.8/s up to 80 HP, 4 s after damage, only with the
  Skill_HealthRegen tech: the old progression's Immunity skill stands in)).
- `gameplay/swim.rs`: SwimmingComponent: WaterTrace (164 cm above the feet down, against the
  water layer `CollisionCache::water` / `PropRays::water_surface` built from parts tagged `water`:
  BP_WaterBox cubes and BP_WaterSpline planes), Deepness >= 0.65 swims (`KinematicBody::swimming`:
  no gravity), afloat at surface - 1.12 m (lowered at 2.5 m/s, never lifted), Deepness 1 = under
  water: oxygen -0.375 / +0.25 per 0.1 s (x / ÷ FitnessSwimmingMultiplier), 4-8 damage per 0.8 s at
  0; the HUD's OxygenStat. main.rs moves the swimmer: camera forward incl. pitch (dives), Space up
  under water (IA_FloatUpwards), 3 m/s or SwimFast 5 x multiplier (drains like a sprint).
  Swimming drops the weapon and blocks draw/aim/shove/throw/roll, and puts out fire. Not ported:
  radiated water, water sounds, the underwater camera effect, swim XP.
- `gameplay/animals.rs`: wildlife (BP_Animal's Deer, Rabbit, Wolf, Bear). Their classes are in
  zombies.json beside the zombies (`ZombieClass::animal` = `AnimalTraits`, from gen_ai.py), so
  spawning (`BP_AISpawner_Animals`: SpawnType 4, ANIMAL SPAWN AMOUNT MULTIPLIER), shooting
  (`Shape::Animal`: a capsule and head sphere from the mesh's bounds, ours), corpses (600 s, on
  their side; the rabbit's death montage), loot (DA_LootTable_Deer...) and the optimiser are the
  zombies'; `LevelZombie` without a `Zombie` brain. The module is the AI controllers' perception
  (sight radius / lose / peripheral, hearing range and MaxAge into CanSeePlayer /
  IsInvestigating) and the trees: deer and rabbits Run Away (a point within 40-80 / 30-40 m of
  one 5 m back from what they see; heard but unseen they freeze, as the BP's task never finishes)
  else graze; wolves and bears snarl, chase and attack within MeleeAttackDistance (2 m between
  capsule centres; `survival::zombie_attacks`, AttackMontage length per bite, 15 % bleed), go to
  noises, else roam. Kills toast "<Animal> Killed, XP Given"; Skill_Huntsman drops a
  Container_AnimalMeat ("Animal Meat", the bag mesh) at the kill. Pure `perceive` / `choose` /
  `step` are unit-tested. Clips: their Locomotion1D samples; quadruped sets
  (`AnimSet::quadruped`) play every action full-body. Voices use the components' overridden
  attenuation (`VOICE_ATTENUATION`, 3 + 40 m). Not ported: predators hunting/scaring deer, the
  antlers, navmesh "reachable" points, BP_AnimalDog, level scaling (LEVEL SCALING ENABLED?, off by
  default; zombies use it too).
- `gameplay/bosses.rs`: the zombie bosses (BP_ZombieBoss's 10 children; zombies.json
  `ZombieClass::boss` = `BossTraits` from gen_ai.py's `boss`: the AttackMontage clip and its
  RateScale 0.5, AnimNotify_ZombieBossAttackRightArm's window, MeleeAttackDistance 5 m, the hand
  trace's 60 cm, the mesh scale (1.1-1.5, set on the entity: `ZombieKind::mesh_scale`), the
  controller's 110° peripheral, BT_ZombieBoss's waits (30 ± 15), the skill point chance, the
  1200 s corpse, the unique sets, ZombieBossBreathLoopCue at 0.2). `zombies.rs` runs them with
  the zombie brain (speeds 0.8 / 5 m/s); chasing, a boss walks in to capsule contact
  (MoveToActor) while `bosses::swing` starts a swing whenever it's within 5 m (actor centres) and
  not `Start_Attacking`: growl, `ActionKind::Attack` on the upper body, Delay(1.35-1.75); in the
  window the Hand_R bone's GlobalTransform within 60 cm of the survivor's capsule lands one hit
  (`survival::zombie_hit`: DamageToDo 28-30, 20 % bleed, 10 % heavy, 16 radiation). Death
  (`combat::kill_zombies`): BossZombieKills, level × 200-400 XP, 25 % skill point, "Zombie Boss
  Killed, XP Given"; corpse "Undead Boss" / "Large Undead Boss" with AddUniqueLoot's sets
  (`interact::track_corpses` puts them in `Containers::spawned`). Hit test: the upright body
  scaled by the mesh and 1.3x thicker (`combat::BOSS_BULK`, ours). No flinch, shove or stomp.
  Not ported: RangedAttack (BP_ZombieBossRock: nothing calls it), the map marker, glowing eyes,
  the achievements.
- `gameplay/particles.rs`: the game's Cascade particle systems on the CPU (assets/data/particles.json,
  `extractor/gen_particles.py` over the FX folders: per emitter the Required/Spawn modules and
  each particle module's baked FRawDistribution table as `Dist` {bias, scale, n, dims, op, lock,
  v}, converted to Bevy units: positions/velocities (X, Z, Y) / 100, mesh sizes axes-swapped
  only, sprite sizes / 100). `EmitterState::tick` is FParticleEmitterInstance's tick: emitter time
  loops over EmitterDuration × EmitterLoops, Rate × RateScale and the BurstList (times are 0-1 of
  the duration), spawn modules through the emitter's GlobalTransform (its scale shrinks the whole
  effect: a guess), then ResetParticleParameters + the update modules each frame. Drawing: one
  entity per particle (pooled per emitter, `ParticleView`), the mesh or a camera-facing quad, and a
  `StandardMaterial` cached by (material, quantised colour and opacity): the cooked materials have
  no graph, so colour = particle colour (the meshes' vertex colours are Synty masks, stripped), unlit/lit and blend from the material,
  opacity = alpha / the emitter's peak alpha (raw alpha can't be it: FX_Fire_AI is 0.03 for life),
  masked (Mat_FX) shown at ≥ 1/3. Sources: `Fx` messages (`Fx::at(..).pointing(x).scaled(..)`;
  UE X along `pointing`) for blasts (FX_Explosion_New, Random Vector In Range scale), the Molotov
  (FX_MolotovEffect, deactivated with the fire), bullet hits (FX_BloodBodyImpact /
  FX_BloodHeadshot ×0.6 / FX_WorldImpact), melee blood, muzzle flashes and brass (items.json
  `firearm.muzzle_flash` / `brass`, from BP_WeaponsPickupComponent), broken rocks
  (FX_HarvestableRock ×0.25); `Emitter` components directly for FX_Fire_AI on burning zombies and
  the survivor (`burning.rs`) and the airdrop crates' coloured smoke (`airdrops::smoke`, off once
  opened); and the level's emitters (`levels/LongdownValley/particles.json`
  from `SdExport levelparticles`, resolved against the owner BPs' templates: 127 of 158 have a
  template), each a `LevelFx` on within its owner's AIO layer 0 (barrel fire 50 m, campfire
  100 m, wreck smoke 75 m until salvaged...). Not ported: Light modules, Collision,
  ParameterDynamic, SubUV flipbooks, PSA_Velocity, Niagara (NS_BloodSplatter, infestation dust,
  bullet trails). The infestations' FX_Flies are `gameplay/infestations.rs`'s own `LevelFx`.
- `gameplay/humans.rs`: the human AI (SmartAI's BP_MasterAICharacter as BP_MasterBandit /
  Military / Scavenger children). Data: zombies.json classes with a `human` block (gen_ai.py
  `human()`: weapons, Accuracy, the controllers' sight 50/55 m and hearing 160 m, Movement
  Speeds by E_AIBehaviour, roam distances/waits, the faction master's kill XP roll and toast
  name from its bytecode, the health row) and `weapons` (BP_AIWeapon_*: Damage by E_HitboxTypeAI,
  Range Attack Distance, Spread (Trace), Range Attack Frequency, magazine, sound, muzzle FX,
  mesh, socket). They are `LevelZombie`s on the zombie brain (`zombies.rs`: `ZombieKind::human`,
  per-class sight, friendly scavengers only chase once hurt, `Zombie::in_sight`; a ranged human
  holds within range in view, melee closes to Melee Distance), drawn with `AnimSet::Survivor`
  and a `Stance` from the weapon. `humans.rs`: SetWeapons (`HumanArms`), hair/beard on `head`,
  the weapon model in its socket (`ItemDb::sockets`), Range Attack (yaw within 35°, a cone of
  Accuracy x Spread around the target's centre, length + 2.5 m, `ray_capsule` vs the survivor,
  props/terrain block; `hitbox` by hit height; `take_hit` + 5 % bleed; sound at 0.5, muzzle Fx,
  a 45 m Noise), reloads (2.5 s, ours), melee swings. Kills (`combat::kill_zombies`):
  human_kills, level x XP roll, Boss? 25 % skill point, "{Bandit|Soldier|Scavenger} Killed".
  Spawned by prefab BP_AISpawningVolume_Prefab elements (`SpawnerDef::human_volume`: 10x10x2 m
  box, 140 m, 2 s, respawning) and the level's 69 BP_AISpawningVolume_RandomChanceToSpawn
  (`SpawnerDef::random_chance`: rolled at load against Chance, the instance's AI Spawn Volume
  box / SpawnRadius / UniqueLoot, 8 s, never respawning, landscape only unless Floors, not
  within 20 m; spawner indices `RANDOM_VOLUME_KEYS` (1 << 22) + actor x 8 + element; Level.cs
  exports their `Spawning AI` with class refs and the box component). Spawns retry 10 points
  (Spawn Retries). A volume's UniqueLoot joins the corpse's loot (`Spawners::unique`,
  `interact::track_corpses`). The wandering trader's backpack hangs on the Backpack socket.
  Not ported: cover/strafe/flee/search, AI-vs-AI, projectiles, hit reactions, the NoSpawn
  area check, settlements, quest givers.
- `gameplay/zones.rs`: safe zones (BP_EnterSafeZone: the level's 5 and the settlement's; Name and
  Sphere radius (default 5000 cm) from actors.json via Level.cs `Zones`; polled every 0.2 s:
  ZoneAreaWidget "Now Entering/Leaving {Name}" (the POI banner's FadeStart keys plus its slide,
  FadeStop before the next), SafeZoneUI's badge (2 s slide), `SafeZones::active` =
  InSafeZone?, which stops `combat::fire` (guns and melee)) and water wells (BP_WaterWell:
  "Water Well: [F] Fill Bottle / Drink", Water {Current} / 2000 saved in `GameState::wells` by
  actor name; `fill` jug 130 / canteen 90 / bottle or plastic bottle 40 into dirty water,
  `drink`, `refill` 0.25 + 5 rain + 1 snow per 5 s; the exposure threshold is ours).
- `gameplay/turrets.rs`: BP_BanditTurret_AI (4 at the bandit base near (-905, -530)) and
  BP_SurvivorTurret_AI (the settlement's), SmartAI's BP_MasterTurret + BP_TurretAIComponent:
  idle sweep (TurretRotateAnimation's 16 s Rotate Alpha over ±90°), Perception Check every
  0.25 s (20 m, AI + a Reaction Tag: `tags` maps classes to Zombie / Bandit / Scavenger /
  EnemyAnimal, FOV ±90°, clear line), RInterpTo at 15, Turret Fire every 0.5 s (3° cone, 40 m,
  Damage by hitbox, 5 % bleed via `humans::hit`, M16_Single_Shot_2_Cue, FX_GunshotRifle),
  magazine 5 / reload 3 s, 100 health from the survivor's rounds (`Shot`, written by
  `combat::fire`) and swings (`harvest::Swing`), explosion at 0. The bandit turret's "Turret:
  [F] Disable" held 5 s from behind (GetBehindObject?) drops DA_TurretGun. The guns are taken out
  of the static cells (`Level::prop_near`, `PropStreaming::hide`) and drawn turning. Turret
  kills give no XP (`NoCredit`, read by `combat::kill_zombies`). Muzzle offset and hit sphere
  are ours (tour step 08zzc_turret). The placed Buildable_Turret / _SmallTurret join the same
  list (`placed_sync`, keyed by `Live::id`; `Stats` per `TurretKind`: 35 m / 30 m, 0.4 / 0.3 s,
  0.1 s reload, head 85, 100 / 75 health saved in `PlacedBuildable::health`); the big one needs
  a generator's power. Their gun is the actor's "Turret" part (`TurretPart`, local rotation).
  `ZombieTargets` lists the attackable ones: `zombies::perceive` picks the nearest seen of the
  survivor / a turret (`Zombie::target`), `survival::zombie_attacks` sends `TurretHit`.
  BuildableObjectHealth is `health_bars`; destroyed ones go via `building::remove_placed`
  (tour step 08zzv_turrets).
- `gameplay/building.rs`: base building's first part. `assets/data/buildables.json`
  (`extractor/gen_buildables.py`, `export_assets.sh --buildables`): the 125 buildable items'
  BuildMesh / BuildActorClass / SnapType / BuildBoundsCheckMultiplier / overlap offset, each
  Buildable_* actor's InteractOptions, Tags and static mesh parts (the construction script of
  the class chain to Buildable_MASTER, InheritableComponentHandler overrides merged, transforms
  composed relative to the nearest mesh part above), the Horde Beacon's DT_HordeSystem waves,
  Rewards, RewardSpawnLoc and dish timeline, BP_NoBuildArea's extent. Build mode = BuildingComponent:
  the inventory's "Place" (`inventory_ui` Action::Place -> `LaunchBuild`) closes the inventory;
  the ghost follows the camera line (850 cm) + AddLocation at RotationZ; green/red by `can_build`
  (BoundsCheck's rules, pure and tested) over `PropRays::overlaps_box` (an oriented box vs
  triangles, `collision::tri_box` SAT) and the landscape between the box's faces; NoBuild =
  BP_NoBuildArea boxes the view line enters (only with BLOCK BUILDING AT MAJOR POIs?) or a
  NoBuild-tagged buildable ("No build zone" every 2 s). LMB places (S_Build_Cue, one used up;
  more left keeps building), RMB stops, wheel ±2°, Shift+wheel ±5 cm up, Shift+E/Q ±90°, R tilts
  to the hit normal, Shift+R levels, Shift+F snapping; BuildBox / BuildingDegrees HUD.
  CheckForSnap: buildables.json `sockets` (gen_buildables.py: each actor's ChildActorComponents
  with ComponentTags, transform composed to the actor, glb space); the line on a placed
  buildable (`PropRays::cast_dynamic` per COLLIDE_KEY + id, matched to the prop hit's distance)
  takes the nearest point tagged with the item's SnapType (`snap_to`), turned by `rotate_snap`
  (RotationZ % 4 / 2 x 180 or % 8 / 2 x 90, ComposeRotators(A, B) = B * A; roofs on floors the
  other way round; GetBuildType = the hit actor's pickup SnapType). Tour step 08zzg_snap
  (foundation, a second snapped 3 m beside, a wall on the edge; it picks open ground: a
  foundation overlapping a road prop is red with snapping on, as in the game).
  `not_building` gates CombatSet, fishing and throws; main.rs's wheel zoom pauses. Placed
  actors (`Buildables`, saved as `GameState::buildables`) draw their parts, collide through
  `CollisionCache::set_dynamic` (key 1 << 44) and offer Move (build mode replacing it) / Pickup
  (back into the inventory) by distance (2.5 m, ours). The Horde Beacon's `Horde` is a pure state
  machine (tested): Interact starts it (TraceForBuildables: nothing within 10 m), the 5 s +
  30/20/10/5 countdown (stoppable), per wave "Wave: N Begin!", spawns 0.5-1 s apart at
  FindLocation (8.5 m disc + 2 m out, projected down; no navmesh) sent to the survivor
  (`LockOnTarget` -> a Roam goal within 5 m), checks every 4 s, "Wave Completed!" + 15 s, the
  last: rewards (shuffled, chance each, max 8) on the ground, spark, metal_med_impact_01_Cue,
  gone. The dish sweeps ±45° while it runs; the light ray pulses (stand-in material). Death turns
  beacons back into pickups. Notify has a colour now (`Notify::colored`; the beacon's are red).
  Tour steps 08zze_horde / 08zzf_horde_end. `gameplay/storage.rs`: placed storage
  (BuildableMaster_Container's 15 classes and the tents; buildables.json `storage` = the
  BP_JigMultiplayer's ContainersSettings[0], gen_buildables.py, only for those two families):
  Interact opens the grid (`Live::storage_key` = `storage::STORAGE_KEYS` + id, past the vehicles')
  titled ContainerName (the pickup's name, or `Live::name`), the prompt shows UpdateStorageStat's
  "Storage: N Items"; Change Container Name = `GameOverlay::Rename` (BuildableRenameObject, typed
  through KeyboardInput, Enter/CONFIRM, Esc/CANCEL, empty = "Container"); Pickup with items =
  `GameOverlay::ConfirmPickup` (ModularYesNoUI), YES -> `Buildables::confirmed` -> DropAll at the
  survivor's feet + pickup. `storage::sync` writes names and contents into
  `PlacedBuildable::name/storage`. Tab doesn't close the rename box. Tour steps 08zzh_storage /
  08zzi_storage_pickup (the tour's `open_ground` finds clear, level ground for building steps).
  `gameplay/sleep.rs`: the beds', sleeping bag's and tents' Sleep (Time of Day <= 630 or >= 1930
  = 6.3 / 19.3 h, else "It's too early to sleep yet, wait until 19:30"), SleepingUI
  (`GameOverlay::Sleep`), Sleep = UDS Transition Time of Day(650, 2 s, EaseInOut 2) forwards
  through midnight (days + 1), closes 2.5 s later. Set Spawn Point (beds, sleeping bag):
  buildables.json `respawn` (the RespawnLocation component) -> `GameState::respawn_point`
  (saved), "Spawn point set". `gameplay/respawn.rs`: death = Client_Died: XPDeath and
  RespawnScreen (`GameOverlay::Respawn`, Esc/Tab don't close it; Time<Virtual> at 0.1x while
  up), stats shown, Respawn - Random (any PlayerStart: tags not exported) / Spawn Point (the
  saved point minus the capsule half-height; "No Respawn Point Set") via the `Respawn` message
  handled in `survival::death_and_respawn`, which then runs Death_PlayerStats; Load A Previous
  Save -> Saves, Quit To Main Menu, Quit -> QuitToDesktop. Vitals don't tick at 0 health. Tour
  steps 08zzj_sleep / 08zzk_respawn (the tour's own health refill skips 08zzk). 08zzj_sleep's bed
  click once didn't place the bed (2026-10-30, ghost green): no spawn point, so 08zzk left the
  survivor dead and ~70 later checks failed. It now re-clicks until the bed is there (logged
  "autoshot sleep bed not placed"). Permadeath:
  `GameState::permadeath` (saved; set from the new game page's PERMADEATH?) makes a death open
  `GameOverlay::Permadeath` = ScreenPermadeath (title, YOU ARE DEAD, Quit To Main Menu / Quit,
  Statistics with ZombieKills only and "1.23km"), no respawn, 0.1x, and deletes the current
  slot (`menus::saves::delete_save`, "{Name} Deleted", `CurrentGame::slot` cleared). Tour step
  08zzkb_permadeath.
  `gameplay/death_loot.rs`: SpawnDeadPlayerLoot + Container_DeadPlayerLoot: on death (the
  `death_loot::Died` message from `survival::death_and_respawn`), unless KEEP INVENTORY ON DEATH?,
  everything worn and carried goes into one loot grid ("Dead Player Container"; worn items keep
  their grids) in a SM_Prop_Crate_04 1 m above the feet that settles at 1 m/s (TraceForFloor);
  `GameState::death_bags` (saved), container keys `DEATH_KEYS` + index, a press opens it ("Dead
  Body Loot", `interact::container_def`). A "Death" skull `WorldMarker` for 1200 s, removed when
  opened; ContainerDeathWidget (skull + "Death Location") over it within 50 m (IN GAME MARKER
  WHEN NEAR TO DEATH LOCATION?), the mesh hidden past 50 m. Tour step 08zzka_death_loot. Tour gotcha:
  synthetic KeyboardInput also presses ButtonInput<KeyCode>; send the Released events too.
  `gameplay/generators.rs`: placed power. buildables.json (dumped with SD_SCRIPT=1 now) has
  each actor's `lights` (Point/SpotLight components; the on-intensity is the largest
  SetIntensity literal in its bytecode), `requires_power`, `switch` (LightOn in its code) and
  `generator` (MaxFuel, Collision radius). `Live::power` = `PowerState` (fuel f64 as UE's
  double, on, light_on saved in `PlacedBuildable::power`; powered/area/warned not). Options:
  Turn On ("Turn Off" while on, "Empty"), Refuel (DA_GasCan -> MaxFuel, "Gas full" / "No gas
  can"), Show Generator Area (a translucent sphere, ours), lamps' Interact (switch); Pickup
  with fuel warns once (10 s window) and powers its range off. Running: 0.01 fuel / 0.2 s,
  scans every 1 s (the first a second after turning on) powering Requires Power? buildables
  whose origin is in range (`power_range`), hum (AmbientLoop), StaticMesh shake (`MainMesh`).
  Lights are children with `BuildLight`, intensity `lit()` ? lumens(intensity as cd, a guess)
  : 0. Tour step 08zzl_power (snapping off on the slope, like Shift+F).
  `gameplay/traps.rs`: the 'PlayerTrap' buildables (buildables.json `trigger` = Box_0,
  `blast_radius`): spikes / razor wire / log wire (25 + white number, `zombies::Trapped`, all
  released 5 / 3 s after the first), bear trap (`Jaws` state machine, 0.2 s cubic close, 10 s hold,
  5 s re-arm; `Jaw` parts turned by `ue_rotator`), landmine (stepped on or any object damage:
  200-225 in 4 m incl. the survivor, gone 0.1 s later). Begin-overlap = capsule vs box per frame.
  Trap kills give no XP (`turrets::NoCredit`). `Trapped` is separate from `HeldStill`, which the
  wandering traders' talk sets and clears. Tour step 08zzn_traps (placed with `add_live`).
  Placed benches (`building::bench_table`): Interact = SetItemsToCraft on the inventory's
  station path via `inventory_ui::PlacedStation` (key `BENCH_KEYS` + id, title the pickup's name):
  crafting workbench CraftingWorkbench, campfire Campfire ("Prepare"), weapons/equipment
  Lvl1WeaponsEquipmentWorkbench, construction bench Lvl1ConstructionWorkbench with
  W_ConstructionWorkbenchUI's STANDARD / MODULAR tabs (`ConstructionTab`; MODULAR =
  Lvl1ConstructionModularWorkbench; the bench keeps it in `Live::crafting_table`). The placed
  campfire burns (FX_Fire emitter, SC_CampFire loop, its 8 cd default light). Tour step
  08zzo_benches (`tour_station_tab` does what a tab click does). The tour's `open_ground` runs out
  of level, clear spots by then: direct placements pass a large slope tolerance.
  `gameplay/build_doors.rs`: the six door Blueprints (Buildable_WoodenDoor / 2 /
  BarricadedMetalDoor / BunkerDoor share one graph: Interact swings StaticMesh to yaw ±90 away
  from the player (DotProduct sign) over a linear 0.5 s timeline, again = Reverse, IsClosed when
  back; Buildable_WoodenGate: Gate_DoorLeft +90 / Gate_DoorRight -90 over 2 s eased; the
  Buildable_GarageDoor: a skeletal mesh (GarageDoor.glb + GarageDoor_AnimFix, exported by
  `--buildables`) posed by `PoseTrack` over its 1.83 s montage, "Requires power" without power,
  ignored while moving). `Live::door` = `DoorState` (not saved: no SaveGame flag), `Leaf` tags
  the turning parts in `spawn_actor`, `part_pose` turns them for collision, redone when a swing
  ends (`Live::unsolid`); the garage blocks with its bind pose unless open. Tour step 08zzp_doors.
  Not ported: the display plaque, horde music, the montages. Buildable health: Buildable_MASTER's
  ReceiveAnyDamage only counts causers tagged CanAttackBuildable, which nothing in the pak has,
  UpdateUIHealth is empty and 0 does nothing, so only the turrets (`turrets.rs`) take damage.
- `gameplay/level_traps.rs`: the level's BP_Landmine (79; BeginPlay removes each with
  rand(0.1, 100) <= 50, prop and all) and BP_BarbedWire / _Child / _Child1 (75). Their Box
  templates are `trigger_box` (UE cm; a test checks them against `target/bpdump`), placed by
  `world_box` on the actor's affine transform. A mine goes off on any pawn beginning to overlap
  (the survivor too: no Player check, unlike the buildable one), the driven vehicle's hit box over
  it, or Damage_Object (`traps::object_hit`), through `traps::MineBlast` (the buildable mine's
  Event_Explode, now shared in `traps::spring`), then its prop goes 0.1 s later. A wire: 8-12 on
  entering, then DamageTimer's 10 every 0.5 s on the last pawn in, cleared by any end overlap,
  never while driving (`WireState::step`, pure). The wire and mine meshes don't block pawns
  (`level::NO_PAWN_COLLISION`, by mesh path: the BPs' StaticMesh is NoCollision / Pawn Ignore).
  `LevelTraps::survivor_hits` records the wire's hits. Tour step 08zzq_level_traps.
- `gameplay/attachments.rs`: weapon attachments (part 1). items.json: firearms' `slots` (gen_items.py
  `weapon_slots`: the SC_<gun> widget's JSIContainers in draw order, ContainerTag +
  OnlySupportTheseItems; `--items` now dumps Inventory/Widgets/Firearms/) and attachments'
  `attachment` (Local_AttachmentType, Local_AttachSocket, FOVZoom for sights, role from
  BP_WeaponsPickupComponent's Suppressors / MuzzleBrakes / Sights / Scopes lists); their ItemStats
  are in `stats`. `attach` (dropping onto the gun, `inventory_ui::item_over_item`), "Remove <name>"
  context entries for ShowAttachments (`Action::Detach`). combat::fire: recoil x
  `recoil_strength` (1 + summed Vertical/HorizontalRecoilControl / 100), DamageChange added,
  effective range / (1 + |muzzle Falloff| / 100) (1.0 when missing: /1.01 bare), `muzzle` (a working
  suppressor: suppressed sound + flash; noise x (1 + WeaponNoise / 100); MUZZLE ATTACHMENT
  DURABILITY wear), hip-fire cone 3 / (1 + HipFireAccuracy / 100) degrees (`combat::cone_dir`).
  Camera FOV = THIRD PERSON FIELD OF VIEW (horizontal, converted to Bevy's vertical); IA_ScopeZoom
  (middle mouse) while aiming toggles the sight's FOVZoom (`Zoom`; built-in: Phantasm/Survivor 45,
  XM250 20, Cerberus 15; `Zoom` carries the scope's `Scope` overlay, by ItemID as the component's
  ScopeFunction picks it). AUTOMATICALLY USE SCOPE WHEN AIMING? zooms on the aim's start.
  `show_attachments` = ProcessAttachments: each attachment's Local_ActorClass mesh (items.json
  `attachment.actor`, gen_items.py `local_actor`: the LocalAttachment BP's StaticMesh and light
  templates, all children of DefaultSceneRoot) spawned under the gun model's socket bone
  (`mesh_sockets`: the gun skeleton's SkeletalMeshSockets, `glb_transform`ed; a missing socket =
  the mesh's origin, as UE does); `ShownAttachments` on the `HeldWeapon` tracks what's shown.
  Tour step 08zzt_attachments (ends side-on, logs the model's parts).
- `gameplay/scopes.rs`: ScopesUI (AcogScope / NormalScope / MosinSniperScope overlays, the
  mask darkening = BackgroundBlurWithMask's stand-in, baked at runtime from the mask PNG),
  SetEnableScope (survivor root and `hud::HideWhenScoped` crosshair + action bar hidden),
  legacy camera shakes (`Sway`: SNIPER_SWAY / AIMING_SWAY tables from FX/CameraShake, tested
  against `target/bpdump/shake` when dumped; sin oscillators, 0.5 s blends, applied in PostUpdate
  on the camera after the spring arm), `sway_for` (the Marksmanship tiers, with the BP's
  1.4-1.5 gap), IA_HoldBreath (Left Shift while zoomed: 0.3 / Marksmanship stamina per 0.01 s,
  shakes blend out; let go = SniperScopeSway_1 + AimingSway_1). Tour step 08zztb_scope.
- `gameplay/tactical.rs`: ToggleTacticalAttachment (K, on foot; vehicles keep K for the flip):
  the drawn gun's Tactical attachments flip State1/State2; on = CheckBatteryLevel (the
  attachment's own battery: items.json `holds` = SC_Battery's container, also used by
  `flashlight::battery` and the inventory's item-over-item), SetIntensity 65000 / 50000 on the
  attachment's SpotLight (`TacticalBeam`, spawned by `show_attachments` from the actor's lights,
  turned to point along UE +X), UseBattery every 0.1 s (only a missing battery turns it off);
  the range finder: a camera ray (terrain + props within 500 m, ours) every 0.1 s into
  `RangeFinderText` ("{m}m", grouped, centre + 30). State per slot for the session (ours).
  Tour step 08zztc_tactical.
- `gameplay/radiation.rs`: BP_RadiationArea_Sphere (10 zones; Level.cs exports ZoneRadius,
  RadiationLevel, Name, AdditionalNotification? / its message), RadiationComponent and the
  decontamination shower. Entering a zone (capsule centre in the sphere, polled 0.2 s): the
  ZoneAreaWidget banner with UI_filter (`zones::PendingBanner` now carries an icon), the zone's
  notification (5 s), RadiationTimer every 0.2 / 0.08 / 0.04 s ÷ RADIATION SEVERITY MULTIPLIER
  (LEVEL 1) (the BP reads the Level 1 row for all levels). A tick: bare-faced +0.75 (ours counts
  up) and 0.1-0.25 Toughness XP; a respirator (Facewear with a FilterUsagePerSecond stat) uses its
  filter (the attachment, `flashlight::battery` / `charge` reused) by that stat x RADIATION FILTER
  DEPLETION MULTIPLIER and adds 0.75 / 2 x (1 - RadiationProtection / 100); "No Filter" /
  "Filter No Longer Working..." then the full dose. HUD `RespiratorDur` (SetFilterUI). Geiger loop
  (sounds.json `Geiger_Counter_Sound_Cue`) on the camera at meter/100 for 6-8 s, restarted by the
  next tick, stopped on leaving the zone (StopRadiation) or when the meter drops (IncreaseRadiation).
  Until 2026-10-30 the cue (no attenuation) got no `audio::Emitter`, so its gain was never applied
  and nothing stopped it: every restart stacked another endless full-volume loop (tour: "voices"). The zones' PostProcess is `post_volumes.rs`'s. Radiated water
  (Level.cs now exports BP_WaterBox / BP_WaterSpline props: Radiated?, ShallowWater?, Scale; two
  bodies, BP_WaterBox_C_0 and BP_WaterSpline_C_1): the capsule overlapping the Cube box or the
  swept spline segments (`SplineMesh::contains`) starts RadiationTimer at 0.15 s; leaving stops it.
  The shower's Light / Light2 parts (`ShowerLamp`) follow `ShowerState::lamp` (red, green on power,
  orange while cooling, green at its end) and a shower spawns FX_Water / FX_Water2
  (FX_Particle_Water, PolygonDog/FX now in gen_particles.py) for 3 s. Radiation weather: no
  Blueprint reacts to it (UDW's preset look only). Zombie hits'
  rad damage x (1 - protection) with a filtered mask (`hit_dose`; bosses not). Filters drop into
  respirators and come out with "Remove Filter". The shower (`Live::shower`): Interact, "No
  power", 300 s cooldown message, IncreaseRadiation(100) for the survivor in its box, once per
  load (the BP's DoOnce). Tour steps 08zzr_radiation, 08zzs_shower.
- `gameplay/head_gear.rs`: BP_PlayerCharacter's head gear. HEADWEAR / FACEWEAR / EYEWEAR /
  ACCESSORY items hang their PickupClass actor (items.json `worn`: the pickup's StaticMesh,
  its transform and OverrideMaterials; `gen_items.py worn()`, the meshes exported with the item
  meshes, the override materials fed to `gen_materials.py`) on DA.PrimaryUnequipSocket(Backpack)
  at EquippedTransform (sockets Headwear / Mask / Eyewear / Neck / Scarf; "Head" is the `head`
  bone). Third person (SetFPMeshesVisibility): all shown, the hat unless HIDE PLAYER HEADWEAR?;
  a hat hides the hair (`models::FaceMesh::Hair`) unless that setting is on, a mask the beard;
  the Barber overlay hides the hat and mask (SetPlayerGearVisibility). Gear is respawned when
  its bone goes (the barber's new head). Tour step 08zzzh_head_gear.
- `gameplay/barber.rs`: BP_Barber (2) + CharacterBarberMenu. Each rolls RandomBool at load:
  the musician woman (Chr_FemaleHair5 blonde, glasses) or the hunter (Chr_MaleHair3 black), A_Idle
  (`AnimSet::Trader`), clippers on Hand_R, hidden past 75 m. "Barber: [F] Interact" (2.4 m, ours)
  -> `GameOverlay::Barber(i)`: the survivor at the Debug point ((-65, 170, 0) yaw -75, Idle), the
  view blended linearly over 0.5 s onto its Camera ((0, 40, 150) yaw 90; `blend_camera` in
  PostUpdate over the frozen spring arm, and back on exit), `hud::HudRoot` hidden, the creator's
  Details / Body column (`creator::spawn_profile_panel`, shared; title "Barber", Exit, Esc after
  `escape_goes_back` unless a text box has the keys; "Has to be numeric" on a bad age). Changes
  edit `CurrentGame::profile` (saved with the slot) and `restyle_survivor` rebuilds the survivor
  live (`models::restyle`: old Head (its attachments go with it) and body parts `Outgoing`, a new
  Head + attachments `Incoming`, `BodyMeshes` emptied and `WornClothing` removed so `dress_player`
  redoes body and clothing; `BuiltFrom` remembers the profile). Not ported: the outline,
  SetPlayerGearVisibility (no headwear/facewear is drawn on our survivor yet). Tour steps
  08zzzb_barber / 08zzzc_barber_exit.
- `gameplay/infestations.rs`: BP_Infestation (+ Floor / Wall / Ceiling / Corner = "Secondary
  Infestation", BP_PrimaryInfestation), BP_InfestationSpline, BP_InfestationManager,
  BP_RandomInfestationSpawnPoint. Class data `assets/data/infestations.json`
  (`extractor/gen_infestations.py`, `export_assets.sh --infestations`: the class chain merged:
  name, health/deviation/MaxHealth, meshes and _D meshes, armor, loot table, unique sets,
  WavesOfEnemies, ExplosionLoc / Flies / light offsets); per instance (Level.cs `Infest`):
  SecondaryInfestations, Splines (actor names), AISpawningTransforms, SpawnZOffset, the random
  points' RandomChance and meshes. 12 primaries, 19 secondaries. The nests' props are taken out of
  the static cells and drawn here (`NestModel`, solid via `PropRays::set_dynamic_from`, bounds
  from the mesh); armor (InfestationLarge_Armor) and the splines' segments
  (`Level::splines_at`) stay static props until they go. Damage_Object (`turrets::Shot` line vs
  bounds, `harvest::Swing` within 2 m, `barrels::ObjectBlast`): an armored primary refuses ("Secondary
  Infestations Not Destroyed In Surrounding Area."); health < MaxHealth / 1.5 starts the waves
  (`Waves`: 1-1.5 s apart at LocToSpawn, `building::LockOnTarget` on the survivor, 30 s between
  waves); below 0: FX_Explosion_Infestation, InfestationsDestroyed + 1, level x 100-200 / 400-500
  x 1.05 x XP MULTIPLIER, red toast, 0.2 s later the _D mesh and the pile (container keyed by the
  actor index, "Infestation Pile" pre-rolled into `Containers::searched`, `container_def` names
  it after the nest; reach is to its bounds). All secondaries down: the primary's armor and
  splines go. Map markers (Marker_Infestation) for primaries and random ones. Saved in
  `GameState::infestations` (destroyed, health, piles, the random roll); loading replays
  ActorLoaded (a destroyed primary removes its secondaries). The manager rolls the random points
  4 s in when every primary is down (once a save). PRIMARY INFESTATIONS ENABLED? off removes
  them all. Pure `Nest::hit` / `destroy` / `Waves::tick` are unit-tested; tour step
  08zzu_infestation. NoSpawn: a random point within 5 m of a buildable's collision
  (`CollisionCache::dynamic_within` over building's COLLIDE_KEY range) doesn't spawn. Not
  ported: NS_InfestationDust (GPU Niagara; its values live in the compiled shader only).
- Interaction montages (`interact::PlayerMontage` -> the survivor's `UpperAction`, BPI_Player.
  PlayMontage: at once, over any montage): `ActionKind::Grab` (Loot_MidShelf_GrabItem, UpperBody)
  when a container opens (`interact::open_montage`: items.json containers, the death crate,
  infestation piles; not corpses, stations, traders) and on the lab generators, power generators,
  light switches (not in Warning), stairwell switches and sirens (`doors::interact_doors`);
  `ActionKind::TableTop` (Loot_TableTop_LeftHand) on a buildable's Option1 when buildables.json
  `interact_montage` (gen_buildables.py: the class chain's PlayMontage on OnInteractBuildable's
  Actor: 30 buildables) and on every buildable pickup. Not wired yet: level doors / gates, vehicles,
  BP_WaterWell, the bandit turret, BP_BuildablePickup, quest spawners.
- `gameplay/post_volumes.rs`: the bounded PostProcess volumes, blended over the level's
  PostProcessVolume_0 (VignetteIntensity 0): infestations (red offset 0.01, grain 0.25; sphere
  PostProcessRadius, half for random nests, none for secondaries or destroyed ones; BlendRadius 1 m)
  and radiation zones (saturation 0.75, green offset 0.002 + shadows 0.002, fringe 2.5, vignette
  0.25, grain 0.5; BlendRadius 2.5 m). Each volume lerps what it overrides by its weight (`Look::
  blend`), zones first. Saturation: the camera's ColorGrading; fringe: Bevy's ChromaticAberration
  (intensity fringe / 100, no start offset); offset, grain and UE's cosine-fourth vignette: two
  full-screen `UiMaterial` nodes under the HUD (`assets/shaders/volume_tint.wesl`, blend
  One+One; `volume_grain.wesl`, blend Dst x (rgb + alpha) since the UNORM target clamps the
  shader's output). gen_materials.py also feeds non-flipbook particle sprite materials (the
  shower's FX_Circle_Mat, the muzzle lens flares). Logged in 08zzu_infestation.
- `gameplay/binoculars.rs`: BP_PlayerCharacter's IA_Binoculars (B) / UseBinos (refused while
  in a vehicle, fishing, swimming, `KinematicBody::falling`, zoomed, building, with an overlay,
  aiming, sprinting; "You do not have Binoculars equipped"), BinocularsVisibility (the Binoculars
  widget: BinocularsOverlay, "{Zoom}x Zoom", "[LMB] Tag Enemy", "{A}m"; HUD and HeadPart hidden;
  the camera at the `camera` socket (eyes) + 14 cm ahead / 8 cm up in PostUpdate; FOV = FIRST
  PERSON FIELD OF VIEW / BinocularsZoomValue in `attachments::camera_fov`; the survivor faces the
  view), BinosDistanceTimer (0.1 s ray: props, terrain, capsules; ShowDistance? only), MarkEnemy
  (LMB: the nearest AI capsule + 5 cm before props / terrain gets `AiMarker` from zombies.json
  `marker`: gen_ai.py `marker()` reads each chain's GetMarker / CanAddMarkerToAI? bytecode) and
  Event_Marker (1 s line of sight to the spawn point, `LevelZombie::home`; 60 s retriggerable;
  death; traders always re-tag, orphaning the old widget). `binocular_input` runs in PreUpdate
  after InputSystems and resets what UsingBinoculars? blocks (LMB, RMB, Tab, F, J, -, C, the
  wheel); the tour presses there too (`tour_binocular_keys`). items.json `binoculars` {zoom,
  distance} (gen_items.py). Mouse look now uses the Controls page's SENSITIVITY / AIMING / SCOPE
  multipliers (0.002 rad/px per unit) and INVERT MOUSE Y (`binoculars::look_sensitivity`, main.rs
  `camera_controls`). Tour steps 08zzzi_binoculars, 08zzzj_binoculars_down.
- `gameplay/loot_refill.rs`: LOOT REGENERATION (BP_JigMultiplayer's StartRefillTimer /
  CheckContainerRefill / ProcessRefillContainer as BP_LootContainer, the picklock and the
  tool-required masters drive them). LOOT REGENERATION DISABLED ON LOAD? writes the
  Difficulty.Refill cvar directly: ticked (the default) = the timers run and a load keeps the
  searched loot; unticked = every load resets the loot and nothing refills (until 2026-11-01 ours
  had it backwards). Read as the session begins. A finished search or picked lock
  (`interact::Searched`) arms LOOT REFILL TIMER (MINUTES) x 60 if the class's BP_JigMultiplayer
  refills (items.json `refills`: gen_items.py, false on the briefcases, POI / compound / laptop
  crates, the meat bag); when it fires the grid goes, or with its loot screen up stays but counts
  unsearched (`Containers::refilled`, `looted()`; the next search adds a roll: ours). Mark As
  Storage Container (BP_LootContainer's Option2; the wheel picks it on the prompt, F presses;
  `mark`: "Container Not Searched" unsearched, `Containers::marked`): "[Storage]", "Mark As Loot
  Container", its timer ends without a refill. Saved as `GameState::loot` (`LootSave`: searched
  grids, refilled, marked, the timers' time left; keys below CORPSE_KEYS that are items.json
  containers, `Refill::saved`), written by `save` like the other savers, restored by `load`
  (ActorLoaded: marked always, the rest only with the setting). Tour steps 08vca_refill
  (08vc_cache turns it on) and 08vcb_storage (which then puts the viewer's setting back).
- `gameplay/outline.rs`: the outline on whatever the interact prompt names (each actor's
  SetRenderCustomDepth(Video.Outline = OUTLINE OBJECTS ENABLED?) on OnBeginInteract;
  M_Highlight_Inst: white, 1 px) and the containers' search icons. The prompt systems
  (`interact::search_containers`, `doors::interact_doors`, wells, radios, barber, turrets,
  vehicles, buildables, salvage) name their target with `Highlight::claim` (first claim wins;
  taken every frame in PostUpdate): `Highlit::Key` (a container key: level / spawned container,
  corpse, trader, airdrop, death crate, quest crate, placed storage), `Actor { class, at }` (the
  class's outlined components from `assets/data/outlines.json`, `extractor/gen_outlines.py`
  over target/bpdump: the components each Blueprint calls it on, children inheriting, found as
  static props within 1 m of the origin), `Door(i)` (moving parts; a lab sliding door's Frame
  instead), `Buildable(id)`, `Entities`, `Props`. Static props are drawn as copies (retried while
  their meshes load); spawned entities' meshes join render layer 29 in place (skinned only for
  characters) and get their layers back after. The mask camera (layer 29, child of the player
  camera, active only while something is outlined) renders into a window-sized image;
  `assets/shaders/outline.wesl` (a full-screen `UiMaterial`) lights pixels next to it, drawn
  through walls (ours). Search icons: DistanceOutline every 0.5 s: CONTAINER SEARCH ICON WHEN
  CLOSE TO?, within 15 m, unlooted, TraceToPlayer clear (static props except its own mesh, the
  landscape); a 25 px MagnifierContainer_UI / _Locked (padlock, tool-required) at items.json
  `magnifier`. The airdrops, death crate and quest crate: their own Magnifier within 7.5 m, no
  trace, hidden while their prompt is up (`close_icon_shown`). `SearchView::key` names the
  prompted container. `PropRays::cast_except` / `CollisionCache::cast_item_except` skip given
  props. Logged in 08vc_cache, and by `tour_outline` in 08q_door / 08qa_well / 08zi_trader.
- `gameplay/zombie_eyes.rs`: the zombies' glowing eyes (BP_MasterZombie / BP_ZombieBoss
  EyeLeftMesh / EyeRightMesh: 1M_Cube on the skeleton's eyesleft / eyesright sockets of the
  `eyes` bone, unlit Colour x EmissiveStrength of the class's M_GlowingEyes instance).
  zombies.json `eyes` (gen_ai.py `eyes`: the components by name, the skeleton sockets; `--ai`
  exports each class's first mesh and its skeleton) and `blood_moon_eyes`. DISABLE ZOMBIE EYES?
  at spawn; `world_events::BloodMoonBoost` swaps the blood moon's on (not the bosses:
  `world_events::answers_blood_moon`); death removes them. The hound has no sockets, no eyes.
  Tour step 08zzda_eyes (and 08zzd_blood_moon logs them; `EyeProbe`).
- `gameplay/ai_health_bar.rs`: BP_PlayerCharacter's TraceForAI (every 0.05 s, 100 m along the
  camera; `combat::ray_hits` shapes, nest boxes, props block), each BPI_AI's AIHealthBarInfo
  (zombies / bosses / deer 100 / wolf 200 / the two human bosses / nests; other humans, bears,
  rabbits none), HUD_Game's ToggleAIHealthBar (created only when absent: the first name stays;
  UpdateHealth a trace later) and AIHealthBarUI (top centre, Alignment Y -1, skull for bosses,
  320 x 12 bar, "{Health} / {MaxHealth}" grouped). AI HEALTH BARS ENABLED? turns it off. Tour:
  08g_armed logs `autoshot ai bar` as it changes.
- `gameplay/projectiles.rs`: projectile weapons (BP_FirearmPickup's DoesWeaponUseAProjectile?
  branch, BP_ProjectileMaster / Grenade / Rocket / FlareRound). items.json `firearm.projectile`
  (gen_items.py `projectile`: the class chain's ProjectileMovement speed and gravity scale,
  StaticMesh, PointLight, the rocket's own MinDamage / MaxDamage; `--items` now dumps
  ItemProjectiles/). `combat::fire` runs the usual magazine / jam / recoil / sound / noise /
  muzzle flash, then ProjectileTraceFromCamera (terrain, props, zombies; no hip cone) and writes
  `Launch` from its muzzle offset towards the hit. Flight: gravity x scale, capped at the speed,
  first hit of terrain / prop / `combat::ray_hits` per step. Grenade: 7.5 m, FirearmDamage
  (rolled, = MinDmg = MaxDmg); rocket: 12.5 m, 250-350; both x DemolitionExpert through
  `throwables::hurt_around` (the survivor too), `ObjectBlast` with the unmultiplied roll /
  MinDamage, FX_Explosion_New / FX_ExplosionLarge_New, explosion_large_06 / 07, 50 m noise,
  white numbers; gone after 10 s unhit. The flare stops at its hit and keeps its light (no
  lifespan). Not ported: RadialForce, the burn decal, the camera shake. Tour step 08zzw_launchers.
- `gameplay/utility_benches.rs`: Buildable_TrashBin, _DisassemblyWorkbench, _BatteryCharger
  and their W_*UI. buildables.json `grids` (gen_buildables.py: the non-storage actors'
  ContainersSettings as [columns, rows, name]); items.json `disassembly` (gen_items.py:
  CanBeDisassembled?, true by default, and DisassemblyItems' ItemID / SpawnChance /
  RandomCountMin-Max). Interact = `open`: the first grid at `Live::storage_key` (saved by
  `storage::sync`), the workbench's "Disassembled" grid at `OUTPUT_KEYS` + id (saved as
  `PlacedBuildable::output`), and a `LootExtras` entry that `inventory_ui::rebuild` draws under
  the grid (a `LootButton` and the second grid). Trash bin "Destroy Items" = `destroy_items`;
  "Disassemble Items" = `disassemble` (pure, tested: rand <= chance, count range x the stack's
  count into the output, the stack consumed; "Item \"{name}\" cannot be disassembled"). The
  charger: `Grid::only` = DA_Battery (OnlySupportTheseItems, new on `Grid`: `accepts` gates
  place / insert / quick move), +2 durability per 5 s while powered, "Generator needed to charge
  battery", the stat "{Battery|Batteries}: N Charging {[No Power]}". Picking a bench up loses
  its contents (the plain Buildable_MASTER Pickup). Tour step 08zzy_utility.
- `gameplay/repair.rs`: repairs (JigContextMenuComp ContextRepair*, JigItemOverItemComp GetValid
  / WeaponRepair / ArmorRepair / RefillRadFilter / PurifyWater). `fix` (DurToFix = clamp(max -
  dur, 0, kit dur), the kit loses DurToFix / (2 for attachments) / ReduceDurabilityAmount, used up
  at <= 1), `over_item` (items.json `over_item` = OverItemOptions; the receiver's type tags
  aren't readable, so its own repair context tag stands in), `purified`. Wired in
  `inventory_ui`: `Action::Repair` ("Repair Weapon / Attachment / Gear", a kit from the item's
  own inventory: `Ctx::find_kit`) and `item_over_item` (now with `OverOut`: spilled items and a
  notice). DurabilityUI / FilterDurabilityUI are unused in the pak. Tour step 08fd_repair.
- `gameplay/melee.rs`: melee attacks (BP_PlayerCharacter's IA_PrimaryAction Completed /
  Triggered, BP_MeleePickup, AnimNotify_MeleeHitDetect(_Power)). Swings on the release:
  ElapsedTime <= 0.5 s normal, more = power (crouched: "You cannot use a power attack while
  crouched", normal). items.json per melee item `melee` (gen_items.py `melee`: Normal / Power
  MeleeAttackMontages, CrouchedMeleeAttackMontage, DelayBetweenSwings, MaxActorsToHitInSwing?) and
  top-level `melee_montages` (the montages' sequence, length, MeleeHitDetect window; `--items`
  dumps the montage folders). One-handed weapons swing 1HMeleeAttack1-4 (animBatonAttack2/3/4/2),
  two-handed Longs_Attack_p_L/LD/R/RU_InPlace, all power Attack_01/02/04_Seq_Montage_RM, crouched or
  looking 45° down Crouch_MeleeAttack (animBatonAttack1); `models::anim::MELEE_CLIPS` indexes
  `ActionKind::Swing` (the clips are in `--anims`' list). Stamina first (StaminaDrain, x rand(6, 8)
  power), rate SwingSpeed x StrengthSpeed x (1 / 0.5, power 0.75 / 0.45), Attacking? lockout =
  `Armed::cooldown` = DelayBetweenSwings x (1 / 2). Hits only inside the montage's window (montage
  time = real x rate), each zombie once a window, normal capped at MaxMeleeHits (MULTI HIT, else
  1), power uncapped x2.5; the arc (2.2 m, ±60°) stands in for the Start/End socket sweeps.
  PowerAttackAnim: the HUD's `hud::Crosshair` turns 360° and scales 1 -> 1.5 -> 1 over 0.5 s,
  again while held past 0.5 s. Tour gotcha: in an unfocused window held buttons come back up
  between frames, so a tour hold re-presses every frame and the hold starts only when none is
  running. Tour step 08zzz_power.
- `gameplay/radio.rs`: radios. The level's 4 BP_Radio (`AmbientLoop` of Radio_Cue, F toggles
  VolumeMultiplier 0 <-> 0.5) and 5 BP_Radio_Text(_Military) (RadioStatic_Cue; F swaps to
  RadioMuffled_Cue and shows RadioTextUI: each DT_Radio line for its delay, then static again;
  F with one up closes it). Their RadioIDs come from actors.json (Level.cs `Notes` now covers
  `BP_Radio_*`); DT_Radio is `assets/data/radio.json` (gen_radio.py, run by `--items`).
  RadioComponent (`MusicRadio`, pure and tested; `Radios::music` by `RadioOwner`: a placed
  Buildable_Radio / _Jukebox id or a vehicle entity): `<game dir>/Radio/*.mp3` (ours: `Radio/`
  under the working directory, gitignored; `SD_RADIO_DIR` overrides; the game ships none),
  StartRadio / StopRadio / PreviousSongRadio / EndReached, played as an `AudioSource` from the
  file's bytes (Bevy's `mp3` feature) with PlayerRadio attenuation and the RADIO slider.
  RadioUI = `GameOverlay::Radio(owner)` (volume 0 / 0.1 / 0.5 / 1, rate 0.25-2 with "No Music
  Playing", TURN ON / OFF, pause, NEXT, PREVIOUS, close). Buildable_Radio's Interact opens it
  (Power: On / Off stat after its first use), the jukebox needs power ("Requires power", stops
  without it), R (VEHICLE RADIO) while driving. check.sh makes two test MP3s with ffmpeg for tour
  step 08zzx_radio. The radio towers (12) are static meshes, already drawn; their pulsing
  light material isn't readable.
- `gameplay/security_cameras.rs`: BP_SecurityCamera_AI (5; BP_MasterCamera < BP_MasterTurret +
  BP_TurretAIComponent): the head (SM_Prop_Security_Camera_Head_01, out of the static props)
  sweeps over Rotate Camera's 12 s cubic curve (`rotate_alpha`: Hermite with the keys' stored
  tangents) at pitch -20, RInterpTo 3; 25 int health from `turrets::Shot` / `harvest::Swing`;
  at 0 FX_Spark_Impact and the base and light ray props go too. Reaction Tags are empty on the
  class and every instance (read from the level package: `SdExport json` of the umap, 200 MB),
  so Perception Check / Alert AI never run: nothing detects. Tour step 08zzcb_camera.
- `gameplay/barrels.rs`: BP_ExplosiveBarrelMaster (the level's 11 BP_ExplosiveBarrel_Child): 25-50
  health; Damage_Object from `turrets::Shot`, `harvest::Swing` and `barrels::ObjectBlast` (the
  frag/pipe bomb/Molotov fire send their DemolitionExpert multiplier, 1 or 1.5, to objects in 10 / 8
  / 5 m, as the BPs do); at 0: FX_Explosion_New, explosion_large_06_Cue, 50 m noise, 75-125 to living
  AIs and the survivor in 8 m with white numbers, the SM_Prop_Barrel_02 prop removed for the
  session. Tour step 08zzm_barrel. Damage numbers in the game come only from guns, melee, frag,
  pipe bomb, barrels, traps, the buildable turret and the nuke (grep of SpawnFloatingDamageText).
- `gameplay/world_events.rs`: BP_EventManager's jets (every 30-3600 s, the instance's
  MaxNumberOfJetsToSpawn 2, 1-3 s apart, BP_Event_JetActor at the survivor's X/Y x 750 and
  300-600 m, 400 m/s for 60 s, RandomJet_Cue through Aircraft; the x750 is the bytecode's, so
  they start hundreds of km away), distant explosions (300-3600 s, explosions_Cue at X/Y x
  rand(-20, 20)) and the Blood Moon (UDS Sunset: 5 % -> `BloodMoon::on`, red moonlight in
  `daynight::light_the_world`, the toast, `BloodMoonBoost` on the zombies alive then: alert
  speed x1.5 in `zombies::act`, swings /1.75 and damage x2 in `survival::zombie_attacks`, XP
  15-25 in `combat::kill_zombies`; every sunrise after the first blood moon settles it with its
  toast, as the BP's never-unbound Sunrise binding does). `force_blood_moon` for the tour
  (08zzd_blood_moon).
- `gameplay/map.rs`: the map (M = IA_Map; MapWidget, BPC_MinimapSystem, BP_MapData) and POIs
  (BP_POIManager). BP_MapData_LongdownValley's texture (`RT_WorldImageCreator_Tex`, a north-up
  capture) as a 4000 px image; `MapData::to_map` = Calculate Actor Position: (Y - mid.Y,
  -(X - mid.X)) / (WorldSize / 4000), so UE +X is up. Zoom/drag are `View` (OnMouseWheel's
  ZoomFactor 0.7975 about the cursor, steps -4..5, -4 = whole map at 0.2675; ClampSides; OnScroll's
  arrow scale and small-POI hiding), unit-tested. The arrow (T_Player, yaw) needs the GPS slot.
  187 POIs: every 1-4 s (DistanceCheck) within ExploredCheckRadius -> `GameState::pois_explored`
  (saved), XP level x 250/50 x 1.05 x XP MULTIPLIER, POIDiscoveredUI's banner (FadeStart keys: opacity
  and the -525 px slide `BANNER_SLIDE`, shared with zones.rs; auto-sized, 3 of its measured heights
  up; plain text, no shadow);
  explored POIs show as W_POIMarker icons (24 px x 5 or 2) with W_POIMarkerTooltip on hover.
  `gameplay/markers.rs`: player markers. With a GPS, a double click (ours: 500 ms / 4 px, the OS
  defaults) goes back to the world through `View::map_point` + `MapData::to_world`
  (CalculateMarkerToWorld) and the top of terrain/props there (Marker Location's trace) and adds a
  `PlayerMarker` (`GameState::map_markers`, saved) with PlayerMarkerSelectionUI's choice
  (`MarkerPicker`: 4 colours, 12 icons, size slider 0.5-4 on Bevy's headless `Slider`, "Compass
  Marker?" = settings key `map/Map Markers/Compass Marker?`, CVar Map.CompassMarker, default on;
  the first markers are white size-2 tents, the BP's own defaults). Double click on a marker
  removes it; on a POI/world marker it removes the last one placed this session (the BP's
  PlayerMarker), else falls through. The special-markers layer and the picker need a GPS
  (ToggleMap). Compass markers: `compass_x` = Compass.NewXPosition (700 tan + 350) / ObjectBehind
  (within 40°) on the HUD's `CompassStrip` (clipped like its RetainerBox). `MarkerRev` bumps
  rebuild the widgets. Quest markers (`quests.rs`) are `WorldMarkers` at size 4 with no
  description, plus their own compass icons. Not ported: the minimap, BackgroundBlur.
- `gameplay/journal.rs` + `journal/info.rs` + `journal/quests.rs`: JournalUI's pages
  (`Journal::page`, starting on INFO each session as OnInitialized does; the tabs are Buttons,
  hover = label x1.1 + click sound). Journal_QuestsUI (`journal/quests.rs`): Show Quest (title,
  description, `QuestLog::journal_branches`: every reached state's branches newest first, past
  ones at 0.5), the Active / Quest Log folders (ZOrder swap; "Quest Log [N]" counts succeeded
  only, the list is failed then succeeded), Journal_QuestButtonUI entries, FORGET ->
  Journal_QuestConfirmForget -> `quests::ForgetQuest` (ForgetQuest, then the 'Forget' state's
  events). `follow_log` rebuilds the page when the log changes. Journal_InfoUI: the watch (two-digit UDS time/date, UDW
  season), the survivor's license (name, sex from the creator's Sex row, age, occupation, LVL),
  SKULL TRACKER kill stats, days survived and distance. UMG Overlay slots default to Left/Top
  (unversioned properties omit it: a slot naming only HAlign_Center sits at the top); `info::slot`
  centres measured text by flex with doubled margins, the same as `overlay_centred`.
- `gameplay/stats.rs`: BP_PlayerController's journal stats (`GameState::stats`, saved):
  zombie/animal kills from `combat::kill_zombies`, DistanceTravelled (cm, every 1 s from the last
  sample; a new pawn restarts it: `PlayerVitals::respawns`), DaysSurvived (+1 at midnight, i.e.
  `EnvironmentSystem::days` going up; 1 in a new game), all zeroed on death with PLAYER JOURNAL
  STATS RESET ON DEATH? (on by default; days too). DaysSurvivedWidget: on entering the game, each
  respawn and 2 s after midnight; 4 s later "Days Survived: N" types at 0.175 s a character,
  holds 2 s, fades 1 s (`popup_at`, tested).
- `gameplay/traders.rs`: traders (BP_TraderMaster's children; `assets/data/traders.json` from
  `extractor/gen_traders.py`, run by `--items`: VendorName, DT_VendorTable row via Enum_VendorTypes'
  display names, mesh, Hair/BeardMeshes, BP_JigMultiplayer's vendor settings, the Narrative
  dialogue's lines and replies). 14 placed traders (the vehicle trader is skipped) are
  `interact::Containers` from `TRADER_KEYS` (1 << 33) whose grid is the stock (rolled like
  StartingItems, again at 10:00 with the money: Event_Reset); F = "Press [F] to talk to X" ->
  `GameOverlay::Dialogue` (BP_CustomNarrativeDefaultUI: speaker, a random opening line, replies;
  the one leading to Event_OpenNPCVendor opens `GameOverlay::Loot(key)`). Reply order is by the
  nodes' X in the graph (a guess). Bodies: AnimBP_Trader plays A_Idle on all (`AnimSet::Trader`),
  hair/beard on `head`, hidden past 350 m, FacePlayer while open. Trading is the inventory
  screen: `InvData::act` snapshots the trader grid, inventory and ground around every action and
  `traders::settle` charges/pays (GetAllItemsSellPrice: count x price x durability%, in Money or
  Credits) or the action is undone with the game's message ("This trader is not accepting
  items", "Items in container", "Trader does not have enough money", "No empty space to receive
  currency"; "Not enough money!" is the BP's dev PrintString, shown as a toast here). Resold
  items get `Stack::price` = price + attachments (SetVendorPriceOfSoldItem). TRADERS HAVE A SET
  AMOUNT OF CURRENCY ENABLED? (a bool row: read it with `Value::Bool`, `survival::difficulty`
  returns 1.0 for bools) gives them 300-600 to spend. W_TraderUI's "Currency: $500 " and
  JSI_Slot's "12$" price tags (bottom-left here). BP_MilitaryTraderSpawner / BP_ShakesConesSpawner
  place their trader at a random Spawn0..4 (Level.cs `Placed`; `item_spawners::point`). Wandering
  traders: BP_MasterTrader is a human AI class (zombies.json `human.trader`, gen_ai.py
  WANDERERS) and a traders.json class (`wandering`, gen_traders.py: WanderingTraderEquipment from
  BeginPlay's SetItemsToSell(22), Dialogue_WanderingTrader); `traders::wanderers` makes a Trader
  for each body met (matched by entity or spawner group), moves its container with it, holds it
  still (`zombies::HeldStill`) while its dialogue or trade screen is open, and drops it when it
  dies (its loot is destroyed: F on the body says so once, `Containers::told`). No restock.
  Prompt: "Press [F] to Trade with Wandering Trader" (`traders::prompt`). Dialogues: each reply's
  NPC nodes (`Reply::branches`, gen_traders.py `branch()`: line, conditions, events, next; nodes
  resolved by ObjectPath export index, names repeat) run through `run_branches` (Narrative's
  first valid node; Condition_CheckMoney takes the money when it passes, before bNot), events:
  Event_OnInteractNPC (the instance named Event_OpenNPCVendor_C_0) = trade screen,
  Event_RandomLaptopQuest = `quests::ScanLaptop` + "Quest pinned to your map"; the NPC's lines
  then play in turn (`FollowUp`, `line_secs`: 25 letters/s, at least 2 s) and close the dialogue.
  Quest givers (BP_QuestGiver_*: the settlement's Settlement Officer, traders.json `quest_giver`:
  no stock, no FacePlayer, the revolver on Sidearm, F sends DataTask InteractWithActor with its
  `argument`) sell intel: 2500 Money for a random laptop quest. Not ported: vehicle
  trader UI, the 3 m walk-away check, body collision, saving stock, the wanderer's map marker,
  AnimBP_StationaryQuestNPC's head turn.
- `gameplay/civilians.rs`: settlers (BP_RandomCivilian_Male/Female; `assets/data/civilians.json`
  from gen_ai.py `civilians()`: bodies, hair, beards, backpacks, walk 1 m/s / 180°/s, capsule,
  BT_RandomCivilian's waits 1±3 / 5±15 s, MoveTo 70 cm, the 20 m wander radius, the 3 name
  tables). Spawned by BP_AISpawner_SettlementSurvivors (`SpawnerDef::settlers`: the master
  spawner's settings, SpawnType 3 = no difficulty row; the level's 4 and the settlement's 2 via
  `Settlers::add`) with `SpawnerRuntime`; `KinematicBody` + AnimSet::Survivor walk/idle;
  `wander_step` (no navmesh: kept within the radius of home); the name tag ("{FIRST} {SURNAME}",
  row 0 = "Interact") over the head while the camera's forward ray meets them within reach,
  gone after F. Shots pass through them (they ignore the Weapons channel).
- `gameplay/harvest.rs`: BP_HarvestableObject_Tree / _Rocks / _ScrapMetal and BP_MeleePickup's
  HitFoliageTree / HitFoliageRock / HitTrashObject. `harvest.json` (`SdExport levelharvest`, also written by
  `level`): the foliage instances whose ISM BodyInstance PhysMaterialOverride has surface Tree
  (SurfaceType16) or Stone (17), minus components that ignore the Weapons channel (the swamp reeds:
  the melee traces are TraceTypeQuery6 = Weapons), plus "trash": the static meshes of the 141
  TrashObject_C actors -> `Level::harvest` by (cell, index). `melee::melee` writes a `Swing`
  (with its MeleeDamage and `power`) at each swing's hit window; `hit_props` casts a 9-ray fan (0.4 / 0.8 / 1.2 m, ±28 cm, 2 m) with
  `CollisionCache::cast_item`. items.json `harvests` = CanDamageTree? (Fireman's Axe, Tactical
  Hatchet) / CanDamageRock? (Pickaxe, Ice Axe, Sledgehammer), else "No damage to tree dealt.".
  6-8 hits (SetMaxHits on the first), ReduceDurability(0.5) each; the tree's last hit: 0.2 s, 8-12
  Wooden Logs, collision off, a falling copy tipping 90° over FallOverTimeline (5 s, flat-tangent
  cubic) away by the BP's odd signed-angle rule (`fall_pitch`), gone at 30 s; the rock: 8-12 Rocks,
  gone at 0.2 s. Harvest Master doubles. Removed props stay gone for the session
  (`PropStreaming::remove`, `PropItem` tags their draw entities). Burnt-out vehicles (78 actors):
  "Burnt Out Vehicle: [F] Salvage", a 10 s "Salvaging" timer (F again or 3.5 m away stops it),
  12-20 Scrap Metal, then "No more Scrap Metal". Trash piles: CanDamageTrashObjects? (Crowbar,
  Pickaxe, Ice Axe, Sledgehammer, else "No damage to trash object dealt."), the swing's damage off
  50 Health (HittingTrash_Cue), at 0 gone at once with WoodBreaking_Cue for 1-5 Scrap Metal (no
  Harvest Master). A power swing counts 3-4 hits on trees and rocks and skips trash piles
  (Event_OnHitPower). Not ported: the camera shake, the white
  hitmarker, FX_HarvestableRock and the wrecks' smoke.
- `gameplay/fishing.rs`: BP_FishingRodPickup + FishingRodPickupComponent + BP_FishingBuoy and
  BP_PlayerCharacter's primary action with the rod (9 = IA_FishingRod draws it; `combat::draw_and_aim`,
  held on Weapon_r only while drawn, the melee stance (a guess for Svr_UpdateCombatState(3)), no
  aiming). LMB: Crouch_MeleeAttack_Montage (animBatonAttack1, `ActionKind::Cast`) at 0.8 with
  Timeline_FishRodMontage's 0.25 s turn to the camera yaw, then CastRod at the montage's
  ActiveMeleeDetect (ours: the BP's trigger isn't readable): a line from the rod tip to camera +
  35 m whose first terrain/prop hit must come after it dips under water (`water_along` over
  `PropRays::water_surface`), else "Too far from water...". Then movement off (main.rs treats
  `Fishing::active` like the landing stun; roll and slot changes are blocked), the lure (x2) at
  the hit bobbing 5-15 cm up / 10-20 down over 1-2 s, a straight line. Every 6-8 s RandomChance(30):
  30 <= rand(0, 100) / FishingMultiplier (the skill makes bites rarer: the game's formula); a
  bite = "You feel a tug on the line", CanCatchFish? for 1.5-2.5 s (the Delays resume at offsets
  15 / 183: read from the raw LatentActionInfo skip offsets, kismet.py hides them), re-roll 4-8 s
  later. LMB in the window = CatchFish: rand(0, 100 / FishingMultiplier) >= 20 Small Raw Fish
  (20-25 Fishing XP), > 5 Raw Fish (40-50), else Large (80-100), FishReelIN_Cue, "You caught a
  fish!"; outside it "You didn't catch anything". `CollisionCache::near_water` now searches a
  one-time index of all ~120 water items: a BP_WaterBox lives in its origin's 64 m cell but can
  reach ~680 m past it, so lake shores had no water (swimming started late there too).
- `gameplay/vehicles.rs`: vehicles. `assets/data/vehicles.json` (`extractor/gen_vehicles.py`,
  `export_assets.sh --vehicles`): BP_VehicleMaster's 17 children with their chains merged
  (VehicleMesh, Colors and the glb's slot-0 material, the ChaosWheeledVehicleMovementComponent's
  EngineSetup / TransmissionSetup / WheelSetups over Chaos's own defaults (CHAOS_* in the script;
  unversioned properties omit defaults, e.g. ChangeUpRPM 4500), each wheel BP's chain, the wheel
  bones' bind positions read from the exported glb, the scene points PlayerLocation /
  ExitPoint_* / FuelBoxCollision / HitBoxCollision, fuel and health components) and DT_VehicleInfo's
  rows by Enum_VehicleSpawner value. Level.cs exports BP_Vehicle* actors' props (VehicleSelection).
  Spawning: per BP_VehicleSpawner RandomFloatInRange(0,100) <= Difficulty.VehicleChance (30, from
  SD_GameInstance's RegisterFloatCVarSetting) while <= 30 exist; BP_VehicleUniqueSpawner (the SWAT
  van) always. Parked vehicles are solid through `PropRays::set_dynamic` (the hit box). F: Get In
  Vehicle (the survivor `KinematicBody::scripted` at PlayerLocation - 30 cm, `Locomotion::Drive` =
  A_Car_IdleShortened, weapons away) or, at the fuel box, UseGasCan (+half the tank); F in it gets
  out under 10 mph at the driver's (else passenger's) exit point. `vehicles::Driving` gates the
  on-foot systems (`gameplay::on_foot` replaces `player_in_control` for movement, weapons, throws,
  roll, fishing...; prompts of doors/containers/wrecks skip while driving; `kill_zombies` now runs
  outside CombatSet so run-over zombies die). `step_drive` (pure, tested) stands in for Chaos:
  torque = MaxTorque x curve / curve peak, RPM from the driven wheels through gear x FinalRatio
  clamped idle..MaxRPM, drive 0 at MaxRPM, auto gears (a MaxRPM under ChangeUpRPM never shifts:
  the SUV tops out at ~29 mph in first, as in the game), Chaos auto-reverse on S, wheel brake
  torques, drag 0.5 rho Cd 2.52 m^2 v^2; ours: rolling 0.015 g, engine braking, input rate 5/s,
  a bicycle-model turn, the body set on the ground under its wheels, props ahead stop it
  (VehicleCollision_Cue at >= 5 mph; the BP's speed-tier damage is never called). Run-over:
  living LevelZombies in the hit box at >= 5 mph take 300-350, the vehicle 5-8 x DAMAGE DEPLETION
  MULTIPLIER / 2.5. Fuel: WasteOfFuelPerTime / WasteTime x FUEL DEPLETION MULTIPLIER (x0.1 under 2
  mph). Event_Noise 25 m every 2 s; H = Cue_Horn. VehicleUI (bottom right: name, HealthBar, FuelBar,
  speed in MPH or KPH). Wheel bones roll and steer in the vehicle's frame. Camera arm 9 m while
  driving (ours). The prompt's options (wheel): Get In Vehicle / Open Storage (Interact_OpenStorage:
  "Vehicle Storage", the class's BP_JigMultiplayer grid, keyed `STORAGE_KEYS` + id: between the
  corpse and trader key ranges, not in `Containers::list`). L = TurnOnOffLights (the class's
  SpotLightComponents, `lights` in vehicles.json, SetIntensity 50000 unitless; not with no fuel or
  the engine off). K = Event_FlipVehicle (< 10 mph, 1 s apart: up 1 m, upright; else "Cannot flip
  vehicle"). EngineSound = MasterVehicle_Audio, a SoundNodeParamCrossFade: gen_sounds.py writes
  each input as a looping cue `MasterVehicle_Audio#i` plus `crossfades` (fade-in/out ranges over
  'RPM' = |mph|); `audio::LoopGain` on the followed entity scales a loop live (its continuous
  pitch modulators aren't applied). Saving: `GameState::world_vehicles` (None until the spawners
  roll once a game; then each vehicle's class, place, yaw, fuel, health, colour, lights, storage
  grid), written every frame by `save_vehicles`, restored instead of rolling. Not ported: push,
  tow, repair, water, smoke/fire FX, RESET VEHICLE SPAWNS?, the vehicle trader, being in a
  vehicle across a save.
- `gameplay/quests.rs`: the Narrative quests (`assets/data/quests.json` from
  `extractor/gen_quests.py`, `export_assets.sh --quests`: each quest's states (kind, events,
  branches) and branches (tasks, destination) by ID; the crate). `QuestLog` (`GameState::quest_log`,
  saved) is the pure engine: `begin` (no-op once started or finished), `data_task`
  (CompleteNarrativeDataTask: task exact, argument case-insensitive or empty = any), `reach`
  (BPT_GoToLocation every 0.2 s from the capsule centre), `forget`, `replay` (a loaded game's
  quests run their visited states' marker/actor events again: a guess at Narrative's refire),
  returning `Out`s: world events and TaskBox ops (BP_CustomNarrativeDefaultUI's delegates: QUEST
  STARTED / UPDATED / SUCCEEDED, branches as "- {desc} {(Optional)} {(n/N)}", FadeInTasks' cubic
  keys). Starters: ShowStartingQuests? (Quest_TheSafeZone each session), BP_StartQuestArea (box
  from actors.json: `Box.BoxExtent`), SCAN LAPTOP (`inventory_ui` Action::ScanLaptop consumes one
  and writes `ScanLaptop`; `pick_laptop_quest` = CallRandomEvent with its never-reset CheckTimes;
  only `QuestDef::playable` quests are in the pool: now all 16). The Eliminate Bandits /
  Military quests: Event_SpawnPrefab (`spawn_quest_prefab`: the prefabs.json camp as entities,
  meshes solid through `CrateCollision`, containers `QUEST_PREFAB_KEYS`), Event_SpawnAIHumanSpawner
  (`SpawnerDef::human_volume` with the event's box, 60 m, no respawn, `QUEST_SPAWNERS`;
  `QuestWorld::human_spawners` maps them to their QuestArgument), `human_kills` (every human
  death sends KillNPC with its volume's argument, else empty = any), Event_RewardGiveItem (its
  ItemSets name no item in the cooked data). RemoveActor drops the camp and the volume.
  `BeginQuest` begins a named quest (the tour's 08zr_quest / 08zy_elimination).
  PowerOnGenerator comes from `doors::interact_doors` (`Generator::quest_argument`).
  BP_GuaranteedSpawner_Classified: registered as an `ItemDb` container class (4 s, 10 % noise);
  `Containers::quest_crates` keys open without a loot screen into `quest_opened`; then
  InteractWithActor, the lid's -65° roll over 1.5 s, and 1.3 s on a random Items weapon on the
  `inventory_ui::Ground`. The crate is solid (`PropRays::set_dynamic_from`). Not ported: quest
  givers, the grab montage.
- `gameplay/airdrops.rs`: BP_EventManager's airdrop timer (300-3600 s, AIRDROPS ENABLED?),
  AirdropAircraft (75 m/s at 450-550 m from the region bounds x DistanceToSpawnAirdrop 5, checks
  every RegionRadius/Speed, drops 1-3 crates 0.1-30 s after crossing the 1750 m region, 1-4 s
  apart) and Master_AirdropContainer (x1.35, sinks at ~9.8 m/s: gravity / LinearDamping 1,
  parachute until it lands, "Airdrop incoming..." at 3 s, a Drop world marker on the map's
  special-markers layer (`map::WorldMarkers`), then a 6 s-hold container "Airdrop" via
  `Containers::add_dropped`, solid via `PropRays::set_dynamic_from`). Level.cs exports the region's
  RegionRadius and the manager's DistanceToSpawnAirdrop; gen_items.py adds the crate classes
  (`ContainerDef::airdrop`). `models::MaterialRename` swaps single material slots (the plane's
  OverrideMaterials). Not ported: the coloured smoke, jets/explosions/blood moon/random events,
  saving crates.
- `src/models/anim.rs`: AnimSequences rebuilt as Bevy clips targeting bones by name; each
  modular body part has its own skeleton copy, so every part gets a player and the same state
  (`Locomotion`) — UE's LeaderPose done by hand. Player_AnimBP's weapon sets are a `Stance`
  (unarmed/rifle/pistol/melee, plus aiming states); one-shot `UpperAction`s (fire, reload, melee
  swings, zombie flinch) play on the upper body via mask groups: each locomotion clip is in the
  graph twice (lower/upper) and the driver crossfades weights itself. Graph node weights must stay
  1.0: Bevy multiplies them into the player weights, and overlapping clips at zero effective weight
  blend as 0/0 = NaN (characters vanished in every crossfade; the tour's `nan_bones` guards it). `wire_late` covers characters tagged `Animated`
  after their (cached) scene already spawned; without it they stand in the bind T-pose. Actions
  have a play rate (`ActionKind::rate`: shoves 1.15, the roll 1.5). Full-body actions (`ActionKind::full_body`:
  the roll, the vault montages) get a second, lower-body node of the clip with its Root translation locked
  (`load_clip_with`), and the locomotion's legs fade under it; `anim::root_forward` gives the root
  motion for gameplay to move the body (`anim::root_motion`: in 3D, with the clip's RateScale). Changing clothes or body parts is staged
  (`models::swap_parts`): new parts spawn hidden (`Incoming`) and show 2 frames after their scene
  is in, and the parts they replace (`Outgoing`) go then, so no T-pose or missing-body frame.
  A part wired after the others (`PartPlayer.current` None) copies a playing sibling's `Snapshot`
  (clips, seek times, weights, action and its serial): before, new clothing parts restarted their
  loop at 0 and replayed the last action, leaving the head and hands out of step. The tour logs
  `phase_spread` per step (`body_parts_move_in_step`).
  `PoseTrack` (props sampled by hand) applies only the tracks that move: constant ones are UE's
  reference pose in UE units (FoldingGate's Root has scale 100) and are left to the glb.
- `src/menus/`: front end rebuilt from `Content/UI/Menus/*` widget blueprints.
  - `mod.rs`: `AppState {MainMenu, InGame}`, `MenuScreen` (one per page), Esc/back rules,
    `UiScale` = UMG's 1920x1080 DPI curve. **All px values are design-canvas units straight from the
    blueprints.**
  - `widgets.rs`: shared style (Montserrat-Bold + outline, button tints 25/50/75% black),
    `anchored()` reproduces a `CanvasPanelSlot` (anchor/offset/size/alignment), wheel scrolling.
  - `main_menu.rs` (MenuWidget + Modes + quit confirm), `new_game.rs` (difficulty selection &
    in-game difficulty), `creator.rs` (CharacterCreatorMenu + 3D preview), `settings.rs`
    (data-driven settings/difficulty panels, persisted to `saves/settings.json`), `saves.rs`
    (SaveMenu over `persistence::SaveSlotManager`), `pause.rs` (PauseScreen, F5 quick save).
  - `data.rs`: `MenuData` (from `assets/data/menus.json`), `CharacterProfile`, `CurrentGame`.
- `src/models/mod.rs`: exported `.glb` loading, material remap from `assets/game/materials.json`,
  modular survivor assembly (`character_parts` mirrors `BP_CharacterCreator`: skinned body parts
  share bind space; static parts hang off the Head mesh's bones at the BP components' relative
  transforms, converted by `ue_relative`: hair/beard on `head`, eyebrows on `eyebrows`, mouth and
  accessories on `head` at (0, 156.9, 0.5) cm, which cancels the bone's bind pose so they follow the head).
- `src/audio/mod.rs`: the game's SoundCues. `Sound` messages (cue, position) pick a weighted
  variant, roll the modulators' pitch/volume, apply the cue's SoundAttenuation over the camera
  distance (Linear or UE's logarithmic curve; no panning/occlusion) and the SoundClass's Audio
  slider × MASTER. Loops (`Emitter` with `follow`, 2D ones too) track their owner and its `LoopGain`; removing `Voiced` stops them.
  Wired: gunshots (FirearmSound), empty click, jam, bullet hits (Head_shot_1-5 /
  bullet_impact_body_thump_01-08), melee whoosh and Body_Flesh hits, explosions, zombie breath
  loops / attack growls / death (hounds: HoundCue / HoundAttackCue), footsteps by surface
  (`footsteps.rs`: the dominant paint layer's PhysMaterial, props = Concrete), flashlight, whistle,
  UI clicks (every `Activate`), doors (`doors::door_sounds`: keycard/keypad lock open/close, lab
  doors, hangar/warehouse squeak; the crowbar's WoodBreak), and `AmbientLoop`s placed from the
  level's actors (`power::ambient_sound`: fires' SC_CampFire, camp generators, the police
  generator's hum while on), the reload montages' AnimNotify_PlaySound (sounds.json
  `action_sounds`: gen_sounds.py maps each montage's notifies through its segments to sequence
  time; `audio::action_sounds` plays them as the survivor's action passes them), and
  `gameplay/ambience.rs` (BP_AmbientSoundController: birds by day, crickets by night, 0.3 x the
  weather's multiplier), and `gameplay/weather_sounds.rs` (Ultra_Dynamic_Weather's MetaSounds,
  read with `extractor/metasound.py <dump.json>`, which prints a MetaSoundSource's graph: inputs,
  variables, nodes with their literals, edges. sounds.json `weather.waves` holds each MetaSound's
  wave arrays as "<MetaSound>.<variable>". The rain/wind loops x4 (the directional graph's two
  components x two halves, wave indices 0-3), whistling, distant thunder every 17 s on two
  players, close thunder from `weather::LightningFlash` (UDW_Lightning_Spawn_Manager: period
  60/14 x (1 ± 0.5), strikes when Thunder/Lightning > rand(5, 10), 1.55-6.5 km, 0.15 s delay per
  km); x 0.1 under a roof (`weather::Sheltered`). No panning or low-pass filters). `audio/music.rs` = BP_Audio_Manager + BP_Audio_Area (11 boxes in
  actors.json with their BoxExtent; the highest priority one the survivor is in): a random
  DelayBetweenMusic wait (Main 40-120 s, others 5-10 s), a random ExplorationMusic track cut at 95 %
  of its length (`wav_seconds` reads the WAV header), 4 s fade on area change unless the same cue,
  combat music (Main only) when CheckCombat finds a chasing zombie within 30 m (every 2 s, COMBAT
  MUSIC ENABLED?): fade 2 s, wait 1 s, fade in 2 s; the Subway's Underground_Cue bed; the shipwreck's
  BP_AudioPoints; MainMenuMusic (0.5) under the menus. The logic is a pure `Director`, unit-tested.
  Not yet: weather, equip/lever/siren sounds, reverb/indoor muffling.
- `src/progression/passive.rs`: PassiveSkillsComponent decompiled (see Skill trees below);
  `GameState::passive` (saved). Level-ups queue in `level_ups`, which `notify.rs` announces.
- `src/{ai,combat,crafting,economy,engine,items,looting,persistence,progression,survival,vehicles,world}`:
  simulation layer from earlier work (`engine::GameState`). Mostly not yet wired into the Bevy world.
- `src/ui/menus.rs`, `src/ui/renderer.rs`: the old terminal renderer, superseded by `src/menus`.
- `README.md` describes the simulation layer; its claims about "authentic" 3D models predate the
  real asset pipeline (the old `scripts/generate_authentic_models.py` made box placeholders).

## Asset pipeline (`extractor/`)

The game is UE 5.3 with unversioned properties; the old umodel build in `../UEViewer-master` can't
read it. `extractor/` is a small .NET 10 CLI on **CUE4Parse** that reads the pak directly:

```
extractor/export_assets.sh --all         # everything below
extractor/export_assets.sh --sounds      # assets/data/sounds.json + decoded waves (after --items)
extractor/export_assets.sh               # UI textures -> assets/game/
extractor/export_assets.sh --models      # character/zombie/vehicle/prop meshes (.glb) + materials.json
extractor/export_assets.sh --level       # LongdownValley: terrain, placements, splines, actors, meshes (~1 GB)
extractor/export_assets.sh --anims       # animation clips (*.anim.json); builds CUE4Parse-Natives for ACL
extractor/export_assets.sh --menu-data   # regenerate assets/data/menus.json (committed)
extractor/export_assets.sh --items       # regenerate assets/data/items.json (committed)
extractor/export_assets.sh --vehicles    # regenerate assets/data/vehicles.json (committed) + vehicle meshes
extractor/export_assets.sh --quests      # regenerate assets/data/quests.json (committed) + the quest crate's meshes
extractor/export_assets.sh --particles   # regenerate assets/data/particles.json (committed); the level's emitters, particle meshes
extractor/export_assets.sh --prefabs     # regenerate assets/data/prefabs.json (committed); prefab meshes (after --level)
extractor/export_assets.sh --buildables  # regenerate assets/data/buildables.json (committed); buildable meshes
```

- Needs the Steam install (default `/ntfs/SteamLibrary/steamapps/common/SurrounDead/SurrounDead`,
  override `SD_GAME_DIR`) and `~/.dotnet` (net10 SDK; set `DOTNET_ROOT`).
- Mappings: `Binaries/Win64/Mappings.usmap` (dumped by UE4SS) has its 2 magic bytes swapped; the
  script fixes a copy in `extractor/mappings/`.
- `SdExport list|tex|json|mesh|raw` (`extractor/Program.cs`; `raw` copies a pak file's bytes, a
  `.ufont` written as `.ttf`, e.g. LetterUI's F_ESRPG handwriting font). A trailing `/` on a package path means
  the whole folder. `json` dumps exports (widget trees, data tables, BP components). This is the
  main tool for reading game data. CUE4Parse writes `\`-joined paths on Linux; `FixPath` moves them.
- `widget_tree.py <Widget.json>` prints a UMG hierarchy with slot layouts. Start here when porting a menu.
  Children are in draw order, resolved through each panel's Slots by export index (until
  2026-10-21 slots were matched by name, which repeats across panels, so widgets ported before
  then may have siblings out of order).
- `export_assets.sh`'s `has --all` was always true (it tested its own argument), so every run
  also rebuilt sounds.json; fixed.
- Timeline lengths and curves *are* in the BP dump (`TimelineTemplate.TimelineLength`, `CurveFloat`).
- **Blueprint logic**: `SD_SCRIPT=1 bin/out/SdExport json <out> <pkg>` also deserializes Kismet
  bytecode, and `kismet.py <dump.json> [function...]` prints it as pseudo-code with jump offsets.
  Gameplay is Blueprint, so this (not Ghidra) is how formulas, timings and branches are read
  (the lockpicking was ported this way). Raw strings: `/home/lev/Qwen/extracted_pak` + `strings`.
- `kismet.py` prints a latent action's resume offset (LatentActionInfo's EX_SkipOffsetConst): a
  Delay's `LatentActionInfo(607, ...)` resumes the ubergraph at 607.
- `gen_menu_data.py` also takes `Save_Difficulties`' class defaults as the difficulty rows'
  `default` (DayLength 30, NightLength 15, AiRespawnTimer 1500...); `SettingsStore::fill_defaults`
  applies row defaults at startup.
- Check rows: `gen_menu_data.py` matches Save_Difficulties booleans by label only for rows that
  have such a field; others take SD_GameInstance's RegisterBoolCVarSetting. Until 2026-10-28 every
  unmatched row got 0, so PRIMARY / RANDOM INFESTATIONS, the four DURABILITY rows, FIREARM
  MALFUNCTION, MELEE MULTI HIT, MELEE DAMAGE DEGRADATION, TRADERS HAVE A SET AMOUNT OF CURRENCY,
  RANDOM EVENTS HAVE A MARKER and LOOT REGENERATION DISABLED ON LOAD were off by default (all on in
  the game).
- `gen_menu_data.py` also fills sliders without a default from SD_GameInstance's
  `RegisterFloatCVarSetting(cvar, value)` calls (31 rows, e.g. VEHICLE SPAWN CHANCE % 30), and
  check rows (their DefaultCheckBoxSetting's CVarName) from `RegisterBoolCVarSetting` (44 rows:
  CROUCH TOGGLE and PLAYER JOURNAL STATS RESET ON DEATH? on, AIRDROP NOTIFICATION ENABLED? and AUTO
  SAVE ENABLED? off...). Before, checks fell back to "on if the label says ENABLED?".
  `saves/settings.json` (version 2) stores only values that differ from the defaults; an older
  file (every row stored) drops values equal to the old guessed fallback where the row now has
  the game's default (`SettingsStore::fill_defaults`).
- `gen_menu_data.py` also exports `DT_Difficulty` as `difficulty_presets`; clicking Peaceful..Nightmare
  applies one (`SettingsStore::apply_preset`, = GameFunctionLibrary.SetDifficulty's 22 CVars).
  Ours, at the user's request (2026-10-24): every other difficulty row is reset to its default
  first, so a preset sets the whole page (the game leaves the other 59 rows as they were).
  Tour step 06a_preset clicks Nightmare then Easy and checks the shown values follow, then puts
  the settings and `CurrentGame::difficulty` back (else the rest of the tour runs on Easy).
- `gen_items.py` also writes each gun's `firearm` block (its pickup BP's BP_WeaponsPickupComponent:
  Enum_Firearms type by value, automatic, malfunction chance, sound radius, RecoilData's four
  fields (kismet.py shows struct members as `?`; the raw bytecode JSON names them), RecoilRate and
  ResetRecoilDivider) and
  `reduce_durability`, plus the AI loot tables (`AI/AILootTables`).
- `gen_sounds.py` (`--sounds`): the cues the port plays (its CUES list, the guns' FirearmSound
  etc. from items.json) flattened from their node graphs (WavePlayer, Random weights, Modulator
  ranges with UE's 0.95-1.05 defaults, Mixer, Looping; SoundCue VolumeMultiplier defaults to 0.75),
  the SoundAttenuation assets (inner radius, falloff, model) and the landscape layers' surfaces.
  Waves are Bink Audio: `SdExport audio` writes `.binka`, `vgmstream-cli` (needed on PATH)
  decodes to WAV under assets/game/ (gitignored). Run it through export_assets.sh (it needs
  SD_USMAP); it refuses to run with an empty listing. `breakingopen` fails to decode.
- `gen_menu_data.py` turns settings/difficulty pages, `IMC_General` key bindings, creator options and
  `Enum_Occupation` into `assets/data/menus.json`. Radio-select options aren't serialized and are
  inferred from the page string table (`"inferred": true`).
- `gen_materials.py` resolves each glTF material name through the MI→master chain to its albedo
  texture, tint and (trees) trunk colour; an untextured master used directly takes its colour
  parameter's default from the mesh export's stub (M_CharacterHair's brown on traders' hair). `BloodColour` is not a tint, and the runtime ignores tint
  alpha (zero on zombies/animals/branches; with the alpha cut-off it made them invisible). Tint choices that contradict the game's own screenshots
  are overridden there with a comment saying so.
- Clothing layers as in the game: BP_PlayerCharacter attaches body armor as one more skinned
  mesh (MC_AttachClothing) and CheckClothing hides nothing under it, so a vest over a bulky coat
  shows the coat poking through, in the original too (same meshes).
- `SdExport level` (`Level.cs`) writes Bevy-space transforms; `SdExport levelactors <out> <umap>`
  rewrites only `actors.json` (spawners, doors, power actors (lab, generators, the lever), item
  spawners, BP_MapData and BP_POIManager (its POI struct as JSON) carry their property overrides as `props`; ladders' and item spawners' moved components
  as `<Component>.RelativeLocation/Rotation`). Placements skip components whose
  OverrideMaterials (their own, else the Blueprint template's `<Name>_GEN_VARIABLE`) are all
  `Invisible` (BP_NavModifierPlane's planes: 8 white slabs, one over the swamp, until 2026-10-24);
  other material overrides are still not exported. **Axis mapping** (verified against
  mesh bounds, PlayerStarts and skeletons): position (X, Z, Y)/100, rotation quat (x, z, y, -w),
  scale (X, Z, Y). Landscape heights: `(loc.z + (h - 32768)/128 * scale.z) / 100`.
- `SdExport animjson` (`Anim.cs`): CUE4Parse can't write glTF animations, so keys are sampled per
  bone into JSON (same axis mapping). Bones whose skeleton `TranslationRetargetingMode` is
  `Skeleton` (all but Root/Pelvis/IK here) take the reference-pose translation, as UE does;
  retargeted clips (ThirdPersonIdle/Walk) otherwise distort the limbs. ACL-compressed clips need `CUE4Parse-Natives.so`, which NuGet
  doesn't ship for Linux; the script builds it from the CUE4Parse repo into `extractor/natives/`.
  They also need `provider.LoadVirtualPaths()` (Program.cs): their codec settings live at
  `/ACLPlugin/...`, a plugin mount point (without it: "Unsupported compressed data type"). `SD_DEBUG=1`
  prints the full exception. Additive clips (Fire_Rifle_Ironsights) come back from CUE4Parse as
  full poses on their base, but with scale 4 on every bone; Anim.cs keeps the reference scale
  (the hair, hung off the head bone, was drawn 4x as big while firing). `check_data.py`
  `character_clips_dont_scale` guards it.
- `gen_quests.py` (`--quests`): every Quest_*.json under Quests/Quests (QuestTemplate's
  states/branches by their IDs, task and event exports resolved by the ObjectPath's export index:
  names repeat, e.g. two Event_RemoveMarker_C_0), DescriptionOverride, BPT_GoToLocation's class
  defaults (500 cm, "Location Name"), and BP_GuaranteedSpawner (ContainerName, DurationToHold,
  noise, Items mapped to items through the data assets' PickupClass, meshes, the Arrow/Scene and
  lid offsets, the Timeline length). Level.cs exports BP_LaptopEventManager's `Quests` and
  BP_StartQuestArea's `Quest`, `Box.BoxExtent` (`Quests(cls)`).
- `gen_terrain.py` packs layer weights (3 RGBA tiles) and albedos (12 tiles) into vertically
  stacked PNGs loaded as texture arrays. The layer→texture pairing is by name (graph not readable).
- `gen_ai.py` (`--ai`) also reads the animals (`SD_SCRIPT=1` dumps of AI/Animals/*): CDO Health /
  DamageToDo / montages, the AI controller's senses, the behaviour tree's branches (labelled by
  their CanSeePlayer / IsInvestigating decorators: speed services, waits, tasks), task graphs
  (Run Away's offsets, FindRandomLocation radii), and the BP graphs through `kismet.py`:
  SetMeshesAndMaterial's random picks (only the cases its RandomIntegerInRange reaches),
  SetLootTable_AI_DA's enum, ApplyMedicalEffect's chances, the XP roll, the corpse Delay.
- `gen_items.py`: 589 items, 28 loot tables, `unique_loot` (DT_UniqueLoot), 109 container classes (with their StaticMesh), `DT_CraftingTableTypes`
  recipes (Player + workbenches), weapon `RandomStats` (ordered: rolls stop at `MaxStats`),
  `DefaultAttachments`, magazine capacity/ammo, equip sockets, and the survivor skeleton's sockets
  (UE rotators converted to glb-space quats) → `assets/data/items.json`. Guns' `slots` come only
  from Inventory/Widgets/Firearms SC_<gun> widgets; other special containers (SC_Battery,
  SC_NVG, SC_Respirator) feed `holds`. `--items` also dumps Inventory/Items/LocalAttachments/
  and, listed from the committed items.json, every gun mesh with slots and its Skeleton (two
  extra `SdExport json` passes) for `mesh_sockets`; the attachments' actor meshes join the
  item-mesh export list.
- `assets/game/` is generated and **gitignored**, like `*.png`/`*.obj`. Don't commit extracted game art.
  (Raw `.uasset`/`.uexp` files under `assets/SurrounDead/` *are* in git from earlier work.)
- Runtime quirks handled in `models`: vertex colours (`COLOR_0`) are stripped (UE materials ignore
  them; hair has zero vertex alpha), `Invisible` material meshes are hidden, glass gets a translucent
  stand-in. Bevy warns about `TEXCOORD_2..7` on every load; that's harmless.

## Reverse-engineering resources (outside the repo, in `/home/lev/Qwen`)

- `ghidra-mcp/` (Ghidra MCP server; see its CLAUDE.md) and `ghidra_projects/Surroundead.gpr`.
  Native code is mostly engine; gameplay lives in Blueprints, so `SdExport json` on the BP usually
  answers questions faster than Ghidra.
- `surroundead_re_summary.txt`, `surroundead_native_classes.txt`: earlier RTTI dumps.
- `SurrounDead-Windows.pak` (copy), `extracted_pak/` (raw extraction), `UE4SS_Setup/`.

## Known gaps / next steps

- Combat: the
  hand fire montage for pistols (only Fire_Rifle_Ironsights exists; its user isn't found). (No
  crossbow exists in this build's items.) Aim offsets (pitch) and crouch/sprint variants of the weapon sets aren't wired, nor
  pistol fire. Zombies ragdoll in the game; here they topple stiffly. No hit reactions, shot alerts,
  sounds, scopes, throwables' tumbling/DemolitionExpert,
  durability loss. Reload time, melee reach/cooldown, aim arm length and fall-off curve are guesses.
- Inventory: the SC_<gun> slot panel (attachments go on by dropping, off by "Remove").
  Amount dialogs (`inventory_ui::SplitDialog` + `AmountKind`): Shift+drop = JigSplitWidget;
  a stack of 2+ dropped outside the panels = DropItemAmountSelector ("Drop {name}", 1..min(count,
  MaxStack), FTrunc(max x slider), starting full; `Ctx::drop_part`); a stackable stack of 2+
  dragged out of a trader's grid = its "Buy {name}" form (moves the amount through `Ctx::split`,
  so `traders::settle` charges it). ConsumeAmountSelector only opens for capacity containers
  that are used (Use / Rip / Fill), and in this build only magazines are capacity containers, so
  it never shows. `gameplay/inspect.rs`: the item inspector (Inspect in the context menu, or
  a double click: `inventory_ui::on_click` on PointerClick `count == 2`, `inspect::double_click`:
  items with a SpecialContainerWidget (items.json `special`, gen_items.py: guns' SC_<gun>,
  backpacks, SC_Battery...) open that window instead: see container windows below) as BP_InspectorCanvas'
  windows (`Inspector::windows`, any number, one per stack; SetInitPosition's ±50 px jitter;
  a press brings one to the front): each a 400 x 400 render (its own camera on render layer 30 +
  slot into a 512 image, its stage 100 m apart, framed on the bounds and started down the
  thinnest axis like the icons: `start_view`), FOV = item FOV x 1.5, wheel 5° within items.json
  `zoom`, LMB drag turns, RMB drag pans (/30 cm), the title drags it, "Price: ${a}", items.json
  `description` and `inventory_ui::stat_lines` (shared with the tooltip); a weapon's attachments
  on their sockets (InstallAttachments via `attachments::placement`). Tour steps 08fb_drop,
  08fc_inspect, 08fcb_inspect_more.
  Container windows (`gameplay/inventory_ui/windows.rs`, ContainerWindowWidget): a double click
  or the context menu's Open on a storage item (items.json `layout` = its SC_ widget's boxes /
  spacers / JSIContainers from gen_items.py `sc_layout`; `grid` indexes `grids` / `contents` in
  PreInitSpecialContainer's AllContainers order, read from the SD_SCRIPT bytecode, which isn't
  the drawing order) opens a window whose grids are `Loc::Window(id, i)` = that stack's
  `contents[i]` (`Ctx::window_stack` resolves the window's `Owner`: the item's grid cell or
  ground spot, recursively for windows in windows). Moving the item closes its window (and those
  inside); worn items, a trader's stock and (with DISABLE BACKPACK IN BACKPACK?) backpacks
  inspect instead (`windows::opens`). The INVENTORY column draws worn containers with the same
  layouts. GetPos (`fit`) once laid out; z 24 / 25, under the inspector. Guns' slot panels,
  SC_Battery / SC_NVG / SC_Respirator: `Opens::NotPorted`. `SD_VIEW_PAGE=bag_window` (any
  `TourPage::named`) shows a tour inventory page in an SD_VIEW shot. Tour step 08fcc_bag_window
  (08fd_repair's `tour_unpack_bag` takes its Lunch Box back out: 08zzk_respawn's beans need the
  pocket room).
- HUD: radial menu. (The action bar has no drawn-slot highlight in the game either:
  neither BP_Ingame nor JSIContainer recolours a slot.)
- Tech tree (`progression/tech.rs`, data `assets/data/techtree.json` from gen_items.py's
  `tech_tree()` over SkillTree + Skills/*): unlock rules and messages, `GameState::tech` (saved),
  points = `ProgressionSystem::unspent_points` (+1 per level, LevellingComponent: 500 XP ×1.325
  ceil per level, cap 50, XPDeath on Pause > Suicide). Wired: Health Regen, Lockpick+, Critical
  Hitter (crit chance ×2), Demolition Expert (frag/pipe bomb/Molotov ×1.5), Night Creature (melee
  ×1.35 before 06:00 / after 21:00), Deathbringer (health 25 once a life; the tooltip says 15),
  Distraction (`whistle.rs`, O), Huntsman (`animals.rs`: the Animal Meat bag), Combat Roll (`roll.rs`, C = IA_Roll: Svr_PlayerRoll's checks, 8 stamina,
  turn to the WASD input, Roll_Montage full-body at RateScale 1.5 with its root motion (5.4 m) moving
  the body, 1 s IsPlayerRolling? then StopBurning), Parkour (`vault.rs`: vaulting while falling,
  its montages and Delays at 1.35), Harvest Master (`harvest.rs`: logs, rocks, scrap ×2). The journal (`gameplay/journal.rs`, J = IA_Journal) shows
  Journal_SkillsUI: passive skills and the clickable tree. Passive skills
  (`progression/passive.rs`, PassiveSkillsComponent): levels/XP/multipliers and the level-up toast
  are ported and wired: Fitness (stamina, sprint speed, sprint XP), Toughness (damage ÷, 2-4 XP
  per hit survived), Sneaking (footstep range ÷, 0.2-0.4 XP per crouched step), Strength (melee
  damage ×, swing rate × StrengthSpeedMultiplier, 1-2 XP per hit), Marksmanship (XP only: 4-6 per
  headshot, 1-2 per body hit; its effects are scope sway and the scoped breath-hold stamina),
  Reloading (reload rate, 2-3 XP per reload), First Aid (healing ×, 5-10 XP), Scavenging (search
  time ÷, 4-6 XP per search or picked container), Thief (Quick Search noise chance ÷; 20-30 XP per
  picked door, 40-60 per picked container). Books and magazines: the READ option (items.json
  `context` = ContextMenuOptions): `<Skill>Book` +250 XP, `<Skill>Magazine` starts the skill's
  XP boost (×2, Fitness ×3, until its next level) or says "Reading this book had no effect...";
  both give character XP (level × 80-100 × 1.05 × XP MULTIPLIER). Fishing XP comes from
  catches (`gameplay/fishing.rs`). Not ported:
  the skills UI (Journal_SkillsUI, Tooltip_PassiveStats).
- Zombie variants: hordes come only from the Horde Beacon (`building.rs`), archetypes (Armored Feral Hound...); the Radiated/Infected
  master materials' look (glow/tint: graph unreadable) and glowing eye meshes; crawlers use the
  standing attack montage like the BP but our attack is a looping locomotion state. The
  The `_SettlementSurvivors` spawners are exported with props but ignored.
  Zombie_EnemyInRange reaches SD_AIComponent.MeleeAttackDistance (200 cm, capsule centres); the
  zombies still use the older 1.5 m feet-to-feet guess (`zombies::MELEE_RANGE`), the animals the
  real value.
  The old `survival::MedicalSystem` / `conditions` and SurvivalStats' infection are the earlier
  simulation's inventions (no longer ticked in the world). Event_HungerDebuff / ThirstDebuff only
  arm the starvation-damage timers (ported). Every hit the survivor takes (zombies, falls,
  starvation, bleed ticks) goes through `survival::take_hit`; death is health <= 0 only (the old
  SurvivalStats infection, which zombie hits used to raise until it killed, isn't the game's) = CalculatePlayerDamage: worn body
  armor then headwear with `IsArmor?` divide by 1 + ArmorProtection/100 and wear when ARMOR
  DURABILITY ENABLED? (`equipment::armor_damage`). SprintingRestricted? (-125 cm/s sprint) is never
  set in the game: OnRep_BodyArmorEquipped? compares ItemIDs 'MilitaryHeavyArmor(2)' while the
  items are 'HeavyMilitaryBodyArmor(2)', so it isn't ported either. AnyDamage divides by the
  passive ToughnessMultiplier first (take_hit and tick_vitals' starvation/bleed hits).
  The optimiser's nearer layers (tick throttling, frozen unseen poses) aren't ported, and the respawn
  queue and culled zombies aren't saved. The hound always uses the German Shepherd skin
  (`Materials` also lists a Doberman one).
- Day/night: UDS clouds/atmosphere aren't simulated; sky/light colours are hand-picked blends.
  Weather lacks wet/snowy/dusty materials, wind on foliage, sounds, and Radiation weather's effect.
  Grass is the data's pale olive (UE's grass shader darkens it; graph unreadable).
- Movement: gravity and jump are CharMoveComp's (GravityScale 1.75, JumpZVelocity 600; jumping
  needs/costs 5 stamina, not crouched or aiming). Crouch (main.rs: Left Ctrl, CROUCH TOGGLE
  setting; 1.2 m/s, Shift = Crouch Sprint 4.2 m/s; capsule 1.64 -> 1.0 m) and OnLanded fall damage
  (survival.rs) are ported; UnCrouch isn't blocked by ceilings (collision has none), the
  CrouchSpringArm curve and Hard_Landing_Montage aren't read. Footsteps (gameplay/footsteps.rs)
  report noise from the clips' Footstep notifies (Anim.cs exports notifies and RateScale);
  the range is ÷ SneakingMultiplier; surface sounds aren't played. Sitting (IA_SitDown, -; main.rs) loops
  SittingPlayer_Anim with movement off; - or Space stands up. Its Event_SitDownRegen only restarts
  the regen timer, which ours never stops, so it adds nothing here. Vaulting (`gameplay/vault.rs`,
  IA_Jump's second event in BP_PlayerCharacter): `plan` = its traces (a 10 cm sphere 1 m ahead
  from ForwardArrow, 72 cm up; 5 cm spheres down for the top and the far side 50 cm in; each
  sphere is 5 parallel rays, ours: one ray fell between a picket fence's boards), > 80 cm over the
  hit = Climb (headroom checks, no broken bone; a thin wall adds JumpingDownFromWall), else GetUp
  (far side within 30 cm) or Vault. Pure and unit-tested against boxes; `phases` has the BP's
  Delays. `KinematicBody::scripted` (the Flying mode) hands the body to the montage's 3D root motion
  (`anim::root_motion`) during each Delay, floors still stopping the feet; after the last Delay the
  body walks again and the clip's horizontal root motion runs out. The vault runs before
  main.rs's movement (`VaultSet`), which then skips the jump. Not ported: NoClimb tags, the
  landscape as a wall, the capsule sweeps along the root motion. A falling body over props still streaming in hangs up to 5 s
  (collision.rs `HOLD_FOR`, ours).
- Flashlight (`gameplay/flashlight.rs`): L, Flashlight slot, battery = the flashlight's default
  attachment (dropped onto it in the inventory, "Remove Battery" in its menu: ours, the game uses
  the SC_Battery slot), 0.0125 per 0.1 s. The HUD's FlashlightDur box (hud.rs) shows the charge
  while on (no fade). Not ported: light function materials, sounds, the mesh in hand; the BP's `Difficulty.BatteryMultiplier` vs the menu's
  BatteryDepletionMultiplier is unresolved (the menu row is used).
- Melee: see `gameplay/melee.rs`. Input events' trigger types are in the class's
  EnhancedInputActionDelegateBinding export (`InputActionDelegateBindings`: TriggerEvent 1
  Triggered, 2 Started, 16 Completed); the event params ElapsedTime / TriggeredTime show up in
  kismet.py as `<EX_Cast>` (read the raw bytecode for the source variable).
- Power: the stairwell light switches (trigger boxes), road-light/camp generators,
  generator hum/shake, the generator's grab montage, saving power
  state (not saved in the game either? unchecked). The lever's timeline curve eases; ours is linear.
- World: (door state isn't saved, as in the game: the door BPs lack EasyMultiSave's actor
  interface, so `XPGiven?`'s SaveGame flag does nothing.) Door sounds. Swimming, particles' lights/collision/flipbooks and the Niagara effects (`gameplay/particles.rs`), the settlements
  (`level/prefabs.rs`: their survivors, traders and quest givers aren't ported).
- Vehicles' push/tow, repair and the reset-spawns button; the vehicle trader; quest givers and settlement NPCs; base building beyond placing, snapping, storage, beds, generators, lights, traps and turrets (the other buildables' functions) (animals: see `gameplay/animals.rs`; traders: `gameplay/traders.rs`; harvesting: `gameplay/harvest.rs`; fishing: `gameplay/fishing.rs`).
- The main menu backdrop cycles `UI/Textures/Backgrounds` screenshots; the original renders the
  `MainMenu` level + `MainMenuLevelSequence`.
- Mode levels (Shooting Range, Tutorial, Vehicle Playground) are disabled. Key rebinding, save
  search/rename and the save menu's "create new slot" box aren't implemented. Discord/Trello/Roadmap
  buttons are omitted. `BackgroundBlur` is approximated with a dark overlay.
- Audio: equip and vehicle sounds; reverb and indoor muffling (SetInside); stereo panning; low-pass filters (the weather's occlusion and wind).
