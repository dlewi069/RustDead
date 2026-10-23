//! In-world gameplay on top of the level: searching containers, inventory, survival vitals,
//! combat.

pub mod airdrops;
pub mod ambience;
pub mod animals;
pub mod bosses;
pub mod building;
pub mod civilians;
pub mod burning;
pub mod combat;
pub mod damage_numbers;
pub mod daynight;
pub mod equipment;
pub mod fishing;
pub mod flashlight;
pub mod footsteps;
pub mod harvest;
pub mod humans;
pub mod hud;
pub mod icons;
pub mod interact;
pub mod inventory_ui;
pub mod items;
pub mod journal;
pub mod keypad;
pub mod lockpick;
pub mod map;
pub mod markers;
pub mod medical;
pub mod weather;
pub mod weather_sounds;
pub mod world_events;
pub mod whistle;
pub mod notify;
pub mod particles;
pub mod quests;
pub mod recoil;
pub mod roll;
pub mod shove;
pub mod stats;
pub mod survival;
pub mod swim;
pub mod throwables;
pub mod traders;
pub mod turrets;
pub mod vehicles;
pub mod vault;
pub mod zombies;
pub mod zones;

use bevy::prelude::*;

pub use combat::{Armed, Dead, PlayerCamera};
pub use interact::{GameOverlay, player_inv};
pub use items::ItemDb;
pub use survival::PlayerVitals;

pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ItemDb::load());
        interact::plugin(app);
        inventory_ui::plugin(app);
        traders::plugin(app);
        zones::plugin(app);
        civilians::plugin(app);
        turrets::plugin(app);
        building::plugin(app);
        world_events::plugin(app);
        airdrops::plugin(app);
        survival::plugin(app);
        bosses::plugin(app);
        humans::plugin(app);
        hud::plugin(app);
        icons::plugin(app);
        combat::plugin(app);
        daynight::plugin(app);
        zombies::plugin(app);
        animals::plugin(app);
        notify::plugin(app);
        lockpick::plugin(app);
        keypad::plugin(app);
        weather::plugin(app);
        weather_sounds::plugin(app);
        footsteps::plugin(app);
        flashlight::plugin(app);
        shove::plugin(app);
        throwables::plugin(app);
        burning::plugin(app);
        particles::plugin(app);
        damage_numbers::plugin(app);
        recoil::plugin(app);
        roll::plugin(app);
        swim::plugin(app);
        vault::plugin(app);
        whistle::plugin(app);
        journal::plugin(app);
        map::plugin(app);
        markers::plugin(app);
        stats::plugin(app);
        ambience::plugin(app);
        harvest::plugin(app);
        fishing::plugin(app);
        vehicles::plugin(app);
        quests::plugin(app);
    }
}

/// Run condition: gameplay input applies (no inventory/loot screen, not dead).
pub fn player_in_control(overlay: Res<GameOverlay>, vitals: Res<PlayerVitals>) -> bool {
    !overlay.is_open() && !vitals.is_dead()
}

/// Run condition: the survivor's own actions (moving, weapons, throws...): in control and not in
/// a vehicle (its seat takes the keys; `vehicles::drive`).
pub fn on_foot(overlay: Res<GameOverlay>, vitals: Res<PlayerVitals>, driving: Res<vehicles::Driving>) -> bool {
    !overlay.is_open() && !vitals.is_dead() && !driving.active()
}
