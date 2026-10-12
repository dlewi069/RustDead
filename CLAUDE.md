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
  and fired at a zombie, side-on with the rifle in hand, a zombie hound, a creator face close-up, a corpse opened as loot, a thunderstorm and a dust storm), writes PNGs, logs FPS/positions/shot
  hits and exits (`src/menus/autoshot.rs`). `SD_START=<n>` pins the PlayerStart. Use it to check
  changes visually.
- Dev builds use `opt-level = 1` (deps at 3); the map is too heavy for unoptimised Bevy.

## Testing workflow (required)

- **While working**: after each change run `scripts/test_related.sh` (tests for the files changed
  vs git HEAD: `src/a/b.rs` -> `cargo test a::b::`, data JSON -> the modules that read it, python
  -> compile + data checks). It lists changed Rust files that have no `#[test]`: add tests.
- **New tests go in the full suite** automatically when they are `#[test]`s in `src/`, functions
  in `CHECKS` of `scripts/check_data.py` (data consistency) or `scripts/check_tour.py` (tour
  assertions), or new steps in `src/menus/autoshot.rs`'s TOUR. Add one of those for every feature.
- **At the end of every prompt**: `scripts/check.sh` (build without warnings, clippy no worse than
  `scripts/clippy_baseline.txt`, all unit tests, data checks, the screenshot tour + assertions;
  ~1.5 min). `--quick` skips the tour. It runs automatically as a Claude Code Stop hook
  (`/home/lev/Qwen/.claude/settings.json`, `--hook`: skipped when nothing changed since the last
  pass; a failure blocks finishing with the report, up to 3 times). Output: `target/check/`.

## Layout

- `src/main.rs`: Bevy app, world setup (`setup_world` on `OnEnter(AppState::InGame)`: player at a
  PlayerStart or the saved position, sun, fog, camera, HUD), player input, zombie chase, and the
  BP_PlayerCharacter spring-arm camera (3 m `ThirdPersonTargetArmLength`, mouse look with the cursor
  captured while playing, shorter and over a shoulder while aiming; aim arm/offset are guesses). Gameplay systems run in `AppState::InGame` + `MenuScreen::None` (+ `player_in_control`).
- `src/level/`: LongdownValley imported from `LongdownValley.umap`.
  - `terrain.rs`: 4591² heightmap → 128 m chunks streamed with distance LOD + skirts; splat
    material (`assets/shaders/terrain.wesl`, an `ExtendedMaterial` over StandardMaterial) blends 11
    paint layers. `Terrain::height(x, z)` is the ground query.
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
  - `collision.rs`: kinematic capsules (`KinematicBody`) vs nearby prop triangles + terrain;
    walls push out, floors/stairs within 0.45 m step carry you. `PropRays::cast` (a SystemParam)
    raycasts the streamed props: bullets stop at them and they block zombie sight. No physics
    engine exists for Bevy 0.20 yet (Avian is on 0.19).
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
  head hits kill, zombie HP/death/XP (level × 5-10 × 1.05), weapons drawn in hand or holstered in the skeleton sockets, BP_Ingame AmmoBox),
  `zombies.rs` (BT_Zombie: roam 10 m / investigate noises / chase; sight 25-40 m and 70°, hearing
  150 m via the `Noise` message gunshots send, hurt -> chase, 275°/s turning; `ZombieKind` indexes
  `assets/data/zombies.json` (`extractor/gen_ai.py`, `export_assets.sh --ai`): every
  BP_MasterZombie child's inherited speeds, HP, damage, attack timing, medical/radiation chances,
  meshes + material skins (random each, like `SetMeshAndMaterial`), loot table, capsule and
  blendspace clips (nearest sample per speed). Shapes: upright, prone (crawler), hound), `daynight.rs`
  (the level's Ultra_Dynamic_Sky clock: 09:00 start, dawn 6 / dusk 19, DAY/NIGHT LENGTH minutes,
  sun arc matching the cached sun, moonlight, eye adaptation via the camera `Exposure`, TimeUI
  clock), `lockpick.rs` (LockpickUI minigame decompiled from the BPs; turning the lock rotates the
  whole LockWidget, pick and screwdriver included, and with no lockpicks left the pick is hidden), `notify.rs` (toasts), `interact.rs` (hold F
  to search containers for the class's `DurationToHold`, overlay state, `dress_player` spawning
  worn clothing glbs onto the survivor via `models::dress`), `icons.rs` (inventory pictures baked at
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
  multipliers, zombie melee, death/respawn, HUD vitals).
- `src/models/anim.rs`: AnimSequences rebuilt as Bevy clips targeting bones by name; each
  modular body part has its own skeleton copy, so every part gets a player and the same state
  (`Locomotion`) — UE's LeaderPose done by hand. Player_AnimBP's weapon sets are a `Stance`
  (unarmed/rifle/pistol/melee, plus aiming states); one-shot `UpperAction`s (fire, reload, melee
  swings, zombie flinch) play on the upper body via mask groups: each locomotion clip is in the
  graph twice (lower/upper) and the driver crossfades weights itself. Graph node weights must stay
  1.0: Bevy multiplies them into the player weights, and overlapping clips at zero effective weight
  blend as 0/0 = NaN (characters vanished in every crossfade; the tour's `nan_bones` guards it). `wire_late` covers characters tagged `Animated`
  after their (cached) scene already spawned; without it they stand in the bind T-pose.
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
- `SdExport list|tex|json|mesh` (`extractor/Program.cs`). A trailing `/` on a package path means
  the whole folder. `json` dumps exports (widget trees, data tables, BP components). This is the
  main tool for reading game data. CUE4Parse writes `\`-joined paths on Linux; `FixPath` moves them.
- `widget_tree.py <Widget.json>` prints a UMG hierarchy with slot layouts. Start here when porting a menu.
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
  Enum_Firearms type by value, automatic, malfunction chance, sound radius, recoil) and
  `reduce_durability`, plus the AI loot tables (`AI/AILootTables`).
- `gen_menu_data.py` turns settings/difficulty pages, `IMC_General` key bindings, creator options and
  `Enum_Occupation` into `assets/data/menus.json`. Radio-select options aren't serialized and are
  inferred from the page string table (`"inferred": true`).
- `gen_materials.py` resolves each glTF material name through the MI→master chain to its albedo
  texture, tint and (trees) trunk colour. `BloodColour` is not a tint, and the runtime ignores tint
  alpha (zero on zombies/animals/branches; with the alpha cut-off it made them invisible). Tint choices that contradict the game's own screenshots
  are overridden there with a comment saying so.
- `SdExport level` (`Level.cs`) writes Bevy-space transforms; `SdExport levelactors <out> <umap>`
  rewrites only `actors.json` (spawners carry their property overrides as `props`). **Axis mapping** (verified against
  mesh bounds, PlayerStarts and skeletons): position (X, Z, Y)/100, rotation quat (x, z, y, -w),
  scale (X, Z, Y). Landscape heights: `(loc.z + (h - 32768)/128 * scale.z) / 100`.
- `SdExport animjson` (`Anim.cs`): CUE4Parse can't write glTF animations, so keys are sampled per
  bone into JSON (same axis mapping). Bones whose skeleton `TranslationRetargetingMode` is
  `Skeleton` (all but Root/Pelvis/IK here) take the reference-pose translation, as UE does;
  retargeted clips (ThirdPersonIdle/Walk) otherwise distort the limbs. ACL-compressed clips need `CUE4Parse-Natives.so`, which NuGet
  doesn't ship for Linux; the script builds it from the CUE4Parse repo into `extractor/natives/`.
- `gen_terrain.py` packs layer weights (3 RGBA tiles) and albedos (12 tiles) into vertically
  stacked PNGs loaded as texture arrays. The layer→texture pairing is by name (graph not readable).
- `gen_items.py`: 589 items, 28 loot tables, 109 container classes, `DT_CraftingTableTypes`
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

- Combat: recoil (`StartRecoil` timelines + RecoilData; the struct fields decompile as `?`), the
  hand fire montage for pistols (only Fire_Rifle_Ironsights exists; its user isn't found), projectile
  weapons (launchers, flare, crossbow), attachments' stat changes. Aim offsets (pitch) and crouch/sprint variants of the weapon sets aren't wired, nor
  pistol fire. Zombies ragdoll in the game; here they topple stiffly. No hit reactions, shot alerts,
  sounds, muzzle flash, throwables, shove (V), scopes,
  durability loss. Reload time, melee reach/cooldown, aim arm length and fall-off curve are guesses.
- Inventory: attachments other than magazines (sights, suppressors: `SC_*` special containers),
  workbench crafting UIs (recipes are exported), stack consume/drop amount selectors, inspect.
- HUD: map/journal/radial menu. (The action bar has no drawn-slot highlight in the game either:
  neither BP_Ingame nor JSIContainer recolours a slot.)
- Skill trees: the game's tech tree (Lockpick+ gates hardened/master locks; the old progression
  `Lockpicking` skill stands in) and passive skills (Thief/Scavenging XP from picking locks).
- Zombie variants: bosses, hordes, archetypes (Armored Feral Hound...); the Radiated/Infected
  master materials' look (glow/tint: graph unreadable) and glowing eye meshes; crawlers use the
  standing attack montage like the BP but our attack is a looping locomotion state. The
  `_ZombieBosses`, `_Animals` and `_SettlementSurvivors` spawners are exported with props but ignored.
  The old `survival::MedicalSystem` / `conditions` and SurvivalStats' hunger/thirst rates and
  infection are the earlier simulation's inventions, not ported from HungerThirstComponent yet.
  The optimiser's nearer layers (tick throttling, frozen unseen poses) aren't ported, and the respawn
  queue and culled zombies aren't saved. The hound always uses the German Shepherd skin
  (`Materials` also lists a Doberman one).
- Day/night: UDS clouds/atmosphere aren't simulated; sky/light colours are hand-picked blends.
  Weather lacks wet/snowy/dusty materials, wind on foliage, sounds, and Radiation weather's effect.
  Grass is the data's pale olive (UE's grass shader darkens it; graph unreadable).
- World: interactive doors (~100 door BPs: locked, keycard, zombie, lab sliding), ladders,
  swimming, fire/smoke particles (158 ParticleSystemComponents), prefab-spawned random events
  (camps, abandoned trucks), lab power and light switches.
- Traders, vehicles, animals, airdrops.
- The main menu backdrop cycles `UI/Textures/Backgrounds` screenshots; the original renders the
  `MainMenu` level + `MainMenuLevelSequence`.
- Mode levels (Shooting Range, Tutorial, Vehicle Playground) are disabled. Key rebinding, save
  search/rename and the save menu's "create new slot" box aren't implemented. Discord/Trello/Roadmap
  buttons are omitted. `BackgroundBlur` is approximated with a dark overlay.
- UI click sound (`UI_click_sound_Cue`) and other audio aren't exported yet.
