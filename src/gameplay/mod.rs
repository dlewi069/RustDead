//! In-world gameplay on top of the level: searching containers, inventory, survival vitals,
//! combat.

pub mod combat;
pub mod daynight;
pub mod equipment;
pub mod hud;
pub mod icons;
pub mod interact;
pub mod inventory_ui;
pub mod items;
pub mod lockpick;
pub mod medical;
pub mod weather;
pub mod notify;
pub mod survival;
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
        weather::plugin(app);
    }
}

/// Run condition: gameplay input applies (no inventory/loot screen, not dead).
pub fn player_in_control(overlay: Res<GameOverlay>, vitals: Res<PlayerVitals>) -> bool {
    !overlay.is_open() && !vitals.is_dead()
}
