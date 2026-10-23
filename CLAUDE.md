# SurrounDead → Rust port

A Rust/Bevy reimplementation of **SurrounDead** (UE 5.3 open-world zombie survival, game build
`Preparation - v0.7.1g`). Remote: `github.com/dlewi069/RustDead`.

The goal is parity with the original: layouts, text, data and art come from the game's own
assets wherever possible, not from invention. When something has to be guessed, say so in a
comment (see `menus/settings.rs::default_value`).

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
  hits and exits (`src/menus/autoshot.rs`). The tour forces vsync on the window (`steady_frames`; uncapped 200+ fps made timed steps flaky). `SD_START=<n>` pins the PlayerStart; `SD_BURST=08g_armed,...` also saves a frame every 0.25 s of those steps. Use it to check
  changes visually.
- Dev builds use `opt-level = 1` (deps at 3); the map is too heavy for unoptimised Bevy.

## Testing workflow (required)

- **While working**: after each change run `scripts/test_related.sh` (tests for the files changed
  vs git HEAD: `src/a/b.rs` -> `cargo test a::b::`, data JSON -> the modules that read it, python
  -> compile + data checks). It lists changed Rust files that have no `#[test]`: add tests.
- **New tests go in the full suite** automatically when they are `#[test]`s in `src/`, functions
  in `CHECKS` of `scripts/check_data.py` (data consistency) or `scripts/check_tour.py` (tour
  assertions), or new steps in `src/menus/autoshot.rs`'s TOUR. Add one of those for every feature.
- **The full cycle runs only once, at the end of a prompt** (the user's rule): not after each
  feature or fix mid-prompt. In between, use `scripts/test_related.sh`, single `cargo test`s and,
  for visual checks, a targeted tour run (`SD_MENU_SHOTS`, `SD_BURST`).
- **At the end of every prompt**: `scripts/check.sh` (build without warnings, clippy no worse than
  `scripts/clippy_baseline.txt`, all unit tests, data checks, the screenshot tour + assertions;
  ~1.5 min). `--quick` skips the tour. It runs automatically as a Claude Code Stop hook
  (`/home/lev/Qwen/.claude/settings.json`, `--hook`: skipped when nothing changed since the last
  pass; a failure blocks finishing with the report, up to 3 times). Output: `target/check/`.

- A targeted tour must match check.sh: `SD_START=3 SD_MENU_SHOTS=<dir> cargo run -q`, then
  `python3 scripts/check_tour.py <log> <dir> <exit code>`. Running `target/debug/surroundead`
  directly loses `CARGO_MANIFEST_DIR` (Bevy's asset root) and nothing loads; without `SD_START`
  the start is random and steps land elsewhere. Flaky steps found and fixed 2026-10-22: 08zg_deer
  (a deer fleeing straight away has the survivor in its blind spot, and the game's Can See Player
  decorator, FlowAbortMode Both, aborts the flight: the step now checks it fled at all), 08ib/08ic
  (a late stomp outlived its step and swallowed the grenade's G: the stomp check waits for the
  move to end, G waits for no shove and an idle weapon), 08zp_fish (the rod tip sits 0.3 m over
  the bank: cast at 15° down, not 35°). Still seen now and then: 08u_ladder_top (2026-10-22, once: back at the
  foot, 8.7 m behind the ladder), 08zg_deer (fled only 2.8 m), 08zh_wolf (a 4-point bite: a hound's
  hit counted again), 08m_hound (effect rolls),
  08ze_swim, 08zf_vault (logs the body's state at the press), a missing last screenshot
  (12_suicide, once). The tour runs on the viewer's saves/settings.json; steps that depend on a
  setting set it themselves (`airdrop_notice`) or work either way (crouch toggle).

- A tour run whose window is hidden (another workspace) gets ~1 fps on Wayland (no frame
  callbacks), so timed steps fail. `env -u WAYLAND_DISPLAY` (XWayland) keeps 60 fps, but Hyprland
  may resize the window mid-run, which changes the UI scale in the shots.

- **Progress note**: keep `/home/lev/Qwen/obsidian/SurrounDead in Rust.md` (the user's Obsidian
  vault) current as features land: What works / Not done yet / dated Log.

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
    flicker, nearest 24 spawned; lab/infestation/lamp lights wait for their systems.
  - Water: BP_WaterBox/WaterSpline surfaces (SplineWaterTEST, PlaneSplineMesh) get a translucent
    water material (Mat_Clear_Water's graph isn't readable; colours are a guess) and don't collide.
    BP_WindTurbine blades spin at Wind x 2 deg per 0.01 s.
  - `doors.rs`: the door BPs decompiled (89 doors: locked/picklock+crowbar, keycard, hangar, lab
    sliding/secure/entrance, warehouse, shipping container, subway travel, the oil rig keypad door).
    Their meshes are taken out of the static cells (`find`) and animated by the BP Timelines'
    lengths; moving parts collide through `CollisionCache`'s dynamic sets. F rules live in the pure
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
    ported (tour step 08zzb_settlement).
  - `item_spawners.rs`: BP_SpecialItemContainerSpawner (the "Police Briefcase" with the
    PoliceStationKeys unique loot = the Police Generator Key, at one of 4 points, random yaw) and
    BP_Keycard_Spawner (3 keycard briefcases, one of 2 points each), rolled per session; the
    briefcases join `interact::Containers` keyed by the spawner's actor index (`spawned`: name +
    DT_UniqueLoot sets added to the roll) and draw their class's StaticMesh (`ContainerDef::mesh`).
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
- Containers (`interact.rs`): the hold is DurationToHold ÷ ScavengingMultiplier × SEARCH TIME
  MULTIPLIER; Quick Search (IA_QuickSearch = Left Shift, held with F; BP_LootContainerWidget) fills
  it 2.85 × ScavengingMultiplier as fast, and every 0.075 s (NoiseEvent) rolls the class's
  ChanceToProduceNoise (10 or 8 %, items.json containers `noise`) ÷ ThiefMultiplier for a 40 m noise,
  once per press. Weapons' SwingSpeed / ReloadBaseSpeed are items.json `swing_speed` /
  `reload_speed`; a magazine's ReloadSpeed stat changes the reload rate by -stat/100
  (GetNewMagInfo). `UpperAction::start_at` plays an action at such a rate.
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
  RPM / Semi-Auto on X / melee swing, RMB aim with Q/E shoulder, R reload from carried ammo,
  hitscan down the camera ray with terrain occlusion, BP_FirearmPickup's formulas: crits,
  `EffectiveDamage` fall-off, "No ammo"/"No mag in firearm", DurabilityCheck jams and per-shot
  ReduceDurability (both behind their difficulty toggles), noise out to the gun's DefaultSoundRadius;
  headshots × FirearmTypeMultiplier (one-shot only with HEADSHOT MODE, which also quarters body
  hits: DamageComponent/DamageMultiplier), melee ×2 into a zombie's back, zombie HP/death/XP (level × 5-10 × 1.05), weapons drawn in hand or holstered in the skeleton sockets, BP_Ingame AmmoBox),
  `damage_numbers.rs` (BFL_AI.SpawnFloatingDamageText + FloatingDamageNumbersUI: with DAMAGE
  NUMBERS ENABLED?, the rounded damage at screen centre easing (InterpTo 3) to a random 30-60 px
  side / 0-90 px up offset, Fade 0.75 -> 0 over 3 s, gone at 2.5 s; Montserrat-ExtraBold 14;
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
  (`gen_items.py` overlays the struct's defaults; null hides a part, e.g. a coat hides torso/arms)), `icons.rs` (inventory pictures baked at
  runtime from each item's inspect mesh/rotation/FOV via render-to-texture, like the game),
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
  bullet trails), the infestations' flies (their template is empty in the dump).
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
  are ours (tour step 08zzc_turret).
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
  Tour steps 08zze_horde / 08zzf_horde_end. Not ported: socket snapping, the buildables' own
  functions (storage, lights, benches, beds, doors), their health, horde music, the montages.
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
  (saved), XP level x 250/50 x 1.05 x XP MULTIPLIER, POIDiscoveredUI's banner (FadeStart keys);
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
  TrashObject_C actors -> `Level::harvest` by (cell, index). `combat::fire` writes a `Swing`
  (with its MeleeDamage) per melee swing; `hit_props` casts a 9-ray fan (0.4 / 0.8 / 1.2 m, ±28 cm, 2 m) with
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
  Harvest Master). Not ported: the power attack's 3-4 hits, the camera shake, the white
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
  tow, radio, repair, water, smoke/fire FX, RESET VEHICLE SPAWNS?, the vehicle trader, being in a
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
  slider × MASTER. Loops (`Emitter` with `follow`) track their owner; removing `Voiced` stops them.
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
  Not yet: weather, equip/lever/radio/siren sounds, reverb/indoor muffling.
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
  as `<Component>.RelativeLocation/Rotation`). **Axis mapping** (verified against
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
  (UE rotators converted to glb-space quats) → `assets/data/items.json`.
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
  hand fire montage for pistols (only Fire_Rifle_Ironsights exists; its user isn't found), projectile
  weapons (launchers, flare, crossbow), attachments' stat changes. Aim offsets (pitch) and crouch/sprint variants of the weapon sets aren't wired, nor
  pistol fire. Zombies ragdoll in the game; here they topple stiffly. No hit reactions, shot alerts,
  sounds, scopes, throwables' tumbling/DemolitionExpert,
  durability loss. Reload time, melee reach/cooldown, aim arm length and fall-off curve are guesses.
- Inventory: attachments other than magazines (sights, suppressors: `SC_*` special containers),
  the buildable benches (construction / weapons / disassembly workbenches, campfire, battery
  charger: placeable as plain actors only, `building.rs`), stack consume/drop amount selectors, inspect.
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
- Melee: durability 0 can't swing; per-hit ReduceDurability(1); MELEE DAMAGE DEGRADATION:
  damage × (dur/100)^0.225. The power attack (PowerMeleeAttackMontages, StaminaDrain × 6-8,
  rate ×0.75) isn't ported: IA_PrimaryAction's trigger events can't be told apart in the dump.
- Power: the stairwell light switches (trigger boxes), road-light/camp generators,
  generator hum/shake, the generator's grab montage, saving power
  state (not saved in the game either? unchecked). The lever's timeline curve eases; ours is linear.
- World: (door state isn't saved, as in the game: the door BPs lack EasyMultiSave's actor
  interface, so `XPGiven?`'s SaveGame flag does nothing.) Door sounds. Swimming, particles' lights/collision/flipbooks and the Niagara effects (`gameplay/particles.rs`), the settlements
  (`level/prefabs.rs`: their survivors, traders and quest givers aren't ported).
- Vehicles' push/tow, radio, repair and the reset-spawns button; the vehicle trader; quest givers and settlement NPCs; base building beyond placing (snapping, the buildables' functions) (animals: see `gameplay/animals.rs`; traders: `gameplay/traders.rs`; harvesting: `gameplay/harvest.rs`; fishing: `gameplay/fishing.rs`).
- The main menu backdrop cycles `UI/Textures/Backgrounds` screenshots; the original renders the
  `MainMenu` level + `MainMenuLevelSequence`.
- Mode levels (Shooting Range, Tutorial, Vehicle Playground) are disabled. Key rebinding, save
  search/rename and the save menu's "create new slot" box aren't implemented. Discord/Trello/Roadmap
  buttons are omitted. `BackgroundBlur` is approximated with a dark overlay.
- Audio: radio, equip and vehicle sounds; reverb and indoor muffling (SetInside); stereo panning; low-pass filters (the weather's occlusion and wind).
