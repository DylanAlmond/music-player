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

    #[error("Failed to seek to position")]
    SeekError(#[from] rodio::source::SeekError),

    #[error("Mutex lock error")]
    LockError,

    #[error("Queue is empty")]
    EmptyQueueError,

    #[error("Index out of bounds")]
    OutOfBoundsError,

    #[error("Failed to emit event")]
    EmitError(#[from] tauri::Error),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub struct AudioState {
    pub queue: Vec<TrackInfo>,
    pub current_index: usize,
    pub duration: Option<u64>,
    pub looped: bool,
    pub handle: AppHandle,
    pub controls: MediaControls,
    pub sender: mpsc::Sender<AudioCommand>,
}

#[derive(Debug, Clone)]
pub enum AudioCommand {
    Queue(Vec<String>),
    Clear,
    Play(usize),
    Pause,
    Resume,
    Prev,
    Next,
    SetPosition(u64),
    SetLooped(bool),
    SetVolume(f32),
}

#[derive(serde::Serialize, Clone)]
#[serde(tag = "type", content = "data")]
enum CommandResponse {
    Queue(Vec<TrackInfo>),
    Play { index: usize, track: TrackInfo },
    Status(String),
    Position(u64),
    Looped(bool),
    Volume(f32),
}

#[derive(serde::Serialize, Clone)]
struct Callback<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct TrackInfo {
    pub index: usize,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: u64,
    pub path: String,
}

#[derive(Clone)]
pub struct AudioPlayer {
    sender: mpsc::Sender<AudioCommand>,
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
                sender: sender,
            };

            let mut last_emit_time = std::time::Instant::now();
            let emit_interval = Duration::from_millis(500);

            loop {
                if let Ok(command) = receiver.try_recv() {
                    println!("Handling audio command...");
                    Self::handle_audio_command(command, &mut state, &sink);
                }

                if !sink.empty() && !sink.is_paused() {
                    Self::track_progress(
                        &sink,
                        &mut state,
                        &app_handle,
                        &mut last_emit_time,
                        emit_interval,
                    );
                }

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
        let (event_name, result): (&str, Result<CommandResponse, AudioError>) = match command {
            AudioCommand::Queue(file_paths) => {
                let mut i: usize = state.queue.len();
                for path in file_paths {
                    let track_info = get_track_info_from_path(&path, i);
                    state.queue.push(track_info);
                    i += 1;
                }

                ("queue", Ok(CommandResponse::Queue(state.queue.clone())))
            }
            AudioCommand::Play(index) => {
                match play_track(&state.queue[index].clone(), &sink, state) {
                    Ok(_) => {
                        state.current_index = index;
                        (
                            "play",
                            Ok(CommandResponse::Play {
                                index,
                                track: state.queue[index].clone(),
                            }),
                        )
                    }
                    Err(e) => ("play", Err(e)),
                }
            }
            AudioCommand::Prev => {
                let track = if state.queue.is_empty() {
                    Err(AudioError::EmptyQueueError)
                } else {
                    if state.current_index > 0 && sink.get_pos().as_secs() < 5 {
                        state.current_index -= 1;
                        Ok(state.queue[state.current_index].clone())
                    } else {
                        Ok(state.queue[state.current_index].clone())
                    }
                };

                match track {
                    Ok(t) => match play_track(&t, &sink, state) {
                        Ok(_) => (
                            "play",
                            Ok(CommandResponse::Play {
                                index: state.current_index,
                                track: t,
                            }),
                        ),
                        Err(e) => ("play", Err(e)),
                    },
                    Err(e) => ("play", Err(e)),
                }
            }
            AudioCommand::Next => {
                let track = if state.queue.is_empty() {
                    Err(AudioError::EmptyQueueError)
                } else {
                    if state.current_index < state.queue.len() - 1 {
                        state.current_index += 1;
                        Ok(state.queue[state.current_index].clone())
                    } else {
                        if state.looped {
                            state.current_index = 0;
                            Ok(state.queue[state.current_index].clone())
                        } else {
                            Err(AudioError::OutOfBoundsError)
                        }
                    }
                };

                match track {
                    Ok(t) => match play_track(&t, &sink, state) {
                        Ok(_) => (
                            "play",
                            Ok(CommandResponse::Play {
                                index: state.current_index,
                                track: t,
                            }),
                        ),
                        Err(e) => ("play", Err(e)),
                    },
                    Err(e) => ("play", Err(e)),
                }
            }
            AudioCommand::Pause => {
                sink.pause();

                state
                    .controls
                    .set_playback(MediaPlayback::Paused { progress: None })
                    .unwrap();
                ("status", Ok(CommandResponse::Status("paused".to_string())))
            }
            AudioCommand::Resume => {
                if state.queue.is_empty() {
                    ("play", Err(AudioError::EmptyQueueError))
                } else {
                    let playback_result = if sink.empty() {
                        match play_track(&state.queue[0].clone(), &sink, state) {
                            Ok(_) => Ok(CommandResponse::Play {
                                index: 0,
                                track: state.queue[0].clone(),
                            }),
                            Err(e) => Err(e),
                        }
                    } else {
                        sink.play();

                        state
                            .controls
                            .set_playback(MediaPlayback::Playing { progress: None })
                            .unwrap();

                        Ok(CommandResponse::Play {
                            index: state.current_index,
                            track: state.queue[state.current_index].clone(),
                        })
                    };

                    ("play", playback_result)
                }
            }
            AudioCommand::SetPosition(position) => {
                match sink.try_seek(Duration::from_secs(position)) {
                    Ok(_) => (
                        "position",
                        Ok(CommandResponse::Position(sink.get_pos().as_secs())),
                    ),
                    Err(e) => ("position", Err(AudioError::SeekError(e))),
                }
            }
            AudioCommand::SetLooped(looped) => {
                state.looped = looped;
                ("looped", Ok(CommandResponse::Looped(state.looped)))
            }
            AudioCommand::SetVolume(volume) => {
                sink.set_volume(volume);
                ("volume", Ok(CommandResponse::Volume(sink.volume())))
            }
            AudioCommand::Clear => {
                sink.stop();
                state.queue.clear();
                state.current_index = 0;

                state.controls.set_playback(MediaPlayback::Stopped).unwrap();

                ("queue", Ok(CommandResponse::Queue(state.queue.clone())))
            }
        };

        let emit_result = match result {
            Ok(data) => state.handle.emit(
                event_name,
                Callback {
                    success: true,
                    data: Some(data),
                    error: None,
                },
            ),
            Err(err) => state.handle.emit(
                event_name,
                Callback::<CommandResponse> {
                    success: false,
                    data: None,
                    error: Some(err.to_string()),
                },
            ),
        };

        if let Err(e) = emit_result {
            eprintln!("{}", AudioError::EmitError(e));
        }
    }

    fn track_progress(
        sink: &Sink,
        state: &mut AudioState,
        app_handle: &AppHandle,
        last_emit_time: &mut std::time::Instant,
        interval: Duration,
    ) {
        if sink.get_pos().as_secs() >= state.duration.unwrap_or(0) {
            if state.queue.is_empty() {
                //
            } else {
                if state.current_index < state.queue.len() - 1 {
                    state.current_index += 1;
                    let _ = state.sender.send(AudioCommand::Play(state.current_index));
                } else {
                    if state.looped {
                        state.current_index = 0;
                        let _ = state.sender.send(AudioCommand::Play(state.current_index));
                    } else {
                        let _ = state.sender.send(AudioCommand::Pause);
                    }
                }
            };
        }

        if !sink.is_paused() && !sink.empty() && last_emit_time.elapsed() >= interval {
            if let Err(e) = app_handle.emit(
                "position",
                Callback {
                    success: true,
                    data: Some(CommandResponse::Position(sink.get_pos().as_secs())),
                    error: None,
                },
            ) {
                eprintln!("{}", AudioError::EmitError(e));
            }
            *last_emit_time = std::time::Instant::now();
        }
    }

    pub fn add_queue(&self, file_paths: Vec<String>) -> Result<(), AudioError> {
        match self.sender.send(AudioCommand::Queue(file_paths)) {
            Ok(_) => Ok(()),
            Err(_) => Err(AudioError::LockError),
        }
    }

    pub fn clear_queue(&self) -> Result<(), AudioError> {
        match self.sender.send(AudioCommand::Clear) {
            Ok(_) => Ok(()),
            Err(_) => Err(AudioError::LockError),
        }
    }

    pub fn play(&self, index: usize) -> Result<(), AudioError> {
        match self.sender.send(AudioCommand::Play(index)) {
            Ok(_) => Ok(()),
            Err(_) => Err(AudioError::LockError),
        }
    }

    pub fn pause(&self) -> Result<(), AudioError> {
        match self.sender.send(AudioCommand::Pause) {
            Ok(_) => Ok(()),
            Err(_) => Err(AudioError::LockError),
        }
    }

    pub fn resume(&self) -> Result<(), AudioError> {
        match self.sender.send(AudioCommand::Resume) {
            Ok(_) => Ok(()),
            Err(_) => Err(AudioError::LockError),
        }
    }

    pub fn next(&self) -> Result<(), AudioError> {
        match self.sender.send(AudioCommand::Next) {
            Ok(_) => Ok(()),
            Err(_) => Err(AudioError::LockError),
        }
    }

    pub fn prev(&self) -> Result<(), AudioError> {
        match self.sender.send(AudioCommand::Prev) {
            Ok(_) => Ok(()),
            Err(_) => Err(AudioError::LockError),
        }
    }

    pub fn set_position(&self, position: u64) -> Result<(), AudioError> {
        match self.sender.send(AudioCommand::SetPosition(position)) {
            Ok(_) => Ok(()),
            Err(_) => Err(AudioError::LockError),
        }
    }

    pub fn set_looped(&self, looped: bool) -> Result<(), AudioError> {
        match self.sender.send(AudioCommand::SetLooped(looped)) {
            Ok(_) => Ok(()),
            Err(_) => Err(AudioError::LockError),
        }
    }

    pub fn set_volume(&self, volume: f32) -> Result<(), AudioError> {
        match self.sender.send(AudioCommand::SetVolume(volume)) {
            Ok(_) => Ok(()),
            Err(_) => Err(AudioError::LockError),
        }
    }
}
