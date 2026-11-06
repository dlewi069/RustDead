//! `SD_VIEW=x,y,z,yaw[,pitch[,hour]]` (Bevy-space metres, degrees; y below the ground is
//! lifted onto it) starts a new game with the survivor held at that spot, the camera turned
//! that way, and after `SD_VIEW_WAIT` seconds (10) saves `SD_VIEW_SHOT` (view.png) and exits.
//! `SD_VIEW_ITEMS=DA_a,DA_b,...` also drops those items at the survivor's feet and opens the
//! inventory 2 s before the shot (VICINITY shows them; checking their pictures); `+DA_a` is
//! worn instead (in its equipment slot).
//! `SD_VIEW_PAGE=<page>` opens it on one of the tour's inventory pages then
//! (`inventory_ui::TourPage::named`: crafting, split, drop, inspect, bag_window...).
//! A dev tool for looking at one place (the swamp's water, a dead deer...), not part of the game.

use bevy::prelude::*;
use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};

use super::{AppState, CharacterProfile, CurrentGame, MenuData, MenuScreen};

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct View {
    pub at: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub hour: Option<f32>,
    pub wait: f32,
    pub shot: String,
    pub items: Vec<String>,
    pub page: Option<String>,
}

impl View {
    /// `x,y,z,yaw[,pitch[,hour]]`.
    pub fn parse(spec: &str) -> Option<Self> {
        let v: Vec<f32> = spec.split(',').map(|s| s.trim().parse().ok()).collect::<Option<_>>()?;
        if v.len() < 4 {
            return None;
        }
        Some(Self { at: Vec3::new(v[0], v[1], v[2]), yaw: v[3], pitch: v.get(4).copied().unwrap_or(15.0), hour: v.get(5).copied(), wait: 10.0, shot: "view.png".into(), items: Vec::new(), page: None })
    }
}

pub struct ViewShotPlugin(pub View);

impl Plugin for ViewShotPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.0.clone()).add_systems(Update, run);
    }
}

#[allow(clippy::too_many_arguments)]
fn run(
    mut commands: Commands,
    view: Res<View>,
    time: Res<Time<Real>>,
    app: Res<State<AppState>>,
    data: Res<MenuData>,
    mut game: ResMut<CurrentGame>,
    (mut next_app, mut next_screen): (ResMut<NextState<AppState>>, ResMut<NextState<MenuScreen>>),
    terrain: Option<Res<crate::level::Terrain>>,
    (mut yaw, mut pitch): (ResMut<crate::gameplay::combat::CameraYawRequest>, ResMut<crate::gameplay::combat::CameraPitchRequest>),
    mut player: Query<(&mut Transform, &mut crate::level::KinematicBody), With<crate::models::CharacterModel>>,
    mut t: Local<f32>,
    mut exit: MessageWriter<AppExit>,
    (items, mut ground, mut overlay, mut dirty): (Option<Res<crate::gameplay::ItemDb>>, ResMut<crate::gameplay::inventory_ui::Ground>, ResMut<crate::gameplay::GameOverlay>, ResMut<crate::gameplay::inventory_ui::Dirty>),
) {
    if *app.get() == AppState::MainMenu {
        if game.state.is_none() {
            game.profile = Some(CharacterProfile::new(&data.creator));
            let mut s = crate::engine::GameState::new();
            (s.player_pos.x, s.player_pos.y, s.player_pos.z) = (view.at.x, view.at.y, view.at.z);
            game.state = Some(s);
            next_app.set(AppState::InGame);
            next_screen.set(MenuScreen::None);
        }
        return;
    }
    *t += time.delta_secs();
    if let Some(s) = game.state.as_mut()
        && let Some(h) = view.hour
    {
        s.environment.time_of_day_hours = h;
    }
    let floor = terrain.as_ref().and_then(|g| g.ground_under(view.at + Vec3::Y * 50.0)).unwrap_or(view.at.y);
    let feet = view.at.with_y(view.at.y.max(floor));
    if let Ok((mut tr, mut body)) = player.single_mut() {
        tr.translation = feet;
        body.vertical_velocity = 0.0;
    }
    yaw.0 = Some(view.yaw.to_radians());
    pitch.0 = Some(view.pitch.to_radians());
    if (!view.items.is_empty() || view.page.is_some()) && *t > view.wait - 2.0 && *t - time.delta_secs() <= view.wait - 2.0 {
        if let Some(db) = items.as_ref() {
            for it in &view.items {
                match it.strip_prefix('+') {
                    Some(worn) => {
                        let inv = crate::gameplay::interact::player_inv(&mut game);
                        if let Some(slot) = inv.slot_for(db, worn) {
                            inv.equip(slot, db.stack(worn, 1));
                        }
                    }
                    None => ground.drop_at(feet, db.stack(it, 1)),
                }
            }
        }
        *overlay = crate::gameplay::GameOverlay::Inventory;
        dirty.0 = true;
        if let Some(page) = view.page.as_deref().and_then(crate::gameplay::inventory_ui::TourPage::named) {
            commands.queue(move |w: &mut World| crate::gameplay::inventory_ui::tour_page(w, page));
        }
    }
    if *t > view.wait && *t - time.delta_secs() <= view.wait {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(view.shot.clone()));
        info!("viewshot: {} at {:.1}", view.shot, view.at);
    }
    if *t > view.wait + 1.5 {
        exit.write(AppExit::Success);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_spec() {
        let v = View::parse("876, 11, -447, 90").unwrap();
        assert_eq!((v.at, v.yaw, v.pitch, v.hour), (Vec3::new(876.0, 11.0, -447.0), 90.0, 15.0, None));
        assert_eq!(View::parse("1,2,3,4,5,22").unwrap().hour, Some(22.0));
        assert!(View::parse("1,2").is_none() && View::parse("a,b,c,d").is_none());
    }
}
