//! Day and night ambience: BP_AmbientSoundController (decompiled), one in the level.
//!
//! - A 2D loop: DaySound (birds__1__Cue) from sunrise, NightSound
//!   (night_and_crickets_ambient_loop_Cue) from sunset.
//! - Its VolumeMultiplier ("Update Volume Multiplier"): Daytime / Nighttime Volume Multiplier (0.3
//!   both) x the weather's: rain 0.1, snow 0.05, dust and fog (fog above 7.5) 0 (their multipliers
//!   aren't set on the class), otherwise 1.
//! - UDW's raining / snowing / dusty events fire at some threshold of the weather values; ours is
//!   above 1 on UDS's 0-10 scale (a guess).

use bevy::audio::{PlaybackMode, Volume};
use bevy::prelude::*;
use rand::Rng;

use crate::audio::{SoundDb, class_volume, pick};
use crate::menus::AppState;
use crate::menus::settings::SettingsStore;

const DAY_SOUND: &str = "birds__1__Cue";
const NIGHT_SOUND: &str = "night_and_crickets_ambient_loop_Cue";
const DAY_VOLUME: f32 = 0.3;
const NIGHT_VOLUME: f32 = 0.3;

/// The playing ambience loop and whether it's the day one.
#[derive(Component)]
struct Ambience {
    day: bool,
    cue_volume: f32,
}

pub fn plugin(app: &mut App) {
    app.add_systems(Update, ambience.run_if(in_state(AppState::InGame)));
}

/// The ambience's volume multiplier for the weather (Update Volume Multiplier's selects: dust,
/// then snow, then rain, then fog).
pub fn weather_volume(w: &super::weather::WeatherValues) -> f32 {
    let on = |v: f32| v > 1.0;
    if on(w.dust) {
        0.0
    } else if on(w.snow) {
        0.05
    } else if on(w.rain) {
        0.1
    } else if w.fog > 7.5 {
        0.0
    } else {
        1.0
    }
}

#[allow(clippy::too_many_arguments)]
fn ambience(
    mut commands: Commands,
    db: Res<SoundDb>,
    store: Res<SettingsStore>,
    assets: Res<AssetServer>,
    game: Res<crate::menus::CurrentGame>,
    weather: Option<Res<super::weather::Weather>>,
    mut playing: Query<(Entity, &Ambience, Option<&mut AudioSink>)>,
    mut played: ResMut<crate::audio::Played>,
) {
    let Some(state) = game.state.as_ref() else { return };
    let hour = state.environment.time_of_day_hours;
    let day = (super::daynight::DAWN..super::daynight::DUSK).contains(&hour);
    let weather = weather.map_or(1.0, |w| weather_volume(&w.0));
    let cue_name = if day { DAY_SOUND } else { NIGHT_SOUND };
    let Some(cue) = db.cues.get(cue_name) else { return };
    let target = (if day { DAY_VOLUME } else { NIGHT_VOLUME }) * weather * class_volume(&db, &store, cue.class.as_deref());
    let mut have = false;
    for (e, a, sink) in &mut playing {
        if a.day != day {
            // Sunrise / Sunset: SetSound swaps the loop.
            commands.entity(e).despawn();
            continue;
        }
        have = true;
        if let Some(mut sink) = sink {
            sink.set_volume(Volume::Linear(a.cue_volume * target));
        }
    }
    if have {
        return;
    }
    let Some(v) = pick(cue, rand::thread_rng().gen_range(0.0..1.0)) else { return };
    let cue_volume = cue.volume;
    for w in &v.waves {
        commands.spawn((
            AudioPlayer::new(assets.load::<AudioSource>(w.clone())),
            PlaybackSettings { mode: PlaybackMode::Loop, volume: Volume::Linear(cue_volume * target), ..default() },
            Ambience { day, cue_volume },
            DespawnOnExit(AppState::InGame),
        ));
    }
    *played.0.entry(cue_name.to_string()).or_insert(0) += 1;
    info!("ambience: {cue_name}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay::weather::WeatherValues;

    #[test]
    fn weather_quietens_the_ambience() {
        let clear = WeatherValues::default();
        assert_eq!(weather_volume(&clear), 1.0);
        assert_eq!(weather_volume(&WeatherValues { rain: 5.0, ..clear }), 0.1);
        assert_eq!(weather_volume(&WeatherValues { snow: 5.0, rain: 5.0, ..clear }), 0.05);
        assert_eq!(weather_volume(&WeatherValues { dust: 5.0, ..clear }), 0.0);
        assert_eq!(weather_volume(&WeatherValues { fog: 8.0, ..clear }), 0.0);
    }
}
