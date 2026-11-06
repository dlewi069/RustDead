//! In-world gameplay on top of the level: searching containers, inventory, survival vitals,
//! combat.

pub mod ai_health_bar;
pub mod airdrops;
pub mod ambience;
pub mod animals;
pub mod bosses;
pub mod barrels;
pub mod binoculars;
pub mod infestations;
pub mod post_volumes;
pub mod inspect;
pub mod build_doors;
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
pub mod generators;
pub mod harvest;
pub mod head_gear;
pub mod humans;
pub mod hud;
pub mod icons;
pub mod interact;
pub mod search_widget;
pub mod loot_refill;
pub mod outline;
pub mod barber;
pub mod inventory_ui;
pub mod items;
pub mod journal;
pub mod keypad;
pub mod level_traps;
pub mod lockpick;
pub mod map;
pub mod markers;
pub mod melee;
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
pub mod sleep;
pub mod respawn;
pub mod death_loot;
pub mod projectiles;
pub mod radiation;
pub mod radio;
pub mod repair;
pub mod attachments;
pub mod scopes;
pub mod security_cameras;
pub mod tactical;
pub mod stats;
pub mod storage;
pub mod survival;
pub mod swim;
pub mod throwables;
pub mod traps;
pub mod traders;
pub mod turrets;
pub mod utility_benches;
pub mod vehicles;
pub mod vault;
pub mod zombie_eyes;
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
        search_widget::plugin(app);
        loot_refill::plugin(app);
        outline::plugin(app);
        zombie_eyes::plugin(app);
        barber::plugin(app);
        ai_health_bar::plugin(app);
        binoculars::plugin(app);
        head_gear::plugin(app);
        inventory_ui::plugin(app);
        traders::plugin(app);
        zones::plugin(app);
        civilians::plugin(app);
        turrets::plugin(app);
        security_cameras::plugin(app);
        building::plugin(app);
        storage::plugin(app);
        sleep::plugin(app);
        generators::plugin(app);
        barrels::plugin(app);
        infestations::plugin(app);
        post_volumes::plugin(app);
        traps::plugin(app);
        level_traps::plugin(app);
        build_doors::plugin(app);
        respawn::plugin(app);
        death_loot::plugin(app);
        radiation::plugin(app);
        attachments::plugin(app);
        projectiles::plugin(app);
        radio::plugin(app);
        melee::plugin(app);
        inspect::plugin(app);
        utility_benches::plugin(app);
        scopes::plugin(app);
        tactical::plugin(app);
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
