//! In-world gameplay on top of the level: searching containers, inventory, survival vitals,
//! combat.

pub mod ambience;
pub mod burning;
pub mod combat;
pub mod damage_numbers;
pub mod daynight;
pub mod equipment;
pub mod flashlight;
pub mod footsteps;
pub mod hud;
pub mod icons;
pub mod interact;
pub mod inventory_ui;
pub mod items;
pub mod journal;
pub mod keypad;
pub mod lockpick;
pub mod medical;
pub mod weather;
pub mod whistle;
pub mod notify;
pub mod recoil;
pub mod shove;
pub mod survival;
pub mod throwables;
pub mod zombies;

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
        survival::plugin(app);
        hud::plugin(app);
        icons::plugin(app);
        combat::plugin(app);
        daynight::plugin(app);
        zombies::plugin(app);
        notify::plugin(app);
        lockpick::plugin(app);
        keypad::plugin(app);
        weather::plugin(app);
        footsteps::plugin(app);
        flashlight::plugin(app);
        shove::plugin(app);
        throwables::plugin(app);
        burning::plugin(app);
        damage_numbers::plugin(app);
        recoil::plugin(app);
        whistle::plugin(app);
        journal::plugin(app);
        ambience::plugin(app);
    }
}

/// Run condition: gameplay input applies (no inventory/loot screen, not dead).
pub fn player_in_control(overlay: Res<GameOverlay>, vitals: Res<PlayerVitals>) -> bool {
    !overlay.is_open() && !vitals.is_dead()
}
