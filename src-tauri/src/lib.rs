use std::vec;
use tauri::{Manager, State};

mod audio_player;
mod util;
use audio_player::{AudioPlayer, TrackInfo};

#[tauri::command]
fn add_queue(state: State<AppState>, file_paths: Vec<String>) -> Result<Vec<TrackInfo>, String> {
    state.audio_player.add_queue(file_paths)
}

#[tauri::command]
fn clear_queue(state: State<AppState>) -> Result<(), String> {
    state.audio_player.clear_queue()
}

#[tauri::command]
fn play(state: State<AppState>, index: usize) -> Result<(), String> {
    state.audio_player.play(index)
}

#[tauri::command]
fn pause(state: State<AppState>) -> Result<(), String> {
    state.audio_player.pause()
}

#[tauri::command]
fn resume(state: State<AppState>) -> Result<(), String> {
    state.audio_player.resume()
}

#[tauri::command]
fn prev(state: State<AppState>) -> Result<(), String> {
    state.audio_player.prev()
}

#[tauri::command]
fn next(state: State<AppState>) -> Result<(), String> {
    state.audio_player.next()
}

#[tauri::command]
fn set_position(state: State<AppState>, position: u64) -> Result<(), String> {
    state.audio_player.set_position(position)
}

#[tauri::command]
fn set_looped(state: State<AppState>, looped: bool) -> Result<(), String> {
    state.audio_player.set_looped(looped)
}

#[tauri::command]
fn set_volume(state: State<AppState>, volume: f32) -> Result<(), String> {
    state.audio_player.set_volume(volume)
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
            add_queue,
            clear_queue,
            play,
            pause,
            resume,
            prev,
            next,
            set_position,
            set_looped,
            set_volume,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
