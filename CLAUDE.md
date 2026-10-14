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
  and fired at a zombie, side-on with the rifle in hand, a roamer shoved (V) then stomped looking down, a frag grenade thrown (G) at a roamer (the survivor steps 8 m back while it's in the air: the blast is 85-115 within 10 m), a Molotov setting a roamer burning, a zombie hound, a creator face close-up, a corpse opened as loot, a thunderstorm and a dust storm, a door opened with F, the keypad door's note and its code typed in, a ladder climbed and stepped off at the top, a lab generator switched on, the police briefcase searched (the generator key) then the police generator fuelled and started and the lever pulled (the garage gate folds up), a crouch walk, an 8 m fall in heavy armor, the flashlight at midnight, sitting down, the journal's tech tree). In-game steps
  close any menu stray input opens (logged as "opened during"), writes PNGs, logs FPS/positions/shot
  hits and exits (`src/menus/autoshot.rs`). `SD_START=<n>` pins the PlayerStart; `SD_BURST=08g_armed,...` also saves a frame every 0.25 s of those steps. Use it to check
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
    `SpawnedActorClass` (Roamer 560, Crawler 120, Radiated 76, Infected 61, hounds; none = never
    spawns, bosses skipped), scaled by its `SpawnType`'s row. `optimize_subjects` is the AIOptimizer side: past
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
  - `collision.rs`: kinematic capsules (`KinematicBody`) vs nearby prop triangles + terrain;
    walls push out, floors/stairs within 0.45 m step carry you. `PropRays::cast` (a SystemParam)
    raycasts the streamed props: bullets stop at them and they block zombie sight. No physics
    engine exists for Bevy 0.20 yet (Avian is on 0.19).
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
- `src/models/anim.rs`: AnimSequences rebuilt as Bevy clips targeting bones by name; each
  modular body part has its own skeleton copy, so every part gets a player and the same state
  (`Locomotion`) — UE's LeaderPose done by hand. Player_AnimBP's weapon sets are a `Stance`
  (unarmed/rifle/pistol/melee, plus aiming states); one-shot `UpperAction`s (fire, reload, melee
  swings, zombie flinch) play on the upper body via mask groups: each locomotion clip is in the
  graph twice (lower/upper) and the driver crossfades weights itself. Graph node weights must stay
  1.0: Bevy multiplies them into the player weights, and overlapping clips at zero effective weight
  blend as 0/0 = NaN (characters vanished in every crossfade; the tour's `nan_bones` guards it). `wire_late` covers characters tagged `Animated`
  after their (cached) scene already spawned; without it they stand in the bind T-pose. Actions
  have a play rate (`ActionKind::rate`: shoves 1.15). Changing clothes or body parts is staged
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
  UI clicks (every `Activate`). Not yet: music, ambience, weather, reload/equip/door/lever sounds.
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
- Timeline lengths and curves *are* in the BP dump (`TimelineTemplate.TimelineLength`, `CurveFloat`).
- **Blueprint logic**: `SD_SCRIPT=1 bin/out/SdExport json <out> <pkg>` also deserializes Kismet
  bytecode, and `kismet.py <dump.json> [function...]` prints it as pseudo-code with jump offsets.
  Gameplay is Blueprint, so this (not Ghidra) is how formulas, timings and branches are read
  (the lockpicking was ported this way). Raw strings: `/home/lev/Qwen/extracted_pak` + `strings`.
- `gen_menu_data.py` also takes `Save_Difficulties`' class defaults as the difficulty rows'
  `default` (DayLength 30, NightLength 15, AiRespawnTimer 1500...); `SettingsStore::fill_defaults`
  applies row defaults at startup.
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
  texture, tint and (trees) trunk colour. `BloodColour` is not a tint, and the runtime ignores tint
  alpha (zero on zombies/animals/branches; with the alpha cut-off it made them invisible). Tint choices that contradict the game's own screenshots
  are overridden there with a comment saying so.
- Clothing layers as in the game: BP_PlayerCharacter attaches body armor as one more skinned
  mesh (MC_AttachClothing) and CheckClothing hides nothing under it, so a vest over a bulky coat
  shows the coat poking through, in the original too (same meshes).
- `SdExport level` (`Level.cs`) writes Bevy-space transforms; `SdExport levelactors <out> <umap>`
  rewrites only `actors.json` (spawners, doors, power actors (lab, generators, the lever) and item
  spawners carry their property overrides as `props`; ladders' and item spawners' moved components
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
- `gen_terrain.py` packs layer weights (3 RGBA tiles) and albedos (12 tiles) into vertically
  stacked PNGs loaded as texture arrays. The layer→texture pairing is by name (graph not readable).
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
  sounds, muzzle flash, scopes, throwables' tumbling/DemolitionExpert, burning particles,
  durability loss. Reload time, melee reach/cooldown, aim arm length and fall-off curve are guesses.
- Inventory: attachments other than magazines (sights, suppressors: `SC_*` special containers),
  workbench crafting UIs (recipes are exported), stack consume/drop amount selectors, inspect.
- HUD: map/journal/radial menu. (The action bar has no drawn-slot highlight in the game either:
  neither BP_Ingame nor JSIContainer recolours a slot.)
- Tech tree (`progression/tech.rs`, data `assets/data/techtree.json` from gen_items.py's
  `tech_tree()` over SkillTree + Skills/*): unlock rules and messages, `GameState::tech` (saved),
  points = `ProgressionSystem::unspent_points` (+1 per level, LevellingComponent: 500 XP ×1.325
  ceil per level, cap 50, XPDeath on Pause > Suicide). Wired: Health Regen, Lockpick+, Critical
  Hitter (crit chance ×2), Demolition Expert (frag/pipe bomb/Molotov ×1.5), Night Creature (melee
  ×1.35 before 06:00 / after 21:00), Deathbringer (health 25 once a life; the tooltip says 15),
  Distraction (`whistle.rs`, O). Not wired: Combat Roll (C, Roll_Montage; also ends burning),
  Parkour, Huntsman, Harvest Master. The journal (`gameplay/journal.rs`, J = IA_Journal) shows
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
  both give character XP (level × 80-100 × 1.05 × XP MULTIPLIER). Not ported: Fishing (no fishing),
  the skills UI (Journal_SkillsUI, Tooltip_PassiveStats).
- Zombie variants: bosses, hordes, archetypes (Armored Feral Hound...); the Radiated/Infected
  master materials' look (glow/tint: graph unreadable) and glowing eye meshes; crawlers use the
  standing attack montage like the BP but our attack is a looping locomotion state. The
  `_ZombieBosses`, `_Animals` and `_SettlementSurvivors` spawners are exported with props but ignored.
  The old `survival::MedicalSystem` / `conditions` and SurvivalStats' infection are the earlier
  simulation's inventions (no longer ticked in the world). Event_HungerDebuff / ThirstDebuff only
  arm the starvation-damage timers (ported). Every hit the survivor takes (zombies, falls,
  starvation, bleed ticks) goes through `survival::take_hit` = CalculatePlayerDamage: worn body
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
  the regen timer, which ours never stops, so it adds nothing here. No swimming, rolling or
  vaulting (the BP's "Wall Thick" climb). A falling body over props still streaming in hangs up to 5 s
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
  interface, so `XPGiven?`'s SaveGame flag does nothing.) Door sounds. Swimming, fire/smoke particles (158 ParticleSystemComponents), prefab-spawned random events
  (camps, abandoned trucks).
- Traders, vehicles, animals, airdrops.
- The main menu backdrop cycles `UI/Textures/Backgrounds` screenshots; the original renders the
  `MainMenu` level + `MainMenuLevelSequence`.
- Mode levels (Shooting Range, Tutorial, Vehicle Playground) are disabled. Key rebinding, save
  search/rename and the save menu's "create new slot" box aren't implemented. Discord/Trello/Roadmap
  buttons are omitted. `BackgroundBlur` is approximated with a dark overlay.
- Audio: music, ambience and most interaction sounds aren't wired yet (see `src/audio`).
