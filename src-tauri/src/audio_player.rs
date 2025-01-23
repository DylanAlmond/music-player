use rodio::{OutputStream, Sink};
use souvlaki::{MediaControlEvent, MediaControls, MediaPlayback, PlatformConfig};
use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use thiserror::Error;

use crate::util;
use util::{get_track_info_from_path, play_track};

#[derive(Error, Debug)]
pub enum AudioError {
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

pub struct AudioState {
    pub queue: Vec<TrackInfo>,
    pub current_index: usize,
    pub duration: Option<u64>,
    pub looped: bool,
    pub handle: AppHandle,
    pub controls: MediaControls,
}

#[derive(Debug, Clone)]
pub enum AudioCommand {
    Queue(Vec<String>, bool),
    Pause,
    Resume,
    Prev,
    Next,
    SetPosition(u64),
    SetLooped(bool),
    SetVolume(f32),
    GetQueue(mpsc::Sender<Vec<TrackInfo>>),
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct TrackInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: u64,
    pub path: String,
}

#[derive(serde::Serialize, Clone)]
pub struct TrackProgress {
    pub position: u64,
    pub duration: u64,
}

#[derive(Clone)]
pub struct AudioPlayer {
    pub sender: mpsc::Sender<AudioCommand>,
}

impl AudioPlayer {
    pub fn new(app_handle: AppHandle) -> AudioPlayer {
        let (sender, receiver) = mpsc::channel();
        let sender_clone = sender.clone();

        Self::spawn_audio_thread(app_handle, receiver, sender_clone);

        AudioPlayer { sender: sender }
    }

    fn spawn_audio_thread(
        app_handle: AppHandle,
        receiver: mpsc::Receiver<AudioCommand>,
        sender: mpsc::Sender<AudioCommand>,
    ) {
        thread::spawn(move || {
            let (_stream, stream_handle) = OutputStream::try_default().unwrap();
            let sink = Sink::try_new(&stream_handle).unwrap();
            let controls = Self::setup_media_controls(&app_handle, sender.clone()).unwrap();

            let mut state = AudioState {
                queue: Vec::new(),
                current_index: 0,
                duration: None,
                looped: false,
                handle: app_handle.clone(),
                controls: controls,
            };

            let mut last_emit_time = std::time::Instant::now();
            let emit_interval = Duration::from_millis(500);

            loop {
                if let Ok(command) = receiver.try_recv() {
                    Self::handle_audio_command(command, &mut state, &sink);
                }

                Self::track_progress(
                    &sink,
                    &mut state,
                    &app_handle,
                    &mut last_emit_time,
                    emit_interval,
                );

                thread::sleep(Duration::from_millis(10));
            }
        });
    }

    fn setup_media_controls(
        app_handle: &AppHandle,
        sender: mpsc::Sender<AudioCommand>,
    ) -> Result<MediaControls, souvlaki::Error> {
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
            hwnd,
        };

        let mut controls = MediaControls::new(config)?;
        controls
            .attach(move |event| Self::handle_media_event(event, &sender))
            .unwrap();
        Ok(controls)
    }

    fn handle_media_event(event: MediaControlEvent, sender: &mpsc::Sender<AudioCommand>) {
        match event {
            MediaControlEvent::Play => sender.send(AudioCommand::Resume).unwrap(),
            MediaControlEvent::Pause => sender.send(AudioCommand::Pause).unwrap(),
            MediaControlEvent::Next => sender.send(AudioCommand::Next).unwrap(),
            MediaControlEvent::Previous => sender.send(AudioCommand::Prev).unwrap(),
            MediaControlEvent::Stop => sender.send(AudioCommand::Pause).unwrap(),
            _ => {}
        }
    }

    fn handle_audio_command(command: AudioCommand, state: &mut AudioState, sink: &Sink) {
        match command {
            AudioCommand::Queue(file_paths, _looped) => {
                state.queue.clear();
                for path in file_paths {
                    let track_info = get_track_info_from_path(&path);
                    state.queue.push(track_info);
                }

                state.current_index = 0;

                if let Err(e) = play_track(&state.queue[state.current_index].clone(), &sink, state)
                {
                    eprintln!("Error playing track: {}", e);
                }
            }
            AudioCommand::Prev => {
                if state.current_index > 0 && sink.get_pos().as_secs() < 5 {
                    state.current_index -= 1;
                    if let Err(e) =
                        play_track(&state.queue[state.current_index].clone(), &sink, state)
                    {
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
                    if let Err(e) =
                        play_track(&state.queue[state.current_index].clone(), &sink, state)
                    {
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

    fn track_progress(
        sink: &Sink,
        state: &mut AudioState,
        app_handle: &AppHandle,
        last_emit_time: &mut std::time::Instant,
        interval: Duration,
    ) {
        if !sink.empty() && sink.get_pos().as_secs() >= state.duration.unwrap_or(0) {
            let next_index = if state.current_index < state.queue.len() - 1 {
                state.current_index + 1
            } else if state.looped {
                0
            } else {
                sink.pause();
                state
                    .controls
                    .set_playback(MediaPlayback::Paused { progress: None })
                    .unwrap();
                return;
            };

            state.current_index = next_index;

            if let Err(e) = play_track(&state.queue[state.current_index].clone(), sink, state) {
                eprintln!("Error playing next track: {}", e);
            }
        }

        if !sink.is_paused() && !sink.empty() && last_emit_time.elapsed() >= interval {
            if let Err(e) = app_handle.emit(
                "track-progress",
                TrackProgress {
                    position: sink.get_pos().as_secs(),
                    duration: state.duration.unwrap_or(0),
                },
            ) {
                eprintln!("Error sending track progress: {}", e);
            }
            *last_emit_time = std::time::Instant::now();
        }
    }

    pub fn play_queue(&self, file_paths: Vec<String>) -> Result<Vec<TrackInfo>, String> {
        match self.sender.send(AudioCommand::Queue(file_paths, false)) {
            Ok(_) => {
                let queue = self.get_queue();
                Ok(queue)
            }
            Err(_) => Err(AudioError::LockError.to_string()),
        }
    }

    pub fn pause(&self) -> Result<(), String> {
        match self.sender.send(AudioCommand::Pause) {
            Ok(_) => Ok(()),
            Err(_) => Err(AudioError::LockError.to_string()),
        }
    }

    pub fn resume(&self) -> Result<(), String> {
        match self.sender.send(AudioCommand::Resume) {
            Ok(_) => Ok(()),
            Err(_) => Err(AudioError::LockError.to_string()),
        }
    }

    pub fn next(&self) -> Result<(), String> {
        match self.sender.send(AudioCommand::Next) {
            Ok(_) => Ok(()),
            Err(_) => Err(AudioError::LockError.to_string()),
        }
    }

    pub fn prev(&self) -> Result<(), String> {
        match self.sender.send(AudioCommand::Prev) {
            Ok(_) => Ok(()),
            Err(_) => Err(AudioError::LockError.to_string()),
        }
    }

    pub fn set_position(&self, position: u64) -> Result<(), String> {
        match self.sender.send(AudioCommand::SetPosition(position)) {
            Ok(_) => Ok(()),
            Err(_) => Err(AudioError::LockError.to_string()),
        }
    }

    pub fn set_looped(&self, looped: bool) -> Result<(), String> {
        match self.sender.send(AudioCommand::SetLooped(looped)) {
            Ok(_) => Ok(()),
            Err(_) => Err(AudioError::LockError.to_string()),
        }
    }

    pub fn set_volume(&self, volume: f32) -> Result<(), String> {
        match self.sender.send(AudioCommand::SetVolume(volume)) {
            Ok(_) => Ok(()),
            Err(_) => Err(AudioError::LockError.to_string()),
        }
    }

    pub fn get_queue(&self) -> Vec<TrackInfo> {
        let (resp_tx, resp_rx) = mpsc::channel();
        if self.sender.send(AudioCommand::GetQueue(resp_tx)).is_ok() {
            resp_rx.recv().unwrap_or_else(|_| Vec::new())
        } else {
            Vec::new()
        }
    }
}
