use pit2sop_core::Pit2Sop;
use pit2sop_core::models::{
    AppStatus, DoingMatch, PatchActionSummary, PendingPatchSummary, ProcessingSummary, SearchResult,
};
use std::path::PathBuf;

fn load_app() -> Result<Pit2Sop, String> {
    Pit2Sop::load().map_err(|error| error.to_string())
}

#[tauri::command]
fn app_status() -> Result<AppStatus, String> {
    load_app()?.status().map_err(|error| error.to_string())
}

#[tauri::command]
fn process_pit(text: String) -> Result<ProcessingSummary, String> {
    load_app()?
        .process_pit(&text)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn doing(text: String) -> Result<Vec<DoingMatch>, String> {
    load_app()?.doing(&text).map_err(|error| error.to_string())
}

#[tauri::command]
fn search(query: String) -> Result<Vec<SearchResult>, String> {
    load_app()?
        .search(&query)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn pending_patches() -> Result<Vec<PendingPatchSummary>, String> {
    load_app()?
        .pending_patches()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn apply_pending_patch(path: String) -> Result<PatchActionSummary, String> {
    load_app()?
        .apply_pending_patch(&PathBuf::from(path))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn reject_pending_patch(path: String) -> Result<PatchActionSummary, String> {
    load_app()?
        .reject_pending_patch(&PathBuf::from(path))
        .map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            app_status,
            process_pit,
            doing,
            search,
            pending_patches,
            apply_pending_patch,
            reject_pending_patch
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
