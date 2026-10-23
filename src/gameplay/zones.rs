//! Safe zones and water wells.
//!
//! - **Safe zones** (BP_EnterSafeZone: the 5 trader camps and safe zones, and a settlement's): a
//!   Sphere (SphereRadius 5000 cm unless the instance sets it). Overlapping it (also polled every
//!   0.2 s from BeginPlay, for a game that starts inside) is Event_EnterZone: HUD_Game.ZoneAreaUI
//!   (ZoneAreaWidget: "Now Entering" over the zone's Name, sliding in from the left with its
//!   FadeStart; a banner already up plays FadeStop first), SafeZoneAreaUI (SafeZoneUI: "SAFE
//!   ZONE" and its icon at the right edge, sliding in over 2 s), and the game instance's
//!   InSafeZone? = true. Leaving is "Now Leaving" and the badge slides back out. In a safe zone
//!   BP_PlayerCharacter's primary action does nothing with a firearm or a melee weapon drawn
//!   (`combat::fire`).
//! - **Water wells** (BP_WaterWell, 13 in the level): "Water Well" with "Fill Bottle" / "Drink"
//!   and the stat "Water: {Current} / {Max}" (MaxWater 2000; CurrentWater 100 unless the
//!   instance sets it, saved per well). Fill Bottle turns the first empty container it can fill
//!   into dirty water: a jug (130), a canteen (90), a bottle or a plastic bottle (40), each
//!   taking that much from the well ("'Water Jug' filled"...); under 40 it's "Not enough water in
//!   well". Drink takes what the survivor's thirst lacks from the well and adds the well's water
//!   to thirst (capped), or says "No water to drink from". Every 5 s a well gains 0.25, +5 in the
//!   rain and +1 in snow (its Actor_Weather_Status exposure).
//!
//! Not ported: the wells' Loot_TableTop_LeftHand_Montage, the outline.

use std::collections::BTreeMap;

use bevy::input::mouse::AccumulatedMouseScroll;
use bevy::prelude::*;

use super::interact::{GameOverlay, Prompt, PromptBar, PromptText};
use super::items::ItemDb;
use super::notify::Notify;
use super::player_inv;
use crate::menus::widgets::{MenuFont, label_colored};
use crate::menus::{AppState, CurrentGame, MenuScreen};

/// BP_EnterSafeZone's Sphere radius (cm) unless the instance sets one.
const ZONE_RADIUS: f32 = 5000.0;
/// Its OverlapTimer.
const ZONE_POLL: f32 = 0.2;
/// BP_WaterWell's MaxWater and CurrentWater.
pub const WELL_MAX: f32 = 2000.0;
const WELL_START: f32 = 100.0;
/// WellTimer: Event_Water every 5 s.
const WELL_TICK: f32 = 5.0;
/// How near a well's prompt shows (the interact trace's reach; ours).
const WELL_REACH: f32 = 2.5;
/// UDS rain / snow (0-10) above which the well counts as exposed (ED_Rain / ED_Snow Exposed;
/// the threshold is ours).
const EXPOSED: f32 = 1.0;

pub struct SafeZone {
    pub name: String,
    pub centre: Vec3,
    pub radius: f32,
}

#[derive(Resource, Default)]
pub struct SafeZones {
    pub list: Vec<SafeZone>,
    /// The zone the survivor is in (GameInstance.InSafeZone?).
    pub inside: Option<usize>,
    collected: bool,
    poll: f32,
}

impl SafeZones {
    pub fn active(&self) -> bool {
        self.inside.is_some()
    }

    /// A zone that came with a spawned prefab (a settlement's).
    pub fn add(&mut self, zone: SafeZone) {
        self.list.push(zone);
    }

    /// The zone `p` is in: the sphere overlaps the survivor's capsule (feet + 0.82 m, r 0.34 m).
    pub fn zone_at(&self, p: Vec3) -> Option<usize> {
        let centre = p + Vec3::Y * 0.82;
        self.list.iter().position(|z| z.centre.distance(centre) <= z.radius + 0.34)
    }
}

/// A level BP_EnterSafeZone: its Name and its Sphere (moved and sized per instance).
pub fn safe_zone(a: &crate::level::ActorInfo) -> Option<SafeZone> {
    if a.class != "BP_EnterSafeZone_C" {
        return None;
    }
    let prop = |k: &str| a.props.as_ref().and_then(|m| m.get(k));
    let name = prop("Name").and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok()).and_then(|v| v.get("SourceString")?.as_str().map(str::to_string)).unwrap_or_default();
    let radius = prop("Sphere.SphereRadius").and_then(|v| v.parse::<f32>().ok()).unwrap_or(ZONE_RADIUS) / 100.0;
    let (rel, _) = crate::level::item_spawners::point(a, "Sphere", Vec3::ZERO);
    Some(SafeZone { name, centre: crate::level::item_spawners::component_world(a, rel, 0.0).translation, radius })
}

pub struct Well {
    pub name: String,
    pub pos: Vec3,
    start: f32,
}

#[derive(Resource, Default)]
pub struct Wells {
    pub list: Vec<Well>,
    collected: bool,
    tick: f32,
    /// The prompt's chosen option (0 Fill Bottle, 1 Drink).
    option: usize,
}

impl Wells {
    /// A BP_WaterWell (the level's, or a spawned prefab's): its CurrentWater unless saved.
    pub fn add(&mut self, a: &crate::level::ActorInfo) {
        let start = a.props.as_ref().and_then(|m| m.get("CurrentWater")).and_then(|v| v.parse().ok()).unwrap_or(WELL_START);
        self.list.push(Well { name: a.name.clone(), pos: a.position(), start });
    }

    /// A well's water (the save's, else the instance's CurrentWater).
    pub fn water(&self, game: &CurrentGame, i: usize) -> f32 {
        let w = &self.list[i];
        game.state.as_ref().and_then(|s| s.wells.get(&w.name).copied()).unwrap_or(w.start)
    }
}

/// Event_UpdateWater: clamped to 0..MaxWater.
fn set_water(game: &mut CurrentGame, name: &str, v: f32) {
    if let Some(s) = game.state.as_mut() {
        s.wells.insert(name.to_string(), v.clamp(0.0, WELL_MAX));
    }
}

/// Fill Bottle's choice: (water taken, item used up, item given, toast), the first container
/// the survivor has that the well holds enough for; `Err` = "Not enough water in well"; `Ok(None)`
/// = nothing to fill (the BP says nothing then).
pub fn fill(water: f32, has: impl Fn(&str) -> bool) -> Result<Option<(f32, &'static str, &'static str, &'static str)>, &'static str> {
    const FILLS: [(f32, &str, &str, &str); 4] = [
        (130.0, "DA_EmptyWaterJug", "DA_DirtyWaterJug", "'Water Jug' filled"),
        (90.0, "DA_EmptyWaterCanteen", "DA_DirtyWaterCanteen", "'Water Canteen' filled"),
        (40.0, "DA_EmptyWaterBottle", "DA_DirtyWaterBottle", "'Water Bottle' filled"),
        (40.0, "DA_PlasticBottle", "DA_DirtyWaterBottle", "'Water Bottle' filled"),
    ];
    if water < 40.0 {
        return Err("Not enough water in well");
    }
    Ok(FILLS.into_iter().find(|f| water >= f.0 && has(f.1)))
}

/// DrinkWater: (the well's water after, thirst after), or `None` when the well is dry.
pub fn drink(water: f32, thirst: f32) -> Option<(f32, f32)> {
    (water > 0.0).then(|| ((water - (100.0 - thirst)).clamp(0.0, WELL_MAX), (thirst + water).min(100.0)))
}

/// Event_Water's gain over one tick.
pub fn refill(rain: f32, snow: f32) -> f32 {
    let mut gain = 0.25;
    if rain >= EXPOSED {
        gain += 5.0;
    }
    if snow >= EXPOSED {
        gain += 1.0;
    }
    gain
}

pub fn plugin(app: &mut App) {
    app.init_resource::<SafeZones>()
        .init_resource::<Wells>()
        .init_resource::<PendingBanner>()
        .add_systems(OnEnter(AppState::InGame), |mut z: ResMut<SafeZones>, mut w: ResMut<Wells>| {
            *z = SafeZones::default();
            *w = Wells::default();
        })
        .add_systems(Update, (collect, track_zones, show_banner, fade_banners, fade_badge, refill_wells).chain().run_if(in_state(AppState::InGame)))
        .add_systems(Update, use_wells.after(crate::level::doors::interact_doors).run_if(in_state(AppState::InGame)).run_if(in_state(MenuScreen::None)));
}

fn collect(level: Option<Res<crate::level::Level>>, mut zones: ResMut<SafeZones>, mut wells: ResMut<Wells>) {
    let Some(level) = level else { return };
    if zones.collected {
        return;
    }
    zones.collected = true;
    wells.collected = true;
    for a in &level.actors {
        if let Some(z) = safe_zone(a) {
            zones.list.push(z);
        }
        if a.class == "BP_WaterWell_C" {
            wells.add(a);
        }
    }
    info!("safe zones: {}, water wells: {}", zones.list.len(), wells.list.len());
}

/// A ZoneAreaWidget to show once the one up has finished its FadeStop.
#[derive(Resource, Default)]
struct PendingBanner(Option<(bool, String)>);

#[allow(clippy::too_many_arguments)]
fn track_zones(
    mut commands: Commands,
    time: Res<Time>,
    font: Res<MenuFont>,
    assets: Res<AssetServer>,
    mut zones: ResMut<SafeZones>,
    mut pending: ResMut<PendingBanner>,
    player: Query<&Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    mut badges: Query<&mut Badge>,
) {
    let Ok(p) = player.single() else { return };
    zones.poll -= time.delta_secs();
    if zones.poll > 0.0 {
        return;
    }
    zones.poll = ZONE_POLL;
    let now = zones.zone_at(p.translation);
    if now == zones.inside {
        return;
    }
    // Event_ExitZone then Event_EnterZone.
    if let Some(old) = zones.inside.take() {
        pending.0 = Some((false, zones.list[old].name.clone()));
        for mut b in &mut badges {
            b.leaving.get_or_insert(0.0);
        }
        info!("left safe zone {}", zones.list[old].name);
    }
    if let Some(new) = now {
        pending.0 = Some((true, zones.list[new].name.clone()));
        for mut b in &mut badges {
            b.leaving = None;
        }
        if badges.is_empty() {
            spawn_badge(&mut commands, &font, &assets);
        }
        info!("entered safe zone {}", zones.list[new].name);
    }
    zones.inside = now;
}

/// ZoneAreaWidget: its age, and FadeStop's once it's going.
#[derive(Component)]
pub struct ZoneBanner {
    pub entering: bool,
    pub name: String,
    age: f32,
    stopping: Option<f32>,
}

#[derive(Component)]
struct BannerBack;
#[derive(Component)]
struct BannerFront;

/// FadeStart's ZoneArea slide (60000 ticks/s): from -525 px in by 1.51 s, back out from 4.29 to 5.55 s.
const SLIDE: [(f32, f32); 4] = [(0.0, -525.0), (90771.0 / 60000.0, 0.0), (257183.0 / 60000.0, 0.0), (332825.0 / 60000.0, -525.0)];
/// FadeStart's length (the same widget as POIDiscoveredUI's) and FadeStop's 0.5 s to -625 px.
const BANNER_END: f32 = 344928.0 / 60000.0;
const STOP_SECS: f32 = 0.5;

fn show_banner(mut commands: Commands, font: Res<MenuFont>, assets: Res<AssetServer>, mut pending: ResMut<PendingBanner>, mut banners: Query<&mut ZoneBanner>) {
    if pending.0.is_none() {
        return;
    }
    // HUD_Game.ZoneAreaUI: a banner already up plays FadeStop; the new one waits for it.
    let mut up = false;
    for mut b in &mut banners {
        b.stopping.get_or_insert(0.0);
        up = true;
    }
    if up {
        return;
    }
    let Some((entering, name)) = pending.0.take() else { return };
    let grey = Color::linear_rgb(0.567708, 0.567708, 0.567708);
    // CanvasPanel slot: bottom left, alignment (0, 2.75); the Border's padding 25 / 50 around the
    // VerticalBox's own 25 / 50.
    commands.spawn((
        Node { position_type: PositionType::Absolute, left: Val::Px(0.0), bottom: Val::Px(1.75 * 75.0), padding: UiRect::new(Val::Px(50.0), Val::Px(100.0), Val::Px(0.0), Val::Px(0.0)), flex_direction: FlexDirection::Column, ..default() },
        ImageNode::new(assets.load("game/SurrounDead/Content/UI/Textures/fade-left.png")).with_color(Color::BLACK.with_alpha(0.0)).with_mode(bevy::ui::widget::NodeImageMode::Stretch),
        ZoneBanner { entering, name: name.clone(), age: 0.0, stopping: None },
        BannerBack,
        GlobalZIndex(5),
        Pickable::IGNORE,
        DespawnOnExit(AppState::InGame),
        children![
            (label_colored(&font, if entering { "Now Entering" } else { "Now Leaving" }, 20.0, grey.with_alpha(0.0)), BannerBack),
            // Image_182 is removed: the safe zone passes no texture.
            (label_colored(&font, name, 34.0, Color::WHITE.with_alpha(0.0)), BannerFront),
        ],
    ));
}

#[allow(clippy::type_complexity)]
fn fade_banners(
    mut commands: Commands,
    time: Res<Time>,
    mut banners: Query<(Entity, &mut ZoneBanner, &mut Node, &mut ImageNode, &Children)>,
    mut texts: Query<(&mut TextColor, Has<BannerFront>)>,
) {
    use super::map::{BANNER_BACK, BANNER_FRONT, track};
    for (e, mut b, mut node, mut img, kids) in &mut banners {
        let dt = time.delta_secs();
        let ZoneBanner { age, stopping, .. } = &mut *b;
        let (back, front, x) = match stopping.as_mut() {
            Some(s) => {
                *s += dt;
                if *s >= STOP_SECS {
                    commands.entity(e).despawn();
                    continue;
                }
                let k = 1.0 - *s / STOP_SECS;
                // FadeStop eases ZoneArea and the name out from where FadeStart left them.
                (track(&BANNER_BACK, *age) * k, track(&BANNER_FRONT, *age) * k, track(&SLIDE, *age) - 625.0 * (1.0 - k))
            }
            None => {
                *age += dt;
                if *age >= BANNER_END {
                    commands.entity(e).despawn();
                    continue;
                }
                (track(&BANNER_BACK, *age), track(&BANNER_FRONT, *age), track(&SLIDE, *age))
            }
        };
        node.left = Val::Px(x);
        img.color.set_alpha(back);
        for k in kids.iter() {
            if let Ok((mut c, f)) = texts.get_mut(k) {
                // RenderOpacity multiplies down the tree.
                c.0.set_alpha(if f { back * front } else { back });
            }
        }
    }
}

/// SafeZoneUI: Anim forward on Construct (2 s), reversed by Destroy, then removed 2 s later.
#[derive(Component)]
struct Badge {
    age: f32,
    leaving: Option<f32>,
}

/// SafeZoneUI's Anim: opacity 0 -> 1 and 160 px -> 0 over 2 s.
const BADGE_SECS: f32 = 2.0;

fn spawn_badge(commands: &mut Commands, font: &MenuFont, assets: &AssetServer) {
    // CanvasPanel slot: anchored at the right edge's middle, alignment (1, 0); Border_0's
    // fade-right brush and padding (0, 10, 10, 10) around "SAFE ZONE" (18, shadow 2) over the icon.
    commands.spawn((
        Node { position_type: PositionType::Absolute, right: Val::Px(0.0), top: Val::Percent(50.0), padding: UiRect::new(Val::Px(0.0), Val::Px(20.0), Val::Px(20.0), Val::Px(20.0)), flex_direction: FlexDirection::Column, align_items: AlignItems::FlexEnd, ..default() },
        ImageNode::new(assets.load("game/SurrounDead/Content/UI/Textures/fade-right.png")).with_color(Color::WHITE.with_alpha(0.0)).with_mode(bevy::ui::widget::NodeImageMode::Stretch),
        Badge { age: 0.0, leaving: None },
        GlobalZIndex(5),
        Pickable::IGNORE,
        DespawnOnExit(AppState::InGame),
        children![
            // (The menu label carries its own shadow, standing in for the block's 2 px one.)
            label_colored(font, "SAFE ZONE", 18.0, Color::WHITE.with_alpha(0.0)),
            (Node { width: Val::Px(64.0), height: Val::Px(64.0), ..default() }, ImageNode::new(assets.load("game/SurrounDead/Content/UI/Textures/SafeZoneImg.png")).with_color(Color::WHITE.with_alpha(0.0))),
        ],
    ));
}

fn fade_badge(mut commands: Commands, time: Res<Time>, mut badges: Query<(Entity, &mut Badge, &mut Node, &mut ImageNode, &Children)>, mut texts: Query<&mut TextColor>, mut images: Query<&mut ImageNode, Without<Badge>>) {
    for (e, mut b, mut node, mut img, kids) in &mut badges {
        let dt = time.delta_secs();
        let Badge { age, leaving } = &mut *b;
        let t = match leaving.as_mut() {
            Some(l) => {
                *l += dt;
                if *l >= BADGE_SECS {
                    commands.entity(e).despawn();
                    continue;
                }
                (age.min(BADGE_SECS) - *l).max(0.0)
            }
            None => {
                *age += dt;
                age.min(BADGE_SECS)
            }
        };
        let s = t / BADGE_SECS;
        let a = s * s * (3.0 - 2.0 * s);
        node.margin.right = Val::Px(-160.0 * (1.0 - a));
        img.color.set_alpha(a);
        for k in kids.iter() {
            if let Ok(mut c) = texts.get_mut(k) {
                c.0.set_alpha(a);
            }
            if let Ok(mut i) = images.get_mut(k) {
                i.color.set_alpha(a);
            }
        }
    }
}

fn refill_wells(time: Res<Time>, weather: Option<Res<super::weather::Weather>>, mut wells: ResMut<Wells>, mut game: ResMut<CurrentGame>) {
    wells.tick += time.delta_secs();
    if wells.tick < WELL_TICK || game.state.is_none() {
        return;
    }
    wells.tick -= WELL_TICK;
    let w = weather.map(|w| w.0).unwrap_or_default();
    let gain = refill(w.rain, w.snow);
    for i in 0..wells.list.len() {
        let v = wells.water(&game, i) + gain;
        set_water(&mut game, &wells.list[i].name, v);
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn use_wells(
    keys: Res<ButtonInput<KeyCode>>,
    scroll: Res<AccumulatedMouseScroll>,
    db: Res<ItemDb>,
    overlay: Res<GameOverlay>,
    driving: Res<super::vehicles::Driving>,
    mut wells: ResMut<Wells>,
    mut game: ResMut<CurrentGame>,
    mut ground: ResMut<super::inventory_ui::Ground>,
    mut notify: MessageWriter<Notify>,
    player: Query<&Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    mut prompt: Query<&mut Visibility, With<Prompt>>,
    mut text: Query<&mut Text, With<PromptText>>,
    mut bar: Query<&mut Node, With<PromptBar>>,
) {
    let (Ok(p), Ok(mut vis)) = (player.single(), prompt.single_mut()) else { return };
    if overlay.is_open() || *vis != Visibility::Hidden || driving.active() {
        return;
    }
    let p = p.translation;
    let Some(i) = wells.list.iter().position(|w| w.pos.distance(p) <= WELL_REACH) else { return };
    const OPTIONS: [&str; 2] = ["Fill Bottle", "Drink"];
    if scroll.delta.y != 0.0 {
        wells.option = (wells.option + 1) % OPTIONS.len();
    }
    let water = wells.water(&game, i);
    *vis = Visibility::Inherited;
    if let Ok(mut n) = bar.single_mut() {
        n.width = Val::Percent(0.0);
    }
    if let Ok(mut t) = text.single_mut() {
        t.0 = format!("Water Well: [F] {}  [Wheel] change\nWater: {} / {}", OPTIONS[wells.option], water.trunc(), WELL_MAX);
    }
    if !keys.just_pressed(KeyCode::KeyF) {
        return;
    }
    let name = wells.list[i].name.clone();
    if wells.option == 0 {
        let pick = {
            let inv = player_inv(&mut game);
            fill(water, |item| inv.count(item) > 0)
        };
        match pick {
            Err(msg) => {
                notify.write(Notify { text: msg.into(), secs: 3.0, color: Color::WHITE });
            }
            Ok(Some((take, used, given, msg))) => {
                let left = {
                    let inv = player_inv(&mut game);
                    inv.remove(used, 1);
                    inv.insert(&db, db.stack(given, 1))
                };
                if let Some(s) = left {
                    ground.drop_at(p, s);
                }
                notify.write(Notify::new(msg));
                set_water(&mut game, &name, water - take);
            }
            Ok(None) => {}
        }
    } else {
        let thirst = game.state.as_ref().map_or(100.0, |s| s.survival.thirst);
        match drink(water, thirst) {
            Some((left, quenched)) => {
                set_water(&mut game, &name, left);
                if let Some(s) = game.state.as_mut() {
                    s.survival.thirst = quenched;
                }
            }
            None => {
                notify.write(Notify::new("No water to drink from"));
            }
        }
    }
}

/// Wells' water in a save, by actor name.
pub type WellSave = BTreeMap<String, f32>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wells_fill_the_biggest_container_they_can() {
        let has = |items: &'static [&'static str]| move |i: &str| items.contains(&i);
        assert_eq!(fill(500.0, has(&["DA_EmptyWaterBottle", "DA_EmptyWaterJug"])), Ok(Some((130.0, "DA_EmptyWaterJug", "DA_DirtyWaterJug", "'Water Jug' filled"))));
        // Too little for the jug: the canteen, then the bottles.
        assert_eq!(fill(100.0, has(&["DA_EmptyWaterJug", "DA_EmptyWaterCanteen"])).unwrap().unwrap().1, "DA_EmptyWaterCanteen");
        assert_eq!(fill(50.0, has(&["DA_EmptyWaterJug", "DA_PlasticBottle"])).unwrap().unwrap().2, "DA_DirtyWaterBottle");
        assert_eq!(fill(39.0, has(&["DA_PlasticBottle"])), Err("Not enough water in well"));
        assert_eq!(fill(500.0, has(&[])), Ok(None));
    }

    #[test]
    fn drinking_and_refilling_follow_the_bp() {
        // Thirst 30 lacks 70: the well loses 70, thirst gains the whole well (capped).
        assert_eq!(drink(500.0, 30.0), Some((430.0, 100.0)));
        assert_eq!(drink(20.0, 30.0), Some((0.0, 50.0)));
        assert_eq!(drink(0.0, 30.0), None);
        assert_eq!(refill(0.0, 0.0), 0.25);
        assert_eq!(refill(6.0, 0.0), 5.25);
        assert_eq!(refill(0.0, 4.0), 1.25);
    }

    #[test]
    fn safe_zones_read_their_instance() {
        let props = [("Name", r#"{"SourceString":"Trader Bob's"}"#), ("Sphere.SphereRadius", "950.0"), ("Sphere.RelativeLocation", r#"{"X":1000.0,"Y":0.0,"Z":0.0}"#)];
        let a = crate::level::ActorInfo {
            class: "BP_EnterSafeZone_C".into(),
            name: "z".into(),
            transform: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 10.0, 0.0, 20.0],
            props: Some(props.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()),
        };
        let z = safe_zone(&a).unwrap();
        assert_eq!((z.name.as_str(), z.radius), ("Trader Bob's", 9.5));
        // UE +X is Bevy +X: the sphere sits 10 m along it.
        assert!((z.centre - Vec3::new(20.0, 0.0, 20.0)).length() < 1e-4, "{}", z.centre);
        let zones = SafeZones { list: vec![z], ..default() };
        assert_eq!(zones.zone_at(Vec3::new(29.0, 0.0, 20.0)), Some(0));
        assert_eq!(zones.zone_at(Vec3::new(31.0, 0.0, 20.0)), None);
    }
}
