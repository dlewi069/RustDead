# SurrounDead — Rust Survival Engine

**Estimated Port Completion: 40%**
*The core engine data layer, physics, survival simulation, AI state machines, economy, and inventory systems are fully mapped. We are currently preparing for 3D mesh imports (converting proprietary UE5 `.uasset` to `.obj`/`.gltf`) and level design construction to bring the game visually to parity with the original.*

A comprehensive, pure Rust reimplementation of **SurrounDead** (open-world post-apocalyptic zombie survival).

Built with a high-performance Entity Component System (ECS) architecture, hardware-accelerated 3D isometric graphics window (OpenGL 4.6), precision raycast ballistics, Tarkov/SurrounDead grid inventory with nested containers, modular base building, vehicle physics with kinetic roadkills, day/night cycles, dynamic airdrops, tactical human bandit AI, special infected, safezone trader economy, lockpicking mini-games, and a full 12-perk character skill progression tree.

---

## Replicated Game Systems

### 1. Character Progression & 12-Skill Tree (`src/progression/`)
- **Leveling & XP Curve**: Earn XP by killing infected (Walker +20 XP, Runner +35 XP, Hound +30 XP, Screamer +45 XP, Spitter +55 XP, Bloater +75 XP, Juggernaut +200 XP, Bandit +85 XP), lockpicking safes (+25 to +250 XP), crafting, discovering POIs, and claiming trader bounties.
- **Skill Disciplines** (5 Levels each):
  - **Combat**:
    - `Marksmanship`: -6% weapon recoil and -5% bullet spread per level.
    - `Melee Mastery`: +10% melee strike damage per level.
    - `Fast Hands`: +8% reload speed per level.
  - **Survival**:
    - `Iron Stomach`: -7% hunger drain rate and food poisoning immunity.
    - `Metabolism`: -7% thirst drain rate per level.
    - `Immunity`: -10% infection buildup and +8% radiation resistance per level.
  - **Athletics**:
    - `Cardio`: +15 max stamina and +10% stamina regeneration speed per level.
    - `Sprinter`: +5% sprint movement speed per level.
    - `Pack Mule`: +8.0 kg inventory carry weight limit per level.
  - **Scavenging & Engineering**:
    - `Lucky Looter`: +10% chance to roll rare/military tier loot from containers.
    - `Lockpicking Mastery`: +15% sweet-spot angle tolerance and slower pick breaking.
    - `Scrapper`: +25% bonus materials yield when disassembling items.
- **Skill Menu (`K`)**: Interactive window displaying level, XP progress bar, unspent skill points, and clickable allocation slots.

---

### 2. Lockpicking & Secure Safes (`src/looting/lockpick.rs`)
- **Containers**: Police cruiser trunks, gun safes, bank vaults, FOB Bravo munitions crates.
- **Lock Difficulty Tiers**: Novice, Adept, Expert, Master.
- **Mechanics**:
  - Rotate pick angle ($0^\circ$ to $180^\circ$) to locate the hidden sweet spot.
  - Apply turning tension with `Space`.
  - Inside sweet spot: Cylinder turns smoothly towards $90^\circ$ to unlock.
  - Off sweet spot: Tension bends and stresses the bobby pin until it breaks.
  - Integrated with `Lockpicking Mastery` skill perk.

---

### 3. Weapon Attachments & Customization (`src/combat/attachments.rs`)
- **Modular Slots**:
  - **Optics**: Iron Sights, Kobra Red Dot (zoom 1.2x, -15% spread), EOTech Holo (zoom 1.3x, -20% spread), ACOG 4x Combat Scope (zoom 4x, -35% spread), Leupold 8x Sniper Scope (zoom 8x, -60% spread).
  - **Muzzle**: Osprey Tactical Suppressor (reduces gunshot noise radius by 80%), Competition Muzzle Compensator (-35% vertical recoil, -30% horizontal recoil).
  - **Magazines**: Standard Mag, PMAG 40-Round Extended (+10 rounds), D-60 60-Round Drum Mag (+30 rounds).
  - **Tactical**: SureFire Weapon Flashlight (illuminates night terrain), PEQ-15 Green Laser Module (-25% hipfire spread).
- Dynamic stats update in real time when attachments are installed.

---

### 4. Special Infected & Boss AI (`src/ai/`)
- **Variants**:
  - `Walker`: Shambling infected, high numbers.
  - `Runner`: High sprint velocity, aggressive chase.
  - `Hound`: Zombie dog, ultra-fast leap attacks targeting player legs.
  - `Screamer`: Emits an ear-splitting alarm shriek upon spotting the player, calling all zombies within a 120-meter radius!
  - `Spitter`: Ranged acid attack dealing corrosive and infectious damage.
  - `Bloater`: Tanky infected that detonates in a toxic gas explosion upon death.
  - `Juggernaut` (Demolisher Boss): 450 HP armored titan with ground-slam shockwaves.
- **Sensory Perception**: Directional sight cones ($120^\circ$, affected by daylight and flashlight), noise hearing radius, investigation behavior.

---

### 5. Hostile Human Bandit AI (`src/ai/bandit.rs`)
- **Rogue Raiders**: Armed human NPCs stationed at FOB camps and crossroads.
- **Weapons**: AK-47, Glock 17, Remington 870 Shotguns.
- **Combat Behavior**:
  - Patrols perimeter until player enters detection zone ($45\text{m}$).
  - Tactical engagement: fires firearms with realistic ballistics, takes cover, strafes, and reloads when empty.
  - Drops weapon, matching ammo, and Survivor Credits upon elimination.

---

### 6. Safezone Trader & Barter Economy (`src/economy/`)
- **Quartermaster Hank**: Neutral merchant stationed at Camp Timber safezone.
- **Barter Trade**: Buy weapons, ammo boxes, medical gear, backpacks, and food using Survivor Credits (`💰`).
- **Dynamic Valuation**: Items evaluated based on rarity (Common to Legendary), durability percentage, and category multipliers.
- **Bounty Board (`T`)**:
  - *Cull the Herd*: Eliminate 8 roaming infected (+200 Credits, +80 XP).
  - *Rogue Bounty*: Eliminate 2 hostile bandits (+450 Credits, +150 XP).
  - *Safecracker Operation*: Pick 1 secure safe (+300 Credits, +100 XP).
  - Interactive claim button in trader window.

---

### 7. Advanced Trauma & Medical Treatments (`src/survival/medical.rs`)
- **Trauma Types**:
  - **Fracture**: Cripples movement into a limp ($55\%$ speed). Treated with `Splint`.
  - **Heavy Bleed**: Rapid arterial blood loss (3.5 HP/sec). Treated with `Tourniquet`.
  - **Infection / Sepsis**: Progressive blood toxicity. Treated with `Antibiotics` or `Antiviral Injector`.
  - **Radiation Sickness**: Cellular poisoning. Treated with `RadAway` or `Iodine Tablets`.
  - **Food Poisoning**: Stomach nausea from dirty water or raw meat. Treated with `Activated Charcoal`.
  - **Pain Shock**: Aim instability and stamina drain. Treated with `Painkillers` or `Morphine`.

---

### 8. Campfire Cooking & Water Purification (`src/crafting/cooking.rs`)
- **Campfire Station**: Physical stone fire ring with fuel decay (add wood planks to refuel).
- **Recipes**:
  - Charred Venison Steak (Raw Venison $\to$ +45 Food).
  - Boiled Clean Water (Dirty Water $\to$ +40 Thirst, zero infection risk).
  - Hearty Hunter Stew (Canned food + meat $\to$ +60 Food, +30 Thirst).
- **Resting / Sleeping**: Skip night hours in beds/sleeping bags to advance daytime and restore full stamina and vitality.

---

### 9. Dynamic World Events & Airdrops (`src/world/events.rs`)
- **C-130 Hercules Airdrop**:
  - Cargo plane audio flyby.
  - Drops a parachute military supply crate falling from 75m altitude.
  - Lands on terrain with a bright red smoke flare visible across the valley.
  - High-tier military loot inside (M4A1, ammo crates, military rucksacks, medkits).
- **Horde Surge**: Periodic swarm alarms triggering aggressive runner packs converging on the player.

---

### 10. Vehicles & Roadkill Physics (`src/vehicles/`)
- **Vehicles**: Sedan, D-150 Pickup Truck, Cherokee 4x4 SUV, Bearcat Armored Truck.
- **Simulation**: Fuel tanks, fuel consumption per km, car battery, engine durability.
- **Trunk Storage**: Secondary grid container for massive loot storage.
- **Kinetic Impact**: High-speed roadkills crush zombies into paste while absorbing a fraction of impact into chassis/engine health.

---

---

### 11. Tactical Night Vision System (`src/survival/night_vision.rs`)
- Reverse engineered from `DT_NightVisionSettings`, `Struct_NightVision`, and `DA_*NVG` assets.
- **Optic Generations**:
  - `Civilian Gen-1`: Green phosphor weak tint, higher sensor noise grain, moderate amplification.
  - `Military Gen-2`: Tactical green phosphor, high gain, reduced grain, 3.5x night amplification.
  - `SpecOps Gen-3`: White phosphor / icy cyan tint, zero grain, wide contrast, power efficient.
- **Battery Simulation**: Discharges over time while engaged; rechargeable with `9V Alkaline Battery` (`battery_9v`).
- **Tactical Utility**: Provides stealth overland navigation and subterranean bunker vision without drawing infected aggro with a flashlight beam.

---

### 12. Authentic Emergency Radio Transceiver (`src/world/radio.rs`)
- Reverse engineered from `DT_Radio` and `Struct_Radio` in SurrounDead's content pack.
- **Frequency Spectrum (80.0 – 180.0 MHz)**:
  - `91.5 MHz` — **CoyoteCall**: Safezone trade route telemetry and merchant outpost advisories.
  - `104.2 MHz` — **FireInTheCity**: Urban quarantine alarm, firestorm alerts, and runner swarm reports.
  - `128.7 MHz` — **MilitaryReinforcements**: FOB Bravo command channel, emergency C-130 Hercules airdrop clearance.
  - `142.0 MHz` — **TheSubway**: Underground research bunker radiation alerts and armory keycard clearance intel.
  - `160.0 MHz` — **ForceFromAfar**: Long-range carrier distress SOS beacon.
- Real-time signal strength computation, static hiss emulation, and dynamic transmission decryption.

---

### 13. Authentic 5-Tier Difficulty System (`src/engine/difficulty.rs`)
- Reverse engineered from `DT_Difficulty` and `Struct_Difficulty`.
- **Difficulty Modes**:
  - `Peaceful`: 0x zombie damage, no bandit aggression, 1.5x loot yield, reduced vitals drain.
  - `Easy`: 0.7x player damage taken, 0.75x zombie health, 1.25x loot yield.
  - `Standard`: 1.0x baseline simulation.
  - `Hardened`: 1.4x zombie damage & health, 1.3x hunger/thirst drain, 0.8x loot yield.
  - `Nightmare`: 2.0x zombie damage & health, 75% runner sprint chance, lethal infection, 0.5x loot yield.

---

### 14. Multi-Slot Save & World Persistence (`src/persistence/save_slots.rs`)
- Reverse engineered from `/Script/EasyMultiSave` architectural structures.
- Multi-slot save management (`Slot 1`, `Slot 2`, `Slot 3`, `AutoSave`, `QuickSave`).
- Enriched metadata headers (playtime, sector, level, difficulty, health, timestamps).
- Instant Quick-Save (`F5`) and Quick-Load (`F9`).

---

### 15. Hardware-Accelerated 3D GUI Window (`src/ui/gui.rs`)
- **3D Isometric Camera**: Tracks player with smooth mouse-wheel zooming (12m to 40m).
- **3D Entities**: Pine trees, asphalt road, military crates, vehicles, campfires with flickering flames, airdrop parachute & smoke columns, animated infected, and bandits.
- **Night Vision Phosphor Shader**: Dynamic phosphor color grading, scanline rasterization, and peripheral vignetting.
- **HUD Overlays**:
  - Health, Stamina, Hunger, Thirst, Infection bars.
  - Status indicators: Fracture limp, bleeding, difficulty mode, and active radio frequency.
  - Weapon gauge with ammo count, durability, and active attachments.
  - Top compass bar with cardinal headings (`N`, `NE`, `E`, `SE`, `S`, `SW`, `W`, `NW`).
  - Crosshair with weapon spread expansion, hitmarkers, and floating damage numbers.
  - Real-time combat radio log.
- **Modals**: Full-screen Grid Inventory (`Tab`), Crafting Bench (`C`), Skill Tree (`K`), Trader & Bounties (`T`), Lockpicking Interface (`L`), Emergency Radio (`U`), World Map (`M`).

---

## Controls

| Key | Action |
| :--- | :--- |
| `W / A / S / D` | Move character / Drive vehicle |
| `Left Shift` (hold) | Sprint (drains stamina, increased speed via Sprinter skill) |
| `Left Click` | Fire firearm / Melee attack / Place building hologram |
| `Right Click` (hold) | Aim down sights (ADS scope zoom) |
| `R` | Reload weapon from pockets ammo |
| `F` | Toggle Tactical Flashlight |
| `N` | Toggle Tactical Night Vision Goggles (NVG) |
| `U` | Toggle Emergency Radio Transceiver |
| `F5` / `F9` | Quick-Save / Quick-Load game state |
| `E` | Interact (Enter/Exit vehicle, open loot containers, doors) |
| `Tab` or `I` | Toggle Grid Inventory & Equipment Paperdoll |
| `C` | Toggle Crafting Bench |
| `K` | Toggle Character Skill Tree & Progression |
| `T` | Toggle Safezone Trader & Bounty Board |
| `L` | Toggle Lockpicking Interface |
| `M` | Toggle Valley Sector Topographical Map |
| `B` | Toggle Holographic Base Building Mode |
| `1 - 4` | Quick-use Medical / Consumable items |
| `Mouse Wheel` | Zoom Camera in / out |

---

## Build & Run

### First-Time Asset Extraction & Setup:
If launching for the first time or setting up authentic models:
```bash
cargo run --bin first_time_setup
```
Alternatively, run the automated setup CLI tool:
```bash
python scripts/run_setup_extractor.py
```
This utility:
1. Detects your legal Steam installation of SurrounDead (`SurrounDead-Windows.pak`).
2. Populates `assets/models/` with placeholder / extracted `.obj` meshes.
3. Verifies audio (`assets/audio/*.wav`) and textures (`assets/textures/*.png`).
4. Ensures clean, drop-in asset loading when running `cargo run --release`.

### Graphical GUI Mode (Default):
```bash
cargo run --release
```

### Run Full Test Suite (37 Integration & Unit Tests):
```bash
cargo test
```


