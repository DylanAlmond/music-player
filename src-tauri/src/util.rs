use lofty::file::{AudioFile, TaggedFileExt};
use lofty::read_from_path;
use lofty::tag::Accessor;
use rodio::{Decoder, Sink};
use souvlaki::{MediaMetadata, MediaPlayback};
use std::fs::File;
use std::io::BufReader;
use std::time::Duration;
use tauri::Emitter;

use crate::audio_player;
use audio_player::{AudioError, AudioState, TrackInfo};

pub fn get_track_info_from_path(path: &str, index: usize) -> TrackInfo {
    if let Ok(tagged_file) = read_from_path(path) {
        let tag = tagged_file.primary_tag();
        let title = tag
            .and_then(|t| t.title().map(|s| s.into_owned()))
            .unwrap_or_else(|| "Unknown Title".to_string());
        let album = tag
            .and_then(|t| t.album().map(|s| s.into_owned()))
            .unwrap_or_else(|| "Unknown Title".to_string());
        let artist = tag
            .and_then(|t| t.artist().map(|s| s.into_owned()))
            .unwrap_or_else(|| "Unknown Title".to_string());

        let duration = tagged_file.properties().duration().as_secs();

        TrackInfo {
            index: index,
            title: title,
            album: album,
            artist: artist,
            duration: duration,
            path: path.to_string(),
        }
    } else {
        TrackInfo {
            index: index,
            title: "Unknown Track".to_string(),
            album: "Unknown Album".to_string(),
            artist: "Unknown Artist".to_string(),
            duration: 0,
            path: path.to_string(),
        }
    }
}

pub fn play_track(
    track_info: &TrackInfo,
    sink: &Sink,
    state: &mut AudioState,
) -> Result<(), AudioError> {
    sink.clear();
    println!("Playing track: {:?}", track_info);

    let file = File::open(&track_info.path)?;
    let source = Decoder::new(BufReader::new(file))?;

    sink.append(source);
    sink.play();

    state.duration = Some(track_info.duration);

    state
        .controls
        .set_metadata(MediaMetadata {
            title: Some(track_info.title.as_str()),
            artist: Some(track_info.artist.as_str()),
            album: Some(track_info.album.as_str()),
            duration: Some(Duration::from_secs(track_info.duration)),
            ..Default::default()
        })
        .unwrap();

    state.handle.emit("track-change", &track_info).unwrap();

    state
        .controls
        .set_playback(MediaPlayback::Playing { progress: None })
        .unwrap();

    Ok(())
}
