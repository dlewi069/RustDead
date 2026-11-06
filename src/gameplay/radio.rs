//! Radios, from Blueprints/Other/Radio, RadioComponent, RadioUI / RadioTextUI, Buildable_Radio,
//! Buildable_Jukebox and the vehicles' radio (decompiled).
//!
//! - **BP_Radio** (4 in the level): its Audio component loops `Sound` (Radio_Cue, a news
//!   broadcast) from BeginPlay at VolumeMultiplier 0. "Radio: [F] Interact" sets it to 0.5 if
//!   it's 0, else back to 0.
//! - **BP_Radio_Text** / **_Military** (5): BeginPlay reads its RadioID's DT_Radio row (each
//!   instance sets one: CoyoteCall, FireInTheCity, MilitaryReinforcements, TheSubway,
//!   ForceFromAfar; `assets/data/radio.json`, gen_radio.py). Its Audio loops RadioStatic_Cue.
//!   Interact: SetSound(RadioMuffled_Cue), then HUD_Game.CreateRadioTextWidget(true, row, self):
//!   with a RadioTextUI already up, this radio goes back to static and the widget closes; else
//!   RadioTextUI (an 800 px box 275 px below the middle, Montserrat-Bold 14, centred, wrapped,
//!   on 50 % black) shows each line for its DelayPerLine, and after the last: static again,
//!   widget gone.
//! - **RadioComponent** (Buildable_Radio, Buildable_Jukebox, every vehicle): BeginPlay lists
//!   `<game directory>/Radio/*.mp3` (FindFiles; the game ships no such folder: players add their
//!   own music). StartRadio: TurnedOn?, unpaused, PlayRate 1, PreviousSong = CurrentSong, a
//!   random file (repeats allowed) opened on the MediaPlayer (looping) and played through the
//!   owner's MediaSound (PlayerRadio attenuation: 6 m + 25 m linear; RadioMusicAudio = the RADIO
//!   slider; VolumeMultiplier 1); its EndReached starts another. StopRadio: off, the player
//!   closed, RadioUI removed. PreviousSongRadio: replays PreviousSong; at its end PreviousSong =
//!   CurrentSong and StartRadio.
//! - **RadioUI** (ToggleRadioUI): 700 x 399, its bottom 5 % above the middle: "RADIO",
//!   CURRENTLY PLAYING: the file name, VOLUME: mute / low / medium / high = MediaSound volume 0 /
//!   0.1 / 0.5 / 1, PLAYBACK RATE: 0.25-2 (SetRate; with nothing playing "No Music Playing" and
//!   the box back to "1"), TURN ON / TURN OFF (StartRadio / StopRadio, which also closes the
//!   UI), pause / play (icon_media_83 / 88), NEXT (StartRadio), PREVIOUS (PreviousSongRadio), the
//!   close button. Esc closes it too (ours: the game leaves the mouse in UI mode).
//! - **Buildable_Radio**: Interact = ToggleRadioUI; the prompt's 'Power' stat says On / Off with
//!   TurnedOn / TurnedOff; picking it up stops the radio. **Buildable_Jukebox**: the same, but
//!   Interact without power says "Requires power", and losing power stops it ("Power: Off").
//! - **Vehicles**: VEHICLE RADIO (R) while driving toggles RadioUI; getting out closes it (the
//!   music plays on).
//!
//! Quirk kept: interacting with a second text radio while another's text is up closes the text
//! and leaves the first radio playing its muffled voice (CreateRadioTextWidget sets only the new
//! radio back to static). Ours: `Radio/` is next to the working directory (the port's game
//! directory; `SD_RADIO_DIR` overrides it); the MP3 decoding is Bevy's (rodio). Not ported: the
//! jukebox's Emissive 0 / 2 while playing, the radio towers' pulsing light material (the radio
//! towers are static meshes, already drawn), the notification's radio icon.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use rand::Rng;
use serde::Deserialize;

use super::interact::{GameOverlay, Prompt, PromptBar, PromptText};
use super::notify::Notify;
use crate::audio::{AmbientLoop, SoundDb, Voiced, attenuate, class_volume};
use crate::menus::settings::SettingsStore;
use crate::menus::widgets::{ButtonPalette, MenuFont, button, label};
use crate::menus::{AppState, MenuScreen};

/// How near the survivor must be to use a level radio (ours).
const REACH: f32 = 2.4;
/// BP_Radio's Interact: VolumeMultiplier 0 <-> 0.5.
const ON_VOLUME: f32 = 0.5;
/// RadioUI's volume buttons (mute, low, medium, high).
pub const VOLUMES: [f32; 4] = [0.0, 0.1, 0.5, 1.0];
/// ComboBoxString_165's DefaultOptions (SelectedOption "1").
pub const RATES: [&str; 8] = ["0.25", "0.5", "0.75", "1", "1.25", "1.5", "1.75", "2"];
/// The MediaSound's attenuation and sound class.
const ATTENUATION: &str = "PlayerRadio";
const CLASS: &str = "RadioMusicAudio";
/// Buildable_Radio's MediaSound sits 40 cm up.
const PLACED_UP: f32 = 0.4;

/// DT_Radio rows (`assets/data/radio.json`).
#[derive(Deserialize, Default, Clone, Debug)]
pub struct RadioTable {
    pub rows: HashMap<String, RadioRow>,
}

#[derive(Deserialize, Default, Clone, Debug)]
pub struct RadioRow {
    pub lines: Vec<String>,
    pub delays: Vec<f32>,
}

impl RadioTable {
    pub fn load() -> Self {
        std::fs::read_to_string("assets/data/radio.json").ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }
}

/// A level radio: BP_Radio (a broadcast to switch on and off) or BP_Radio_Text(_Military) (its
/// DT_Radio row).
#[derive(Clone, Debug)]
pub struct LevelRadio {
    pub actor: String,
    pub class: String,
    pub at: Vec3,
    pub text: Option<String>,
    /// BP_Radio's VolumeMultiplier.
    pub volume: f32,
    /// Its AmbientLoop.
    pub sound: Option<Entity>,
}

/// BP_Radio's Interact: 0.5 if it's 0, else 0.
pub fn toggle_volume(v: f32) -> f32 {
    if v == 0.0 { ON_VOLUME } else { 0.0 }
}

/// RadioTextUI: the row, the line showing (Row) and the time left on its Delay.
#[derive(Clone, Debug, PartialEq)]
pub struct TextShow {
    /// The radio it was opened on (RadioRef).
    pub radio: usize,
    pub row: String,
    pub line: usize,
    pub left: f32,
}

impl TextShow {
    pub fn new(radio: usize, row: &str, table: &RadioTable) -> Self {
        let delay = table.rows.get(row).and_then(|r| r.delays.first().copied()).unwrap_or(0.0);
        Self { radio, row: row.into(), line: 0, left: delay }
    }

    /// The Delay running out: the next line, or None at the end (Row == LastIndex + 1).
    pub fn step(&mut self, dt: f32, table: &RadioTable) -> bool {
        self.left -= dt;
        if self.left > 0.0 {
            return true;
        }
        let Some(row) = table.rows.get(&self.row) else { return false };
        self.line += 1;
        if self.line >= row.lines.len() {
            return false;
        }
        self.left += row.delays.get(self.line).copied().unwrap_or(0.0);
        true
    }

    pub fn text<'a>(&self, table: &'a RadioTable) -> &'a str {
        table.rows.get(&self.row).and_then(|r| r.lines.get(self.line)).map_or("", String::as_str)
    }
}

/// Whose RadioComponent: a placed buildable (its id) or a vehicle.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RadioOwner {
    Placed(usize),
    Vehicle(Entity),
}

/// A RadioComponent's state.
#[derive(Clone, Debug, PartialEq)]
pub struct MusicRadio {
    pub on: bool,
    pub paused: bool,
    pub current: String,
    pub previous: String,
    pub rate: f32,
    /// The MediaSound's VolumeMultiplier.
    pub volume: f32,
    /// The file the MediaPlayer has open (None: closed, or OpenFile on a missing name).
    pub playing: Option<String>,
    /// Playing PreviousSong (its EndReached is EndReached_Previous).
    pub replaying: bool,
    /// Bumped with every OpenFile.
    pub serial: u32,
}

impl Default for MusicRadio {
    fn default() -> Self {
        Self { on: false, paused: false, current: String::new(), previous: String::new(), rate: 1.0, volume: 1.0, playing: None, replaying: false, serial: 0 }
    }
}

impl MusicRadio {
    /// StartRadio, with Array_Random's pick.
    pub fn start(&mut self, songs: &[String], pick: usize) {
        self.on = true;
        self.paused = false;
        self.rate = 1.0;
        self.previous = self.current.clone();
        self.replaying = false;
        if !songs.is_empty() {
            self.current = songs[pick % songs.len()].clone();
            self.playing = Some(self.current.clone());
            self.serial += 1;
        }
    }

    /// StopRadio.
    pub fn stop(&mut self) {
        self.on = false;
        self.playing = None;
        self.replaying = false;
    }

    /// PreviousSongRadio: PreviousSong opened again (nothing if there's none).
    pub fn previous(&mut self) {
        self.paused = false;
        self.rate = 1.0;
        self.replaying = true;
        self.playing = (!self.previous.is_empty()).then(|| self.previous.clone());
        self.serial += 1;
    }

    /// EndReached_Start / EndReached_Previous.
    pub fn ended(&mut self, songs: &[String], pick: usize) {
        if self.replaying {
            self.previous = self.current.clone();
        }
        self.start(songs, pick);
    }

    /// The MediaSound has a media player (RadioUI's checks): between StartRadio and StopRadio.
    pub fn has_player(&self) -> bool {
        self.on
    }
}

/// UE's Conv_DoubleToString (SanitizeFloat): "1.0", "0.25"; RadioUI selects it in the box, which
/// keeps its choice when the text isn't one of its options ("1.0" isn't).
pub fn sanitize(v: f32) -> String {
    let s = format!("{v}");
    if s.contains('.') { s } else { format!("{s}.0") }
}

/// FindFiles(<dir>, "mp3"): the file names, sorted (ours: the order FindFiles returns isn't set).
pub fn find_songs(dir: &std::path::Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.to_ascii_lowercase().ends_with(".mp3"))
        .collect();
    out.sort();
    out
}

/// GetGameDirectory + "Radio" (ours: the working directory; SD_RADIO_DIR overrides).
pub fn radio_dir() -> std::path::PathBuf {
    std::env::var_os("SD_RADIO_DIR").map_or_else(|| std::path::PathBuf::from("Radio"), Into::into)
}

#[derive(Resource, Default)]
pub struct Radios {
    collected: bool,
    pub level: Vec<LevelRadio>,
    pub table: RadioTable,
    pub text: Option<TextShow>,
    /// The music radios that have been used.
    pub music: HashMap<RadioOwner, MusicRadio>,
    /// FindFiles' list (read when the game starts).
    pub songs: Vec<String>,
    /// Loaded songs.
    sources: HashMap<String, Handle<AudioSource>>,
    /// RadioUI's playback-rate box: open, and what it shows.
    pub rate_open: bool,
    pub rate_shown: String,
}

impl Radios {
    pub fn get(&mut self, owner: RadioOwner) -> &mut MusicRadio {
        self.music.entry(owner).or_default()
    }

    pub fn is_on(&self, owner: RadioOwner) -> bool {
        self.music.get(&owner).is_some_and(|r| r.on)
    }
}

/// A playing song (the MediaSound), for one radio and one OpenFile.
#[derive(Component)]
pub struct RadioVoice {
    pub owner: RadioOwner,
    serial: u32,
}

#[derive(Component)]
struct RadioTextUi;

#[derive(Component)]
struct RadioTextLine;

#[derive(Component)]
struct RadioUi;

#[derive(Component, Clone, Copy, PartialEq, Debug)]
enum RadioButton {
    Close,
    Volume(usize),
    RateBox,
    Rate(usize),
    Power,
    Pause,
    Next,
    Previous,
}

#[derive(Component)]
struct SongText;

#[derive(Component)]
struct PowerText;

#[derive(Component)]
struct PauseImage;

#[derive(Component)]
struct RateText;

#[derive(Component)]
struct RateList;

pub fn plugin(app: &mut App) {
    app.init_resource::<Radios>()
        .add_systems(OnEnter(AppState::InGame), |mut r: ResMut<Radios>| {
            let table = RadioTable::load();
            let songs = find_songs(&radio_dir());
            info!("radio: {} song(s) in {}", songs.len(), radio_dir().display());
            *r = Radios { table, songs, rate_shown: "1".into(), ..default() };
        })
        .add_systems(Update, (collect, text_show, music, radio_ui, vehicle_radio).chain().run_if(in_state(AppState::InGame)))
        .add_systems(Update, interact.after(crate::level::doors::interact_doors).run_if(in_state(AppState::InGame)).run_if(in_state(MenuScreen::None)))
        .add_observer(on_button);
}

fn collect(mut commands: Commands, level: Option<Res<crate::level::Level>>, mut radios: ResMut<Radios>) {
    let Some(level) = level else { return };
    if radios.collected {
        return;
    }
    radios.collected = true;
    for a in &level.actors {
        let text = match a.class.as_str() {
            "BP_Radio_C" => None,
            // RadioID: the instance's, else the class default "Test".
            "BP_Radio_Text_C" | "BP_Radio_Text_Military_C" => {
                Some(a.props.as_ref().and_then(|p| p.get("RadioID")).map_or("Test".to_string(), |v| v.trim_matches('"').to_string()))
            }
            _ => continue,
        };
        let at = crate::level::affine(&a.transform).translation;
        // The Audio component: Sound (Radio_Cue) at volume 0, or RadioStatic_Cue.
        let (cue, volume, active) = if text.is_some() { ("RadioStatic_Cue", 1.0, true) } else { ("Radio_Cue", ON_VOLUME, false) };
        let sound = commands.spawn((Transform::from_translation(at), AmbientLoop { cue: cue.into(), volume, active }, DespawnOnExit(AppState::InGame))).id();
        radios.level.push(LevelRadio { actor: a.name.clone(), class: a.class.clone(), at, text, volume: 0.0, sound: Some(sound) });
    }
    info!("radio: {} level radios", radios.level.len());
}

/// Swap a level radio's looping cue (Audio.SetSound): the loop restarts with it.
fn set_sound(commands: &mut Commands, loops: &mut Query<&mut AmbientLoop>, e: Option<Entity>, cue: &str) {
    let Some(e) = e else { return };
    if let Ok(mut l) = loops.get_mut(e)
        && l.cue != cue
    {
        l.cue = cue.into();
        commands.entity(e).try_remove::<Voiced>();
    }
}

/// "Radio: [F] Interact" on the level radios.
#[allow(clippy::too_many_arguments)]
fn interact(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    overlay: Res<GameOverlay>,
    driving: Res<super::vehicles::Driving>,
    mut radios: ResMut<Radios>,
    mut loops: Query<&mut AmbientLoop>,
    player: Query<&Transform, (With<crate::models::CharacterModel>, With<crate::level::KinematicBody>)>,
    mut prompt: Query<&mut Visibility, With<Prompt>>,
    mut text: Query<&mut Text, With<PromptText>>,
    mut bar: Query<&mut Node, With<PromptBar>>,
    mut highlight: ResMut<super::outline::Highlight>,
) {
    let (Ok(p), Ok(mut vis)) = (player.single(), prompt.single_mut()) else { return };
    if overlay.is_open() || *vis != Visibility::Hidden || driving.active() {
        return;
    }
    let p = p.translation;
    let Some(i) = radios.level.iter().enumerate().filter(|(_, r)| r.at.distance(p) <= REACH).min_by(|a, b| a.1.at.distance(p).total_cmp(&b.1.at.distance(p))).map(|(i, _)| i) else { return };
    *vis = Visibility::Inherited;
    highlight.claim(super::outline::Highlit::Actor { class: radios.level[i].class.clone(), at: radios.level[i].at });
    if let Ok(mut n) = bar.single_mut() {
        n.width = Val::Percent(0.0);
    }
    if let Ok(mut t) = text.single_mut() {
        // ShowInteractWidget("Radio", GetInteractOptions: Option1 "Interact").
        t.0 = "Radio: [F] Interact".into();
    }
    if !keys.just_pressed(KeyCode::KeyF) {
        return;
    }
    let r = radios.level[i].clone();
    match r.text {
        None => {
            let v = toggle_volume(r.volume);
            radios.level[i].volume = v;
            if let Some(Ok(mut l)) = r.sound.map(|e| loops.get_mut(e)) {
                l.active = v > 0.0;
            }
            info!("radio: {} volume {v}", r.actor);
        }
        Some(row) => {
            set_sound(&mut commands, &mut loops, r.sound, "RadioMuffled_Cue");
            if radios.text.take().is_some() {
                // CreateRadioTextWidget with one up: this radio (RadioRef) back to static.
                set_sound(&mut commands, &mut loops, r.sound, "RadioStatic_Cue");
                info!("radio: {} text closed", r.actor);
            } else {
                let show = TextShow::new(i, &row, &radios.table);
                info!("radio: {} text {row}: {:?}", r.actor, show.text(&radios.table));
                radios.text = Some(show);
            }
        }
    }
}

/// RadioTextUI: each line for its delay; at the end the radio's static and the widget goes.
fn text_show(
    mut commands: Commands,
    time: Res<Time>,
    font: Res<MenuFont>,
    mut radios: ResMut<Radios>,
    mut loops: Query<&mut AmbientLoop>,
    uis: Query<Entity, With<RadioTextUi>>,
    mut lines: Query<&mut Text, With<RadioTextLine>>,
) {
    let Radios { text, table, level, .. } = &mut *radios;
    let Some(show) = text.as_mut() else {
        uis.iter().for_each(|e| commands.entity(e).despawn());
        return;
    };
    if !show.step(time.delta_secs(), table) {
        let sound = level.get(show.radio).and_then(|r| r.sound);
        info!("radio: text {} ended", show.row);
        *text = None;
        set_sound(&mut commands, &mut loops, sound, "RadioStatic_Cue");
        uis.iter().for_each(|e| commands.entity(e).despawn());
        return;
    }
    let line = show.text(table).to_string();
    if uis.is_empty() {
        // SizeBox_0: 800 wide, anchored to the middle, 275 px below it, centred.
        let root = commands
            .spawn((
                Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), justify_content: JustifyContent::Center, ..default() },
                GlobalZIndex(30),
                Pickable::IGNORE,
                DespawnOnExit(AppState::InGame),
                RadioTextUi,
            ))
            .id();
        let border = commands
            .spawn((
                Node { position_type: PositionType::Absolute, top: Val::Percent(50.0), margin: UiRect::top(Val::Px(275.0)), width: Val::Px(800.0), padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)), justify_content: JustifyContent::Center, ..default() },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
                ChildOf(root),
            ))
            .id();
        commands.spawn((label(&font, line, 14.0), TextLayout::justify(Justify::Center), Node { margin: UiRect::axes(Val::Px(10.0), Val::Px(5.0)), ..default() }, RadioTextLine, ChildOf(border)));
    } else if let Ok(mut t) = lines.single_mut()
        && t.0 != line
    {
        t.0 = line;
    }
}

/// Where a radio is (its MediaSound), if its owner is still there.
fn owner_at(owner: RadioOwner, placed: &super::building::Buildables, vehicles: &Query<&GlobalTransform, With<super::vehicles::Vehicle>>) -> Option<Vec3> {
    match owner {
        RadioOwner::Placed(id) => placed.list.iter().find(|l| l.id == id).map(|l| l.transform.translation + Vec3::Y * PLACED_UP),
        RadioOwner::Vehicle(e) => vehicles.get(e).ok().map(|t| t.translation()),
    }
}

/// The MediaPlayers: open the file, follow the owner, volume / rate / pause, EndReached; owners
/// gone (picked up) stop; a jukebox without power stops.
#[allow(clippy::too_many_arguments)]
fn music(
    mut commands: Commands,
    db: Res<SoundDb>,
    store: Res<SettingsStore>,
    placed: Res<super::building::Buildables>,
    bdb: Res<super::building::BuildableDb>,
    mut radios: ResMut<Radios>,
    mut audio: ResMut<Assets<AudioSource>>,
    vehicles: Query<&GlobalTransform, With<super::vehicles::Vehicle>>,
    mut voices: Query<(Entity, &RadioVoice, Option<&mut AudioSink>)>,
    listener: Query<&GlobalTransform, With<crate::audio::Listener>>,
) {
    let ear = listener.iter().next().map(|t| t.translation());
    let att = db.attenuation.get(ATTENUATION).copied();
    let class = class_volume(&db, &store, Some(CLASS));
    let mut rng = rand::thread_rng();
    let owners: Vec<RadioOwner> = radios.music.keys().copied().collect();
    for owner in owners {
        let at = owner_at(owner, &placed, &vehicles);
        let unpowered = match owner {
            RadioOwner::Placed(id) => placed.list.iter().find(|l| l.id == id).is_some_and(|l| bdb.actors.get(&l.actor).is_some_and(|d| d.requires_power) && !l.power.powered),
            RadioOwner::Vehicle(_) => false,
        };
        let r = radios.get(owner);
        // OnDestroy / ReceiveDestroyed (picked up), the jukebox's EventPoweredOff: StopRadio.
        if r.on && (at.is_none() || unpowered) {
            info!("radio: {owner:?} stopped ({})", if at.is_none() { "gone" } else { "no power" });
            r.stop();
        }
        let voice = voices.iter_mut().find(|(_, v, _)| v.owner == owner);
        let want = r.playing.clone().filter(|_| r.on);
        match (voice, want) {
            (Some((e, v, sink)), Some(_)) if v.serial == r.serial => {
                let Some(mut sink) = sink else { continue };
                // EndReached: StartRadio again (or after PreviousSong, EndReached_Previous).
                if sink.empty() && !r.paused {
                    let songs = radios.songs.clone();
                    let pick = rng.gen_range(0..songs.len().max(1));
                    let r = radios.get(owner);
                    r.ended(&songs, pick);
                    info!("radio: {owner:?} song ended, now {:?}", r.playing);
                    commands.entity(e).despawn();
                    continue;
                }
                if sink.is_paused() != r.paused {
                    if r.paused { sink.pause() } else { sink.play() }
                }
                sink.set_speed(r.rate);
                let gain = match (at, ear, att) {
                    (Some(at), Some(ear), Some(a)) => attenuate(at.distance(ear), &a),
                    _ => 1.0,
                };
                sink.set_volume(Volume::Linear(r.volume * class * gain));
            }
            (voice, want) => {
                if let Some((e, _, _)) = voice {
                    commands.entity(e).despawn();
                }
                let Some(song) = want else { continue };
                let (volume, rate, serial) = (r.volume, r.rate, r.serial);
                // OpenFile(<game dir>/Radio/<song>).
                let handle = match radios.sources.get(&song) {
                    Some(h) => h.clone(),
                    None => match std::fs::read(radio_dir().join(&song)) {
                        Ok(bytes) => {
                            let h = audio.add(AudioSource { bytes: Arc::from(bytes) });
                            radios.sources.insert(song.clone(), h.clone());
                            h
                        }
                        Err(err) => {
                            warn!("radio: can't open {song}: {err}");
                            radios.get(owner).playing = None;
                            continue;
                        }
                    },
                };
                info!("radio: {owner:?} playing {song}");
                commands.spawn((
                    AudioPlayer::new(handle),
                    PlaybackSettings { mode: PlaybackMode::Once, volume: Volume::Linear(volume * class), speed: rate, ..default() },
                    RadioVoice { owner, serial },
                    DespawnOnExit(AppState::InGame),
                ));
            }
        }
    }
    // Voices whose radio stopped.
    for (e, v, _) in &voices {
        if radios.music.get(&v.owner).is_none_or(|r| !r.on || r.playing.is_none() || r.serial != v.serial) {
            commands.entity(e).despawn();
        }
    }
}

/// Open a radio's RadioUI (ToggleRadioUI), unless it's open: then close it.
pub fn toggle_ui(overlay: &mut GameOverlay, owner: RadioOwner) {
    *overlay = if *overlay == GameOverlay::Radio(owner) { GameOverlay::None } else { GameOverlay::Radio(owner) };
}

/// VEHICLE RADIO (R) while driving; getting out closes RadioUI.
fn vehicle_radio(keys: Res<ButtonInput<KeyCode>>, screen: Res<State<MenuScreen>>, driving: Res<super::vehicles::Driving>, mut overlay: ResMut<GameOverlay>) {
    if let GameOverlay::Radio(RadioOwner::Vehicle(v)) = *overlay
        && driving.vehicle != Some(v)
    {
        *overlay = GameOverlay::None;
    }
    let Some(v) = driving.vehicle else { return };
    if *screen.get() == MenuScreen::None && keys.just_pressed(KeyCode::KeyR) && matches!(*overlay, GameOverlay::None | GameOverlay::Radio(_)) {
        toggle_ui(&mut overlay, RadioOwner::Vehicle(v));
        info!("radio: vehicle radio {}", if overlay.is_open() { "open" } else { "closed" });
    }
}

/// RadioUI while a radio's is open: built on opening, its texts kept up to date.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn radio_ui(
    mut commands: Commands,
    overlay: Res<GameOverlay>,
    font: Res<MenuFont>,
    assets: Res<AssetServer>,
    mut radios: ResMut<Radios>,
    uis: Query<Entity, With<RadioUi>>,
    mut texts: ParamSet<(Query<&mut Text, With<SongText>>, Query<&mut Text, With<PowerText>>, Query<&mut Text, With<RateText>>)>,
    mut pause: Query<&mut ImageNode, With<PauseImage>>,
    mut list: Query<&mut Visibility, With<RateList>>,
) {
    let GameOverlay::Radio(owner) = *overlay else {
        uis.iter().for_each(|e| commands.entity(e).despawn());
        radios.rate_open = false;
        return;
    };
    if uis.is_empty() {
        // Construct: the box shows PlayRate if it's one of its options.
        let r = radios.get(owner).clone();
        let shown = sanitize(r.rate);
        if RATES.contains(&shown.as_str()) {
            radios.rate_shown = shown;
        }
        spawn_ui(&mut commands, &font, &assets, &radios.rate_shown);
        return;
    }
    let r = radios.get(owner).clone();
    let song = r.current.clone();
    if let Ok(mut t) = texts.p0().single_mut()
        && t.0 != song
    {
        t.0 = song;
    }
    // Get_PowerText_Text.
    let power = if r.on { "TURN OFF" } else { "TURN ON" };
    if let Ok(mut t) = texts.p1().single_mut()
        && t.0 != power
    {
        t.0 = power.into();
    }
    let shown = radios.rate_shown.clone();
    if let Ok(mut t) = texts.p2().single_mut()
        && t.0 != shown
    {
        t.0 = shown;
    }
    // Get_PauseImg_Brush: icon_media_88 (play) while paused, else 83.
    if let Ok(mut img) = pause.single_mut() {
        img.image = assets.load(icon(if r.paused { "icon_media/icon_media_88" } else { "icon_media/icon_media_83" }));
    }
    let open = radios.rate_open;
    for mut v in &mut list {
        *v = if open { Visibility::Inherited } else { Visibility::Hidden };
    }
}

fn icon(name: &str) -> String {
    format!("game/SurrounDead/Content/UI/CleanFlatIcons/Textures/png_128/icon/{name}.png")
}

/// RadioUI's tree (widget_tree.py on UI/Widgets/Other/RadioUI).
fn spawn_ui(commands: &mut Commands, font: &MenuFont, assets: &AssetServer, rate: &str) {
    let half = Color::srgba(0.0, 0.0, 0.0, 0.5);
    // NextButton & co: 75 / 85 / 95 % black.
    let dark = ButtonPalette { normal: Color::srgba(0.0, 0.0, 0.0, 0.75), hovered: Color::srgba(0.0, 0.0, 0.0, 0.85), pressed: Color::srgba(0.0, 0.0, 0.0, 0.95), chosen: Color::srgba(0.0, 0.0, 0.0, 0.95) };
    // Image buttons: white / BCBCBC / 898989 tints, no background.
    let clear = ButtonPalette { normal: Color::NONE, hovered: Color::NONE, pressed: Color::NONE, chosen: Color::NONE };
    let root = commands
        .spawn((Node { position_type: PositionType::Absolute, width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, GlobalZIndex(40), DespawnOnExit(AppState::InGame), RadioUi))
        .id();
    // Overlay_1: 700 x 399 anchored to the middle, Alignment (0.5, 1.05).
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                top: Val::Percent(50.0),
                margin: UiRect { left: Val::Px(-350.0), top: Val::Px(-399.0 * 1.05), ..default() },
                width: Val::Px(700.0),
                min_height: Val::Px(399.0),
                padding: UiRect::all(Val::Px(20.0)),
                border_radius: BorderRadius::all(Val::Px(50.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.01, 0.01, 0.01, 0.85)),
            ChildOf(root),
        ))
        .id();
    let column = commands.spawn((Node { flex_direction: FlexDirection::Column, width: Val::Percent(100.0), padding: UiRect::all(Val::Px(20.0)), row_gap: Val::Px(0.0), ..default() }, ChildOf(panel))).id();
    // Border_0: "RADIO" 28, centred.
    commands.spawn((
        Node { align_self: AlignSelf::Center, padding: UiRect::horizontal(Val::Px(20.0)), border_radius: BorderRadius::all(Val::Px(15.0)), ..default() },
        BackgroundColor(half),
        ChildOf(column),
        children![(label(font, "RADIO", 28.0), Node { margin: UiRect::horizontal(Val::Px(20.0)), ..default() })],
    ));
    let row = |commands: &mut Commands, margin: UiRect| {
        commands
            .spawn((Node { margin, align_items: AlignItems::Center, border_radius: BorderRadius::all(Val::Px(5.0)), ..default() }, BackgroundColor(half), ChildOf(column)))
            .id()
    };
    let playing = row(commands, UiRect { left: Val::Px(20.0), top: Val::Px(20.0), right: Val::Px(20.0), bottom: Val::Px(5.0) });
    commands.spawn((label(font, "CURRENTLY PLAYING: ", 16.0), Node { margin: UiRect::left(Val::Px(5.0)), ..default() }, ChildOf(playing)));
    commands.spawn((label(font, "", 12.0), SongText, ChildOf(playing)));
    let volume = row(commands, UiRect { left: Val::Px(20.0), right: Val::Px(20.0), bottom: Val::Px(5.0), ..default() });
    commands.spawn((label(font, "VOLUME: ", 16.0), Node { margin: UiRect::left(Val::Px(5.0)), ..default() }, ChildOf(volume)));
    for (k, name) in ["icon_media/icon_media_12", "icon_media/icon_media_9", "icon_media/icon_media_10", "icon_media/icon_media_11"].into_iter().enumerate() {
        commands.spawn((
            button(Node { margin: UiRect::horizontal(Val::Px(10.0)), width: Val::Px(32.0), height: Val::Px(32.0), ..default() }, clear, RadioButton::Volume(k)),
            ChildOf(volume),
            children![(ImageNode::new(assets.load(icon(name))), Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, Pickable::IGNORE)],
        ));
    }
    let rates = row(commands, UiRect { left: Val::Px(20.0), right: Val::Px(20.0), bottom: Val::Px(5.0), ..default() });
    commands.spawn((label(font, "PLAYBACK RATE: ", 16.0), Node { margin: UiRect::all(Val::Px(5.0)), ..default() }, ChildOf(rates)));
    // ComboBoxString_165: the choice, and its list under it while open.
    let combo = commands
        .spawn((
            button(Node { flex_grow: 1.0, margin: UiRect { left: Val::Px(10.0), top: Val::Px(5.0), right: Val::Px(250.0), bottom: Val::Px(5.0) }, padding: UiRect::horizontal(Val::Px(6.0)), ..default() }, ButtonPalette { normal: Color::srgba(0.0, 0.0, 0.0, 0.1), hovered: Color::srgba(0.0, 0.0, 0.0, 0.1), pressed: Color::srgba(0.0, 0.0, 0.0, 0.1), chosen: Color::srgba(0.0, 0.0, 0.0, 0.1) }, RadioButton::RateBox),
            BorderColor::all(Color::WHITE),
            ChildOf(rates),
        ))
        .id();
    commands.entity(combo).insert(Node { flex_grow: 1.0, margin: UiRect { left: Val::Px(10.0), top: Val::Px(5.0), right: Val::Px(250.0), bottom: Val::Px(5.0) }, padding: UiRect::horizontal(Val::Px(6.0)), border: UiRect::all(Val::Px(1.0)), ..default() });
    commands.spawn((label(font, rate, 14.0), RateText, Pickable::IGNORE, ChildOf(combo)));
    let list = commands
        .spawn((Node { position_type: PositionType::Absolute, top: Val::Percent(100.0), left: Val::Px(0.0), flex_direction: FlexDirection::Column, width: Val::Percent(100.0), ..default() }, BackgroundColor(Color::srgba(0.05, 0.05, 0.05, 0.95)), Visibility::Hidden, GlobalZIndex(41), RateList, ChildOf(combo)))
        .id();
    for (k, r) in RATES.into_iter().enumerate() {
        commands.spawn((button(Node { padding: UiRect::horizontal(Val::Px(6.0)), ..default() }, dark, RadioButton::Rate(k)), ChildOf(list), children![(label(font, r, 14.0), Pickable::IGNORE)]));
    }
    let wide = |top: f32, bottom: f32| Node { margin: UiRect { left: Val::Px(200.0), right: Val::Px(200.0), top: Val::Px(top), bottom: Val::Px(bottom) }, justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() };
    commands.spawn((button(wide(0.0, 0.0), dark, RadioButton::Power), ChildOf(column), children![(label(font, "TURN ON", 18.0), PowerText, Pickable::IGNORE)]));
    commands.spawn((
        button(wide(5.0, 0.0), dark, RadioButton::Pause),
        ChildOf(column),
        children![(ImageNode::new(assets.load(icon("icon_media/icon_media_83"))), Node { width: Val::Px(32.0), height: Val::Px(32.0), ..default() }, PauseImage, Pickable::IGNORE)],
    ));
    commands.spawn((button(wide(5.0, 5.0), dark, RadioButton::Next), ChildOf(column), children![(label(font, "NEXT", 18.0), Pickable::IGNORE)]));
    commands.spawn((button(wide(0.0, 0.0), dark, RadioButton::Previous), ChildOf(column), children![(label(font, "PREVIOUS", 18.0), Pickable::IGNORE)]));
    // Button_430: the close icon, top right.
    commands.spawn((
        button(Node { position_type: PositionType::Absolute, top: Val::Px(20.0), right: Val::Px(20.0), width: Val::Px(32.0), height: Val::Px(32.0), ..default() }, clear, RadioButton::Close),
        ChildOf(panel),
        children![(ImageNode::new(assets.load(icon("icon_common/icon_common_35"))), Node { width: Val::Percent(100.0), height: Val::Percent(100.0), ..default() }, Pickable::IGNORE)],
    ));
}

/// RadioUI's buttons.
fn on_button(ev: On<Activate>, buttons: Query<&RadioButton>, mut overlay: ResMut<GameOverlay>, mut radios: ResMut<Radios>, mut notify: MessageWriter<Notify>) {
    let Ok(&b) = buttons.get(ev.entity) else { return };
    let GameOverlay::Radio(owner) = *overlay else { return };
    press(b, owner, &mut overlay, &mut radios, &mut notify, rand::thread_rng().gen_range(0..usize::MAX));
}

/// A RadioUI button (also the tour's way in).
fn press(b: RadioButton, owner: RadioOwner, overlay: &mut GameOverlay, radios: &mut Radios, notify: &mut MessageWriter<Notify>, roll: usize) {
    let songs = radios.songs.clone();
    match b {
        RadioButton::Close => *overlay = GameOverlay::None,
        RadioButton::Volume(k) => radios.get(owner).volume = VOLUMES[k],
        RadioButton::RateBox => radios.rate_open = !radios.rate_open,
        RadioButton::Rate(k) => {
            radios.rate_open = false;
            let r = radios.get(owner);
            if r.has_player() {
                r.rate = RATES[k].parse().unwrap_or(1.0);
                radios.rate_shown = RATES[k].into();
            } else {
                notify.write(Notify::new("No Music Playing"));
                radios.rate_shown = RATES[3].into();
            }
        }
        RadioButton::Power => {
            let r = radios.get(owner);
            if r.on {
                // StopRadio (which closes the UI), CurrentSong = "".
                r.stop();
                r.current.clear();
                *overlay = GameOverlay::None;
            } else {
                r.start(&songs, roll);
            }
            info!("radio: {owner:?} turned {} ({:?})", if r.on { "on" } else { "off" }, r.playing);
        }
        RadioButton::Pause => {
            let r = radios.get(owner);
            if r.has_player() {
                r.paused = !r.paused;
            }
        }
        RadioButton::Next => {
            radios.get(owner).start(&songs, roll);
        }
        RadioButton::Previous => {
            radios.get(owner).previous();
        }
    }
}

/// The tour's RadioUI clicks, by name.
pub fn tour_press(name: &str, owner: RadioOwner, overlay: &mut GameOverlay, radios: &mut Radios, notify: &mut MessageWriter<Notify>, roll: usize) {
    let b = match name {
        "power" => RadioButton::Power,
        "pause" => RadioButton::Pause,
        "next" => RadioButton::Next,
        "previous" => RadioButton::Previous,
        "close" => RadioButton::Close,
        "mute" => RadioButton::Volume(0),
        "low" => RadioButton::Volume(1),
        "rate 1.5" => RadioButton::Rate(5),
        _ => return,
    };
    press(b, owner, overlay, radios, notify, roll);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> RadioTable {
        RadioTable { rows: HashMap::from([("Two".to_string(), RadioRow { lines: vec!["a".into(), "b".into()], delays: vec![6.0, 3.0] })]) }
    }

    #[test]
    fn the_broadcast_toggles_between_half_and_nothing() {
        assert_eq!(toggle_volume(0.0), 0.5);
        assert_eq!(toggle_volume(0.5), 0.0);
    }

    #[test]
    fn text_lines_take_their_own_delays() {
        let t = table();
        let mut s = TextShow::new(0, "Two", &t);
        assert_eq!(s.text(&t), "a");
        assert!(s.step(5.9, &t));
        assert_eq!(s.text(&t), "a");
        assert!(s.step(0.2, &t));
        assert_eq!(s.text(&t), "b");
        assert!(s.step(2.7, &t));
        assert!(!s.step(0.2, &t), "after the last line's 3 s it ends");
    }

    #[test]
    fn the_radio_starts_stops_and_goes_back() {
        let songs = vec!["a.mp3".to_string(), "b.mp3".to_string()];
        let mut r = MusicRadio::default();
        r.start(&songs, 0);
        assert!(r.on && r.current == "a.mp3" && r.playing.as_deref() == Some("a.mp3") && r.previous.is_empty());
        r.rate = 1.5;
        r.start(&songs, 1);
        assert_eq!((r.current.as_str(), r.previous.as_str(), r.rate), ("b.mp3", "a.mp3", 1.0));
        r.previous();
        assert_eq!(r.playing.as_deref(), Some("a.mp3"));
        assert_eq!(r.current, "b.mp3", "PreviousSongRadio doesn't touch CurrentSong");
        // EndReached_Previous: PreviousSong = CurrentSong, then StartRadio (which copies it again).
        r.ended(&songs, 0);
        assert_eq!((r.current.as_str(), r.previous.as_str(), r.replaying), ("a.mp3", "b.mp3", false));
        r.stop();
        assert!(!r.on && r.playing.is_none() && !r.has_player());
    }

    #[test]
    fn no_songs_turns_it_on_silently() {
        let mut r = MusicRadio::default();
        r.start(&[], 3);
        assert!(r.on && r.playing.is_none() && r.current.is_empty());
        // Nothing before: PreviousSongRadio opens nothing.
        r.previous();
        assert_eq!(r.playing, None);
    }

    #[test]
    fn rates_show_like_ue_strings() {
        assert_eq!(sanitize(1.0), "1.0");
        assert_eq!(sanitize(0.25), "0.25");
        assert!(!RATES.contains(&sanitize(1.0).as_str()), "the box keeps '1' for 1.0");
        assert!(RATES.contains(&sanitize(1.5).as_str()));
    }

    #[test]
    fn songs_are_the_mp3_files() {
        let dir = std::env::temp_dir().join(format!("sd_radio_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for f in ["b.mp3", "a.MP3", "notes.txt"] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        assert_eq!(find_songs(&dir), vec!["a.MP3".to_string(), "b.mp3".to_string()]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_level_text_rows_exist() {
        let t = RadioTable::load();
        for row in ["CoyoteCall", "FireInTheCity", "MilitaryReinforcements", "TheSubway", "ForceFromAfar", "Test"] {
            let r = t.rows.get(row).unwrap_or_else(|| panic!("{row} missing"));
            assert_eq!(r.lines.len(), r.delays.len(), "{row}");
        }
    }
}
