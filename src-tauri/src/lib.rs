use lofty::file::{AudioFile, TaggedFileExt};
use lofty::read_from_path;
use lofty::tag::Accessor;
use rodio::{Decoder, OutputStream, Sink};
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, PlatformConfig};
use std::fs::File;
use std::io::{self, BufReader};
use std::sync::mpsc;
use std::time::Duration;
use std::{thread, vec};
use tauri::{AppHandle, Emitter, Manager, State};
use thiserror::Error;

/// Custom error type for the audio player
#[derive(Error, Debug)]
enum AudioError {
    #[error("Failed to create audio output stream")]
    StreamError(#[from] rodio::StreamError),

    #[error("Failed to create audio sink")]
    SinkError(#[from] rodio::PlayError),

    #[error("I/O error: {0}")]
    IoError(#[from] io::Error),

    #[error("Failed to decode audio file")]
    DecoderError(#[from] rodio::decoder::DecoderError),

    #[error("Mutex lock error")]
    LockError,
}

#[derive(Clone)]
struct AudioPlayer {
    sender: mpsc::Sender<AudioCommand>,
}

struct AudioState {
    queue: Vec<TrackInfo>,
    current_index: usize,
    duration: Option<u64>,
    looped: bool,
    handle: AppHandle,
    controls: MediaControls,
}

enum AudioCommand {
    Queue(Vec<String>, bool),
    Play(String),
    Pause,
    Resume,
    Stop,
    Prev,
    Next,
    SetPosition(u64),
    SetLooped(bool),
    SetVolume(f32),
    GetQueue(mpsc::Sender<Vec<TrackInfo>>),
}

#[derive(serde::Serialize, Clone, Debug)]
struct TrackInfo {
    title: String,
    artist: String,
    album: String,
    duration: u64,
    path: String,
}

#[derive(serde::Serialize, Clone)]
struct TrackProgress {
    position: u64,
    duration: u64,
}

impl AudioPlayer {
    fn new(app_handle: AppHandle) -> AudioPlayer {
        let (sender, receiver) = mpsc::channel();
        let sender_clone = sender.clone();

        thread::spawn(move || {
            let (_stream, stream_handle) = OutputStream::try_default().unwrap();
            let sink = Sink::try_new(&stream_handle).unwrap();

            #[cfg(not(target_os = "windows"))]
            let hwnd = None;

            #[cfg(target_os = "windows")]
            let hwnd = {
                let window = app_handle.get_webview_window("main").unwrap();
                let hwnd = window.hwnd().unwrap();

                Some(hwnd.0)
            };

            let config = PlatformConfig {
                dbus_name: "my_player",
                display_name: "My Player",
                hwnd: hwnd,
            };

            let mut state = AudioState {
                queue: Vec::new(),
                current_index: 0,
                duration: None,
                looped: false,
                handle: app_handle.clone(),
                controls: MediaControls::new(config).unwrap(),
            };

            // The closure must be Send and have a static lifetime.
            state
                .controls
                .attach({
                    move |event: MediaControlEvent| match event {
                        MediaControlEvent::Play => {
                            sender_clone.send(AudioCommand::Resume).unwrap();
                        }
                        MediaControlEvent::Pause => {
                            sender_clone.send(AudioCommand::Pause).unwrap();
                        }
                        MediaControlEvent::Toggle => todo!(),
                        MediaControlEvent::Next => {
                            sender_clone.send(AudioCommand::Next).unwrap();
                        }
                        MediaControlEvent::Previous => {
                            sender_clone.send(AudioCommand::Prev).unwrap();
                        }
                        MediaControlEvent::Stop => todo!(),
                        MediaControlEvent::Seek(_seek_direction) => todo!(),
                        MediaControlEvent::SeekBy(_seek_direction, _duration) => todo!(),
                        MediaControlEvent::SetPosition(media_position) => {
                            sender_clone
                                .send(AudioCommand::SetPosition(media_position.0.as_secs()))
                                .unwrap();
                        }
                        MediaControlEvent::SetVolume(volume) => {
                            sender_clone
                                .send(AudioCommand::SetVolume(volume as f32))
                                .unwrap();
                        }
                        MediaControlEvent::OpenUri(_) => todo!(),
                        MediaControlEvent::Raise => todo!(),
                        MediaControlEvent::Quit => todo!(),
                    }
                })
                .unwrap();

            let mut last_emit_time = std::time::Instant::now();
            let emit_interval = Duration::from_millis(500);

            loop {
                if let Ok(command) = receiver.try_recv() {
                    match command {
                        AudioCommand::Queue(file_paths, _looped) => {
                            state.queue.clear();
                            for path in file_paths {
                                let track_info = get_track_info_from_path(&path);
                                state.queue.push(track_info);
                            }

                            state.current_index = 0;

                            if let Err(e) = play_track(
                                &state.queue[state.current_index].clone(),
                                &sink,
                                &mut state,
                            ) {
                                eprintln!("Error playing track: {}", e);
                            }
                        }
                        AudioCommand::Play(path) => {
                            state.queue.clear();
                            let track_info = get_track_info_from_path(&path);
                            state.queue.push(track_info);
                            state.current_index = 0;

                            if let Err(e) = play_track(
                                &state.queue[state.current_index].clone(),
                                &sink,
                                &mut state,
                            ) {
                                eprintln!("Error playing track: {}", e);
                            }
                        }
                        AudioCommand::Prev => {
                            if state.current_index > 0 && sink.get_pos().as_secs() < 5 {
                                state.current_index -= 1;
                                if let Err(e) = play_track(
                                    &state.queue[state.current_index].clone(),
                                    &sink,
                                    &mut state,
                                ) {
                                    eprintln!("Error playing track: {}", e);
                                }
                            } else {
                                if let Err(e) = sink.try_seek(Duration::from_secs(0)) {
                                    eprintln!("Error playing track: {}", e);
                                }
                            }
                        }
                        AudioCommand::Next => {
                            if state.current_index < state.queue.len() - 1 {
                                state.current_index += 1;
                                if let Err(e) = play_track(
                                    &state.queue[state.current_index].clone(),
                                    &sink,
                                    &mut state,
                                ) {
                                    eprintln!("Error playing track: {}", e);
                                }
                            }
                        }
                        AudioCommand::Pause => {
                            sink.pause();
                            state
                                .controls
                                .set_playback(MediaPlayback::Paused { progress: None })
                                .unwrap();
                        }
                        AudioCommand::Resume => {
                            sink.play();
                            state
                                .controls
                                .set_playback(MediaPlayback::Playing { progress: None })
                                .unwrap();
                        }
                        AudioCommand::Stop => sink.stop(),
                        AudioCommand::SetPosition(position) => {
                            println!("Seeking to: {}", position);
                            if let Err(e) = sink.try_seek(Duration::from_secs(position)) {
                                eprintln!("Error playing track: {}", e);
                            }
                        }
                        AudioCommand::SetLooped(looped) => {
                            println!("Setting looping: {}", looped);
                            state.looped = looped;
                        }
                        AudioCommand::SetVolume(volume) => {
                            println!("Setting volume: {}", volume);
                            sink.set_volume(volume);
                        }
                        AudioCommand::GetQueue(response_tx) => {
                            let _ = response_tx.send(state.queue.clone());
                        }
                    }
                }

                if !sink.empty() && sink.get_pos().as_secs() == state.duration.unwrap_or(0) {
                    if state.current_index < state.queue.len() - 1 {
                        state.current_index += 1;

                        let track_path = state.queue[state.current_index].clone();
                        if let Err(e) = play_track(&track_path, &sink, &mut state) {
                            eprintln!("Error playing next track: {}", e);
                        }
                    } else if state.looped {
                        state.current_index = 0;

                        let track_path = state.queue[state.current_index].clone();
                        if let Err(e) = play_track(&track_path, &sink, &mut state) {
                            eprintln!("Error playing next track: {}", e);
                        }
                    } else {
                        sink.pause();
                        state
                            .controls
                            .set_playback(MediaPlayback::Paused { progress: None })
                            .unwrap();
                    }
                }

                if !sink.is_paused() && !sink.empty() && last_emit_time.elapsed() >= emit_interval {
                    // println!("Emitting track progress");
                    if let Err(e) = app_handle.emit(
                        "track-progress",
                        TrackProgress {
                            position: sink.get_pos().as_secs(),
                            duration: state.duration.unwrap_or(0),
                        },
                    ) {
                        eprintln!("Error sending track progress: {}", e);
                    };

                    last_emit_time = std::time::Instant::now();
                }

                thread::sleep(Duration::from_millis(10));
            }
        });
        AudioPlayer { sender: sender }
    }

    fn get_queue(&self) -> Vec<TrackInfo> {
        let (resp_tx, resp_rx) = mpsc::channel();
        self.sender.send(AudioCommand::GetQueue(resp_tx)).unwrap();
        resp_rx.recv().unwrap_or_else(|_| Vec::new())
    }
}

fn get_track_info_from_path(path: &str) -> TrackInfo {
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
            title: title,
            album: album,
            artist: artist,
            duration: duration,
            path: path.to_string(),
        }
    } else {
        TrackInfo {
            title: "Unknown Track".to_string(),
            album: "Unknown Album".to_string(),
            artist: "Unknown Artist".to_string(),
            duration: 0,
            path: path.to_string(),
        }
    }
}

fn play_track(
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

#[tauri::command]
fn set_position(state: State<AppState>, position: u64) -> Result<(), String> {
    match state
        .audio_player
        .sender
        .send(AudioCommand::SetPosition(position))
    {
        Ok(_) => Ok(()),
        Err(_) => Err(AudioError::LockError.to_string()),
    }
}

#[tauri::command]
fn set_looped(state: State<AppState>, looped: bool) -> Result<(), String> {
    match state
        .audio_player
        .sender
        .send(AudioCommand::SetLooped(looped))
    {
        Ok(_) => Ok(()),
        Err(_) => Err(AudioError::LockError.to_string()),
    }
}

#[tauri::command]
fn set_volume(state: State<AppState>, volume: f32) -> Result<(), String> {
    match state
        .audio_player
        .sender
        .send(AudioCommand::SetVolume(volume))
    {
        Ok(_) => Ok(()),
        Err(_) => Err(AudioError::LockError.to_string()),
    }
}

#[tauri::command]
fn play(state: State<AppState>, file_path: String) -> Result<Vec<TrackInfo>, String> {
    match state
        .audio_player
        .sender
        .send(AudioCommand::Play(file_path))
    {
        Ok(_) => {
            let queue = state.audio_player.get_queue();
            Ok(queue)
        }
        Err(_) => Err(AudioError::LockError.to_string()),
    }
}

#[tauri::command]
fn play_queue(state: State<AppState>, file_paths: Vec<String>) -> Result<Vec<TrackInfo>, String> {
    match state
        .audio_player
        .sender
        .send(AudioCommand::Queue(file_paths, false))
    {
        Ok(_) => {
            let queue = state.audio_player.get_queue();
            Ok(queue)
        }
        Err(_) => Err(AudioError::LockError.to_string()),
    }
}

#[tauri::command]
fn pause(state: State<AppState>) -> Result<(), String> {
    match state.audio_player.sender.send(AudioCommand::Pause) {
        Ok(_) => Ok(()),
        Err(_) => Err(AudioError::LockError.to_string()),
    }
}

#[tauri::command]
fn resume(state: State<AppState>) -> Result<(), String> {
    match state.audio_player.sender.send(AudioCommand::Resume) {
        Ok(_) => Ok(()),
        Err(_) => Err(AudioError::LockError.to_string()),
    }
}

#[tauri::command]
fn stop(state: State<AppState>) -> Result<(), String> {
    match state.audio_player.sender.send(AudioCommand::Stop) {
        Ok(_) => Ok(()),
        Err(_) => Err(AudioError::LockError.to_string()),
    }
}

#[tauri::command]
fn prev(state: State<AppState>) -> Result<(), String> {
    match state.audio_player.sender.send(AudioCommand::Prev) {
        Ok(_) => Ok(()),
        Err(_) => Err(AudioError::LockError.to_string()),
    }
}

#[tauri::command]
fn next(state: State<AppState>) -> Result<(), String> {
    match state.audio_player.sender.send(AudioCommand::Next) {
        Ok(_) => Ok(()),
        Err(_) => Err(AudioError::LockError.to_string()),
    }
}

struct AppState {
    audio_player: AudioPlayer,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle();

            app.manage(AppState {
                audio_player: AudioPlayer::new(handle.clone()),
            });

            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            play,
            play_queue,
            pause,
            resume,
            stop,
            prev,
            next,
            set_position,
            set_looped,
            set_volume,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
